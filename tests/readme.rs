//! The examples of the readme have to work.
//!
//! A block of Vey code in the readme that is marked with `<!-- check -->` is
//! a whole program, so it is checked here. The other blocks are pieces that
//! show one thing and are left alone.

mod common;

use std::fs;
use std::path::PathBuf;
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
