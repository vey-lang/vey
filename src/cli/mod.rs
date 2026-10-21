//! The `vey` command line interface.
//!
//! The commands are split into small modules so that each one can be read on
//! its own: `run` and `check` execute programs, `build` produces output
//! files, `site` writes static sites, `dev` serves a project while it is
//! edited, and `scaffold` creates new projects.

pub mod args;
pub mod build;
pub mod dev;
pub mod entry;
pub mod error;
pub mod highlight;
pub mod output;
pub mod run;
pub mod scaffold;
pub mod site;

pub(crate) use build::{cmd_build, cmd_web};
pub(crate) use dev::cmd_dev;
pub(crate) use output::print_help;
pub(crate) use run::{cmd_check, cmd_run, cmd_test};
pub(crate) use scaffold::cmd_new;
