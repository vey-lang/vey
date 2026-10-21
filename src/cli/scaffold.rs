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
    let parsed = parse_args(args);
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

    let files: [(&str, String); 11] = [
        ("main.vey", PROJECT_MAIN.to_string()),
        ("404.vey", PROJECT_404.to_string()),
        ("style.css", PROJECT_STYLE.to_string()),
        ("tests/main.vey", PROJECT_TEST.to_string()),
        ("public/logo.svg", PROJECT_LOGO.to_string()),
        ("public/favicon.svg", PROJECT_FAVICON.to_string()),
        ("public/robots.txt", ROBOTS.to_string()),
        ("package.json", PROJECT_PACKAGE.replace("{name}", &project)),
        ("README.md", PROJECT_README.replace("{name}", &project)),
        (".gitignore", GITIGNORE.to_string()),
        ("vey.toml", PROJECT_CONFIG.replace("{name}", &project)),
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

pub(crate) const PROJECT_MAIN: &str = r##"use "./style.css"

component App() {
  main.page {
    p.kicker { "Vey" }
    h1 { "A small language for the web." }
    p.lede {
      "Vey compiles to JavaScript, so the same file runs in the browser and on \
       the server. Types are optional, and checked when you write them."
    }
    hr
    dl.list {
      dt { "vey dev" }
      dd { "Starts the development server." }
      dt { "vey build" }
      dd { "Writes the site to dist/." }
      dt { "vey test" }
      dd { "Runs the tests of the project." }
    }
    hr
    footer { "Built with Vey." }
  }
}

bind("#app", App())
"##;

pub(crate) const PROJECT_404: &str = r##"use "./style.css"

component NotFound() {
  main.page {
    p.kicker { "404" }
    h1 { "Nothing here." }
    p.lede { "The address may be wrong, or the page may have moved." }
    hr
    p { a href="/" { "Go back home" } }
  }
}

bind("#app", NotFound())
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
  line-height: 1.65;
  -webkit-font-smoothing: antialiased;
}

.page {
  max-width: 40rem;
  margin: 0 auto;
  padding: 6rem 1.5rem 5rem;
}

.kicker {
  margin: 0;
  color: var(--muted);
  font-family: var(--mono);
  font-size: 0.6875rem;
  letter-spacing: 0.16em;
  text-transform: uppercase;
}

h1 {
  margin: 1.25rem 0 0;
  font-family: var(--serif);
  font-size: clamp(2.25rem, 8vw, 3.25rem);
  font-weight: 400;
  line-height: 1.1;
  letter-spacing: -0.02em;
  text-wrap: balance;
}

.lede {
  margin: 1.5rem 0 0;
  max-width: 32rem;
  font-size: 1.0625rem;
  color: var(--muted);
}

hr {
  margin: 3.5rem 0;
  border: 0;
  border-top: 1px solid var(--line);
}

.list {
  margin: 0;
}

.list dt {
  font-family: var(--mono);
  font-size: 0.8125rem;
  color: var(--muted);
}

.list dd {
  margin: 0 0 1.1rem;
  padding-left: 10.5rem;
}

@media (max-width: 34rem) {
  .list dd {
    padding-left: 0;
    margin-bottom: 1.5rem;
  }
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

code {
  font-family: var(--mono);
  font-size: 0.875em;
}

footer {
  margin-top: 0;
  color: var(--muted);
  font-size: 0.8125rem;
}
"##;

pub(crate) const PROJECT_TEST: &str = r##"fn add(a: number, b: number) -> number {
  return a + b
}

test("adds numbers", fn() {
  assert(add(2, 3) == 5, "the sum should be 5")
})
"##;

const ROBOTS: &str = "User-agent: *\nAllow: /\n";

const GITIGNORE: &str = "/dist\n/node_modules\n*.cjs\n.DS_Store\n";

const PROJECT_PACKAGE: &str = r##"{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "description": "A site built with Vey.",
  "scripts": {
    "dev": "vey dev",
    "build": "vey build",
    "test": "vey test",
    "check": "vey check main.vey"
  }
}
"##;

const PROJECT_CONFIG: &str = r##"# The settings of the project.

[project]
name = "{name}"

[site]
title = "{name}"
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

The site is written to `dist/`. That folder holds everything the server
needs, so it can be uploaded as it is.

## Tests

```sh
npm test        # or: vey test
```

## Layout

| Path | What it is |
| --- | --- |
| `main.vey` | the page |
| `404.vey` | the page of an unknown address |
| `style.css` | the styles of the site |
| `public/` | the files that are copied as they are |
| `tests/` | the tests of the project |
"##;
