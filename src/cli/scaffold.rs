//! The `vey new` command and the project template.

use std::fs;
use std::path::Path;

use crate::cli::args::parse_args;

/// The mark of the language, in the colour of the text around it.
pub(crate) const PROJECT_LOGO: &str = include_str!("../../logo.svg");

/// The mark as a favicon: the same shape on a black ground, so that it is
/// visible on every browser.
pub(crate) const PROJECT_FAVICON: &str = include_str!("../../favicon.svg");

pub(crate) fn cmd_new(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    let Some(name) = parsed.files.first() else {
        eprintln!("usage: vey new <directory>");
        return 1;
    };
    let root = Path::new(name);
    if root.exists() {
        if !root.is_dir() {
            eprintln!("{} already exists and is not a directory", name);
            return 1;
        }
        let not_empty = fs::read_dir(root)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
        if not_empty && !parsed.force {
            eprintln!("{} already exists and is not empty", name);
            eprintln!("pass --force to fill in missing files");
            return 1;
        }
    }

    if let Err(err) = fs::create_dir_all(root) {
        eprintln!("cannot create {}: {}", name, err);
        return 1;
    }

    // The name of the project is used in a few places.
    let project = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| name.clone());

    let files: [(&str, String); 8] = [
        ("src/main.vey", PROJECT_MAIN.to_string()),
        ("src/styles/site.css", PROJECT_STYLE.to_string()),
        ("public/logo.svg", PROJECT_LOGO.to_string()),
        ("public/favicon.svg", PROJECT_FAVICON.to_string()),
        ("public/robots.txt", ROBOTS.to_string()),
        (
            "package.json",
            PROJECT_PACKAGE.replace("{name}", &package_name(&project)),
        ),
        ("README.md", PROJECT_README.replace("{name}", &project)),
        (".gitignore", GITIGNORE.to_string()),
    ];

    for (file, content) in &files {
        let path = root.join(file);
        if path.exists() {
            println!("skipped {}", file);
            continue;
        }
        if let Some(parent) = path.parent() {
            if let Err(err) = fs::create_dir_all(parent) {
                eprintln!("cannot create {}: {}", parent.display(), err);
                return 1;
            }
        }
        if let Err(err) = fs::write(&path, content) {
            eprintln!("cannot write {}: {}", path.display(), err);
            return 1;
        }
        println!("created {}", file);
    }

    println!();
    println!("project ready in {}", name);
    println!("next:");
    println!("  cd {}", name);
    println!("  vey dev");
    0
}

/// The name of the package of a project.
///
/// npm wants a name that is small, without spaces and not starting with a
/// dot.
fn package_name(project: &str) -> String {
    let name: String = project
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let name = name.trim_matches('-').to_string();
    if name.is_empty() {
        "vey-app".to_string()
    } else {
        name
    }
}

pub(crate) const PROJECT_MAIN: &str = r##"use "./styles/site.css"

style {
  .start {
    display: grid;
    align-content: center;
    justify-items: center;
    gap: 1.5rem;
    min-height: 100svh;
    padding: 3rem 1.5rem;
    text-align: center;
  }

  .mark {
    width: 52px;
    height: 52px;
  }

  @media (prefers-color-scheme: dark) {
    .mark { filter: invert(1); }
  }
}

component App() {
  main.start {
    img.mark src="/logo.svg" alt="" width="52" height="52"
    h1 { "Welcome to Vey" }
  }
}

bind("#app", App())
"##;

pub(crate) const PROJECT_STYLE: &str = r##"/* The styles of the site.
   The palette is black and white; the dark mode is the same, turned around. */

:root {
  color-scheme: light dark;

  --sans: ui-sans-serif, system-ui, -apple-system, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  --serif: ui-serif, Georgia, Cambria, "Times New Roman", serif;
  --mono: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;

  --bg: #ffffff;
  --fg: #0b0b0b;
  --muted: #6e6e6e;
  --line: #e3e3e3;
}

@media (prefers-color-scheme: dark) {
  :root {
    --bg: #0b0b0b;
    --fg: #f4f4f4;
    --muted: #8d8d8d;
    --line: #262626;
  }
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  background: var(--bg);
  color: var(--fg);
  font-family: var(--sans);
  font-size: 16px;
  line-height: 1.6;
  -webkit-font-smoothing: antialiased;
}

h1 {
  margin: 0;
  font-family: var(--serif);
  font-size: clamp(1.875rem, 7vw, 2.75rem);
  font-weight: 400;
  line-height: 1.1;
  letter-spacing: -0.02em;
  text-wrap: balance;
}

.code {
  margin: 0;
  color: var(--muted);
  font-family: var(--mono);
  font-size: 0.75rem;
  letter-spacing: 0.16em;
}

.lede {
  margin: 0;
  max-width: 26rem;
  color: var(--muted);
}

a {
  color: inherit;
  text-decoration: none;
  border-bottom: 1px solid var(--line);
  transition: border-color 0.15s ease;
}

a:hover {
  border-bottom-color: var(--fg);
}
"##;

const ROBOTS: &str = "User-agent: *\nAllow: /\n";

const GITIGNORE: &str = "/.dist\n/node_modules\n*.cjs\n.DS_Store\n";

const PROJECT_PACKAGE: &str = r##"{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "description": "A site built with Vey.",
  "scripts": {
    "dev": "vey dev",
    "start": "vey run",
    "build": "vey build",
    "test": "vey test",
    "check": "vey check"
  }
}
"##;

const PROJECT_README: &str = r##"# {name}

A site built with [Vey](https://vey-lang.dev).

## Development

```sh
npm run dev     # or: vey dev
```

The server watches the files of the project, rebuilds when one of them
changes and reloads the page in the browser.

## Build

```sh
npm run build   # or: vey build
```

The site is written to `.dist/`. That folder holds everything the server
needs, so it can be uploaded as it is.

To serve the built site the way a host serves it:

```sh
npm start       # or: vey run
```

## Layout

| Path | What it is |
| --- | --- |
| `src/main.vey` | the page |
| `src/styles/site.css` | the styles of the site |
| `public/` | the files that are copied as they are |

## Styles

The styles of a page may be written in a `style { }` block, next to the
markup they belong to:

```vey
style {
  h1 { font-size: 2rem; }
}
```

A stylesheet that is shared by several pages belongs in `src/styles/`.

## Pages

`src/404.vey` is the page of an unknown address. Without it, Vey writes a
plain page of its own.
"##;
