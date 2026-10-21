//! The `vey run`, `vey test` and `vey check` commands.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::entry::{collect_vey_files, discover_entry, entry_from};
use crate::cli::output::{node_script, report, temp_file};

pub(crate) fn cmd_run(args: &[String]) -> i32 {
    let file = match args.first() {
        Some(file) => PathBuf::from(file),
        None => match discover_entry(Path::new(".")) {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("usage: vey run <file.vey>");
                eprintln!("{}", err);
                return 1;
            }
        },
    };

    let bundle = match compile_project(&file) {
        Ok(bundle) => bundle,
        Err(err) => {
            report(&err);
            return 1;
        }
    };

    let script = node_script(&bundle);
    let temp = temp_file("vey-run", "cjs");
    if let Err(err) = fs::write(&temp, script) {
        eprintln!("cannot write temporary file: {}", err);
        return 1;
    }

    let status = Command::new("node")
        .arg("--enable-source-maps")
        .arg(&temp)
        .status();
    let _ = fs::remove_file(&temp);

    match status {
        Ok(status) => status.code().unwrap_or(1),
        Err(err) => {
            eprintln!("cannot start node: {}", err);
            eprintln!("node is required to run Vey programs");
            1
        }
    }
}

pub(crate) fn cmd_test(args: &[String]) -> i32 {
    let parsed = parse_args(args);
    let inputs: Vec<PathBuf> = if parsed.files.is_empty() {
        let tests = PathBuf::from("tests");
        if tests.is_dir() {
            vec![tests]
        } else {
            eprintln!("no tests/ directory found");
            eprintln!("create tests/main.vey or pass a file: vey test <file.vey>");
            return 1;
        }
    } else {
        parsed.files.iter().map(PathBuf::from).collect()
    };

    let mut files = Vec::new();
    for input in inputs {
        if input.is_dir() {
            collect_vey_files(&input, &mut files);
        } else if input
            .extension()
            .is_some_and(|extension| extension == "vey")
        {
            files.push(input);
        }
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        eprintln!("no .vey test files found");
        return 1;
    }

    let mut failed = 0;
    for file in &files {
        println!("test {}", file.display());
        let bundle = match compile_project(file) {
            Ok(bundle) => bundle,
            Err(err) => {
                report(&err);
                failed += 1;
                continue;
            }
        };

        let temp = temp_file("vey-test", "cjs");
        if let Err(err) = fs::write(&temp, node_script(&bundle)) {
            eprintln!("cannot write temporary file: {}", err);
            failed += 1;
            continue;
        }

        let status = Command::new("node")
            .arg("--enable-source-maps")
            .arg(&temp)
            .env("VEY_TEST", "1")
            .status();
        let _ = fs::remove_file(&temp);

        match status {
            Ok(status) if status.success() => {}
            Ok(_) => failed += 1,
            Err(err) => {
                eprintln!("cannot start node: {}", err);
                eprintln!("node is required to run Vey tests");
                failed += 1;
            }
        }
    }

    if failed == 0 {
        println!("{} file(s) passed", files.len());
        0
    } else {
        eprintln!("{} file(s) failed", failed);
        1
    }
}

pub(crate) fn cmd_check(args: &[String]) -> i32 {
    let parsed = parse_args(args);
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey check <file.vey>");
            return 1;
        }
    };

    match compile_project(&entry) {
        Ok(bundle) => {
            println!(
                "ok: {} module(s), {} style source(s)",
                bundle.modules.len(),
                bundle.styles.len()
            );
            0
        }
        Err(err) => {
            report(&err);
            1
        }
    }
}
