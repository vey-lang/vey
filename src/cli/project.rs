//! What a project is.
//!
//! A project of pages is built into a site, a project of a server is built
//! into a program of Node, and a library is only checked and tested. The
//! kind is written in `vey.toml` as `kind = "page"`, `kind = "server"` or
//! `kind = "lib"`, so that a project says what it is instead of leaving it
//! to a guess. A project that does not say is read: an entry that starts a
//! server is a server, and everything else is a site of pages.

use std::fs;
use std::path::Path;

use crate::cli::config::Config;

/// The shape of a project.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Kind {
    /// Pages are built into a site and served.
    Page,
    /// A program of Node that answers for itself.
    Server,
    /// A library, which has nothing to serve.
    Lib,
}

/// The kind of a project, from its settings or from its entry.
pub(crate) fn detect(entry: &Path, config: &Config) -> Result<Kind, String> {
    if let Some(name) = config.kind.as_deref() {
        return match name {
            "page" => Ok(Kind::Page),
            "server" => Ok(Kind::Server),
            "lib" | "library" => Ok(Kind::Lib),
            other => Err(format!(
                "'{}' is not a kind of project: the kinds are page, server and lib",
                other
            )),
        };
    }
    if entry
        .file_name()
        .is_some_and(|name| name == "lib.vey")
    {
        return Ok(Kind::Lib);
    }
    if starts_a_server(entry) {
        return Ok(Kind::Server);
    }
    Ok(Kind::Page)
}

/// Whether the entry of the project starts a server.
///
/// The program says so itself, with `server(...)`, `.start()` or
/// `.listen(...)`. A project that writes its kind in `vey.toml` is never
/// read this way.
fn starts_a_server(entry: &Path) -> bool {
    let Ok(text) = fs::read_to_string(entry) else {
        return false;
    };
    text.contains("server(") || text.contains(".start()") || text.contains(".listen(")
}
