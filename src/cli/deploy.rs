//! The `vey deploy` command.
//!
//! The command builds the site and says where it is, so that a host may
//! serve it. With a target, the site is copied into a folder that the host
//! already watches.

use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::args::parse_args;
use crate::cli::build::OUTPUT_FOLDER;
use crate::cli::entry::entry_from;
use crate::cli::output::print_command_help;

pub(crate) fn cmd_deploy(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("deploy");
        return 0;
    }

    let entry = match entry_from(&parsed.files) {
        Ok(entry) => entry,
        Err(err) => {
            eprintln!("{}", err);
            eprintln!("usage: vey deploy [entry.vey|directory]");
            return 1;
        }
    };

    let root = crate::cli::entry::project_root(&entry);
    let output = parsed
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join(OUTPUT_FOLDER));

    let build = crate::cli::build::build_site(
        &entry,
        &output,
        parsed.title.as_deref(),
        parsed.force,
    );
    if build != 0 {
        return build;
    }

    // The site may be copied into a folder that a host watches.
    if let Some(target) = &parsed.target {
        let target = PathBuf::from(target);
        if let Err(err) = copy_folder(&output, &target) {
            eprintln!("cannot copy the site into {}: {}", target.display(), err);
            return 1;
        }
        println!("the site is in {}", target.display());
        return 0;
    }

    println!("the site is in {}", output.display());
    println!("serve that folder with any host of static files.");
    0
}

/// Copy a folder into another one.
fn copy_folder(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    if !from.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "the site was not built",
        ));
    }
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let source = entry.path();
        let target = to.join(&name);
        if source.is_dir() {
            copy_folder(&source, &target)?;
        } else {
            fs::copy(&source, &target)?;
        }
    }
    Ok(())
}
