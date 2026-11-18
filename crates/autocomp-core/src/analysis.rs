use crate::ir::{Instruction, Module, Value};

#[derive(Clone, Debug, Default)]
pub struct FeatureSet {
    pub function_count: usize,
    pub instruction_count: usize,
    pub call_count: usize,
    pub constant_count: usize,
}

impl FeatureSet {
    pub fn from_module(module: &Module) -> Self {
        let mut features = Self {
            function_count: module.functions.len(),
            ..Self::default()
        };
        for func in &module.functions {
            for inst in &func.instructions {
                features.instruction_count += 1;
                match inst {
                    Instruction::Const { .. } => features.constant_count += 1,
                    Instruction::Call { .. } => features.call_count += 1,
                    _ => {}
                }
            }
            for _ in &func.args {
                features.instruction_count += 1;
            }
        }
        features
    }

    pub fn hot_functions(&self, counts: &std::collections::HashMap<String, u64>) -> Vec<String> {
        let mut names: Vec<_> = counts.iter().map(|(k, v)| (k.clone(), *v)).collect();
        names.sort_by(|a, b| b.1.cmp(&a.1));
        names.into_iter().take(5).map(|(k, _)| k).collect()
    }
}

pub fn is_zero(value: &Value) -> bool {
    matches!(value, Value::Const(0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_detection() {
        assert!(is_zero(&Value::Const(0)));
        assert!(!is_zero(&Value::Const(1)));
        assert!(!is_zero(&Value::Local(3)));
    }
}
