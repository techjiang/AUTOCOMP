use autocomp_core::{AutoCompiler, CompileOptions, Profile, ProfileFormat, Workload};
use std::io::Write;
use std::process::{Command, Stdio};
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
            println!("autocomp <command> [options]\n\nCommands:\n  cc               C compiler\n  c++              C++ compiler\n  profile          collect profile\n  optimize         PGO build\n  report           optimization report (HTML)\n  bench            quick benchmark\n\nOptions:\n  -O3              enable optimization level 3\n  --instrument     emit instrumented object\n  --pgo=<file>     compile with profile\n  --output=<file>  output path for profile/report\n  --workload=<s>   workload label for auto PGO
  --no-run         skip running the generated demo binary");
            Ok(())
        }
        other => Err(format!("unknown command: {other}")),
    }
}

fn source_from(args: &[String]) -> Option<PathBuf> {
    args.iter().filter(|a| !a.starts_with('-')).find(|a| a.ends_with(".ac") || a.ends_with(".c") || a.ends_with(".cc") || a.ends_with(".cpp") || a.ends_with(".rs")).map(PathBuf::from)
}

fn option_value(args: &[String], name: &str) -> Option<String> {
    args.iter().find_map(|a| a.strip_prefix(name).map(str::to_string))
}

fn load_profile(args: &[String]) -> Result<Option<Profile>, String> {
    match option_value(args, "--pgo=") {
        Some(path) => Ok(Some(Profile::from_text(&fs::read_to_string(path).map_err(|e| e.to_string())?)?)),
        None => Ok(None),
    }
}

fn print_result(label: &str, result: &autocomp_core::CompileResult) {
    println!("{label} {}", result.products.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
    for decision in &result.decisions {
        println!("  [decision] {decision}");
    }
}

fn compile(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let profile = load_profile(args)?;
    let mut options = CompileOptions {
        optimization_level: args.iter().position(|a| a == "-O3").map(|_| 3).unwrap_or(0),
        instrument: args.iter().any(|a| a == "--instrument"),
        pgo_profile: option_value(args, "--pgo=").map(PathBuf::from),
    };
    if options.instrument {
        options.pgo_profile = None;
    }
    let mut compiler = AutoCompiler::default();
    let result = match profile.as_ref() {
        Some(profile) if !options.instrument => compiler.compile_with_pgo(&source, profile, &options)?,
        _ => compiler.compile(&source, &options)?,
    };
    print_result("compiled", &result);
    Ok(())
}

fn auto_pgo(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let workload_text = option_value(args, "--workload=").unwrap_or_else(|| "default workload".into());
    let workload = Workload {
        argv: workload_text.split_whitespace().map(str::to_string).collect(),
        stdin: None,
        timeout: Duration::from_secs(60),
    };
    let mut compiler = AutoCompiler::default();
    let options = CompileOptions {
        optimization_level: args.iter().position(|a| a == "-O3").map(|_| 3).unwrap_or(0),
        ..CompileOptions::default()
    };
    let result = compiler.compile_auto_pgo(&source, &workload, &options)?;
    print_result("auto-pgo", &result);
    if !args.iter().any(|a| a == "--no-run") {
        if let Some(program) = result.products.iter().find(|p| p.extension().and_then(|e| e.to_str()) == Some("bin")) {
            run_workload(program, &workload)?;
        }
    }
    Ok(())
}

fn profile(args: &[String]) -> Result<(), String> {
    let source = source_from(args).ok_or("missing source file")?;
    let output = option_value(args, "--output=").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("autoprof.txt"));
    let mut compiler = AutoCompiler::default();
    let instrumented = compiler.compile(&source, &CompileOptions { instrument: true, ..CompileOptions::default() })?;
    let workload = Workload { argv: vec![source.display().to_string()], stdin: None, timeout: Duration::from_secs(60) };
    let collected = autocomp_core::WorkloadProfile::from_workload(&workload, &instrumented)?;
    let text = collected.render(ProfileFormat::Text)?;
    fs::write(&output, &text).map_err(|e| e.to_string())?;
    println!("profile written to {} ({} samples)", output.display(), collected.sample_count);
    print!("{text}");
    Ok(())
}

fn report(args: &[String]) -> Result<(), String> {
    let input = args.iter().find(|a| !a.starts_with('-')).ok_or("missing source file")?;
    let source = if input.ends_with(".o") {
        PathBuf::from(format!("{}.ac", input.trim_end_matches(".o")))
    } else {
        PathBuf::from(input)
    };
    let output = option_value(args, "--output=").map(PathBuf::from).unwrap_or_else(|| source.with_extension("report.html"));
    let profile = load_profile(args)?;
    let mut compiler = AutoCompiler::default();
    let options = CompileOptions {
        pgo_profile: option_value(args, "--pgo=").map(PathBuf::from),
        ..CompileOptions::default()
    };
    let result = match profile.as_ref() {
        Some(profile) => compiler.compile_with_pgo(&source, profile, &options)?,
        None => compiler.compile(&source, &options)?,
    };
    let html = autocomp_core::report::render_html(&format!("AUTOCOMP Report · {}", source.display()), &result, profile.as_ref());
    fs::write(&output, html).map_err(|e| e.to_string())?;
    println!("report written to {}", output.display());
    Ok(())
}

fn run_workload(program: &std::path::Path, workload: &Workload) -> Result<(), String> {
    let mut command = Command::new(program);
    command.args(&workload.argv[1..]);
    if workload.stdin.is_some() {
        command.stdin(Stdio::null());
    }
    let output = command.output().map_err(|e| format!("cannot run {}: {e}", program.display()))?;
    std::io::stdout().write_all(&output.stdout).map_err(|e| e.to_string())?;
    std::io::stderr().write_all(&output.stderr).map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!("workload exited with {}", output.status));
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
