//! Writing a static site.
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use vey::compiler::{compile_project, Bundle};

use crate::cli::entry::{default_title, project_root};
use crate::cli::error::BuildError;
use crate::cli::output::{escape_html, html_document};

pub(crate) struct SiteReport {
    pub(crate) pages: usize,
    pub(crate) assets: usize,
}

/// The path without any `.` or `..` in it, with links resolved.
///
/// The path does not have to exist, so the deepest part that does is
/// resolved and the rest is kept as it is.
pub(crate) fn absolute(path: &Path) -> PathBuf {
    let start = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };

    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    let mut current = start.clone();
    loop {
        if let Ok(resolved) = current.canonicalize() {
            let mut out = resolved;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match current.file_name() {
            Some(name) => {
                rest.push(name.to_os_string());
                if !current.pop() {
                    return start;
                }
            }
            None => return start,
        }
    }
}

pub(crate) fn write_site(
    entry: &Path,
    output: &Path,
    title: Option<&str>,
) -> Result<SiteReport, BuildError> {
    // The paths are compared after they are resolved, so that a relative path
    // or a `..` cannot point at a directory that holds the project.
    let root = absolute(&project_root(entry));
    let output = absolute(output);

    if output == root {
        return Err(BuildError::from(
            "refusing to write the build into the project root",
        ));
    }
    if root.starts_with(&output) {
        return Err(BuildError::from(format!(
            "refusing to write the build into {}: it contains the project",
            output.display()
        )));
    }
    if output.is_file() {
        return Err(BuildError::from(format!(
            "{} is a file, not a directory",
            output.display()
        )));
    }

    let mut watch = Vec::new();
    let files = site_files(entry, title, &mut watch)?;
    if output.exists() {
        fs::remove_dir_all(&output).map_err(|err| {
            BuildError::from(format!("cannot clean {}: {}", output.display(), err))
        })?;
    }
    fs::create_dir_all(&output)
        .map_err(|err| BuildError::from(format!("cannot create {}: {}", output.display(), err)))?;

    let mut assets = 0;
    for (name, bytes) in &files {
        let path = output.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                BuildError::from(format!("cannot create {}: {}", parent.display(), err))
            })?;
        }
        fs::write(&path, bytes)
            .map_err(|err| BuildError::from(format!("cannot write {}: {}", path.display(), err)))?;
        if name != "index.html" && name != "404.html" {
            assets += 1;
        }
    }

    Ok(SiteReport { pages: 2, assets })
}

pub(crate) fn site_files(
    entry: &Path,
    title: Option<&str>,
    watch: &mut Vec<PathBuf>,
) -> Result<HashMap<String, Vec<u8>>, BuildError> {
    let root = project_root(entry);
    let page_title = title
        .map(str::to_string)
        .unwrap_or_else(|| default_title(entry));

    let bundle = compile_for_site(entry, watch)?;
    let favicon = root.join("public/favicon.svg").is_file();
    let not_found = root.join("404.vey");
    let not_found_body = if not_found.is_file() {
        let bundle = compile_for_site(&not_found, watch)?;
        html_document(&bundle, &format!("{} · 404", page_title), favicon)
    } else {
        default_404_page(&page_title)
    };

    // Static files are copied first so generated pages always win over a
    // stray public/index.html or public/404.html.
    let mut files = HashMap::new();
    let public = root.join("public");
    if public.is_dir() {
        let root = absolute(&root);
        copy_public(&public, "", &mut files, &root)?;
    }
    files.insert(
        "index.html".to_string(),
        html_document(&bundle, &page_title, favicon).into_bytes(),
    );
    files.insert("404.html".to_string(), not_found_body.into_bytes());

    Ok(files)
}

pub(crate) fn copy_public(
    directory: &Path,
    prefix: &str,
    files: &mut HashMap<String, Vec<u8>>,
    root: &Path,
) -> Result<(), BuildError> {
    let entries = fs::read_dir(directory)
        .map_err(|err| BuildError::from(format!("cannot read {}: {}", directory.display(), err)))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".DS_Store" {
            continue;
        }
        // A link that points outside the project would put files of the
        // machine into the site, so it is left out.
        if path.is_symlink() && !stays_inside(&path, root) {
            eprintln!("skipped {}: it points outside the project", path.display());
            continue;
        }
        let key = format!("{}{}", prefix, name);
        if path.is_dir() {
            copy_public(&path, &format!("{}/", key), files, root)?;
        } else if let Ok(bytes) = fs::read(&path) {
            files.insert(key, bytes);
        }
    }
    Ok(())
}

/// Whether the path, once its links are resolved, stays inside the root.
fn stays_inside(path: &Path, root: &Path) -> bool {
    let Ok(resolved) = path.canonicalize() else {
        return false;
    };
    resolved.starts_with(root)
}

pub(crate) fn compile_for_site(
    entry: &Path,
    watch: &mut Vec<PathBuf>,
) -> Result<Bundle, BuildError> {
    let entry = entry.canonicalize().unwrap_or_else(|_| entry.to_path_buf());
    watch.push(entry.clone());
    match compile_project(&entry) {
        Ok(bundle) => {
            watch.push(bundle.entry_path.clone());
            for module in &bundle.modules {
                watch.push(module.path.clone());
            }
            for style in &bundle.styles {
                watch.push(style.path.clone());
            }
            Ok(bundle)
        }
        Err(err) => {
            // Keep watching the file that failed so fixing it triggers a
            // rebuild even when it lives outside the project root.
            watch.push(PathBuf::from(&err.file));
            Err(BuildError::from_vey(&err))
        }
    }
}

pub(crate) fn default_404_page(title: &str) -> String {
    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>404 · {title}</title>
<style>
  :root {{
    color-scheme: light dark;
    --bg: #ffffff;
    --fg: #0b0b0b;
    --muted: #6e6e6e;
    --line: #e3e3e3;
  }}
  @media (prefers-color-scheme: dark) {{
    :root {{ --bg: #0b0b0b; --fg: #f4f4f4; --muted: #8d8d8d; --line: #262626; }}
  }}
  * {{ box-sizing: border-box; }}
  body {{
    margin: 0;
    min-height: 100vh;
    display: grid;
    place-items: center;
    background: var(--bg);
    color: var(--fg);
    font-family: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
    line-height: 1.65;
    -webkit-font-smoothing: antialiased;
  }}
  main {{ max-width: 32rem; padding: 3rem 1.5rem; }}
  .code {{
    margin: 0;
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 0.8125rem;
    letter-spacing: 0.16em;
    color: var(--muted);
  }}
  h1 {{
    margin: 1.25rem 0 0;
    font-family: ui-serif, Georgia, Cambria, "Times New Roman", serif;
    font-size: clamp(2rem, 7vw, 2.75rem);
    font-weight: 400;
    line-height: 1.12;
    letter-spacing: -0.02em;
  }}
  p {{ margin: 1.25rem 0 0; color: var(--muted); }}
  hr {{ margin: 2.5rem 0; border: 0; border-top: 1px solid var(--line); }}
  a {{ color: inherit; text-decoration: none; border-bottom: 1px solid var(--line); }}
  a:hover {{ border-bottom-color: var(--fg); }}
</style>
</head>
<body>
<main>
  <p class="code">404</p>
  <h1>Nothing here.</h1>
  <p>The link may be broken, or the page may have moved.</p>
  <hr>
  <p><a href="/">Go back home</a></p>
</main>
</body>
</html>
"##,
        title = escape_html(title)
    )
}
