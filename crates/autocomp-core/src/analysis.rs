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
        features.instruction_count = module.functions.iter().map(|f| f.instructions.len() + f.args.len()).sum();
        features.constant_count = module.functions.iter().flat_map(|f| &f.instructions).filter(|i| matches!(i, Instruction::Const { .. })).count();
        features.call_count = module.functions.iter().flat_map(|f| &f.instructions).filter(|i| matches!(i, Instruction::Call { .. })).count();
        features
    }

    pub fn hot_functions(&self, counts: &std::collections::HashMap<String, u64>) -> Vec<String> {
        ranked_profile_entries(counts, 5).into_iter().map(|(name, _)| name).collect()
    }
}

pub fn ranked_profile_entries(counts: &std::collections::HashMap<String, u64>, limit: usize) -> Vec<(String, u64)> {
    let mut entries: Vec<_> = counts.iter().filter(|(_, v)| **v > 0).map(|(k, v)| (k.clone(), *v)).collect();
    entries.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    entries.truncate(limit);
    entries
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

    #[test]
    fn hot_functions_are_sorted() {
        let mut counts = std::collections::HashMap::new();
        counts.insert("cold".into(), 1);
        counts.insert("hot".into(), 99);
        let features = FeatureSet::default();
        assert_eq!(features.hot_functions(&counts), vec!["hot".to_string(), "cold".to_string()]);
    }
}
