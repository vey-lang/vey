//! Shared helpers for the integration tests.
//!
//! Every test binary uses a different part of this module, so unused helpers
//! are expected.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A unique temporary directory for one test.
pub fn temp_dir(label: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let index = COUNTER.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "vey-test-{}-{}-{}-{}",
        label,
        std::process::id(),
        nanos,
        index
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// Run a Vey source file as a program.
///
/// The program is built as a node script and run with node, the way a
/// script is run.
pub fn run_file(path: &Path) -> Output {
    let script = build_script(path);
    let output = Command::new("node")
        .arg("--enable-source-maps")
        .arg(&script)
        .output()
        .expect("failed to start node");
    let _ = fs::remove_file(&script);
    output
}

/// Build a Vey file as a node script and return the path of the script.
pub fn build_script(path: &Path) -> PathBuf {
    let dir = temp_dir("script");
    let script = dir.join("program.cjs");
    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(path)
        .arg("--node")
        .arg("-o")
        .arg(&script)
        .output()
        .expect("failed to start vey");
    assert!(
        output.status.success(),
        "cannot build {}:\n{}",
        path.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    script
}

/// Write `source` to `main.vey` inside a fresh directory and run it.
pub fn run_source(label: &str, source: &str) -> Output {
    let dir = temp_dir(label);
    let file = dir.join("main.vey");
    fs::write(&file, source).unwrap();
    run_file(&file)
}

/// Check a file and return the output of the check.
pub fn check_file(path: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("check")
        .arg(path)
        .output()
        .expect("failed to start vey")
}

/// Check a source and return the output of the check.
pub fn check_source(label: &str, source: &str) -> Output {
    let dir = temp_dir(label);
    let file = dir.join("main.vey");
    fs::write(&file, source).unwrap();
    Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("check")
        .arg(&file)
        .output()
        .expect("failed to start vey")
}

/// Run a source that is expected to succeed and return its stdout.
pub fn run_ok(label: &str, source: &str) -> String {
    let output = run_source(label, source);
    assert!(
        output.status.success(),
        "expected success, got:\n{}",
        stderr(&output)
    );
    stdout(&output)
}

/// Run a source that is expected to fail at runtime and return its error.
pub fn run_fails(label: &str, source: &str) -> String {
    let output = run_source(label, source);
    assert!(
        !output.status.success(),
        "expected a failure, got:\n{}",
        stdout(&output)
    );
    stderr(&output)
}

/// Check a source that is expected to fail and return its error.
pub fn run_err(label: &str, source: &str) -> String {
    let output = check_source(label, source);
    assert!(
        !output.status.success(),
        "expected a failure, got:\n{}",
        stdout(&output)
    );
    stderr(&output)
}

/// Run a source and check that stdout contains `expected`.
pub fn assert_prints(label: &str, source: &str, expected: &str) {
    let text = run_ok(label, source);
    assert!(
        text.contains(expected),
        "expected {expected:?} in output:\n{text}"
    );
}

/// Compile a source file into a standalone Node script.
pub fn build_node_script(label: &str, source: &str) -> PathBuf {
    let dir = temp_dir(label);
    let app = dir.join("app.vey");
    fs::write(&app, source).unwrap();
    let bundle = dir.join("app.cjs");
    let output = Command::new(env!("CARGO_BIN_EXE_vey"))
        .arg("build")
        .arg(&app)
        .arg("--node")
        .arg("-o")
        .arg(&bundle)
        .output()
        .expect("failed to start vey");
    assert!(
        output.status.success(),
        "build failed:\n{}",
        stderr(&output)
    );
    bundle
}
