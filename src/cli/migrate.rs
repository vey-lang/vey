//! The `vey migrate` command.
//!
//! A file may say which version of the language it was written for, with a
//! comment such as `// vey: 0.7`. The command brings the files of a project
//! to the version of this compiler, and says what it changed.

use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use vey::parser::Parser;

use crate::cli::args::parse_args;
use crate::cli::entry::collect_tool_files;
use crate::cli::output::print_command_help;

pub(crate) fn cmd_migrate(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("migrate");
        return 1 - 1;
    }

    let mut files = Vec::new();
    if parsed.files.is_empty() {
        collect_tool_files(Path::new("."), &mut files);
    } else {
        for name in &parsed.files {
            let path = PathBuf::from(name);
            if path.is_dir() {
                collect_tool_files(&path, &mut files);
            } else if path.is_file() {
                files.push(path);
            } else {
                eprintln!("cannot find {}", path.display());
                return 1;
            }
        }
    }
    files.sort();
    files.dedup();
    if files.is_empty() {
        eprintln!("no .vey files were found");
        return 1;
    }

    let current = env!("CARGO_PKG_VERSION");
    let mut changed = 0;
    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("cannot read {}: {}", file.display(), err);
                return 1;
            }
        };
        let label = file.display().to_string();
        let mut parser = match Parser::new(&source, &label) {
            Ok(parser) => parser,
            Err(err) => {
                eprintln!("{}", err);
                return 1;
            }
        };
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(err) => {
                eprintln!("{}", err);
                return 1;
            }
        };

        if let Some(version) = program.version.as_deref() {
            match compare_versions(version, current) {
                Some(Ordering::Greater) => {
                    eprintln!(
                        "cannot migrate {}: version {} is newer than this compiler ({})",
                        file.display(),
                        version,
                        current
                    );
                    return 1;
                }
                None => {
                    eprintln!(
                        "cannot migrate {}: cannot compare version '{}'",
                        file.display(),
                        version
                    );
                    return 1;
                }
                Some(Ordering::Less | Ordering::Equal) => {}
            }
        }

        let migrated = migrate_source(&source, program.version.as_deref(), current);
        if migrated == source {
            continue;
        }
        changed += 1;
        if parsed.write {
            if let Err(err) = fs::write(file, &migrated) {
                eprintln!("cannot write {}: {}", file.display(), err);
                return 1;
            }
            println!("migrated {}", file.display());
        } else {
            // The name of the file is said, so that the files may be told
            // apart.
            println!("// {}", file.display());
            print!("{}", migrated);
            if !migrated.ends_with('\n') {
                println!();
            }
        }
    }

    if !parsed.write && changed > 0 {
        eprintln!("{} file(s) are not of this version.", changed);
        eprintln!("pass --write to write them.");
        return 1;
    }
    0
}

fn compare_versions(left: &str, right: &str) -> Option<Ordering> {
    let left: Vec<u64> = left
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let right: Vec<u64> = right
        .split('.')
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    for index in 0..left.len().max(right.len()) {
        let left_part = left.get(index).copied().unwrap_or(0);
        let right_part = right.get(index).copied().unwrap_or(0);
        match left_part.cmp(&right_part) {
            Ordering::Equal => {}
            ordering => return Some(ordering),
        }
    }
    Some(Ordering::Equal)
}

/// Bring a source to the version of this compiler.
///
/// The version of the file is the first thing that changes, so that the
/// compiler may say that a file is too old.
fn migrate_source(source: &str, version: Option<&str>, current: &str) -> String {
    let wanted = format!("// vey: {}", current);
    if version == Some(current) {
        return source.to_string();
    }
    let mut out = String::new();
    let mut written = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("// vey:") {
            if !written {
                out.push_str(&wanted);
                out.push('\n');
                written = true;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    if !written {
        // The version goes at the top of the file.
        out = format!("{}\n{}", wanted, out);
    }
    out
}
