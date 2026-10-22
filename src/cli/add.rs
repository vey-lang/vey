//! The `vey add` command.
//!
//! The command adds a package of npm to the project, so that a program may
//! use it with `use "npm:name"`.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::args::parse_args;
use crate::cli::output::print_command_help;

pub(crate) fn cmd_add(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("add");
        return 0;
    }

    let Some(name) = parsed.files.first() else {
        eprintln!("usage: vey add <package>");
        return 1;
    };
    let package = name.trim_start_matches("npm:");

    let root = Path::new(".");
    let manifest = root.join("package.json");
    if !manifest.is_file() {
        eprintln!("there is no package.json here, so there is no project to add to");
        return 1;
    }

    // The package is written in the manifest, so that `npm install` knows
    // about it.
    let text = match fs::read_to_string(&manifest) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("cannot read {}: {}", manifest.display(), err);
            return 1;
        }
    };
    let updated = add_dependency(&text, package);
    if let Err(err) = fs::write(&manifest, &updated) {
        eprintln!("cannot write {}: {}", manifest.display(), err);
        return 1;
    }
    println!("added {} to package.json", package);

    // The package is installed, so that it may be used at once.
    let mut npm = Command::new("npm");
    npm.arg("install").arg(package);
    match npm.status() {
        Ok(status) if status.success() => {}
        Ok(_) => {
            eprintln!("npm could not install {}", package);
            eprintln!("run 'npm install' yourself");
        }
        Err(err) => {
            eprintln!("cannot start npm: {}", err);
            eprintln!("install {} yourself", package);
        }
    }
    0
}

/// Add a package to the dependencies of a manifest.
///
/// The manifest is JSON, and the name of the package is written in the
/// place where the other dependencies are.
fn add_dependency(text: &str, package: &str) -> String {
    let key = format!("\"{}\"", package);
    if text.contains(&key) {
        return text.to_string();
    }
    // The dependencies are the last object of the manifest, and the
    // dependencies of the program are the first of them.
    let marker = "\"dependencies\"";
    let Some(start) = text.find(marker) else {
        return text.to_string();
    };
    let Some(brace) = text[start..].find('{') else {
        return text.to_string();
    };
    let brace = start + brace;
    let insert = brace + 1;
    let mut out = String::new();
    out.push_str(&text[..insert]);
    out.push_str(&format!("\n    \"{}\": \"latest\",", package));
    out.push_str(&text[insert..]);
    out
}
