//! The `vey docs` command.
//!
//! The documentation of a program is written in the doc comments of its
//! files. The command gathers them and writes a page that says what every
//! piece of the program is for.

use std::fs;
use std::path::{Path, PathBuf};

use vey::parser::Parser;

use crate::cli::args::parse_args;
use crate::cli::entry::collect_tool_files;
use crate::cli::output::print_command_help;

pub(crate) fn cmd_docs(args: &[String]) -> i32 {
    let parsed = match parse_args("docs", args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("docs");
        return 0;
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

    let mut out = String::from("# The program\n\n");
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

        out.push_str(&format!("## {}\n\n", label));
        // The doc comments, in the order they were written.
        for doc in &program.docs {
            if doc.text.is_empty() {
                continue;
            }
            out.push_str(&doc.text);
            out.push('\n');
        }
        // The names the program gives to the outside world.
        let mut names = Vec::new();
        for stmt in &program.body {
            if let Some((name, exported)) = stmt.exported_name() {
                if exported {
                    names.push(name.to_string());
                }
            }
        }
        if !names.is_empty() {
            out.push_str("\nThe program gives to the outside world:\n\n");
            for name in names {
                out.push_str(&format!("- `{}`\n", name));
            }
        }
        out.push('\n');
    }

    match &parsed.output {
        Some(path) => {
            if let Err(err) = fs::write(path, &out) {
                eprintln!("cannot write {}: {}", path, err);
                return 1;
            }
            println!("wrote {}", path);
        }
        None => print!("{}", out),
    }
    0
}
