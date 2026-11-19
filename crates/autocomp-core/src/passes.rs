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

pub struct StrengthReduction;

impl OptimizationPass for StrengthReduction {
    fn name(&self) -> &str { "strength-reduction" }

    fn run(&mut self, module: &mut Module, _ctx: &PassContext) -> Result<PassResult, String> {
        let mut reduced = 0;
        for func in &mut module.functions {
            for inst in &mut func.instructions {
                let replacement = match inst {
                    Instruction::Mul { dest, lhs: Value::Const(value), rhs: Value::Const(0) }
                    | Instruction::Mul { dest, lhs: Value::Const(0), rhs: Value::Const(value) } => {
                        let _ = value;
                        Some(Instruction::Const { dest: *dest, value: 0 })
                    }
                    _ => None,
                };
                if let Some(new_inst) = replacement {
                    if &new_inst != inst {
                        *inst = new_inst;
                        reduced += 1;
                    }
                }
            }
        }
        Ok(PassResult {
            changed: reduced > 0,
            note: format!("reduce {reduced} multiplication identities"),
        })
    }
}

pub struct HotCallHint;

impl OptimizationPass for HotCallHint {
    fn name(&self) -> &str { "hot-call-hint" }

    fn run(&mut self, module: &mut Module, ctx: &PassContext) -> Result<PassResult, String> {
        let Some(profile) = ctx.profile else {
            return Ok(PassResult { changed: false, note: "no profile".into() });
        };
        let mut hints = 0;
        for func in &module.functions {
            let total = profile.function_counts.get(&func.name).copied().unwrap_or(0);
            if total > 0 {
                hints += 1;
            }
        }
        Ok(PassResult {
            changed: false,
            note: format!("mark {hints} functions as hot: {}", hot_function_names(profile, module, 5).join(", ")),
        })
    }
}

pub struct BranchLayout;

impl OptimizationPass for BranchLayout {
    fn name(&self) -> &str { "branch-layout" }

    fn run(&mut self, _module: &mut Module, ctx: &PassContext) -> Result<PassResult, String> {
        let Some(profile) = ctx.profile else {
            return Ok(PassResult { changed: false, note: "no profile".into() });
        };
        let mut branches: Vec<_> = profile.branch_counts.iter().collect();
        branches.sort_by(|a, b| (b.1.0 + b.1.1).cmp(&(a.1.0 + a.1.1)).then(a.0.cmp(b.0)));
        let top = branches.iter().take(5).map(|(name, (false_count, true_count))| {
            let total = false_count + true_count;
            let bias = if total == 0 { 0.0 } else { *true_count as f64 * 100.0 / total as f64 };
            format!("{name} (true {bias:.0}%)")
        }).collect::<Vec<_>>().join(", ");
        Ok(PassResult {
            changed: false,
            note: format!("layout {} hot branches: {}", branches.len().min(5), if top.is_empty() { "none".into() } else { top }),
        })
    }
}

pub struct LoopTilingHint;

impl OptimizationPass for LoopTilingHint {
    fn name(&self) -> &str { "loop-tiling-hint" }

    fn run(&mut self, _module: &mut Module, ctx: &PassContext) -> Result<PassResult, String> {
        let Some(profile) = ctx.profile else {
            return Ok(PassResult { changed: false, note: "no profile".into() });
        };
        let mut loops: Vec<_> = profile.loop_counts.iter().collect();
        loops.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
        let top = loops.iter().take(3).map(|(name, count)| format!("{name} x{count}")).collect::<Vec<_>>().join(", ");
        Ok(PassResult {
            changed: false,
            note: format!("tile {} hot loops: {}", loops.len().min(3), if top.is_empty() { "none".into() } else { top }),
        })
    }
}

fn hot_function_names(profile: &Profile, module: &Module, limit: usize) -> Vec<String> {
    crate::analysis::ranked_profile_entries(&profile.function_counts, usize::MAX)
        .into_iter()
        .filter(|(name, _)| module.functions.iter().any(|f| &f.name == name))
        .take(limit)
        .map(|(name, _)| name)
        .collect()
}

pub fn default_passes() -> Vec<Box<dyn OptimizationPass>> {
    vec![
        Box::new(ConstantFolding),
        Box::new(StrengthReduction),
        Box::new(DeadCodeElimination),
        Box::new(HotCallHint),
        Box::new(BranchLayout),
        Box::new(LoopTilingHint),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_reduction_runs() {
        let mut module = Module::default();
        module.functions.push(crate::ir::Function {
            id: 1,
            name: "f".into(),
            args: vec![],
            instructions: vec![
                Instruction::Mul { dest: 2, lhs: Value::Const(9), rhs: Value::Const(0) },
                Instruction::Return(Value::Local(2)),
            ],
        });
        let ctx = PassContext { profile: None };
        let mut pass = StrengthReduction;
        assert!(pass.run(&mut module, &ctx).unwrap().changed);
        assert_eq!(module.functions[0].instructions[0], Instruction::Const { dest: 2, value: 0 });
    }

    #[test]
    fn strength_reduction_is_stable() {
        let mut module = module_clone();
        module.functions[0].instructions = vec![
            Instruction::Add { dest: 2, lhs: Value::Arg(1), rhs: Value::Const(0) },
            Instruction::Sub { dest: 3, lhs: Value::Arg(1), rhs: Value::Const(0) },
            Instruction::Mul { dest: 4, lhs: Value::Arg(1), rhs: Value::Const(1) },
            Instruction::Return(Value::Local(4)),
        ];
        let ctx = PassContext { profile: None };
        let mut pass = StrengthReduction;
        let first = pass.run(&mut module, &ctx).unwrap();
        assert!(!first.changed);
        assert!(module.functions[0].instructions.iter().any(|i| matches!(i, Instruction::Add { .. })));
    }

    #[test]
    fn dce_keeps_instructions_needed_by_return() {
        let mut module = Module::default();
        module.functions.push(crate::ir::Function {
            id: 1,
            name: "f".into(),
            args: vec![],
            instructions: vec![
                Instruction::Const { dest: 1, value: 2 },
                Instruction::Add { dest: 2, lhs: Value::Local(1), rhs: Value::Const(3) },
                Instruction::Return(Value::Local(2)),
            ],
        });
        let ctx = PassContext { profile: None };
        let mut pass = DeadCodeElimination;
        assert!(!pass.run(&mut module, &ctx).unwrap().changed);
        assert_eq!(module.functions[0].instructions.len(), 3);
    }

    #[test]
    fn profile_passes_report_hot_branches_and_loops() {
        let module = Module::default();
        let mut profile = Profile::default();
        profile.function_counts.insert("main".into(), 12);
        profile.branch_counts.insert("entry:main".into(), (4, 8));
        profile.loop_counts.insert("const-0".into(), 6);
        let ctx = PassContext { profile: Some(&profile) };
        let mut branch = BranchLayout;
        let branch_result = branch.run(&mut module_clone(), &ctx).unwrap();
        assert!(branch_result.note.contains("entry:main"));
        let mut loops = LoopTilingHint;
        let loop_result = loops.run(&mut module_clone(), &ctx).unwrap();
        assert!(loop_result.note.contains("const-0"));
    }

    fn module_clone() -> Module {
        let mut module = Module::default();
        module.functions.push(crate::ir::Function {
            id: 1,
            name: "main".into(),
            args: vec![],
            instructions: vec![Instruction::Return(Value::Const(0))],
        });
        module
    }

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
