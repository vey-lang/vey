//! The configuration of a project.
//!
//! The settings of a project are written in `vey.toml` in the folder of
//! the project. The options of the command line are stronger than the file,
//! so that a program may be run in another way for a moment.

use std::fs;
use std::path::Path;

/// The name of the file of the settings.
pub(crate) const FILE: &str = "vey.toml";

/// The settings of a project.
#[derive(Default, Clone)]
pub(crate) struct Config {
    /// The port that the server listens on.
    pub(crate) port: Option<u16>,
    /// The host that the server listens on.
    pub(crate) host: Option<String>,
    /// The title of the site.
    pub(crate) title: Option<String>,
    /// The folder the site is written to.
    pub(crate) out: Option<String>,
    /// The length of a line that is still easy to read.
    pub(crate) max_line: Option<usize>,
    /// Whether the site is made smaller.
    pub(crate) minify: Option<bool>,
}

impl Config {
    /// The settings of the project that the folder belongs to.
    ///
    /// The file is looked for in the folder and above it, so that the
    /// settings of a project are found from any of its folders.
    pub(crate) fn find(folder: &Path) -> Config {
        let mut root = folder.to_path_buf();
        // The settings of a project are not looked for above the home of
        // the machine, so that a folder of the machine does not change the
        // way a program is built by accident.
        for _ in 0..8 {
            let file = root.join(FILE);
            if file.is_file() {
                if let Ok(text) = fs::read_to_string(&file) {
                    return Config::parse(&text);
                }
            }
            match root.parent() {
                Some(parent) if parent != root => root = parent.to_path_buf(),
                _ => break,
            }
        }
        Config::default()
    }

    /// The settings of a file of text.
    ///
    /// The file is small: a name, a value, and a section that the name is
    /// in. A name that is written twice keeps the value that was written
    /// last.
    pub(crate) fn parse(text: &str) -> Config {
        let mut config = Config::default();
        let mut section = String::new();
        for line in text.lines() {
            let line = line.trim();
            // A comment is a word that begins with a mark.
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            // A section is a name between two marks.
            if line.starts_with('[') {
                if let Some(end) = line.find(']') {
                    section = line[1..end].trim().to_string();
                }
                continue;
            }
            let Some((name, value)) = line.split_once('=') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            // A value that is a text is between two marks.
            let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                value[1..value.len() - 1].to_string()
            } else {
                value.to_string()
            };
            let key = if section.is_empty() {
                name.to_string()
            } else {
                format!("{}.{}", section, name)
            };
            config.set(&key, &value);
        }
        config
    }

    /// The value of a name is kept.
    fn set(&mut self, key: &str, value: &str) {
        match key {
            "port" => self.port = value.parse().ok(),
            "host" => self.host = Some(value.to_string()),
            "title" => self.title = Some(value.to_string()),
            "out" => self.out = Some(value.to_string()),
            "lint.maxLine" | "maxLine" => self.max_line = value.parse().ok(),
            "build.minify" | "minify" => self.minify = Some(value == "true"),
            _ => {}
        }
    }
}
