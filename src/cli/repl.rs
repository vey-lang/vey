//! The `vey repl` command.
//!
//! The prompt reads a line of the language, builds the program that has been
//! written so far and runs it. What the program prints is shown, and the
//! state of the program is kept between the lines.

use std::fs;
use std::io::{self, BufRead, Write};
use std::process::Command;

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::output::{node_script, print_command_help, temp_file};

pub(crate) fn cmd_repl(args: &[String]) -> i32 {
    let parsed = match parse_args("repl", args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("repl");
        return 0;
    }

    println!("Vey {}. Type a line and press enter. Ctrl-D leaves.", env!("CARGO_PKG_VERSION"));
    println!("The state of the program is kept between the lines.");

    let mut source = String::new();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        print!("> ");
        let _ = io::stdout().flush();
        let Some(line) = lines.next() else {
            break;
        };
        let Ok(line) = line else {
            break;
        };
        let line = line.trim_end().to_string();
        if line.trim() == "exit" || line.trim() == "quit" {
            break;
        }
        if line.trim().is_empty() {
            continue;
        }

        source.push_str(&line);
        source.push('\n');

        match run(&source) {
            Ok(output) => {
                print!("{}", output);
            }
            Err(err) => {
                eprintln!("{}", err);
                // The line that was wrong is taken back, so that the program
                // may be fixed.
                source.truncate(source.len() - line.len() - 1);
            }
        }
    }

    println!();
    0
}

/// Build the program and run it, and return what it printed.
fn run(source: &str) -> Result<String, String> {
    let dir = temp_file("vey-repl", "dir");
    let folder = std::path::PathBuf::from(&dir);
    if let Err(err) = fs::create_dir_all(&folder) {
        return Err(format!("cannot make a folder for the program: {}", err));
    }
    let file = folder.join("main.vey");
    if let Err(err) = fs::write(&file, source) {
        return Err(format!("cannot write the program: {}", err));
    }

    let bundle = match compile_project(&file) {
        Ok(bundle) => bundle,
        Err(err) => {
            let _ = fs::remove_dir_all(&folder);
            return Err(err.to_string());
        }
    };
    let script = folder.join("program.cjs");
    if let Err(err) = fs::write(&script, node_script(&bundle)) {
        let _ = fs::remove_dir_all(&folder);
        return Err(format!("cannot write the program: {}", err));
    }

    let output = Command::new("node")
        .arg(&script)
        .output()
        .map_err(|err| format!("cannot start node: {}", err))?;
    let _ = fs::remove_dir_all(&folder);

    let mut text = String::from_utf8_lossy(&output.stdout).to_string();
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr).to_string();
        if !error.trim().is_empty() {
            return Err(error.trim().to_string());
        }
    }
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    Ok(text)
}
