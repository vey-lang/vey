//! The `vey run`, `vey test` and `vey check` commands.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::entry::{collect_vey_files, entry_from};
use crate::cli::output::{node_script, report, temp_file};

pub(crate) fn cmd_run(args: &[String]) -> i32 {
    // The argument may be a file or a folder that holds one.
    let file = match entry_from(args) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("usage: vey run <file.vey>");
            eprintln!("{}", err);
            return 1;
        }
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
            // A project without tests is not a failure.
            println!("no tests found");
            println!("add a file in tests/ to start testing");
            return 0;
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
        println!("no tests found");
        return 0;
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
            eprintln!("usage: vey check [file.vey|directory]");
            return 1;
        }
    };

    // The entry is checked first, and then every other page of the project.
    let mut watch = Vec::new();
    let mut failed = Vec::new();
    let mut pages = 0;
    for file in project_pages(&entry, &mut watch) {
        pages += 1;
        if let Err(err) = compile_project(&file) {
            report(&err);
            failed.push(file);
        }
    }

    if failed.is_empty() {
        println!("ok: {} page(s) checked", pages);
        0
    } else {
        eprintln!("{} page(s) could not be checked", failed.len());
        1
    }
}

/// The pages of a project, the entry first.
fn project_pages(entry: &Path, watch: &mut Vec<PathBuf>) -> Vec<PathBuf> {
    let mut pages = vec![entry.to_path_buf()];
    let mut files = Vec::new();
    let folder = match entry.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    collect_vey_files(&folder, &mut files);
    files.sort();
    for file in files {
        if file == entry {
            continue;
        }
        watch.push(file.clone());
        pages.push(file);
    }
    pages
}
