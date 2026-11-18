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
                let Some((signature, body_start)) = parse_function_signature(line) else {
                    continue;
                };
                let name = signature.name;
                let arg_ids: Vec<Id> = (0..signature.arg_count).map(|_| {
                    let id = next_id;
                    next_id += 1;
                    id
                }).collect();
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
        Ok(module)
    }

    pub fn fresh_id(&mut self) -> Id {
        let id = self.next_id;
        self.next_id += 1;
        id
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
    let name = line
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect::<String>();
    if name.is_empty() || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
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

fn parse_value(token: &str, arg_ids: &[Id]) -> Value {
    token.trim().parse::<i64>().map(Value::Const).unwrap_or_else(|_| {
        if let Some(stripped) = token.trim().strip_prefix('%') {
            if let Ok(index) = stripped.parse::<usize>() {
                if let Some(id) = arg_ids.get(index) {
                    return Value::Arg(*id);
                }
            }
        }
        Value::Local(token.trim().trim_start_matches('%').bytes().fold(0, |acc, b| acc.wrapping_mul(131).wrapping_add(b as u64)))
    })
}

fn parse_instruction_into(line: &str, module: &mut Module, function_index: usize, next_id: &mut Id) {
    let line = line.trim().trim_end_matches(';').trim();
    if line.is_empty() || line.starts_with('{') {
        return;
    }
    let arg_ids = module.functions[function_index].args.clone();
    let instruction = if line.starts_with("return") {
        let value = line.strip_prefix("return").unwrap_or("").trim();
        Some(Instruction::Return(if value.is_empty() {
            Value::Const(0)
        } else {
            parse_value(value, &arg_ids)
        }))
    } else if let Some(rest) = line.strip_prefix('%') {
        let (dest_token, rest) = rest.split_once('=').unwrap_or((rest, ""));
        let dest = dest_token.trim().parse::<Id>().unwrap_or_else(|_| {
            let id = *next_id;
            *next_id += 1;
            id
        });
        let rest = rest.trim();
        if let Some(callee_expr) = rest.strip_prefix("call ") {
            let callee = callee_expr.split('(').next().unwrap_or(callee_expr).trim().to_string();
            let args_text = callee_expr.split_once('(').and_then(|(_, r)| r.rsplit_once(')')).map(|(a, _)| a).unwrap_or("");
            let args = args_text.split(',').map(|a| a.trim()).filter(|a| !a.is_empty()).map(|a| parse_value(a, &arg_ids)).collect();
            Some(Instruction::Call { dest: Some(dest), callee, args })
        } else if let Some(expr) = rest.strip_prefix("add ") {
            let mut parts = expr.split_whitespace();
            let lhs = parts.next().map(|v| parse_value(v, &arg_ids)).unwrap_or(Value::Const(0));
            let rhs = parts.next().map(|v| parse_value(v, &arg_ids)).unwrap_or(Value::Const(0));
            Some(Instruction::Add { dest, lhs, rhs })
        } else if let Some(expr) = rest.strip_prefix("mul ") {
            let mut parts = expr.split_whitespace();
            let lhs = parts.next().map(|v| parse_value(v, &arg_ids)).unwrap_or(Value::Const(0));
            let rhs = parts.next().map(|v| parse_value(v, &arg_ids)).unwrap_or(Value::Const(0));
            Some(Instruction::Mul { dest, lhs, rhs })
        } else {
            rest.parse::<i64>().ok().map(|value| Instruction::Const { dest, value })
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
    fn parse_ir_from_source() {
        let dir = std::env::temp_dir().join(format!("autocomp-ir-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("sample.ac");
        std::fs::write(&path, "int add(int a, int b) {\n  %0 = add %a, %b;\n  return %0;\n}\nint main() {\n  %1 = call add(2, 3);\n  return %1;\n}\n").unwrap();
        let module = Module::from_source(&path).unwrap();
        assert_eq!(module.functions.len(), 2);
        assert_eq!(module.functions[0].name, "add");
        assert_eq!(module.functions[0].instructions.len(), 2);
        assert_eq!(module.functions[0].instructions[0], Instruction::Add { dest: 3, lhs: Value::Arg(1), rhs: Value::Arg(2) });
        assert_eq!(module.functions[1].instructions[0], Instruction::Call { dest: Some(5), callee: "add".into(), args: vec![Value::Const(2), Value::Const(3)] });
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
