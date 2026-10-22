//! The `vey migrate` command.
//!
//! A file may say which version of the language it was written for, with a
//! comment such as `// vey: 0.7`. The command brings the files of a project
//! to the version of this compiler, and says what it changed.

use std::fs;
use std::path::{Path, PathBuf};

use vey::parser::Parser;

use crate::cli::args::parse_args;
use crate::cli::entry::collect_vey_files;
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
        collect_vey_files(Path::new("."), &mut files);
    } else {
        for name in &parsed.files {
            let path = PathBuf::from(name);
            if path.is_dir() {
                collect_vey_files(&path, &mut files);
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
            print!("{}", migrated);
        }
    }

    if !parsed.write && changed > 0 {
        eprintln!("{} file(s) are not of this version.", changed);
        eprintln!("pass --write to write them.");
    }
    0
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
