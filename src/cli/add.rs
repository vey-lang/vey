//! The `vey add` command.
//!
//! The command adds a package of npm to the project, so that a program may
//! use it with `use "npm:name"`.

use std::fs;
use std::path::Path;
use std::process::Command;

use crate::cli::args::parse_args;
use crate::cli::json::{self, Value};
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
    let updated = match add_dependency(&text, package) {
        Ok(updated) => updated,
        Err(err) => {
            eprintln!("cannot update {}: {}", manifest.display(), err);
            return 1;
        }
    };
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
            return 1;
        }
        Err(err) => {
            eprintln!("cannot start npm: {}", err);
            eprintln!("install {} yourself", package);
            return 1;
        }
    }
    0
}

/// Add a package to the dependencies of a manifest.
///
/// The manifest is parsed as JSON so package names in unrelated values do not
/// affect where the dependency is written.
fn add_dependency(text: &str, package: &str) -> Result<String, String> {
    let (value, rest) =
        json::read(text).ok_or_else(|| "package.json is not valid JSON".to_string())?;
    if !rest.trim().is_empty() {
        return Err("package.json has text after its JSON value".to_string());
    }
    let Value::Object(mut manifest) = value else {
        return Err("package.json must contain an object".to_string());
    };

    if let Some((_, dependencies)) = manifest.iter_mut().find(|(key, _)| key == "dependencies") {
        let Value::Object(dependencies) = dependencies else {
            return Err("package.json dependencies must be an object".to_string());
        };
        if dependencies.iter().any(|(name, _)| name == package) {
            return Ok(text.to_string());
        }
        dependencies.push((package.to_string(), json::text("latest")));
    } else {
        manifest.push((
            "dependencies".to_string(),
            Value::Object(vec![(package.to_string(), json::text("latest"))]),
        ));
    }

    Ok(format!("{}\n", Value::Object(manifest).to_json()))
}
