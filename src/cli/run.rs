//! The `vey run`, `vey test` and `vey check` commands.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::dev::{serve, DEFAULT_PORT};
use crate::cli::entry::{collect_vey_files, default_title, entry_from};
use crate::cli::output::{node_script, report, temp_file};

pub(crate) fn cmd_run(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    // The page that is served is found the same way as everywhere else.
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey run [entry.vey|directory] [--port 4000]");
            return 1;
        }
    };

    let host = parsed
        .host
        .clone()
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parsed.port.unwrap_or(DEFAULT_PORT);
    let title = parsed
        .title
        .clone()
        .unwrap_or_else(|| default_title(&entry));

    // The site is served the way a host would serve it: without the watcher
    // and without the reload.
    serve(&entry, &title, &host, port, false)
}

pub(crate) fn cmd_test(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
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
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey check [file.vey|directory]");
            return 1;
        }
    };

    // A file that is named is checked on its own, a project is checked as a
    // whole.
    let whole_project = parsed
        .files
        .first()
        .map(|file| Path::new(file).is_dir())
        .unwrap_or(true);

    let mut watch = Vec::new();
    let mut failed = Vec::new();
    let mut pages = 0;
    let files = if whole_project {
        project_pages(&entry, &mut watch)
    } else {
        vec![entry.clone()]
    };
    for file in files {
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
