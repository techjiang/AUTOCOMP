use autocomp_core::{AutoCompiler, CompileOptions, Profile, Workload};
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    match command {
        "cc" | "c++" => compile(&args[2..]),
        "optimize" => auto_pgo(&args[2..]),
        "profile" => profile(&args[2..]),
        "help" | "--help" | "-h" => {
            println!("autocomp <command> [options]\n\nCommands:\n  cc\n  c++\n  profile\n  optimize");
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn source_from(args: &[String]) -> Option<PathBuf> {
    args.iter().filter(|a| !a.starts_with('-')).find(|a| a.ends_with(".c") || a.ends_with(".cc") || a.ends_with(".cpp") || a.ends_with(".rs")).map(PathBuf::from)
}

fn compile(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let options = CompileOptions {
        optimization_level: args.iter().position(|a| a == "-O3").map(|_| 3).unwrap_or(0),
        instrument: args.iter().any(|a| a == "--instrument"),
        pgo_profile: args.iter().find_map(|a| a.strip_prefix("--pgo=").map(PathBuf::from)),
    };
    let mut compiler = AutoCompiler::default();
    let mut result = if let Some(profile_path) = &options.pgo_profile {
        let text = std::fs::read_to_string(profile_path).map_err(|e| e.to_string())?;
        let mut profile = Profile::default();
        for line in text.lines() {
            let mut parts = line.split_whitespace();
            if let (Some(name), Some(count)) = (parts.next(), parts.next().and_then(|s| s.parse().ok())) {
                profile.function_counts.insert(name.into(), count);
            }
        }
        compiler.compile_with_pgo(&source, &profile, &options)?
    } else {
        compiler.compile(&source, &options)?
    };
    result.decisions.push(format!("pass outputs: {}", result.products.len()));
    println!("compiled {} -> {:?}", source.display(), result.products);
    Ok(())
}

fn auto_pgo(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let workload = Workload { argv: Vec::new(), stdin: None, timeout: Duration::from_secs(60) };
    let options = CompileOptions::default();
    let mut compiler = AutoCompiler::default();
    let result = compiler.compile_auto_pgo(&source, &workload, &options)?;
    println!("auto-pgo {:?}", result.products);
    Ok(())
}

fn profile(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    println!("profile plan for {}", source.display());
    Ok(())
}
