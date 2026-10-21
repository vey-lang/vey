//! Finding the entry file of a project.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

/// The name of the entry file of a project.
///
/// The entry is always called `main.vey`, so that every project looks the
/// same.
pub(crate) const ENTRY_NAME: &str = "main.vey";

pub(crate) fn entry_from(files: &[String]) -> Result<PathBuf, String> {
    if let Some(first) = files.first() {
        let path = PathBuf::from(first);
        if path.is_dir() {
            return discover_entry(&path);
        }
        if path.is_file() {
            return Ok(path);
        }
        return Err(format!("cannot find {}", path.display()));
    }
    discover_entry(Path::new("."))
}

pub(crate) fn discover_entry(directory: &Path) -> Result<PathBuf, String> {
    // The pages are in `src` when that folder is there, and in the project
    // itself otherwise.
    let mut folders = Vec::new();
    let source = directory.join("src");
    if source.is_dir() {
        folders.push(source);
    }
    folders.push(directory.to_path_buf());

    for folder in &folders {
        let candidate = folder.join(ENTRY_NAME);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }

    // If the caller stands one level above the project, look a little deeper
    // before giving up. Only a single match is accepted so a workspace with
    // several projects keeps requiring an explicit entry.
    let mut found = Vec::new();
    find_entries(directory, 0, &mut found);
    found.sort();
    found.dedup();
    match found.len() {
        0 => Err(format!(
            "no entry file found in {} (looked for {})",
            directory.display(),
            ENTRY_NAME
        )),
        1 => Ok(found.remove(0)),
        _ => Err(format!(
            "multiple entry files found: {} (pass one explicitly)",
            found
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

pub(crate) fn find_entries(directory: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    if depth >= 3 {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || is_ignored_dir(&name) {
                continue;
            }
            find_entries(&path, depth + 1, found);
        } else if kind.is_file() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name == ENTRY_NAME {
                found.push(path);
            }
        }
    }
}

pub(crate) fn is_ignored_dir(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | "target"
            | ".git"
            | "dist"
            | ".dist"
            | "build"
            | ".vey"
            | "tests"
            | "examples"
    )
}

pub(crate) fn project_root(entry: &Path) -> PathBuf {
    let parent = entry
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    // A folder that is called `src` belongs to the project above it.
    if parent.file_name().is_some_and(|name| name == "src") {
        return parent
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }
    parent.to_path_buf()
}

pub(crate) fn default_title(entry: &Path) -> String {
    let stem = entry
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_string())
        .unwrap_or_else(|| "Vey".to_string());
    let root = project_root(entry);
    let name = if matches!(stem.as_str(), "index" | "main" | "app" | "page") {
        if root == Path::new(".") {
            env::current_dir()
                .ok()
                .and_then(|path| {
                    path.file_name()
                        .map(|name| name.to_string_lossy().to_string())
                })
                .unwrap_or_else(|| "Vey".to_string())
        } else {
            root.file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "Vey".to_string())
        }
    } else {
        stem
    };

    let title = name
        .split(|c: char| c == '-' || c == '_' || c.is_whitespace())
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if title.is_empty() {
        "Vey".to_string()
    } else {
        title
    }
}

// ---------------------------------------------------------------------------
// Walking a project directory
// ---------------------------------------------------------------------------

pub(crate) fn collect_vey_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                "node_modules" | "target" | ".git" | "dist" | ".dist" | "build" | ".vey"
            ) {
                continue;
            }
            collect_vey_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "vey") {
            files.push(path);
        }
    }
}

pub(crate) fn collect_files(directory: &Path, files: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                "node_modules" | "target" | ".git" | "dist" | ".dist" | "build" | ".vey"
            ) {
                continue;
            }
            collect_files(&path, files);
        } else {
            files.push(path);
        }
    }
}
