//! The `vey build` and `vey web` commands.
use std::fs;
use std::path::{Path, PathBuf};

use vey::compiler::compile_project;

use crate::cli::args::{parse_args, ParsedArgs};
use crate::cli::entry::{default_title, entry_from, project_root};
use crate::cli::output::{html_document, node_script, replace_extension, report};
use crate::cli::site::write_site;

pub(crate) fn cmd_build(args: &[String]) -> i32 {
    let parsed = parse_args(args);

    let node_output = parsed
        .output
        .as_deref()
        .is_some_and(|output| output.ends_with(".cjs") || output.ends_with(".js"));
    if parsed.node || node_output {
        return build_node_script(&parsed);
    }

    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey build [entry.vey|directory] [-o dist]");
            return 1;
        }
    };

    let root = project_root(&entry);
    let output = parsed
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("dist"));

    match write_site(&entry, &output, parsed.title.as_deref()) {
        Ok(report) => {
            println!(
                "built {} page(s) into {}",
                report.pages.len(),
                output.display()
            );
            for page in &report.pages {
                println!("  {}", output.join(page).display());
            }
            if report.assets > 0 {
                println!("copied {} asset(s) from public/", report.assets);
            }
            println!("deploy the {} folder directly.", output.display());
            0
        }
        Err(err) => {
            eprintln!("{}", err);
            1
        }
    }
}

pub(crate) fn build_node_script(parsed: &ParsedArgs) -> i32 {
    let Some(file) = parsed.files.first() else {
        eprintln!("usage: vey build <file.vey> --node [-o out.cjs]");
        return 1;
    };

    let bundle = match compile_project(Path::new(file)) {
        Ok(bundle) => bundle,
        Err(err) => {
            report(&err);
            return 1;
        }
    };

    let output = parsed
        .output
        .clone()
        .unwrap_or_else(|| replace_extension(file, "cjs"));
    if let Err(err) = fs::write(&output, node_script(&bundle)) {
        eprintln!("cannot write {}: {}", output, err);
        return 1;
    }

    println!("built {}", output);
    0
}

pub(crate) fn cmd_web(args: &[String]) -> i32 {
    let parsed = parse_args(args);
    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey web [entry.vey] [-o index.html]");
            return 1;
        }
    };

    let bundle = match compile_project(&entry) {
        Ok(bundle) => bundle,
        Err(err) => {
            report(&err);
            return 1;
        }
    };

    let output = parsed
        .output
        .clone()
        .unwrap_or_else(|| replace_extension(&entry.display().to_string(), "html"));
    let title = parsed
        .title
        .clone()
        .unwrap_or_else(|| default_title(&entry));
    let favicon = project_root(&entry).join("public/favicon.svg").is_file();

    if let Err(err) = fs::write(&output, html_document(&bundle, &title, favicon)) {
        eprintln!("cannot write {}: {}", output, err);
        return 1;
    }

    println!("built {}", output);
    0
}
