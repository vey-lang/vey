//! The `vey new` command and the project template.
use std::fs;
use std::path::Path;

use crate::cli::args::parse_args;

/// The logo that is copied into a new project.
pub(crate) const PROJECT_LOGO: &str = include_str!("../../logo.svg");

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

    let files: [(&str, &str); 9] = [
        ("main.vey", PROJECT_MAIN),
        ("404.vey", PROJECT_404),
        ("style.css", PROJECT_STYLE),
        ("tests/main.vey", PROJECT_TEST),
        ("public/logo.svg", PROJECT_LOGO),
        ("public/favicon.svg", PROJECT_LOGO),
        ("public/robots.txt", "User-agent: *\nAllow: /\n"),
        ("README.md", PROJECT_README),
        (".gitignore", "/dist\n*.cjs\n.DS_Store\nnode_modules\n"),
    ];

    for (file, content) in files {
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
            eprintln!("cannot write {}: {}", file, err);
            return 1;
        }
        println!("created {}", file);
    }

    println!();
    println!("project ready in {}", name);
    println!("next:");
    println!("  cd {}", name);
    println!("  vey run dev");
    0
}

pub(crate) const PROJECT_MAIN: &str = r##"use "./style.css"

component App() {
  main.welcome {
    img.sparkle src="/logo.svg" alt="Vey" width="76" height="76"
    h1 { "Welcome to " span { "Vey" } }
    p.file { "main.vey" }
  }
}

bind("#app", App())
"##;

pub(crate) const PROJECT_404: &str = r##"use "./style.css"

component NotFound() {
  main.error-page {
    img.sparkle src="/logo.svg" alt="Vey" width="64" height="64"
    p.error-code { "404" }
    h1 { "This page is missing." }
    p { "The link may be broken, or the page may have moved." }
    a.btn.primary href="/" { "Back home" }
  }
}

bind("#app", NotFound())
"##;

pub(crate) const PROJECT_STYLE: &str = r##":root {
  color-scheme: dark;
  --bg: #0f172a;
  --surface: #1e293b;
  --border: #334155;
  --text: #e2e8f0;
  --muted: #94a3b8;
  --accent: #818cf8;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
  min-height: 100vh;
  font-family: system-ui, -apple-system, "Segoe UI", sans-serif;
  background:
    radial-gradient(760px 420px at 50% -10%, rgba(129, 140, 248, 0.15), transparent 70%),
    var(--bg);
  color: var(--text);
  -webkit-font-smoothing: antialiased;
}

body::before {
  content: "";
  position: fixed;
  inset: 0;
  background-image:
    linear-gradient(rgba(148, 163, 184, 0.055) 1px, transparent 1px),
    linear-gradient(90deg, rgba(148, 163, 184, 0.055) 1px, transparent 1px);
  background-size: 72px 72px;
  -webkit-mask-image: radial-gradient(circle at 50% 44%, black, transparent 70%);
  mask-image: radial-gradient(circle at 50% 44%, black, transparent 70%);
  pointer-events: none;
}

a {
  color: inherit;
  text-decoration: none;
}

.welcome {
  position: relative;
  z-index: 1;
  display: grid;
  min-height: 100vh;
  min-height: 100svh;
  place-content: center;
  padding: 40px;
  text-align: center;
}

.sparkle {
  display: block;
  width: 76px;
  height: 76px;
  aspect-ratio: 1;
  margin: 0 auto 30px;
  filter: drop-shadow(0 0 30px rgba(129, 140, 248, 0.35));
}

.welcome h1 {
  margin: 0;
  font-size: clamp(44px, 10vw, 88px);
  font-weight: 600;
  letter-spacing: -0.055em;
  line-height: 0.98;
  text-wrap: balance;
}

.welcome h1 span {
  color: var(--accent);
}

.file {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  margin: 30px auto 0;
  color: var(--muted);
  font: 12px/1 ui-monospace, SFMono-Regular, Menlo, monospace;
  letter-spacing: 0.08em;
}

.file::before {
  content: "";
  width: 20px;
  height: 1px;
  background: currentColor;
  opacity: 0.5;
}

.btn {
  display: inline-block;
  padding: 11px 22px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--surface);
  color: var(--text);
  font: inherit;
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s ease, border-color 0.15s ease;
}

.btn:hover {
  border-color: var(--accent);
  background: #24324a;
}

.btn.primary {
  border-color: transparent;
  background: var(--accent);
  color: var(--bg);
}

.btn.primary:hover {
  background: #93a0ff;
}

.btn:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 3px;
}

.error-page {
  position: relative;
  z-index: 1;
  display: grid;
  min-height: 100vh;
  min-height: 100svh;
  place-items: center;
  padding: 32px;
  text-align: center;
}

.error-page .sparkle {
  width: 64px;
  height: 64px;
  margin-bottom: 20px;
}

.error-code {
  margin: 0;
  color: var(--accent);
  font-size: clamp(72px, 16vw, 120px);
  font-weight: 800;
  letter-spacing: -0.05em;
}

.error-page h1 {
  margin: 0 0 10px;
  font-size: clamp(22px, 4vw, 30px);
}

.error-page p {
  max-width: 400px;
  margin: 0 auto 24px;
  color: var(--muted);
  line-height: 1.6;
}

@media (max-width: 600px) {
  .welcome {
    padding: 24px;
  }
}
"##;

pub(crate) const PROJECT_TEST: &str = r##"fn greeting() -> string {
  return "Vey"
}

test("greets", fn() {
  assert(greeting() == "Vey", "expected Vey")
})
"##;

pub(crate) const PROJECT_README: &str = r##"# Vey project

A website written in [Vey](https://vey.dev): components, CSS and a static site
builder in one binary.

## Development

```sh
vey run dev
```

The dev server prints a local URL, rebuilds on every save and reloads the
browser. Compile errors show up in the page instead of the terminal only.

```sh
vey dev --port 3000
```

## Build

```sh
vey build
```

This writes a deployable folder:

```
dist/
  index.html
  404.html
  ...files from public/
```

Upload `dist/` to Netlify, Vercel, GitHub Pages, S3 or any static host.

Use a different output folder with `-o`:

```sh
vey build -o site
```

## Styles

CSS lives in `style.css`, imported from `main.vey`:

```vey
use "./style.css"
```

Small one-off styles can live inside the entry file:

```vey
style {
  .badge { color: rebeccapurple }
}
```

Both forms are inlined into the built page, so there is no extra CSS request.

## Tests

```sh
vey test
```

## Files

- `main.vey` — the app
- `style.css` — styles
- `404.vey` — not found page
- `public/` — static assets copied into `dist/` (sparkle logo, favicon, robots.txt)
"##;
