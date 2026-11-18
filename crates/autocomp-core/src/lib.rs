pub mod analysis;
pub mod ir;
pub mod passes;
pub mod profile;
pub mod report;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use analysis::FeatureSet;
pub use ir::{Function, Id, Instruction, Module, Value};
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

#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub function_counts: HashMap<String, u64>,
    pub branch_counts: HashMap<String, (u64, u64)>,
    pub loop_counts: HashMap<String, u64>,
}

impl Profile {
    pub fn merge(mut self, other: &Profile) -> Profile {
        for (k, v) in &other.function_counts {
            *self.function_counts.entry(k.clone()).or_insert(0) += v;
        }
        for (k, v) in &other.branch_counts {
            let entry = self.branch_counts.entry(k.clone()).or_insert((0, 0));
            entry.0 += v.0;
            entry.1 += v.1;
        }
        for (k, v) in &other.loop_counts {
            *self.loop_counts.entry(k.clone()).or_insert(0) += v;
        }
        self
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
            let Some(count) = parts.next().and_then(|v| v.parse::<u64>().ok()) else {
                return Err(format!("invalid profile count on line {}", line_number + 1));
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
        Ok(CompileResult {
            products: vec![object],
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
        result.products.extend(instrumented.products);
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
    Ok(object)
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
    fn profile_text_round_trip() {
        let text = "main 12\nbranch:main 3 9\nloop:main 20\n";
        let profile = Profile::from_text(text).unwrap();
        assert_eq!(profile.function_counts["main"], 12);
        assert_eq!(profile.branch_counts["main"], (3, 9));
        assert_eq!(profile.loop_counts["main"], 20);
        assert_eq!(Profile::from_text(&profile.to_text()).unwrap().to_text(), profile.to_text());
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
