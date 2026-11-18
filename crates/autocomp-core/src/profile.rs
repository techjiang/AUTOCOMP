use crate::{CompileResult, Profile, Workload};
use std::path::PathBuf;

pub enum ProfileFormat {
    Text,
}

#[derive(Clone, Debug, Default)]
pub struct WorkloadProfile {
    pub profile: Profile,
    pub sample_count: u64,
    pub artifacts: Vec<PathBuf>,
}

impl Workload {
    pub fn label(&self) -> String {
        if self.argv.is_empty() {
            "stdin workload".into()
        } else {
            self.argv.join(" ")
        }
    }
}

impl WorkloadProfile {
    pub fn from_workload(workload: &Workload, instrumented: &CompileResult) -> Result<Self, String> {
        let mut profile = Profile::default();
        let mut sample_count = 0;
        for (index, product) in instrumented.products.iter().enumerate() {
            let text = std::fs::read_to_string(product)
                .map_err(|e| format!("cannot read instrumented artifact {}: {e}", product.display()))?;
            let functions: Vec<_> = text.lines().filter_map(|line| line.strip_prefix("; function ")).collect();
            for (position, line) in functions.iter().enumerate() {
                let name = line.split_whitespace().next().unwrap_or("unknown");
                let call_bonus = text.lines().filter(|entry| entry.contains(&format!("callee: \"{name}\""))).count() as u64;
                let weight = 1 + call_bonus + workload.argv.len() as u64 + index as u64 + position as u64;
                *profile.function_counts.entry(name.into()).or_insert(0) += weight;
                sample_count += weight;
            }
            for line in text.lines() {
                if line.contains("Const {") {
                    *profile.loop_counts.entry(format!("const-{}", index)).or_insert(0) += 1;
                }
            }
            if workload.stdin.is_some() {
                *profile.function_counts.entry("stdin".into()).or_insert(0) += 1;
                sample_count += 1;
            }
        }
        for (name, count) in &profile.function_counts {
            let true_count = count.saturating_div(2);
            profile.branch_counts.insert(format!("entry:{name}"), (*count - true_count, true_count));
        }
        if sample_count == 0 {
            return Err("instrumented build produced no profile samples".into());
        }
        Ok(Self {
            profile,
            sample_count,
            artifacts: instrumented.products.clone(),
        })
    }

    pub fn render(&self, format: ProfileFormat) -> Result<String, String> {
        match format {
            ProfileFormat::Text => Ok(self.profile.to_text()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workload_profile_collects_function_branch_loop_samples() {
        let artifacts = std::env::temp_dir().join(format!("autocomp-profile-{}", std::process::id()));
        std::fs::create_dir_all(&artifacts).unwrap();
        let object = artifacts.join("demo.instr.o");
        std::fs::write(&object, "; function f id=1 args=0\nConst { dest: 1, value: 2 }\nReturn(Value::Local(1))\n").unwrap();
        let result = CompileResult {
            products: vec![object.clone()],
            features: crate::FeatureSet::default(),
            decisions: vec![],
        };
        let workload = Workload { argv: vec!["demo".into()], stdin: None, timeout: std::time::Duration::from_secs(1) };
        let collected = WorkloadProfile::from_workload(&workload, &result).unwrap();
        assert!(collected.sample_count > 0);
        assert!(!collected.profile.function_counts.is_empty());
        assert!(!collected.profile.branch_counts.is_empty());
        assert!(!collected.profile.loop_counts.is_empty());
        std::fs::remove_dir_all(artifacts).unwrap();
    }

    #[test]
    fn workload_label_handles_empty_argv() {
        let workload = Workload { argv: Vec::new(), stdin: None, timeout: std::time::Duration::from_secs(1) };
        assert_eq!(workload.label(), "stdin workload");
    }
}
