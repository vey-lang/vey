//! Building and starting a program of Node.
//!
//! A project of a server is one program: the compiler turns it into a file
//! of JavaScript, and Node runs that file. The development server and
//! `vey run` build the same file the same way, so what is worked on is what
//! is run.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use vey::compiler::compile_project;

use crate::cli::error::BuildError;
use crate::cli::output::{node_script, report_warnings};

/// Build a program into a file, and say which files the build read.
///
/// The files are watched while the program is worked on, so that a change
/// in an imported module or a stylesheet restarts it as well.
pub(crate) fn build(entry: &Path, script: &Path) -> Result<Vec<PathBuf>, BuildError> {
    let bundle = compile_project(entry).map_err(|err| BuildError::from_vey(&err))?;
    report_warnings(&bundle.warnings);
    if let Some(parent) = script.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|err| {
                BuildError::from(format!("cannot create {}: {}", parent.display(), err))
            })?;
        }
    }
    fs::write(script, node_script(&bundle))
        .map_err(|err| BuildError::from(format!("cannot write {}: {}", script.display(), err)))?;

    let mut watched = vec![entry.to_path_buf()];
    for module in &bundle.modules {
        watched.push(module.path.clone());
    }
    for style in &bundle.styles {
        watched.push(style.path.clone());
    }
    watched.sort();
    watched.dedup();
    Ok(watched)
}

/// Start a built program of Node.
///
/// The port and the host are given to the program as `PORT` and `HOST`, so
/// that the program of a template listens where the command line says. A
/// program that chooses its own address keeps it.
pub(crate) fn start(
    script: &Path,
    root: &Path,
    host: &str,
    port: u16,
    dev: bool,
) -> std::io::Result<Child> {
    let mut command = Command::new("node");
    command
        .arg("--enable-source-maps")
        .arg(script)
        .current_dir(root)
        .env("PORT", port.to_string())
        .env("HOST", host)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    if dev {
        // The program learns that it is worked on, so that the page it
        // serves listens for the next build. The number is new for every
        // build, so that a page writes itself again when it changes.
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        command
            .env("VEY_DEV", "1")
            .env("VEY_DEV_VERSION", nanos.to_string());
    }
    command.spawn()
}

/// End a program, and wait until it is gone.
///
/// The process is asked to stop the way a server of Vey stops on its own:
/// the requests that are running are finished, the sockets are closed, and
/// the program leaves. A program that does not leave is ended.
pub(crate) fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .arg("-TERM")
            .arg(child.id().to_string())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let started = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => {
                if started.elapsed() > std::time::Duration::from_millis(500) {
                    let _ = child.kill();
                    let _ = child.wait();
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return;
            }
        }
    }
}
