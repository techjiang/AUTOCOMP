use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub type Id = u64;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Const(i64),
    Local(Id),
    Arg(Id),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    Const { dest: Id, value: i64 },
    Add { dest: Id, lhs: Value, rhs: Value },
    Mul { dest: Id, lhs: Value, rhs: Value },
    Call { dest: Option<Id>, callee: String, args: Vec<Value> },
    Return(Value),
}

#[derive(Clone, Debug)]
pub struct Function {
    pub id: Id,
    pub name: String,
    pub args: Vec<Id>,
    pub instructions: Vec<Instruction>,
}

#[derive(Clone, Debug, Default)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
    pub next_id: Id,
}

impl Module {
    pub fn from_source(path: &Path) -> Result<Self, String> {
        let source = fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut module = Module {
            name: path.file_stem().and_then(|s| s.to_str()).unwrap_or("module").into(),
            ..Module::default()
        };
        let mut next_id = 1;
        for line in source.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") || line.starts_with('#') {
                continue;
            }
            let sig = line.split(|c: char| c == '(' || c.is_whitespace()).next().unwrap_or("");
            let name = sig.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '_').to_string();
            if name.is_empty() {
                continue;
            }
            let args = line
                .split_once('(')
                .and_then(|(_, rest)| rest.split_once(')'))
                .map(|(args, _)| args.split(',').filter(|s| !s.trim().is_empty()).count() as Id)
                .unwrap_or(0);
            let arg_ids = (0..args).map(|_| { let id = next_id; next_id += 1; id }).collect();
            module.functions.push(Function {
                id: next_id,
                name,
                args: arg_ids,
                instructions: Vec::new(),
            });
            next_id += 1;
        }
        module.next_id = next_id;
        if module.functions.is_empty() {
            return Err("no functions found".into());
        }
        Ok(module)
    }

    pub fn fresh_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn function_index(&self, name: &str) -> Option<usize> {
        self.functions.iter().position(|f| f.name == name)
    }
}

#[derive(Clone, Debug, Default)]
pub struct CallGraph {
    pub edges: HashMap<String, Vec<String>>,
}

impl CallGraph {
    pub fn from_module(module: &Module) -> Self {
        let mut edges = HashMap::new();
        for func in &module.functions {
            let mut callees = Vec::new();
            for inst in &func.instructions {
                if let Instruction::Call { callee, .. } = inst {
                    callees.push(callee.clone());
                }
            }
            edges.insert(func.name.clone(), callees);
        }
        Self { edges }
    }
}
