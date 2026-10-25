//! A stand-in for `java` that runs on every platform. Not a module: the
//! tests compile it on its own (`testing::fake_java`) and copy it into a
//! runtime's `bin`, where it acts out the `<name>.fake` file beside it, one
//! step per line:
//!
//! - `print TEXT`, `eprint TEXT`: TEXT on standard output or error
//! - `write TEXT > FILE`: TEXT into FILE
//! - `args > FILE`, `args >> FILE`: the arguments on one line, like `"$*"`
//! - `args-lines > FILE`: one argument per line
//! - `check FILE = TEXT`: exit 1 unless FILE holds TEXT
//! - `sleep SECONDS`, `exit CODE`
//!
//! `$NAME` in a step is the environment variable NAME, `$*` the arguments
//! on one line. With no file it does nothing and exits 0.

use std::io::Write;
use std::process::ExitCode;
use std::time::Duration;
use std::{env, fs, thread};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let script = env::current_exe()
        .map(|exe| exe.with_extension("fake"))
        .and_then(fs::read_to_string)
        .unwrap_or_default();
    for line in script.lines().map(|line| expand(line, &args)) {
        let (step, rest) = line.split_once(' ').unwrap_or((&line, ""));
        match step {
            "" => {}
            "print" => println!("{rest}"),
            "eprint" => eprintln!("{rest}"),
            "check" => {
                let (file, text) = rest.split_once(" = ").expect("check FILE = TEXT");
                let found = fs::read_to_string(file).unwrap_or_default();
                if found.trim_end() != text {
                    eprintln!("{file} holds {found:?}, not {text:?}");
                    return ExitCode::FAILURE;
                }
            }
            "write" => {
                let (text, file) = rest.rsplit_once(" > ").expect("write TEXT > FILE");
                fs::write(file, format!("{text}\n")).expect("the file is writable");
            }
            "args" | "args-lines" => {
                let text = if step == "args" {
                    format!("{}\n", args.join(" "))
                } else {
                    args.iter().map(|arg| format!("{arg}\n")).collect()
                };
                let (append, file) = match rest.split_once(' ') {
                    Some((">>", file)) => (true, file),
                    Some((">", file)) => (false, file),
                    _ => panic!("{step} > FILE or {step} >> FILE"),
                };
                fs::OpenOptions::new()
                    .create(true)
                    .write(true)
                    .append(append)
                    .truncate(!append)
                    .open(file)
                    .and_then(|mut out| out.write_all(text.as_bytes()))
                    .expect("the file is writable");
            }
            "sleep" => thread::sleep(Duration::from_secs(rest.parse().expect("whole seconds"))),
            "exit" => {
                std::io::stdout().flush().ok();
                return ExitCode::from(rest.parse::<u8>().expect("an exit code"));
            }
            _ => panic!("unknown step `{step}`"),
        }
    }
    ExitCode::SUCCESS
}

/// The line with `$*` replaced by the arguments and each `$NAME` by that
/// environment variable.
fn expand(line: &str, args: &[String]) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(at) = rest.find('$') {
        out.push_str(&rest[..at]);
        if rest[at + 1..].starts_with('*') {
            out.push_str(&args.join(" "));
            rest = &rest[at + 2..];
            continue;
        }
        let name_len = rest[at + 1..]
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len() - at - 1);
        let name = &rest[at + 1..at + 1 + name_len];
        out.push_str(&env::var(name).unwrap_or_default());
        rest = &rest[at + 1 + name_len..];
    }
    out.push_str(rest);
    out
}
