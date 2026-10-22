//! The `vey run`, `vey test` and `vey check` commands.
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use vey::compiler::compile_project;

use crate::cli::args::parse_args;
use crate::cli::config::Config;
use crate::cli::dev::{serve, DEFAULT_PORT};
use crate::cli::entry::{collect_tool_files, collect_vey_files, default_title, entry_from, project_root};
use crate::cli::output::{node_script, print_command_help, report, report_warnings, temp_file};

pub(crate) fn cmd_run(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("run");
        return 0;
    }
    // The page that is served is found the same way as everywhere else.
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey run [entry.vey|directory] [--port 4000]");
            return 1;
        }
    };

    // The settings of the project may say where the server listens and
    // what the title of the site is.
    let config = Config::find(&project_root(&entry));
    let host = parsed
        .host
        .clone()
        .or_else(|| config.host.clone())
        .unwrap_or_else(|| "127.0.0.1".to_string());
    let port = parsed.port.or(config.port).unwrap_or(DEFAULT_PORT);
    let title = parsed
        .title
        .clone()
        .or_else(|| config.title.clone())
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
    if parsed.help {
        print_command_help("test");
        return 0;
    }
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
        } else {
            eprintln!("cannot find {}", input.display());
            return 1;
        }
    }
    files.sort();
    files.dedup();

    if files.is_empty() {
        println!("no tests found");
        return 0;
    }

    if parsed.watch {
        // The tests are run again and again, as long as the files change.
        loop {
            let (failed, watched_files) = run_tests(&files, &parsed);
            if !wait_for_change(&watched_files) {
                break;
            }
            if failed > 0 {
                continue;
            }
        }
        return 0;
    }

    let (failed, _) = run_tests(&files, &parsed);
    if failed == 0 {
        println!("{} file(s) passed", files.len());
        0
    } else {
        eprintln!("{} file(s) failed", failed);
        1
    }
}

/// Run the tests of the files, and say how many of them failed.
fn run_tests(
    files: &[PathBuf],
    parsed: &crate::cli::args::ParsedArgs,
) -> (usize, Vec<PathBuf>) {
    let mut failed = 0;
    // The number of the tests that passed and of the ones that failed.
    let mut passed = 0usize;
    let mut broken = 0usize;
    let mut watched_files = files.to_vec();
    for file in files {
        if let Some(parent) = file.parent() {
            watched_files.push(parent.to_path_buf());
        }
    }
    for file in files {
        println!("test {}", file.display());
        let bundle = match compile_project(file) {
            Ok(bundle) => bundle,
            Err(err) => {
                let error_path = PathBuf::from(&err.file);
                if let Some(parent) = error_path.parent() {
                    watched_files.push(parent.to_path_buf());
                }
                watched_files.push(error_path);
                report(&err);
                failed += 1;
                continue;
            }
        };
        for module in &bundle.modules {
            watched_files.push(module.path.clone());
            if let Some(parent) = module.path.parent() {
                watched_files.push(parent.to_path_buf());
            }
        }
        for style in &bundle.styles {
            watched_files.push(style.path.clone());
            if let Some(parent) = style.path.parent() {
                watched_files.push(parent.to_path_buf());
            }
        }

        let temp = temp_file("vey-test", "cjs");
        if let Err(err) = fs::write(&temp, node_script(&bundle)) {
            eprintln!("cannot write temporary file: {}", err);
            failed += 1;
            continue;
        }
        report_warnings(&bundle.warnings);

        // The test may take a while, but not for ever.
        let output = Command::new("node")
            .arg("--enable-source-maps")
            .arg(&temp)
            .env("VEY_TEST", "1")
            .envs(node_path(file))
            .envs(test_filter(parsed))
            .output();
        let _ = fs::remove_file(&temp);

        let output = match output {
            Ok(output) => output,
            Err(err) => {
                eprintln!("cannot start node: {}", err);
                eprintln!("node is required to run Vey tests");
                failed += 1;
                continue;
            }
        };
        let text = String::from_utf8_lossy(&output.stdout).to_string();
        print!("{}", text);
        let errors = String::from_utf8_lossy(&output.stderr).to_string();
        if !errors.is_empty() {
            eprint!("{}", errors);
        }
        // The number of the tests is said by the program, so that the one
        // that runs them may tell it.
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("VEY_TESTS: ") {
                let mut parts = rest.split(", ");
                if let Some(good) = parts.next() {
                    passed += good
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                }
                if let Some(bad) = parts.next() {
                    broken += bad
                        .split_whitespace()
                        .next()
                        .and_then(|n| n.parse().ok())
                        .unwrap_or(0);
                }
            }
        }

        if output.status.success() {
            continue;
        }
        failed += 1;
    }
    if passed + broken > 0 {
        println!("{} test(s) passed, {} failed", passed, broken);
    }
    watched_files.sort();
    watched_files.dedup();
    (failed, watched_files)
}

/// Wait until one of the files changes.
///
/// The files are looked at now and then, and the wait ends as soon as one
/// of them is not the same as it was.
fn wait_for_change(files: &[PathBuf]) -> bool {
    let previous: Vec<(PathBuf, u64, u64)> = files
        .iter()
        .map(|path| {
            let metadata = fs::metadata(path).ok();
            let modified = metadata
                .as_ref()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_nanos() as u64)
                .unwrap_or(0);
            let size = metadata.as_ref().map(|metadata| metadata.len()).unwrap_or(0);
            (path.clone(), modified, size)
        })
        .collect();

    loop {
        thread::sleep(Duration::from_millis(300));
        let current: Vec<(PathBuf, u64, u64)> = files
            .iter()
            .map(|path| {
                let metadata = fs::metadata(path).ok();
                let modified = metadata
                    .as_ref()
                    .and_then(|metadata| metadata.modified().ok())
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map(|duration| duration.as_nanos() as u64)
                    .unwrap_or(0);
                let size = metadata.as_ref().map(|metadata| metadata.len()).unwrap_or(0);
                (path.clone(), modified, size)
            })
            .collect();
        if current != previous {
            return true;
        }
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
    if parsed.help {
        print_command_help("check");
        return 0;
    }
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
        let mut files = project_pages(&entry, &mut watch);
        // The folder of the tests holds pages of the language as well, and
        // they are checked before they are run.
        let mut tests = Vec::new();
        let folders = project_root(&entry).join("tests");
        if folders.is_dir() {
            collect_tool_files(&folders, &mut tests);
        }
        tests.sort();
        for file in tests {
            if files.contains(&file) {
                continue;
            }
            watch.push(file.clone());
            files.push(file);
        }
        files
    } else {
        vec![entry.clone()]
    };
    for file in files {
        pages += 1;
        match compile_project(&file) {
            Ok(bundle) => report_warnings(&bundle.warnings),
            Err(err) => {
                report(&err);
                failed.push(file);
            }
        }
    }

    if failed.is_empty() {
        println!("ok: {} page(s) checked", pages);
        0
    } else {
        eprintln!("{} of {} page(s) could not be checked", failed.len(), pages);
        1
    }
}

/// The name of the tests that are run, as the runtime wants it.
fn test_filter(parsed: &crate::cli::args::ParsedArgs) -> Vec<(String, String)> {
    match &parsed.filter {
        Some(filter) => vec![("VEY_TEST_FILTER".to_string(), filter.clone())],
        None => Vec::new(),
    }
}

/// The folders node looks for the packages of npm in.
///
/// The script of a test lives in a temporary folder, so the folders of the
/// project are named here.
fn node_path(file: &Path) -> Vec<(String, String)> {
    let mut folders = Vec::new();
    let mut folder = file.parent().map(Path::to_path_buf);
    while let Some(current) = folder {
        let modules = current.join("node_modules");
        if modules.is_dir() {
            folders.push(modules.to_string_lossy().to_string());
        }
        folder = current.parent().map(Path::to_path_buf);
    }
    if folders.is_empty() {
        return Vec::new();
    }
    vec![("NODE_PATH".to_string(), folders.join(":"))]
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
