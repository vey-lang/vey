//! The examples of the readme have to work.
//!
//! A block of Vey code in the readme that is marked with `<!-- check -->` is
//! a whole program, so it is checked here. The other blocks are pieces that
//! show one thing and are left alone.

mod common;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The blocks of the readme that are checked.
fn marked_blocks(readme: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut rest = readme;
    while let Some(start) = rest.find("```vey\n") {
        let after = &rest[start + 7..];
        let Some(end) = after.find("```") else {
            break;
        };
        let block = &after[..end];
        let tail = &after[end..];
        // The marker stands right after the block.
        let marker = tail.find("<!-- check -->").unwrap_or(usize::MAX);
        let next = tail.find("```vey\n").unwrap_or(usize::MAX);
        if marker < next {
            blocks.push(block.to_string());
        }
        rest = &tail[1..];
    }
    blocks
}

#[test]
fn the_examples_of_the_readme_are_valid() {
    let readme = fs::read_to_string("README.md").expect("the readme is missing");
    let blocks = marked_blocks(&readme);
    assert!(!blocks.is_empty(), "no examples are marked in the readme");

    for (index, block) in blocks.iter().enumerate() {
        let dir: PathBuf = common::temp_dir("readme");
        let file = dir.join("main.vey");
        fs::write(&file, block).unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_vey"))
            .arg("check")
            .arg(&file)
            .output()
            .expect("failed to start vey");
        assert!(
            output.status.success(),
            "the example {} of the readme does not work:\n{}\n{}",
            index + 1,
            block,
            common::stderr(&output)
        );
    }
}

#[test]
fn every_helper_of_the_runtime_is_in_the_readme() {
    let readme = fs::read_to_string("README.md").expect("the readme is missing");
    let builtins = fs::read_to_string("src/builtins.rs").expect("the list of helpers is missing");
    let names = helper_names(&builtins);
    assert!(!names.is_empty(), "no helpers were found");

    for name in names {
        assert!(
            readme.contains(&format!("`{}(", name))
                || readme.contains(&format!("`{}`", name))
                || readme.contains(&format!("`{}.", name)),
            "the readme does not mention '{}'",
            name
        );
    }
}

#[test]
fn every_helper_of_the_runtime_has_a_type() {
    // The names of the helpers and their types are two lists that have
    // drifted before, so a test keeps them together.
    let builtins = fs::read_to_string("src/builtins.rs").expect("the list of helpers is missing");
    let types = fs::read_to_string("src/types/builtins.rs").expect("the types are missing");
    let names = helper_names(&builtins);

    // Only the arms of `builtin_type` are read, because the names of the
    // methods of a value are in the same file.
    let body = types
        .split("fn builtin_type")
        .nth(1)
        .unwrap_or_default()
        .split("\n}\n")
        .next()
        .unwrap_or_default();
    let mut typed = std::collections::HashSet::new();
    for name in body.split('"').skip(1).step_by(2) {
        typed.insert(name.to_string());
    }

    let missing: Vec<&String> = names.iter().filter(|name| !typed.contains(*name)).collect();
    assert!(
        missing.is_empty(),
        "these helpers have no type: {:?}",
        missing
    );
}

/// The names of the helpers of the language.
fn helper_names(builtins: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in builtins.lines() {
        let line = line.trim();
        let Some(start) = line.find('"') else {
            continue;
        };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let name = &rest[..end];
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            names.push(name.to_string());
        }
    }
    names
}

#[test]
fn every_file_of_the_project_is_in_the_layout() {
    let readme = fs::read_to_string("README.md").expect("the readme is missing");
    let mut files = Vec::new();
    for folder in ["src", "tests"] {
        collect_rust_files(Path::new(folder), &mut files);
    }
    assert!(!files.is_empty(), "no files were found");

    for file in files {
        let name = file
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        assert!(
            readme.contains(&name),
            "the readme does not mention '{}'",
            file.display()
        );
    }
}

/// The Rust files of a folder and the folders below it.
fn collect_rust_files(folder: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rust_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}
