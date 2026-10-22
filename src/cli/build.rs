//! The `vey build` command.
use std::fs;
use std::path::PathBuf;

use vey::compiler::compile_project;

use crate::cli::args::{parse_args, ParsedArgs};
use crate::cli::entry::{entry_from, project_root};
use crate::cli::output::{node_script, print_command_help, replace_extension, report, report_warnings};
use crate::cli::site::write_site;

/// The folder the site is written to.
pub(crate) const OUTPUT_FOLDER: &str = ".dist";

pub(crate) fn cmd_build(args: &[String]) -> i32 {
    // The `--force` mark is read here, and not by every command.
    let rest: Vec<String> = args
        .iter()
        .filter(|arg| arg.as_str() != "--force")
        .cloned()
        .collect();
    let args = &rest[..];
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("build");
        return 0;
    }

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
            eprintln!("usage: vey build [entry.vey|directory] [-o .dist]");
            return 1;
        }
    };

    let root = project_root(&entry);
    let output = parsed
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(OUTPUT_FOLDER));

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
            // A page that cannot be built is reported, but the others are
            // still written. The command fails, so that a pipeline that
            // builds a site notices the broken page.
            for (page, err) in &report.failed {
                eprintln!("cannot build {}: {}", page, err);
            }
            if !report.failed.is_empty() {
                eprintln!(
                    "{} page(s) could not be built.",
                    report.failed.len()
                );
                return 1;
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

/// Build the site of a project into a folder.
///
/// The command returns 1 when a page could not be built.
pub(crate) fn build_site(entry: &std::path::Path, output: &std::path::Path, title: Option<&str>) -> i32 {
    match write_site(entry, output, title) {
        Ok(report) => {
            for page in &report.pages {
                println!("  {}", output.join(page).display());
            }
            if report.assets > 0 {
                println!("copied {} asset(s) from public/", report.assets);
            }
            for (page, err) in &report.failed {
                eprintln!("cannot build {}: {}", page, err);
            }
            if !report.failed.is_empty() {
                return 1;
            }
            0
        }
        Err(err) => {
            eprintln!("{}", err);
            1
        }
    }
}

pub(crate) fn build_node_script(parsed: &ParsedArgs) -> i32 {
    // A file that is named is the program, otherwise the entry of the project
    // is used.
    let file = match parsed.files.first() {
        Some(file) => PathBuf::from(file),
        None => match entry_from(&[]) {
            Ok(entry) => entry,
            Err(err) => {
                eprintln!("{}", err);
                eprintln!("usage: vey build <file.vey> --node [-o out.cjs]");
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
    report_warnings(&bundle.warnings);

    let output = parsed
        .output
        .clone()
        .unwrap_or_else(|| replace_extension(&file.to_string_lossy(), "cjs"));
    if let Err(err) = fs::write(&output, node_script(&bundle)) {
        eprintln!("cannot write {}: {}", output, err);
        return 1;
    }

    println!("built {}", output);
    0
}
