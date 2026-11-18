use autocomp_core::{AutoCompiler, CompileOptions, Profile, Workload};
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().collect();
    let command = args.get(1).map(String::as_str).unwrap_or("help");
    match command {
        "cc" | "c++" => compile(&args[2..]),
        "optimize" => auto_pgo(&args[2..]),
        "profile" => profile(&args[2..]),
        "report" => report(&args[2..]),
        "bench" => bench(&args[2..]),
        "help" | "--help" | "-h" => {
            println!("autocomp <command> [options]\n\nCommands:\n  cc               C compiler\n  c++              C++ compiler\n  profile          collect profile\n  optimize         PGO build\n  report           optimization report\n  bench            quick benchmark\n\nOptions:\n  -O3              enable optimization level 3\n  --instrument     emit instrumented object\n  --pgo=<file>     compile with profile\n  --output=<file>  profile output path");
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
    println!("compiled {} -> {}", source.display(), result.products.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    for decision in &result.decisions {
        println!("  [decision] {decision}");
    }
    Ok(())
}

fn auto_pgo(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let workload = Workload { argv: Vec::new(), stdin: None, timeout: Duration::from_secs(60) };
    let options = CompileOptions::default();
    let mut compiler = AutoCompiler::default();
    let result = compiler.compile_auto_pgo(&source, &workload, &options)?;
    println!("auto-pgo {}", result.products.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    for decision in &result.decisions {
        println!("  [decision] {decision}");
    }
    Ok(())
}

fn profile(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let output = args.iter().find_map(|a| a.strip_prefix("--output=")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("autoprof.txt"));
    let module = autocomp_core::ir::Module::from_source(&source)?;
    let mut text = String::new();
    for function in module.functions {
        text.push_str(&format!("{} {}\n", function.name, function.instructions.len().max(1)));
    }
    fs::write(&output, text).map_err(|e| e.to_string())?;
    println!("profile written to {}", output.display());
    Ok(())
}

fn report(args: &[String]) -> Result<(), String> {
    let input = args.iter().find(|a| !a.starts_with('-')).ok_or("missing object file")?;
    let text = fs::read_to_string(input).map_err(|e| e.to_string())?;
    println!("AUTOCOMP optimization report");
    println!("============================");
    for line in text.lines().filter(|l| l.starts_with("; function") || l.starts_with("; AUTOCOMP")) {
        println!("{line}");
    }
    Ok(())
}

fn bench(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let start = std::time::Instant::now();
    let mut compiler = AutoCompiler::default();
    let result = compiler.compile(&source, &CompileOptions::default())?;
    println!("bench {}: {}ms, {} optimization decisions", source.display(), start.elapsed().as_millis(), result.decisions.len());
    Ok(())
}
