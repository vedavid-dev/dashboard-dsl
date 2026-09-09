//! All filesystem access for the compiler lives here; the library is I/O-free.

use std::path::Path;
use std::process::ExitCode;
use vedavid_dashboard_dsl as dsl;

const USAGE: &str = "\
vedavid-dash compile <file.yaml> [-o out.json]
vedavid-dash lint <path>... [--deny-warnings]
vedavid-dash schema render-tree
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("compile") => compile(&args[1..]),
        Some("lint") => lint(&args[1..]),
        Some("schema") => schema(&args[1..]),
        Some("--help") | Some("-h") | None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("unknown subcommand `{other}`\n\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

fn compile(args: &[String]) -> ExitCode {
    let mut input = None;
    let mut output = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-o" | "--output" => match it.next() {
                Some(p) => output = Some(p.clone()),
                None => return fail("-o needs a path"),
            },
            other => input = Some(other.to_string()),
        }
    }
    let Some(input) = input else {
        return fail("compile needs a file");
    };

    let yaml = match std::fs::read_to_string(&input) {
        Ok(s) => s,
        Err(e) => return fail(&format!("cannot read {input}: {e}")),
    };

    let (tree, diags) = dsl::compile_with_diagnostics(&yaml);
    report(&input, &diags);
    let Some(tree) = tree else {
        return ExitCode::from(1);
    };
    let json = dsl::to_json(&tree);
    match output {
        Some(path) => match std::fs::write(&path, json + "\n") {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => fail(&format!("cannot write {path}: {e}")),
        },
        None => {
            println!("{json}");
            ExitCode::SUCCESS
        }
    }
}

fn lint(args: &[String]) -> ExitCode {
    let deny_warnings = args.iter().any(|a| a == "--deny-warnings");
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    if paths.is_empty() {
        return fail("lint needs at least one path");
    }

    let mut errors = 0usize;
    let mut warnings = 0usize;
    for path in paths {
        for file in expand(Path::new(path)) {
            let yaml = match std::fs::read_to_string(&file) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("{}: cannot read: {e}", file.display());
                    errors += 1;
                    continue;
                }
            };
            let (_, diags) = dsl::compile_with_diagnostics(&yaml);
            report(&file.display().to_string(), &diags);
            errors += diags.iter().filter(|d| d.is_error()).count();
            warnings += diags.iter().filter(|d| !d.is_error()).count();
        }
    }

    if errors > 0 || (deny_warnings && warnings > 0) {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

fn schema(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("render-tree") => {
            let s = schemars::schema_for!(dsl::RenderTree);
            match serde_json::to_string_pretty(&s) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(e) => fail(&format!("cannot serialize the schema: {e}")),
            }
        }
        Some("dsl") => fail("the input schema is not generated yet; see README"),
        _ => fail("schema needs `render-tree`"),
    }
}

fn expand(path: &Path) -> Vec<std::path::PathBuf> {
    if path.is_dir() {
        let mut out = Vec::new();
        let Ok(entries) = std::fs::read_dir(path) else {
            return out;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                out.extend(expand(&p));
            } else if matches!(
                p.extension().and_then(|e| e.to_str()),
                Some("yaml") | Some("yml")
            ) {
                out.push(p);
            }
        }
        out.sort();
        out
    } else {
        vec![path.to_path_buf()]
    }
}

fn report(file: &str, diags: &[dsl::Diagnostic]) {
    for d in diags {
        eprintln!("{file}: {d}");
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("{message}");
    ExitCode::from(2)
}
