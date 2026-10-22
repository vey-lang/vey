//! The `vey fmt` command.
//!
//! The formatter writes the files in the usual shape: the marks of a block
//! are put under the line that opens it, and the lines that close it are
//! put back where the block began.

use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::args::parse_args;
use crate::cli::entry::collect_vey_files;
use crate::cli::output::print_command_help;

/// The number of spaces of one level.
const INDENT: usize = 2;

pub(crate) fn cmd_fmt(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("fmt");
        return 0;
    }

    let files = match collect_files(&parsed.files) {
        Ok(files) => files,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if files.is_empty() {
        eprintln!("no .vey files were found");
        return 1;
    }

    let mut changed = 0;
    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("cannot read {}: {}", file.display(), err);
                return 1;
            }
        };
        let formatted = format_source(&source);
        if formatted == source {
            continue;
        }
        changed += 1;
        if parsed.write {
            if let Err(err) = fs::write(file, &formatted) {
                eprintln!("cannot write {}: {}", file.display(), err);
                return 1;
            }
            println!("wrote {}", file.display());
        } else {
            // The name of the file is said, so that the files may be told
            // apart.
            println!("// {}", file.display());
            print!("{}", formatted);
            if !formatted.ends_with('\n') {
                println!();
            }
        }
    }

    if !parsed.write && changed > 0 {
        eprintln!("{} file(s) are not in the usual shape.", changed);
        eprintln!("pass --write to write them.");
    }
    0
}

/// The files that are named, or every file of the project.
fn collect_files(files: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    if files.is_empty() {
        collect_vey_files(Path::new("."), &mut out);
        return Ok(out);
    }
    for name in files {
        let path = PathBuf::from(name);
        if path.is_dir() {
            collect_vey_files(&path, &mut out);
        } else if path.is_file() {
            out.push(path);
        } else {
            return Err(format!("cannot find {}", path.display()));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Write a source in the usual shape.
pub(crate) fn format_source(source: &str) -> String {
    let mut out = String::new();
    let mut level: usize = 0;
    let mut lines = source.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            // A line with nothing on it stays empty, and the line before an
            // empty line keeps its space.
            if !out.is_empty() && !out.ends_with("\n\n") {
                out.push('\n');
            }
            continue;
        }

        let (opens, closes) = braces(trimmed);
        // A line that closes a block is put back where the block began.
        let mut here = level as isize;
        if closes > 0 && trimmed.starts_with('}') {
            here -= 1;
        }
        if here < 0 {
            here = 0;
        }

        if !out.is_empty() && !out.ends_with("\n\n") {
            out.push('\n');
        }
        for _ in 0..(here as usize * INDENT) {
            out.push(' ');
        }
        out.push_str(trimmed);

        level = (level as isize + opens as isize - closes as isize).max(0) as usize;
    }

    if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// The marks that a line opens and the marks that it closes.
///
/// The marks that are in a text or in a comment do not count.
fn braces(line: &str) -> (usize, usize) {
    let mut opens = 0usize;
    let mut closes = 0usize;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' | '\'' => {
                // The text of a line may hold any mark.
                let quote = c;
                while let Some(next) = chars.next() {
                    if next == '\\' {
                        chars.next();
                        continue;
                    }
                    if next == quote {
                        break;
                    }
                }
            }
            '/' if chars.peek() == Some(&'/') => break,
            '{' => opens += 1,
            '}' => closes += 1,
            _ => {}
        }
    }
    (opens, closes)
}
