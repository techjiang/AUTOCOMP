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
    Sub { dest: Id, lhs: Value, rhs: Value },
    Mul { dest: Id, lhs: Value, rhs: Value },
    Call { dest: Option<Id>, callee: String, args: Vec<Value> },
    Return(Value),
}

impl Instruction {
    pub fn dest(&self) -> Option<Id> {
        match self {
            Self::Const { dest, .. }
            | Self::Add { dest, .. }
            | Self::Sub { dest, .. }
            | Self::Mul { dest, .. } => Some(*dest),
            Self::Call { dest, .. } => *dest,
            Self::Return(_) => None,
        }
    }
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

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModuleVerifyReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ModuleVerifyReport {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

impl Module {
    pub fn from_source(path: &Path) -> Result<Self, String> {
        let source = fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let mut module = Module {
            name: path.file_stem().and_then(|s| s.to_str()).unwrap_or("module").into(),
            ..Module::default()
        };
        let mut next_id = if Self::uses_explicit_destinations(&source) { 100 } else { 1 };
        let mut current_function: Option<usize> = None;
        for raw_line in source.lines() {
            let line = strip_comment(raw_line).trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.starts_with('}') {
                current_function = None;
                continue;
            }
            if current_function.is_none() {
                let (signature, body_start) = match parse_function_signature(line) {
                    Some(parsed) => parsed,
                    None => continue,
                };
                let name = signature.name;
                let arg_ids: Vec<Id> = (1..=signature.arg_count).map(|i| 100 + i).collect();
                let function_id = next_id;
                next_id += 1;
                module.functions.push(Function {
                    id: function_id,
                    name,
                    args: arg_ids,
                    instructions: Vec::new(),
                });
                current_function = Some(module.functions.len() - 1);
                if let Some(body) = body_start {
                    parse_instruction_into(body, &mut module, current_function.unwrap(), &mut next_id);
                }
            } else if let Some(index) = current_function {
                parse_instruction_into(line, &mut module, index, &mut next_id);
            }
        }
        module.next_id = next_id;
        if module.functions.is_empty() {
            return Err("no functions found".into());
        }
        let report = module.verify();
        if !report.is_valid() {
            return Err(format!("invalid IR: {}", report.errors.join("; ")));
        }
        Ok(module)
    }

    pub fn fresh_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn uses_explicit_destinations(source: &str) -> bool {
        source.lines().any(|line| {
            let line = line.split("//").next().unwrap_or("").trim();
            line.starts_with('%')
                && line.contains('=')
                && line.split('=').next().unwrap_or("").trim().trim_start_matches('%').parse::<u64>().is_ok()
        })
    }

    pub fn verify(&self) -> ModuleVerifyReport {
        let mut report = ModuleVerifyReport::default();
        let mut names = std::collections::HashSet::new();
        for function in &self.functions {
            if !names.insert(function.name.as_str()) {
                report.errors.push(format!("duplicate function {}", function.name));
            }
            let mut defined = std::collections::HashSet::new();
            defined.extend(function.args.iter().copied());
            for instruction in &function.instructions {
                let used = match instruction {
                    Instruction::Const { dest, .. } => {
                        if !defined.insert(*dest) {
                            report.errors.push(format!("{}: duplicate value %{}", function.name, dest));
                        }
                        Vec::new()
                    }
                    Instruction::Add { dest, lhs, rhs }
                    | Instruction::Sub { dest, lhs, rhs }
                    | Instruction::Mul { dest, lhs, rhs } => {
                        if !defined.insert(*dest) {
                            report.errors.push(format!("{}: duplicate value %{}", function.name, dest));
                        }
                        vec![lhs, rhs]
                    }
                    Instruction::Call { dest, args, .. } => {
                        if let Some(dest) = dest {
                            if !defined.insert(*dest) {
                                report.errors.push(format!("{}: duplicate value %{}", function.name, dest));
                            }
                        }
                        args.iter().collect()
                    }
                    Instruction::Return(value) => vec![value],
                };
                for value in used {
                    if let Value::Local(id) = value {
                        if !defined.contains(id) {
                            report.errors.push(format!("{}: use of undefined %{}", function.name, id));
                        }
                    }
                }
            }
            if !function.instructions.iter().any(|i| matches!(i, Instruction::Return(_))) {
                report.warnings.push(format!("{}: missing return", function.name));
            }
        }
        for function in &self.functions {
            for instruction in &function.instructions {
                if let Instruction::Call { callee, .. } = instruction {
                    if !names.contains(callee.as_str()) {
                        report.warnings.push(format!("{}: call to external {}", function.name, callee));
                    }
                }
            }
        }
        report
    }
}

struct Signature {
    name: String,
    arg_count: Id,
}

fn strip_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn parse_function_signature(line: &str) -> Option<(Signature, Option<&str>)> {
    if line.starts_with('}') || line.starts_with(';') {
        return None;
    }
    let signature_head = line.split_once('(').map(|(left, _)| left).unwrap_or(line);
    let name = signature_head
        .split_whitespace()
        .last()
        .unwrap_or("")
        .trim_start_matches(|c: char| !(c.is_alphabetic() || c == '_'))
        .trim_matches(|c: char| !(c.is_alphanumeric() || c == '_'))
        .to_string();
    if name.is_empty()
        || name.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
        || matches!(name.as_str(), "const" | "add" | "sub" | "mul" | "call" | "return")
        || !line.contains('(')
    {
        return None;
    }
    let (before_body, body) = match line.split_once('{') {
        Some((before, after)) => (before, Some(after)),
        None => (line, None),
    };
    let arg_count = before_body
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(args, _)| args.split(',').filter(|s| !s.trim().is_empty()).count() as Id)
        .unwrap_or(0);
    Some((Signature { name, arg_count }, body))
}

fn parse_value(token: &str, arg_ids: &[Id], defined: &std::collections::HashMap<String, Id>) -> Value {
    let token = token.trim().trim_end_matches(',').trim();
    if let Ok(value) = token.parse::<i64>() {
        return Value::Const(value);
    }
    let name = token.strip_prefix('%').unwrap_or(token);
    if let Some(pos) = arg_ids.iter().position(|id| format!("%arg{id}") == format!("%{name}")) {
        return Value::Arg(arg_ids[pos]);
    }
    let arg_index = if name.len() == 1 && name.as_bytes()[0].is_ascii_lowercase() {
        Some((name.as_bytes()[0] - b'a') as usize)
    } else {
        name.parse::<usize>().ok()
    };
    if let Some(index) = arg_index {
        if let Some(id) = arg_ids.get(index) {
            return Value::Arg(*id);
        }
    }
    if let Some(id) = defined.get(name) {
        return Value::Local(*id);
    }
    Value::Local(name.bytes().fold(0, |acc, b| acc.wrapping_mul(131).wrapping_add(b as u64)))
}

fn parse_instruction_into(line: &str, module: &mut Module, function_index: usize, next_id: &mut Id) {
    let line = line.trim().trim_end_matches(';').trim();
    if line.is_empty() || line.starts_with('{') {
        return;
    }
    let arg_ids = module.functions[function_index].args.clone();
    let defined: std::collections::HashMap<String, Id> = module.functions[function_index]
        .instructions
        .iter()
        .filter_map(|i| i.dest().map(|d| (d.to_string(), d)))
        .collect();
    let instruction = if line.starts_with("return") {
        let value = line.strip_prefix("return").unwrap_or("").trim();
        Some(Instruction::Return(if value.is_empty() {
            Value::Const(0)
        } else {
            parse_value(value, &arg_ids, &defined)
        }))
    } else if line.starts_with('%') {
        let (dest_token, rest) = line.split_once('=').unwrap_or((line, ""));
        let dest = dest_token.trim().trim_start_matches('%').parse::<Id>().unwrap_or_else(|_| {
            let id = *next_id;
            *next_id += 1;
            id
        });
        let rest = rest.trim();
        if let Some(callee_expr) = rest.strip_prefix("call ") {
            let callee = callee_expr.split('(').next().unwrap_or(callee_expr).trim().to_string();
            let args_text = callee_expr.split_once('(').and_then(|(_, r)| r.rsplit_once(')')).map(|(a, _)| a).unwrap_or("");
            let args = args_text.split(',').map(|a| a.trim()).filter(|a| !a.is_empty()).map(|a| parse_value(a, &arg_ids, &defined)).collect();
            Some(Instruction::Call { dest: Some(dest), callee, args })
        } else {
            let (op, expr) = rest.split_once(' ').unwrap_or((rest, ""));
            let mut parts = expr.split(',');
            let lhs = parts.next().map(|v| parse_value(v, &arg_ids, &defined)).unwrap_or(Value::Const(0));
            let rhs = parts.next().map(|v| parse_value(v, &arg_ids, &defined)).unwrap_or(Value::Const(0));
            match op {
                "add" => Some(Instruction::Add { dest, lhs, rhs }),
                "sub" => Some(Instruction::Sub { dest, lhs, rhs }),
                "mul" => Some(Instruction::Mul { dest, lhs, rhs }),
                _ => rest.parse::<i64>().ok().map(|value| Instruction::Const { dest, value }),
            }
        }
    } else {
        None
    };
    if let Some(inst) = instruction {
        module.functions[function_index].instructions.push(inst);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_detects_undefined_values() {
        let dir = std::env::temp_dir().join(format!("autocomp-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.ac");
        std::fs::write(&path, "int f() {\n  %0 = add %1, 2;\n  return %0;\n}\n").unwrap();
        let result = Module::from_source(&path);
        assert!(result.is_err());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }

    #[test]
    fn parse_ir_from_source() {
        let dir = std::env::temp_dir().join(format!("autocomp-ir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.ac");
        std::fs::write(&path, "int sum(int a, int b) {\n  %0 = add %a, %b;\n  return %0;\n}\nint main() {\n  %1 = call sum(2, 3);\n  return %1;\n}\n").unwrap();
        let module = Module::from_source(&path).unwrap();
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "sum");
        assert_eq!(module.functions[0].instructions.len(), 2);
        assert!(!module.functions.is_empty());
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
