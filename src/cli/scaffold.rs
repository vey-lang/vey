//! The `vey new` command and the project template.

use std::fs;
use std::path::Path;

use crate::cli::args::parse_args;
use crate::cli::output::print_command_help;

/// The mark of the language, in the colour of the text around it.
pub(crate) const PROJECT_LOGO: &str = include_str!("../../logo.svg");

/// The mark as a favicon: the same shape on a black ground, so that it is
/// visible on every browser.
pub(crate) const PROJECT_FAVICON: &str = include_str!("../../favicon.svg");

pub(crate) fn cmd_new(args: &[String]) -> i32 {
    let parsed = match parse_args("new", args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        print_command_help("new");
        return 0;
    }
    let force = parsed.force;
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
        if not_empty && !force {
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

    let template = match template_from(parsed.template.as_deref()) {
        Ok(template) => template,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };

    let files = match template {
        Template::Page => page_files(&project),
        Template::Server => server_files(&project),
        Template::Lib => lib_files(&project),
    };

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
    match template {
        Template::Page => println!("  vey dev"),
        // A server is a program of Node: the same command builds it, runs
        // it, and starts it again when a file changes.
        Template::Server => println!("  vey dev"),
        // A library has no page, so the file of it is named.
        Template::Lib => println!("  vey check src/lib.vey"),
    }
    0
}

/// The shape of a new project.
#[derive(Clone, Copy, PartialEq)]
enum Template {
    Page,
    Server,
    Lib,
}

/// The name of the template of a project.
fn template_from(name: Option<&str>) -> Result<Template, String> {
    match name {
        None => Ok(Template::Page),
        Some("page") => Ok(Template::Page),
        Some("server") => Ok(Template::Server),
        Some("lib") => Ok(Template::Lib),
        Some(other) => Err(format!(
            "there is no template named '{}': the templates are page, server and lib",
            other
        )),
    }
}

/// The files of a project with a page.
fn page_files(project: &str) -> Vec<(&'static str, String)> {
    vec![
        ("src/main.vey", PROJECT_MAIN.to_string()),
        ("src/site.vey", PROJECT_SITE.to_string()),
        ("src/styles/site.css", PROJECT_STYLE.to_string()),
        ("public/logo.svg", PROJECT_LOGO.to_string()),
        ("public/favicon.svg", PROJECT_FAVICON.to_string()),
        ("public/robots.txt", ROBOTS.to_string()),
        ("tests/site.vey", PROJECT_TEST.to_string()),
        (
            "package.json",
            PROJECT_PACKAGE.replace("{name}", &package_name(project)),
        ),
        ("README.md", PROJECT_README.replace("{name}", project)),
        (
            "vey.toml",
            CONFIG
                .replace("{name}", project)
                .replace("{kind}", "page"),
        ),
        (".gitignore", GITIGNORE.to_string()),
        ("Dockerfile", PAGE_DOCKERFILE.to_string()),
        (".github/workflows/ci.yml", ci_workflow("vey check")),
    ]
}

/// The files of a project with a server.
fn server_files(project: &str) -> Vec<(&'static str, String)> {
    let mut files = page_files(project);
    for file in files.iter_mut() {
        match file.0 {
            "src/main.vey" => file.1 = SERVER_MAIN.to_string(),
            "src/site.vey" => file.1 = SERVER_SITE.to_string(),
            "tests/site.vey" => file.1 = SERVER_TEST.to_string(),
            "package.json" => {
                file.1 = SERVER_PACKAGE.replace("{name}", &package_name(project))
            }
            "README.md" => file.1 = SERVER_README.replace("{name}", project),
            "Dockerfile" => file.1 = SERVER_DOCKERFILE.to_string(),
            "vey.toml" => {
                file.1 = CONFIG
                    .replace("{name}", project)
                    .replace("{kind}", "server")
            }
            _ => {}
        }
    }
    files
}

/// The files of a project that is a library.
///
/// A library has no page to show and no server to run, so it has no
/// container either: the image of it would have nothing to do.
fn lib_files(project: &str) -> Vec<(&'static str, String)> {
    vec![
        ("src/lib.vey", LIB_MAIN.to_string()),
        ("tests/lib.vey", LIB_TEST.to_string()),
        (
            "package.json",
            LIB_PACKAGE.replace("{name}", &package_name(project)),
        ),
        ("README.md", LIB_README.replace("{name}", project)),
        (".gitignore", GITIGNORE.to_string()),
        (".github/workflows/ci.yml", ci_workflow("vey check src/lib.vey")),
    ]
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

/// The settings of a new project.
const CONFIG: &str = r#"# The settings of the project.
#
# The options of the command line are stronger than these.

# The shape of the project: page, server or lib.
#
# A page is built into a site, a server is built into a program of Node,
# and a library is only checked and tested. Without this line, a project is
# read from its entry: a program that starts a server is a server, and
# everything else is a site of pages.
kind = "{kind}"

# The port that the server listens on.
port = 4000

# The host that the server listens on.
host = "127.0.0.1"

# The title of the site.
title = "{name}"

# The folder the site is written to.
out = ".dist"

[lint]
# The length of a line that is still easy to read.
maxLine = 100
"#;

const PROJECT_MAIN: &str = r##"// The entry of the site: the page is put on the screen.
//
// `vey dev` serves the site and reloads it when a file changes.
// `vey build` writes it into `.dist/`, which a host may serve.
//
// The parts of the page live in `src/site.vey`, so that a test may use them
// without putting anything on a screen.

use "./site.vey" as site

bind("#app", site.App())
"##;

/// The page itself: the styles it uses and the component it shows.
const PROJECT_SITE: &str = r##"use "./styles/site.css"

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

export component App() {
  main.start {
    img.mark src="/logo.svg" alt="" width="52" height="52"
    h1 { "Welcome to Vey" }
  }
}
"##;

/// The first test of a new site: the page that the site shows.
const PROJECT_TEST: &str = r##"use "../src/site.vey" as site

test("the welcome page greets the visitor", fn() {
  let page = html(site.App())
  assert(includes(page, "Welcome to Vey"), "the heading should be in the page")
})
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

/// A project with a server.
const SERVER_MAIN: &str = r##"// A server that answers with a page.
//
// `vey dev` builds the program, runs it and starts it again when a file
// changes. `vey build` writes `.dist/server.cjs`, which a host runs.

use "./site.vey" as site

// Where the server listens is a setting of the machine, so that the same
// program runs on this machine, in a container and on a host.
let port = int(env("PORT", "4000"))
let host = env("HOST", "127.0.0.1")

let app = server(port, { host: host })

app.get("/", fn(req) {
  print(req.path)
  return response(html(site.App("world")), 200, { "Content-Type": "text/html; charset=utf-8" })
})

// The files of `public/` are served as they are, so the page may ask for
// `/logo.svg` and `/favicon.svg`.
app.static("public")

app.start()
"##;

/// The page that the server answers with.
///
/// Nothing here starts anything, so a test may use the component as well.
const SERVER_SITE: &str = r##"use "./styles/site.css"

style {
  body {
    margin: 0;
    padding: 3rem 1.5rem;
    font-family: ui-sans-serif, system-ui, sans-serif;
  }
}

export component App(name: string) {
  main {
    h1 { "Hello, " + name }
  }
}
"##;

/// The first test of a new server: the page that it answers with.
const SERVER_TEST: &str = r##"use "../src/site.vey" as site

test("the server greets the one that opens the page", fn() {
  let page = html(site.App("world"))
  assert(includes(page, "Hello, world"), "the greeting should be in the page")
})
"##;

const SERVER_README: &str = r##"# {name}

A server written with [Vey](https://vey-lang.dev).

## Development

```sh
npm run dev     # or: vey dev
```

The program is built, run, and built and started again when a file it
reads changes. The page is at http://127.0.0.1:4000. A program that does
not build leaves the server stopped, so an answer never comes from code
that the files no longer say; the mistake is printed in the terminal.

`PORT` and `HOST` are given to the program, so `vey dev --port 4100` moves
it. A program chooses its own address with its `server()` call.

## Build and run

```sh
npm run build   # or: vey build          -> .dist/server.cjs
npm start       # or: vey run            -> build and run the program
```

The program is one file of Node, so a host only needs node:

```sh
vey build
node --enable-source-maps .dist/server.cjs
```

## Check and test

```sh
npm run check   # or: vey check
npm test        # or: vey test
```

## Layout

| Path | What it is |
| --- | --- |
| `src/main.vey` | the server: the routes and the start |
| `src/site.vey` | the page that the server answers with |
| `src/styles/site.css` | the styles of the page |
| `public/` | the files that are served as they are |
| `tests/site.vey` | the tests of the page |
| `vey.toml` | the settings: the kind of the project, the port and the host |
"##;

/// The scripts of the package of a server.
const SERVER_PACKAGE: &str = r##"{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "description": "A server written with Vey.",
  "scripts": {
    "dev": "vey dev",
    "start": "vey run",
    "build": "vey build",
    "test": "vey test",
    "check": "vey check"
  }
}
"##;

/// A library.
const LIB_MAIN: &str = r##"// A library of helpers.
//
// The names that are given to the outside world with `export` may be used
// by the modules that import this one.

/// The greeting of a name.
export fn greet(name: string) -> string {
  return "Hello, " + name
}

/// The number of the letters of a text.
export fn countLetters(text: string) -> number {
  return len(text)
}
"##;

const LIB_PACKAGE: &str = r##"{
  "name": "{name}",
  "version": "0.1.0",
  "private": true,
  "description": "A library written with Vey.",
  "scripts": {
    "check": "vey check src/lib.vey",
    "test": "vey test"
  }
}
"##;

/// The first test of a new library.
const LIB_TEST: &str = r##"use "../src/lib.vey" as lib

test("the greeting names the one it greets", fn() {
  assert(lib.greet("world") == "Hello, world", "greet should say hello")
})

test("the letters of a text are counted", fn() {
  assert(lib.countLetters("vey") == 3, "vey has three letters")
})
"##;

const LIB_README: &str = r##"# {name}

A library written with [Vey](https://vey-lang.dev).

## Check and test

```sh
npm run check   # or: vey check src/lib.vey
npm test        # or: vey test
```

## Layout

| Path | What it is |
| --- | --- |
| `src/lib.vey` | the library |
| `tests/lib.vey` | the tests of the library |
"##;

/// The recipe of a container that serves the site of the project.
///
/// The site is built when the container is made, so a page that cannot be
/// built stops the image from being made. It is served by Vey itself, the
/// way a host serves it; the address is every one of the machine, because
/// the container is reached from outside of it.
const PAGE_DOCKERFILE: &str = r##"FROM node:22-alpine

WORKDIR /app

# The compiler that builds and serves the project.
RUN npm install -g vey

COPY . .

RUN vey build

EXPOSE 4000
CMD ["vey", "run", "--host", "0.0.0.0", "--port", "4000"]
"##;

/// The recipe of a container that runs the server of the project.
///
/// The server is one program of Node, which is what the container runs.
const SERVER_DOCKERFILE: &str = r##"FROM node:22-alpine

WORKDIR /app

# The compiler that builds the project.
RUN npm install -g vey

COPY . .

# The server is built into one file, so that the container only runs it.
RUN vey build

# The server of a container is reached from outside of it.
ENV HOST=0.0.0.0
ENV PORT=4000

EXPOSE 4000
CMD ["node", "--enable-source-maps", ".dist/server.cjs"]
"##;

/// The recipe of the continuous integration.
///
/// The command that checks the project is different for every shape of a
/// project, so it is put in the place of `{check}`.
fn ci_workflow(check: &str) -> String {
    CI_WORKFLOW.replace("{check}", check)
}

const CI_WORKFLOW: &str = r##"name: ci

on:
  push:
    branches: [main]
  pull_request:

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 22
      - run: npm install -g vey
      - run: {check}
      - run: vey test
"##;

const GITIGNORE: &str = "/.dist\n/.vey\n/node_modules\n*.cjs\n.DS_Store\n";

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
| `src/main.vey` | the entry: the page is put on the screen |
| `src/site.vey` | the component of the page and its styles |
| `src/styles/site.css` | the styles of the site |
| `public/` | the files that are copied as they are |
| `tests/site.vey` | the tests of the page |

## Styles

The styles of a page may be written in a `style { }` block, next to the
markup they belong to:

```vey
style {
  h1 { font-size: 2rem; }
}
```

A stylesheet that is shared by several pages belongs in `src/styles/`.

## Packages

A package of npm is used with the `npm:` prefix:

```vey
use "npm:marked" as marked
```

Install it first with `npm install marked`. A package needs Node, so it may
only be used in a program and not in a page of the site.

## Pages

`src/404.vey` is the page of an unknown address. Without it, Vey writes a
plain page of its own.

## Tests

```sh
npm test       # or: vey test
```

`tests/site.vey` tests the page of the site: it renders the component and
looks at what it says. The page is imported from `src/site.vey`, so the test
uses the same file the site uses, and nothing is put on a screen.
"##;
