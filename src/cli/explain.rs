//! The `vey explain` command.
//!
//! The command says the type of the expression that is written at a place
//! of a file, so that a reader may know what a value is without running the
//! program.

use std::fs;

use vey::parser::Parser;
use vey::types;

use crate::cli::args::parse_args;
use crate::cli::output::print_command_help;

pub(crate) fn cmd_explain(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("explain");
        return 0;
    }

    let Some(target) = parsed.files.first() else {
        eprintln!("usage: vey explain <file.vey>:<line>:<column>");
        return 1;
    };

    let (file, line, col) = match parse_target(target) {
        Some(parts) => parts,
        None => {
            eprintln!("'{}' is not a file, a line and a column", target);
            eprintln!("usage: vey explain <file.vey>:<line>:<column>");
            return 1;
        }
    };

    let source = match fs::read_to_string(&file) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("cannot read {}: {}", file, err);
            return 1;
        }
    };

    let mut parser = match Parser::new(&source, &file) {
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

    match types::explain(&program, &file, line, col) {
        Ok(ty) => {
            println!("{}:{}:{}: {}", file, line, col, ty);
            0
        }
        Err(err) => {
            eprintln!("{}", err);
            1
        }
    }
}

/// A file, a line and a column, written as `file.vey:12:4`.
fn parse_target(target: &str) -> Option<(String, usize, usize)> {
    let mut parts = target.rsplitn(3, ':');
    let col = parts.next()?.parse().ok()?;
    let line = parts.next()?.parse().ok()?;
    let file = parts.next()?;
    if file.is_empty() {
        return None;
    }
    Some((file.to_string(), line, col))
}
