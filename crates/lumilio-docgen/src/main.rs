//! `cargo run -p lumilio-docgen -- ia [--check]` writes (or checks) the
//! generated IA path tables; `-- compare` lists where the handwritten IA and
//! the code's annotations disagree.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lumilio_docgen::{compare, generate, known_ids, scan, stale, validate, write};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = root();
    let (paths, mut problems) = scan(&root);
    problems.extend(validate(&paths, &known_ids(&root)));
    if !problems.is_empty() {
        for problem in &problems {
            eprintln!("{problem}");
        }
        return ExitCode::FAILURE;
    }
    match args.first().map(String::as_str) {
        Some("ia") => {
            let files = generate(&paths);
            if args.iter().any(|arg| arg == "--check") {
                let stale = stale(&root, &files);
                if stale.is_empty() {
                    return ExitCode::SUCCESS;
                }
                eprintln!("stale: {stale:?}; run `cargo run -p lumilio-docgen -- ia`");
                return ExitCode::FAILURE;
            }
            if let Err(error) = write(&root, &files) {
                eprintln!("could not write docs/ia/paths: {error}");
                return ExitCode::FAILURE;
            }
            println!(
                "{} annotated path(s) in {} file(s)",
                paths.len(),
                files.len()
            );
            ExitCode::SUCCESS
        }
        Some("compare") => {
            for (page, difference) in compare(&root, &paths) {
                if difference.claimed_only.is_empty() && difference.annotated_only.is_empty() {
                    continue;
                }
                println!("== {page}");
                for row in &difference.claimed_only {
                    println!("  手写称已实现，代码里没有注解: {row}");
                }
                for row in &difference.annotated_only {
                    println!("  代码有注解，手写表里没有 ✅ 行: {row}");
                }
            }
            ExitCode::SUCCESS
        }
        _ => {
            eprintln!("usage: lumilio-docgen ia [--check] | compare");
            ExitCode::FAILURE
        }
    }
}
