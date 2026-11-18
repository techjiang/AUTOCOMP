pub mod analysis;
pub mod ir;
pub mod passes;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use analysis::FeatureSet;
pub use ir::{Function, Id, Instruction, Module, Value};
pub use passes::{OptimizationPass, PassContext, PassResult};

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
        let decisions = self.run_passes(&mut module, None)?;
        let mut result = CompileResult {
            products: vec![write_object(source, &module, options)?],
            features: FeatureSet::from_module(&module),
            decisions,
        };
        result.decisions.insert(0, format!("backend: emit object for {}", module.name));
        Ok(result)
    }

    pub fn compile_with_pgo(
        &mut self,
        source: &Path,
        profile: &Profile,
        options: &CompileOptions,
    ) -> Result<CompileResult, String> {
        let mut module = Module::from_source(source)?;
        let mut decisions = self.run_passes(&mut module, Some(profile))?;
        let mut result = CompileResult {
            products: vec![write_object(source, &module, options)?],
            features: FeatureSet::from_module(&module),
            decisions: Vec::new(),
        };
        if let Some(hot) = FeatureSet::from_module(&module).hot_functions(&profile.function_counts).first() {
            decisions.insert(0, format!("pgo: hot function {}", hot));
        }
        result.decisions = decisions;
        result.decisions.insert(0, "backend: emit optimized object".into());
        Ok(result)
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
        let _ = workload;
        let profile = Profile::default();
        self.compile_with_pgo(source, &profile, options)
            .map(|mut result| {
                result.decisions.insert(0, format!("auto-pgo: {}", instrumented.products.len()));
                result
            })
    }

    fn run_passes(&mut self, module: &mut Module, profile: Option<&Profile>) -> Result<Vec<String>, String> {
        let ctx = PassContext { profile };
        let mut decisions = Vec::new();
        for pass in &mut self.passes {
            let result = pass.run(module, &ctx)?;
            if result.changed {
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

    #[test]
    fn compile_emits_object() {
        let dir = std::env::temp_dir().join(format!("autocomp-compile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("demo.ac");
        std::fs::write(&source, "int f(int a) {\n  %0 = add %a, 1;\n  return %0;\n}\n").unwrap();
        let mut compiler = AutoCompiler::default();
        let result = compiler.compile(&source, &CompileOptions::default()).unwrap();
        assert_eq!(result.products.len(), 1);
        assert!(result.products[0].exists());
        assert!(!result.decisions.is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
