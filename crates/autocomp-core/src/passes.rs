use crate::ir::{Id, Instruction, Module, Value};
use crate::Profile;

pub trait OptimizationPass {
    fn name(&self) -> &str;
    fn run(&mut self, module: &mut Module, ctx: &PassContext) -> Result<PassResult, String>;
}

#[derive(Clone, Copy)]
pub struct PassContext<'a> {
    pub profile: Option<&'a Profile>,
}

#[derive(Clone, Debug, Default)]
pub struct PassResult {
    pub changed: bool,
    pub note: String,
}

pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &str { "constant-folding" }

    fn run(&mut self, module: &mut Module, _ctx: &PassContext) -> Result<PassResult, String> {
        let mut folded_count = 0;
        for func in &mut module.functions {
            for inst in &mut func.instructions {
                let folded = match inst {
                    Instruction::Add { dest, lhs: Value::Const(a), rhs: Value::Const(b) } => {
                        Some(Instruction::Const { dest: *dest, value: a.wrapping_add(*b) })
                    }
                    Instruction::Sub { dest, lhs: Value::Const(a), rhs: Value::Const(b) } => {
                        Some(Instruction::Const { dest: *dest, value: a.wrapping_sub(*b) })
                    }
                    Instruction::Mul { dest, lhs: Value::Const(a), rhs: Value::Const(b) } => {
                        Some(Instruction::Const { dest: *dest, value: a.wrapping_mul(*b) })
                    }
                    _ => None,
                };
                if let Some(new_inst) = folded {
                    *inst = new_inst;
                    folded_count += 1;
                }
            }
        }
        Ok(PassResult {
            changed: folded_count > 0,
            note: format!("fold {folded_count} constant arithmetic expressions"),
        })
    }
}

pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str { "dce" }

    fn run(&mut self, module: &mut Module, _ctx: &PassContext) -> Result<PassResult, String> {
        let mut removed = 0;
        for func in &mut module.functions {
            let mut used = std::collections::HashSet::new();
            for inst in func.instructions.iter().rev() {
                let needed = match inst {
                    Instruction::Return(_) | Instruction::Call { dest: None, .. } => true,
                    Instruction::Call { dest: Some(dest), .. }
                    | Instruction::Const { dest, .. }
                    | Instruction::Add { dest, .. }
                    | Instruction::Sub { dest, .. }
                    | Instruction::Mul { dest, .. } => used.contains(dest),
                };
                if needed {
                    match inst {
                        Instruction::Add { lhs, rhs, .. }
                        | Instruction::Sub { lhs, rhs, .. }
                        | Instruction::Mul { lhs, rhs, .. } => {
                            collect_value_ids(lhs, &mut used);
                            collect_value_ids(rhs, &mut used);
                        }
                        Instruction::Call { args, .. } => {
                            for arg in args {
                                collect_value_ids(arg, &mut used);
                            }
                        }
                        Instruction::Return(value) => collect_value_ids(value, &mut used),
                        Instruction::Const { .. } => {}
                    }
                }
            }
            let before = func.instructions.len();
            func.instructions.retain(|inst| match inst {
                Instruction::Const { dest, .. } => used.contains(dest),
                _ => true,
            });
            removed += before - func.instructions.len();
        }
        Ok(PassResult {
            changed: removed > 0,
            note: format!("remove {removed} unused constants"),
        })
    }
}

fn collect_value_ids(value: &Value, used: &mut std::collections::HashSet<Id>) {
    match value {
        Value::Local(id) | Value::Arg(id) => { used.insert(*id); }
        Value::Const(_) => {}
    }
}

pub struct HotCallHint;

impl OptimizationPass for HotCallHint {
    fn name(&self) -> &str { "hot-call-hint" }

    fn run(&mut self, _module: &mut Module, ctx: &PassContext) -> Result<PassResult, String> {
        let count = ctx.profile.map(|p| p.function_counts.values().filter(|v| **v > 0).count()).unwrap_or(0);
        Ok(PassResult {
            changed: false,
            note: format!("profile functions with samples: {count}"),
        })
    }
}

pub fn default_passes() -> Vec<Box<dyn OptimizationPass>> {
    vec![
        Box::new(ConstantFolding),
        Box::new(DeadCodeElimination),
        Box::new(HotCallHint),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_folding_and_dce() {
        let mut module = Module::default();
        module.functions.push(crate::ir::Function {
            id: 1,
            name: "f".into(),
            args: vec![],
            instructions: vec![
                Instruction::Const { dest: 1, value: 2 },
                Instruction::Const { dest: 2, value: 3 },
                Instruction::Add { dest: 3, lhs: Value::Const(2), rhs: Value::Const(3) },
                Instruction::Return(Value::Local(3)),
            ],
        });
        let ctx = PassContext { profile: None };
        let mut folding = ConstantFolding;
        assert!(folding.run(&mut module, &ctx).unwrap().changed);
        assert_eq!(module.functions[0].instructions[2], Instruction::Const { dest: 3, value: 5 });
        let mut dce = DeadCodeElimination;
        assert!(dce.run(&mut module, &ctx).unwrap().changed);
        assert_eq!(module.functions[0].instructions, vec![Instruction::Const { dest: 3, value: 5 }, Instruction::Return(Value::Local(3))]);
    }
}
