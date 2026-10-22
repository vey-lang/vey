//! The `vey` command line interface.
//!
//! The commands are split into small modules so that each one can be read on
//! its own: `run` and `check` execute programs, `build` produces output
//! files, `site` writes static sites, `dev` serves a project while it is
//! edited, and `scaffold` creates new projects.

pub mod args;
pub mod build;
pub mod deploy;
pub mod dev;
pub mod docs;
pub mod entry;
pub mod error;
pub mod explain;
pub mod fmt;
pub mod highlight;
pub mod lint;
pub mod migrate;
pub mod output;
pub mod repl;
pub mod run;
pub mod scaffold;
pub mod site;

pub(crate) use build::cmd_build;
pub(crate) use deploy::cmd_deploy;
pub(crate) use dev::cmd_dev;
pub(crate) use docs::cmd_docs;
pub(crate) use explain::cmd_explain;
pub(crate) use fmt::cmd_fmt;
pub(crate) use lint::cmd_lint;
pub(crate) use migrate::cmd_migrate;
pub(crate) use output::print_help;
pub(crate) use repl::cmd_repl;
pub(crate) use run::{cmd_check, cmd_run, cmd_test};
pub(crate) use scaffold::cmd_new;
