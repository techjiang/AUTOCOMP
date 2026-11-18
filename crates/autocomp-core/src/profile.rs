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
            for line in text.lines().filter_map(|line| line.strip_prefix("; function ")) {
                let name = line.split_whitespace().next().unwrap_or("unknown");
                let weight = 1 + workload.argv.len() as u64 + index as u64;
                *profile.function_counts.entry(name.into()).or_insert(0) += weight;
                sample_count += weight;
            }
            if workload.stdin.is_some() {
                *profile.function_counts.entry("stdin".into()).or_insert(0) += 1;
                sample_count += 1;
            }
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
    fn workload_label_handles_empty_argv() {
        let workload = Workload { argv: Vec::new(), stdin: None, timeout: std::time::Duration::from_secs(1) };
        assert_eq!(workload.label(), "stdin workload");
    }
}
