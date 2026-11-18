use crate::ir::{Instruction, Module, Value};
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
        let mut changed = false;
        for func in &mut module.functions {
            for inst in &mut func.instructions {
                let folded = match inst {
                    Instruction::Add { dest, lhs: Value::Const(a), rhs: Value::Const(b) } => {
                        Some(Instruction::Const { dest: *dest, value: a.wrapping_add(*b) })
                    }
                    Instruction::Mul { dest, lhs: Value::Const(a), rhs: Value::Const(b) } => {
                        Some(Instruction::Const { dest: *dest, value: a.wrapping_mul(*b) })
                    }
                    _ => None,
                };
                if let Some(new_inst) = folded {
                    *inst = new_inst;
                    changed = true;
                }
            }
        }
        Ok(PassResult { changed, note: "fold constant arithmetic".into() })
    }
}

pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &str { "dce" }

    fn run(&mut self, module: &mut Module, _ctx: &PassContext) -> Result<PassResult, String> {
        let mut changed = false;
        for func in &mut module.functions {
            let before = func.instructions.len();
            func.instructions.retain(|inst| !matches!(inst, Instruction::Const { .. }));
            changed |= func.instructions.len() != before;
        }
        Ok(PassResult { changed, note: "remove unused constants".into() })
    }
}

pub struct HotCallHint;

impl OptimizationPass for HotCallHint {
    fn name(&self) -> &str { "hot-call-hint" }

    fn run(&mut self, _module: &mut Module, ctx: &PassContext) -> Result<PassResult, String> {
        let count = ctx.profile.map(|p| p.function_counts.len()).unwrap_or(0);
        Ok(PassResult {
            changed: false,
            note: format!("profile functions: {count}"),
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
            ],
        });
        let ctx = PassContext { profile: None };
        let mut folding = ConstantFolding;
        assert!(folding.run(&mut module, &ctx).unwrap().changed);
        assert_eq!(module.functions[0].instructions[2], Instruction::Const { dest: 3, value: 5 });
        let mut dce = DeadCodeElimination;
        assert!(dce.run(&mut module, &ctx).unwrap().changed);
        assert!(module.functions[0].instructions.is_empty());
    }
}
