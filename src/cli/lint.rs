//! The `vey lint` command.
//!
//! The linter looks for the smells of a program: the pieces of code that do
//! nothing, the lines that are too long, the nesting that is too deep and
//! the names that are too near one another.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use vey::ast::*;
use vey::lexer::Pos;
use vey::parser::Parser;
use vey::suggest::edit_distance;

use crate::cli::args::parse_args;
use crate::cli::entry::collect_tool_files;
use crate::cli::output::print_command_help;

/// The length of a line that is still easy to read.
const MAX_LINE: usize = 100;

/// How deep the code may be nested.
const MAX_DEPTH: usize = 4;

/// How near two names may be and still be told apart.
const MIN_DISTANCE: usize = 2;

pub(crate) fn cmd_lint(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("lint");
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

    let mut found = 0;
    for file in &files {
        let source = match fs::read_to_string(file) {
            Ok(source) => source,
            Err(err) => {
                eprintln!("cannot read {}: {}", file.display(), err);
                return 1;
            }
        };
        let label = file.display().to_string();

        // The lines that are too long.
        for (index, line) in source.lines().enumerate() {
            if line.chars().count() > MAX_LINE {
                println!(
                    "{}:{}:1: the line is {} characters long, and {} is the most that is easy to read",
                    label,
                    index + 1,
                    line.chars().count(),
                    MAX_LINE
                );
                found += 1;
            }
        }

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

        let mut linter = Linter {
            file: label,
            found: 0,
        };
        linter.walk_program(&program);
        found += linter.found;
    }

    if found == 0 {
        println!("no smells were found");
    } else {
        println!("{} smell(s) were found", found);
    }
    0
}

/// The files that are named, or every file of the project.
fn collect_files(files: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    if files.is_empty() {
        collect_tool_files(Path::new("."), &mut out);
        return Ok(out);
    }
    for name in files {
        let path = PathBuf::from(name);
        if path.is_dir() {
            collect_tool_files(&path, &mut out);
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

struct Linter {
    file: String,
    found: usize,
}

impl Linter {
    fn say(&mut self, pos: Pos, message: impl Into<String>) {
        println!("{}:{}:{}: {}", self.file, pos.line, pos.col, message.into());
        self.found += 1;
    }

    fn walk_program(&mut self, program: &Program) {
        let mut names: HashMap<String, Pos> = HashMap::new();
        self.walk_body(&program.body, 0, &mut names);
    }

    fn walk_body(
        &mut self,
        body: &[Stmt],
        depth: usize,
        names: &mut HashMap<String, Pos>,
    ) {
        if depth > MAX_DEPTH {
            // The depth is reported once, at the deepest place.
            if let Some(stmt) = body.first() {
                let pos = stmt.pos();
                self.say(
                    pos,
                    format!("the code is nested {} levels deep", MAX_DEPTH + 1),
                );
            }
            return;
        }
        for stmt in body {
            self.walk_stmt(stmt, depth, names);
        }
    }

    fn walk_stmt(&mut self, stmt: &Stmt, depth: usize, names: &mut HashMap<String, Pos>) {
        match stmt {
            Stmt::Try(decl) => {
                if let Some(catch) = &decl.catch {
                    if catch.body.is_empty() {
                        self.say(decl.pos, "the catch block is empty");
                    }
                    self.walk_body(&catch.body, depth + 1, names);
                }
                if let Some(finally) = &decl.finally {
                    self.walk_body(&finally.body, depth + 1, names);
                }
                self.walk_body(&decl.body.body, depth + 1, names);
            }
            Stmt::If(decl) => {
                self.walk_body(&decl.then_block.body, depth + 1, names);
                if let Some(branch) = &decl.else_branch {
                    self.walk_stmt(branch, depth + 1, names);
                }
            }
            Stmt::IfLet(decl) => {
                self.walk_body(&decl.then_block.body, depth + 1, names);
                if let Some(branch) = &decl.else_branch {
                    self.walk_stmt(branch, depth + 1, names);
                }
            }
            Stmt::While(decl) => self.walk_body(&decl.body.body, depth + 1, names),
            Stmt::WhileLet(decl) => self.walk_body(&decl.body.body, depth + 1, names),
            Stmt::For(decl) => self.walk_body(&decl.body.body, depth + 1, names),
            Stmt::Repeat(decl) => self.walk_body(&decl.body.body, depth + 1, names),
            Stmt::Block(block, _) => self.walk_body(&block.body, depth + 1, names),
            Stmt::Fn(decl) => {
                self.remember(&decl.name, decl.pos, names);
                // The names of a function are its own, so they are not said
                // against the names of another function.
                let mut inner = names.clone();
                self.walk_body(&decl.body.body, depth + 1, &mut inner);
            }
            Stmt::Component(decl) => {
                self.remember(&decl.name, decl.pos, names);
            }
            Stmt::Var(decl) => {
                if let BindingPattern::Name { name, pos, .. } = &decl.pattern {
                    self.remember(name, *pos, names);
                }
            }
            Stmt::Enum(decl) => {
                self.remember(&decl.name, decl.pos, names);
            }
            _ => {}
        }
    }

    /// Remember a name, and say when it is too near another one.
    fn remember(&mut self, name: &str, pos: Pos, names: &mut HashMap<String, Pos>) {
        for (other, other_pos) in names.iter() {
            if other == name {
                continue;
            }
            let distance = edit_distance(other, name);
            if distance > 0 && distance < MIN_DISTANCE {
                let message = format!(
                    "'{}' is too near '{}', which is on line {}",
                    name, other, other_pos.line
                );
                self.say(pos, message);
                break;
            }
        }
        names.insert(name.to_string(), pos);
    }
}
