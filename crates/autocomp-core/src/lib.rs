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
    pub fn compile(&mut self, source: &Path, _options: &CompileOptions) -> Result<CompileResult, String> {
        let mut module = Module::from_source(source)?;
        self.run_passes(&mut module, None)?;
        Ok(CompileResult {
            products: vec![source.with_extension("o")],
            features: FeatureSet::from_module(&module),
            decisions: Vec::new(),
        })
    }

    pub fn compile_with_pgo(
        &mut self,
        source: &Path,
        profile: &Profile,
        _options: &CompileOptions,
    ) -> Result<CompileResult, String> {
        let mut module = Module::from_source(source)?;
        self.run_passes(&mut module, Some(profile))?;
        Ok(CompileResult {
            products: vec![source.with_extension("o")],
            features: FeatureSet::from_module(&module),
            decisions: Vec::new(),
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
        let _ = workload;
        let profile = Profile::default();
        self.compile_with_pgo(source, &profile, options)
            .map(|mut result| {
                result.decisions.insert(0, format!("auto-pgo: {}", instrumented.products.len()));
                result
            })
    }

    fn run_passes(&mut self, module: &mut Module, profile: Option<&Profile>) -> Result<(), String> {
        let ctx = PassContext { profile };
        for pass in &mut self.passes {
            let _ = pass.run(module, &ctx)?;
        }
        Ok(())
    }
}
