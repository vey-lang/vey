//! The `vey build` command.
use std::fs;
use std::path::PathBuf;

use vey::compiler::compile_project;

use crate::cli::args::{parse_args, ParsedArgs};
use crate::cli::config::Config;
use crate::cli::entry::{entry_from, project_root};
use crate::cli::output::{
    node_script, print_command_help, replace_extension, report, report_warnings,
};
use crate::cli::program;
use crate::cli::project::{self, Kind};
use crate::cli::site::write_site;

/// The folder the site is written to.
pub(crate) const OUTPUT_FOLDER: &str = ".dist";

pub(crate) fn cmd_build(args: &[String]) -> i32 {
    let parsed = match parse_args("build", args) {
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
    // The settings of the project may say where the site is written and
    // what its title is.
    let config = Config::find(&root);
    if reject_unsupported_minify(&config) {
        return 1;
    }
    let kind = match project::detect(&entry, &config) {
        Ok(kind) => kind,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    let output = parsed
        .output
        .map(PathBuf::from)
        .or_else(|| config.out.as_ref().map(PathBuf::from))
        .unwrap_or_else(|| root.join(OUTPUT_FOLDER));
    // A server is a program, not a site: `vey build` writes it the way
    // `vey build --node` does, into the folder of the build.
    if kind == Kind::Server {
        if parsed.title.is_some() {
            eprintln!("--title does nothing for a server project: its pages name themselves");
            return 1;
        }
        if parsed.force {
            eprintln!("--force does nothing for a server project: it writes one file");
            return 1;
        }
        return build_server_program(&entry, &output);
    }
    if kind == Kind::Lib {
        eprintln!("this project is a library: there is no site to build");
        eprintln!("name the file to build it as a program: vey build src/lib.vey --node");
        return 1;
    }
    let title = parsed.title.clone().or_else(|| config.title.clone());

    match write_site(&entry, &output, title.as_deref(), parsed.force) {
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
                eprintln!("{} page(s) could not be built.", report.failed.len());
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

/// Build the program of a server into the folder of the build.
fn build_server_program(entry: &std::path::Path, output: &std::path::Path) -> i32 {
    // The folder of the build is where the program goes, unless the name
    // of a file was given.
    let named = output
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension == "cjs" || extension == "js");
    let script = if named {
        output.to_path_buf()
    } else {
        output.join("server.cjs")
    };
    match program::build(entry, &script) {
        Ok(_) => {
            println!("built {}", script.display());
            println!("run it with: node --enable-source-maps {}", script.display());
            0
        }
        Err(err) => {
            eprintln!("{}", err);
            1
        }
    }
}

pub(crate) fn reject_unsupported_minify(config: &Config) -> bool {
    if config.unsupported_minify {
        eprintln!("[build].minify = true is not supported yet; remove it from vey.toml");
        true
    } else {
        false
    }
}

/// Build the site of a project into a folder.
///
/// The command returns 1 when a page could not be built.
pub(crate) fn build_site(
    entry: &std::path::Path,
    output: &std::path::Path,
    title: Option<&str>,
    force: bool,
) -> i32 {
    match write_site(entry, output, title, force) {
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

    if reject_unsupported_minify(&Config::find(&project_root(&file))) {
        return 1;
    }

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
    // The folder the file goes into may not be there yet, so it is made.
    // `vey build --node -o .dist/server.cjs` is the way a container is
    // built, and `.dist` does not exist before the site is built.
    if let Some(parent) = std::path::Path::new(&output).parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(err) = fs::create_dir_all(parent) {
                eprintln!("cannot create {}: {}", parent.display(), err);
                return 1;
            }
        }
    }
    if let Err(err) = fs::write(&output, node_script(&bundle)) {
        eprintln!("cannot write {}: {}", output, err);
        return 1;
    }

    println!("built {}", output);
    0
}
