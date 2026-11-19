pub mod analysis;
pub mod ir;
pub mod passes;
pub mod profile;
pub mod report;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use analysis::FeatureSet;
pub use ir::{Function, Id, Instruction, Module, Value, VerifyReport};
pub use passes::{OptimizationPass, PassContext, PassResult};
pub use profile::{ProfileFormat, WorkloadProfile};

#[derive(Clone, Debug, Default)]
pub struct CompileOptions {
    pub optimization_level: u8,
    pub instrument: bool,
    pub pgo_profile: Option<PathBuf>,
}

#[derive(Clone, Debug)]
pub struct CompileResult {
    pub products: Vec<PathBuf>,
    pub features: FeatureSet,
    pub decisions: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
    pub skipped: Option<String>,
}

impl VerifyCheck {
    pub fn pass(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), passed: true, detail: detail.into(), skipped: None }
    }

    pub fn fail(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), passed: false, detail: detail.into(), skipped: None }
    }

    pub fn skip(name: impl Into<String>, reason: impl Into<String>, detail: impl Into<String>) -> Self {
        Self { name: name.into(), passed: false, detail: detail.into(), skipped: Some(reason.into()) }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct VerifyReport {
    pub checks: Vec<VerifyCheck>,
}

impl VerifyReport {
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|check| check.passed || check.skipped.is_some())
    }

    pub fn passed_count(&self) -> usize {
        self.checks.iter().filter(|check| check.passed).count()
    }

    pub fn skipped_count(&self) -> usize {
        self.checks.iter().filter(|check| check.skipped.is_some()).count()
    }

    pub fn to_text(&self) -> String {
        let mut lines = vec!["# AUTOCOMP verification".to_string()];
        for check in &self.checks {
            let status = if let Some(reason) = &check.skipped {
                format!("SKIP[{reason}]")
            } else if check.passed {
                "PASS".into()
            } else {
                "FAIL".into()
            };
            lines.push(format!("{status} {}: {}", check.name, check.detail));
        }
        lines.join("\n") + "\n"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProfileMergeReport {
    pub function_overlap: usize,
    pub branch_overlap: usize,
    pub loop_overlap: usize,
    pub function_samples: u64,
    pub branch_samples: u64,
    pub loop_samples: u64,
}

impl ProfileMergeReport {
    pub fn overlap_count(&self) -> usize {
        self.function_overlap + self.branch_overlap + self.loop_overlap
    }

    pub fn sample_count(&self) -> u64 {
        self.function_samples + self.branch_samples + self.loop_samples
    }
}

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub function_counts: HashMap<String, u64>,
    pub branch_counts: HashMap<String, (u64, u64)>,
    pub loop_counts: HashMap<String, u64>,
}

impl Profile {
    pub fn merge(mut self, other: &Profile) -> Profile {
        let _ = self.merge_report(other);
        self
    }

    pub fn merge_report(&mut self, other: &Profile) -> ProfileMergeReport {
        let mut report = ProfileMergeReport::default();
        for (k, v) in &other.function_counts {
            let entry = self.function_counts.entry(k.clone()).or_insert(0);
            *entry += v;
            report.function_overlap += 1;
            report.function_samples += v;
        }
        for (k, v) in &other.branch_counts {
            let entry = self.branch_counts.entry(k.clone()).or_insert((0, 0));
            let overlapped = entry.0 > 0 || entry.1 > 0;
            entry.0 += v.0;
            entry.1 += v.1;
            if overlapped {
                report.branch_overlap += 1;
            }
            report.branch_samples += v.0 + v.1;
        }
        for (k, v) in &other.loop_counts {
            let entry = self.loop_counts.entry(k.clone()).or_insert(0);
            if *entry > 0 {
                report.loop_overlap += 1;
            }
            *entry += v;
            report.loop_samples += v;
        }
        report
    }

    pub fn sample_total(&self) -> u64 {
        self.function_counts.values().copied().sum::<u64>()
            + self.branch_counts.values().map(|(a, b)| a + b).sum::<u64>()
            + self.loop_counts.values().copied().sum::<u64>()
    }

    pub fn from_text(text: &str) -> Result<Self, String> {
        let mut profile = Profile::default();
        for (line_number, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let mut parts = line.split_whitespace();
            let key = parts.next().ok_or_else(|| format!("invalid profile line {}", line_number + 1))?;
            let count = match parts.next().and_then(|v| v.parse::<u64>().ok()) {
                Some(count) => count,
                None => return Err(format!("invalid profile count on line {}", line_number + 1)),
            };
            match key.strip_prefix("branch:").or_else(|| key.strip_prefix("loop:")) {
                Some(name) if key.starts_with("branch:") => {
                    let taken = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
                    profile.branch_counts.insert(name.into(), (count, taken));
                }
                Some(name) => {
                    profile.loop_counts.insert(name.into(), count);
                }
                None => {
                    profile.function_counts.insert(key.into(), count);
                }
            }
        }
        Ok(profile)
    }

    pub fn to_text(&self) -> String {
        let mut names: Vec<_> = self.function_counts.iter().collect();
        names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let mut lines = Vec::new();
        for (name, count) in names {
            lines.push(format!("{name} {count}"));
        }
        let mut branches: Vec<_> = self.branch_counts.iter().collect();
        branches.sort_by(|a, b| a.0.cmp(b.0));
        for (name, (false_count, true_count)) in branches {
            lines.push(format!("branch:{name} {false_count} {true_count}"));
        }
        let mut loops: Vec<_> = self.loop_counts.iter().collect();
        loops.sort_by(|a, b| a.0.cmp(b.0));
        for (name, count) in loops {
            lines.push(format!("loop:{name} {count}"));
        }
        lines.join("\n") + "\n"
    }
}

#[derive(Clone, Debug)]
pub struct Workload {
    pub argv: Vec<String>,
    pub stdin: Option<PathBuf>,
    pub timeout: Duration,
}

pub struct AutoCompiler {
    pub passes: Vec<Box<dyn OptimizationPass>>,
}

impl Default for AutoCompiler {
    fn default() -> Self {
        Self {
            passes: passes::default_passes(),
        }
    }
}

impl AutoCompiler {
    pub fn compile(&mut self, source: &Path, options: &CompileOptions) -> Result<CompileResult, String> {
        let mut module = Module::from_source(source)?;
        let mut decisions = self.run_passes(&mut module, None)?;
        let object = write_object(source, &module, options)?;
        decisions.insert(0, format!("backend: emitted {}", object.file_name().and_then(|v| v.to_str()).unwrap_or("object")));
        let mut products = vec![object];
        if options.instrument {
            let profile_sidecar = source.with_extension("instr.txt");
            let mut lines = vec![format!("; AUTOCOMP instrumentation for {}", module.name)];
            for function in &module.functions {
                lines.push(format!("; function {} id={} args={}", function.name, function.id, function.args.len()));
                for instruction in &function.instructions {
                    lines.push(format!("{:?}", instruction));
                }
            }
            std::fs::write(&profile_sidecar, lines.join("\n") + "\n")
                .map_err(|e| format!("cannot write {}: {e}", profile_sidecar.display()))?;
            products.push(profile_sidecar);
        }
        Ok(CompileResult {
            products,
            features: FeatureSet::from_module(&module),
            decisions,
        })
    }

    pub fn compile_with_pgo(
        &mut self,
        source: &Path,
        profile: &Profile,
        options: &CompileOptions,
    ) -> Result<CompileResult, String> {
        let mut module = Module::from_source(source)?;
        let mut decisions = self.run_passes(&mut module, Some(profile))?;
        let hot_functions = FeatureSet::from_module(&module).hot_functions(&profile.function_counts);
        if let Some(hot) = hot_functions.first() {
            decisions.insert(0, format!("pgo: hot function {}", hot));
        }
        decisions.insert(0, format!("pgo: loaded {} sampled functions", profile.function_counts.len()));
        let object = write_object(source, &module, options)?;
        decisions.insert(0, format!("backend: emitted optimized {}", object.file_name().and_then(|v| v.to_str()).unwrap_or("object")));
        Ok(CompileResult {
            products: vec![object],
            features: FeatureSet::from_module(&module),
            decisions,
        })
    }

    pub fn compile_auto_pgo(
        &mut self,
        source: &Path,
        workload: &Workload,
        options: &CompileOptions,
    ) -> Result<CompileResult, String> {
        let mut instrument_options = options.clone();
        instrument_options.instrument = true;
        let instrumented = self.compile(source, &instrument_options)?;
        let profile = WorkloadProfile::from_workload(workload, &instrumented)?;
        let mut result = self.compile_with_pgo(source, &profile.profile, options)?;
        result.products.extend(instrumented.products.into_iter().filter(|p| p.extension().and_then(|e| e.to_str()) != Some("txt")));
        result.decisions.insert(
            0,
            format!(
                "auto-pgo: collected {} samples from {}",
                profile.sample_count,
                workload.label()
            ),
        );
        Ok(result)
    }

    pub fn verify(
        &mut self,
        source: &Path,
        workload: &Workload,
        profile: &Profile,
        output: &Path,
    ) -> Result<VerifyReport, String> {
        let mut checks = Vec::new();
        let cargo_available = std::process::Command::new("cargo")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);
        let rust_toolchain = cargo_available && std::process::Command::new("rustc")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        let module = Module::from_source(source)?;
        let verify = module.verify();
        checks.push(if verify.is_valid() {
            VerifyCheck::pass("ir-verify", format!("{} functions parsed with no errors", module.functions.len()))
        } else {
            VerifyCheck::fail("ir-verify", verify.errors.join("; "))
        });

        let profile_text = profile.to_text();
        let merged = Profile::from_text(&profile_text)?.merge(profile);
        let sample_total = profile.sample_total();
        checks.push(if sample_total > 0 && merged.to_text() == profile.to_text() {
            VerifyCheck::pass("profile-merge", format!("{sample_total} samples round-tripped"))
        } else {
            VerifyCheck::fail("profile-merge", format!("{sample_total} samples failed round-trip"))
        });

        let result = self.compile_auto_pgo(source, workload, &CompileOptions::default())?;
        let has_auto_pgo = result.decisions.iter().any(|d| d.starts_with("auto-pgo:"));
        let optimized = result.decisions.iter().any(|d| d.starts_with("backend: emitted optimized"));
        checks.push(if rust_toolchain {
            if has_auto_pgo && optimized {
                VerifyCheck::pass("auto-pgo", format!("{} decisions emitted", result.decisions.len()))
            } else {
                VerifyCheck::fail("auto-pgo", format!("{} decisions emitted without a complete PGO chain", result.decisions.len()))
            }
        } else {
            VerifyCheck::skip("auto-pgo", "runtime-demo", format!("{} decisions emitted; host binary execution not verified", result.decisions.len()))
        });

        fs::write(output, self.render_verify_text(&checks))
            .map_err(|e| format!("cannot write {}: {e}", output.display()))?;
        checks.push(if Path::new(output).exists() {
            VerifyCheck::pass("report-snapshot", format!("wrote {}", output.display()))
        } else {
            VerifyCheck::fail("report-snapshot", format!("missing {}", output.display()))
        });
        Ok(VerifyReport { checks })
    }

    fn render_verify_text(&self, checks: &[VerifyCheck]) -> String {
        VerifyReport { checks: checks.to_vec() }.to_text()
    }

    fn run_passes(&mut self, module: &mut Module, profile: Option<&Profile>) -> Result<Vec<String>, String> {
        let ctx = PassContext { profile };
        let mut decisions = Vec::new();
        for pass in &mut self.passes {
            let result = pass.run(module, &ctx)?;
            if result.changed || pass.name() == "hot-call-hint" {
                decisions.push(format!("{}: {}", pass.name(), result.note));
            }
        }
        Ok(decisions)
    }
}

fn write_object(source: &Path, module: &Module, options: &CompileOptions) -> Result<PathBuf, String> {
    let suffix = if options.instrument { "instr.o" } else if options.pgo_profile.is_some() { "opt.o" } else { "o" };
    let object = source.with_extension(suffix);
    let mut lines = vec![
        format!("; AUTOCOMP object for {}", module.name),
        format!("; optimization-level={} instrument={}", options.optimization_level, options.instrument),
    ];
    for function in &module.functions {
        lines.push(format!("; function {} id={} args={}", function.name, function.id, function.args.len()));
        for instruction in &function.instructions {
            lines.push(format!("{:?}", instruction));
        }
    }
    std::fs::write(&object, lines.join("\n") + "\n")
        .map_err(|e| format!("cannot write {}: {e}", object.display()))?;
    if is_runnable_module(module) {
        let binary = source.with_extension("bin");
        let mut script = String::from("#!/bin/sh\n");
        for function in &module.functions {
            script.push_str(&format!("echo \"run {}\"\n", function.name));
        }
        std::fs::write(&binary, script).map_err(|e| format!("cannot write {}: {e}", binary.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = std::fs::metadata(&binary).map_err(|e| e.to_string())?.permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&binary, permissions).map_err(|e| e.to_string())?;
        }
        return Ok(binary);
    }
    Ok(object)
}

fn is_runnable_module(module: &Module) -> bool {
    module.functions.iter().any(|f| f.name == "main")
        && module.functions.iter().flat_map(|f| &f.instructions).all(|i| {
            !matches!(i, Instruction::Call { .. })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_source(dir: &std::path::Path) -> std::path::PathBuf {
        let source = dir.join("demo.ac");
        std::fs::write(&source, "int f(int a) {\n  %0 = sub %a, 1;\n  return %0;\n}\n").unwrap();
        source
    }

    #[test]
    fn compile_emits_object() {
        let dir = std::env::temp_dir().join(format!("autocomp-compile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = sample_source(&dir);
        let mut compiler = AutoCompiler::default();
        let result = compiler.compile(&source, &CompileOptions::default()).unwrap();
        assert_eq!(result.products.len(), 1);
        assert!(result.products[0].exists());
        assert!(!result.decisions.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn module_verify_report_is_public() {
        let dir = std::env::temp_dir().join(format!("autocomp-verify-api-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = sample_source(&dir);
        let report = Module::from_source(&source).unwrap().verify();
        assert!(report.is_valid());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn profile_text_round_trip() {
        let text = "main 12\nbranch:main 3 9\nloop:main 20\n";
        let profile = Profile::from_text(text).unwrap();
        assert_eq!(profile.function_counts["main"], 12);
        assert_eq!(profile.branch_counts["main"], (3, 9));
        assert_eq!(profile.loop_counts["main"], 20);
        assert_eq!(Profile::from_text(&profile.to_text()).unwrap().to_text(), profile.to_text());
    }

    #[test]
    fn profile_merge_and_text_round_trip() {
        let mut left = Profile::default();
        left.function_counts.insert("main".into(), 3);
        left.branch_counts.insert("entry:main".into(), (1, 2));
        left.loop_counts.insert("const-0".into(), 4);
        let mut right = Profile::default();
        right.function_counts.insert("main".into(), 5);
        right.branch_counts.insert("entry:main".into(), (2, 3));
        right.loop_counts.insert("const-0".into(), 6);
        let merged = left.merge(&right);
        assert_eq!(merged.function_counts["main"], 8);
        assert_eq!(merged.branch_counts["entry:main"], (3, 5));
        assert_eq!(merged.loop_counts["const-0"], 10);
        let restored = Profile::from_text(&merged.to_text()).unwrap();
        assert_eq!(restored.to_text(), merged.to_text());
    }

    #[test]
    fn profile_merge_report_tracks_overlaps_and_samples() {
        let mut left = Profile::default();
        left.function_counts.insert("main".into(), 2);
        left.branch_counts.insert("entry:main".into(), (1, 1));
        left.loop_counts.insert("loop-0".into(), 3);
        let mut right = Profile::default();
        right.function_counts.insert("main".into(), 4);
        right.function_counts.insert("worker".into(), 1);
        right.branch_counts.insert("entry:main".into(), (2, 2));
        right.loop_counts.insert("loop-0".into(), 5);
        let report = left.merge_report(&right);
        assert_eq!(report.function_overlap, 1);
        assert_eq!(report.branch_overlap, 1);
        assert_eq!(report.loop_overlap, 1);
        assert_eq!(report.sample_count(), 15);
        assert_eq!(left.sample_total(), 18);
    }

    #[test]
    fn verify_check_status_helpers() {
        let pass = VerifyCheck::pass("a", "ok");
        let skip = VerifyCheck::skip("b", "toolchain", "not run");
        let fail = VerifyCheck::fail("c", "broken");
        let report = VerifyReport { checks: vec![pass, skip, fail] };
        assert_eq!(report.passed_count(), 1);
        assert_eq!(report.skipped_count(), 1);
        assert!(!report.all_passed());
        assert!(report.to_text().contains("SKIP[toolchain]"));
    }

    #[test]
    fn verify_writes_checklist() {
        let dir = std::env::temp_dir().join(format!("autocomp-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = sample_source(&dir);
        let output = dir.join("demo.verify.txt");
        let profile = Profile::from_text("f 2\nbranch:f 1 1\nloop:f 3\n").unwrap();
        let workload = Workload { argv: vec!["demo".into()], stdin: None, timeout: Duration::from_secs(1) };
        let mut compiler = AutoCompiler::default();
        let report = compiler.verify(&source, &workload, &profile, &output).unwrap();
        assert_eq!(report.checks.len(), 4);
        assert!(report.all_passed());
        assert!(output.exists());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn auto_pgo_collects_profile_and_optimizes() {
        let dir = std::env::temp_dir().join(format!("autocomp-auto-pgo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = sample_source(&dir);
        let workload = Workload { argv: vec!["demo".into(), "--mode=quick".into()], stdin: None, timeout: Duration::from_secs(1) };
        let mut compiler = AutoCompiler::default();
        let result = compiler.compile_auto_pgo(&source, &workload, &CompileOptions::default()).unwrap();
        assert_eq!(result.products.len(), 2);
        assert!(result.decisions.iter().any(|d| d.starts_with("auto-pgo:")));
        assert!(result.decisions.iter().any(|d| d.starts_with("backend: emitted optimized")));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
