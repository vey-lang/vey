//! Turning a compiled bundle into runnable output.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vey::compiler::Bundle;
use vey::sourcemap;
use vey::{format_error, VeyError};

pub(crate) fn node_script(bundle: &Bundle) -> String {
    let prefix = format!(
        "{runtime}\n\nconst V = createVeyRuntime(globalThis, require);\n\n(async function () {{\n",
        runtime = vey::runtime::source()
    );
    let prefix_lines = prefix.matches('\n').count();
    let map = sourcemap::inline(&bundle.sources, &bundle.mappings, prefix_lines);
    format!(
        "{prefix}{code}\n}})().catch(function (err) {{\n  console.error(err && err.vey ? err.message : (err && err.stack ? err.stack : err));\n  process.exitCode = 1;\n}});\n{map}\n",
        code = bundle.code
    )
}

pub(crate) fn browser_script(bundle: &Bundle) -> String {
    let runtime = vey::runtime::source().replace("</script", "<\\/script");
    let code = bundle.code.replace("</script", "<\\/script");
    let prefix = format!(
        "{runtime}\n(function () {{\n  const V = createVeyRuntime(window, null);\n  (async function () {{\n"
    );
    let prefix_lines = prefix.matches('\n').count();
    let map = sourcemap::inline(&bundle.sources, &bundle.mappings, prefix_lines);
    format!(
        "{prefix}{code}\n  }})().catch(function (err) {{\n    console.error(err);\n    const box = document.createElement(\"pre\");\n    box.style.cssText = \"color:#b00020;white-space:pre-wrap;font:14px ui-monospace,monospace;padding:16px\";\n    box.textContent = \"Vey error: \" + ((err && err.vey) ? err.message : ((err && err.stack) || err));\n    document.body.appendChild(box);\n  }});\n}})();\n{map}\n"
    )
}

pub(crate) fn html_document(bundle: &Bundle, title: &str, favicon: bool) -> String {
    let script = browser_script(bundle);
    let styles = bundle
        .styles
        .iter()
        .map(|style| {
            let name = style
                .path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| "style".to_string());
            format!("/* {} */\n{}", name, style.content.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n")
        .replace("</style", "<\\/style");
    format!(
        r#"{head}
<body>
<div id="app"></div>
<noscript><p style="font:14px system-ui,sans-serif;padding:16px">This Vey page needs JavaScript to run.</p></noscript>
<script>
{script}
</script>
</body>
</html>
"#,
        head = html_head(title, &styles, favicon),
        script = script
    )
}

pub(crate) fn html_head(title: &str, styles: &str, favicon: bool) -> String {
    let icon = if favicon {
        "<link rel=\"icon\" href=\"/favicon.svg\" type=\"image/svg+xml\">"
    } else {
        ""
    };
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="Vey">
{icon}
<title>{title}</title>
<style>
  :root {{ color-scheme: light dark; }}
  body {{ margin: 0; font-family: system-ui, -apple-system, "Segoe UI", sans-serif; }}
  #app {{ min-height: 100vh; }}
</style>
{styles}
</head>
"#,
        icon = icon,
        title = escape_html(title),
        styles = if styles.is_empty() {
            String::new()
        } else {
            format!("<style data-vey>\n{}\n</style>", styles)
        }
    )
}

pub(crate) fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub(crate) fn replace_extension(file: &str, extension: &str) -> String {
    let path = Path::new(file);
    let stem = path
        .file_stem()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "out".to_string());
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    parent
        .join(format!("{}.{}", stem, extension))
        .display()
        .to_string()
}

pub(crate) fn temp_file(prefix: &str, extension: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    env::temp_dir().join(format!(
        "{}-{}-{}.{}",
        prefix,
        std::process::id(),
        nanos,
        extension
    ))
}

pub(crate) fn report(err: &VeyError) {
    let source = fs::read_to_string(&err.file).unwrap_or_default();
    eprintln!("{}", format_error(err, &source));
}

/// The warnings of a program, printed one under the other.
pub(crate) fn report_warnings(warnings: &[vey::warn::Warning]) {
    for warning in warnings {
        eprintln!("{}", warning);
        if let Some(hint) = &warning.hint {
            eprintln!("  hint: {}", hint);
        }
    }
}

pub(crate) fn print_help() {
    println!(
        r#"Vey {version}

usage:
  vey run [entry|dir]              serve the site
  vey dev [entry|dir]              serve the site with a live reload
  vey test [file|dir]              run .vey test files
  vey build [entry|dir] [-o out]   build the site into .dist/
  vey build <file.vey> --node      build a standalone node script
  vey new <directory>              create a project
  vey check [file|dir]             check a file or the whole project
  vey fmt [file|dir]               write the files in the usual shape
  vey lint [file|dir]              report the smells of the program
  vey repl                         try the language in a prompt
  vey explain <file>:<line>:<col>  say the type of an expression
  vey docs [file|dir] [-o out]     write the documentation of the program
  vey migrate [file|dir]           bring the program to this version
  vey deploy [entry|dir]           build the site and say where it is
  vey version                     print version
  vey help                        show this message

build options:
  -o, --out <dir>                 output directory (default: .dist)
  --title <text>                  page title
  --node                          emit a node script instead of a site
  --force                         write the files even when they are there

server options:
  --port <n>                      port to listen on (default: 4000)
  --host <host>                   host to bind (default: 127.0.0.1)

test options:
  --filter <text>                 run only the tests whose name has the text
  --watch                         run the tests again when a file changes

other options:
  --template <name>               the shape of a new project: page, server or lib
  -h, --help                      the help of the command

example:
  vey new myapp
  cd myapp
  vey dev      # the site with a live reload
  vey run      # the site the way a host serves it
  vey build    # the site in .dist/
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}

/// The help of one command.
pub(crate) fn print_command_help(command: &str) {
    let text = match command {
        "run" => r#"usage: vey run [entry|dir] [--port <n>] [--host <host>]

Serve the site the way a host would serve it: it is built once and then
served, without a watcher and without the reload.

  --port <n>      the port to listen on (default: 4000)
  --host <host>   the host to bind (default: 127.0.0.1)
  --title <text>  the title of the page
  -h, --help      this message
"#,
        "dev" => r#"usage: vey dev [entry|dir] [--port <n>] [--host <host>]

Serve the site for the work: it watches the files of the project, rebuilds
when one of them changes and reloads the page in the browser.

  --port <n>      the port to listen on (default: 4000)
  --host <host>   the host to bind (default: 127.0.0.1)
  --title <text>  the title of the page
  -h, --help      this message
"#,
        "build" => r#"usage: vey build [entry|dir] [-o <dir>]

Build the site into .dist/, or a single file into a node script.

  -o, --out <dir>   the folder the site is written to (default: .dist)
  --title <text>    the title of the page
  --node            emit a node script instead of a site
  --force           write the files even when they are there
  -h, --help        this message
"#,
        "test" => r#"usage: vey test [file|dir] [--filter <text>] [--watch]

Run the .vey files under tests/, or the files that are named.

  --filter <text>   run only the tests whose name has the text
  --watch           run the tests again when a file changes
  -h, --help        this message
"#,
        "new" => r#"usage: vey new <directory> [--template <name>] [--force]

Create a project. The template is one of page, server or lib.

  --template <name>  the shape of the project: page, server or lib
  --force            fill in the files that are missing
  -h, --help         this message
"#,
        "check" => r#"usage: vey check [file|dir]

Check a file or the whole project, and report what is wrong.

  -h, --help   this message
"#,
        "fmt" => r#"usage: vey fmt [file|dir] [--write]

Write the files in the usual shape. The files are printed when --write is
not given, so that the change can be seen first.

  --write   write the files
  -h, --help  this message
"#,
        "lint" => r#"usage: vey lint [file|dir]

Report the smells of the program: the empty catch, the long lines, the
nesting that is too deep and the names that are too near one another.

  -h, --help   this message
"#,
        "repl" => r#"usage: vey repl

Try the language in a prompt. A line is read, the program is built and run,
and what it prints is shown. The state of the program is kept between the
lines.

  -h, --help   this message
"#,
        "explain" => r#"usage: vey explain <file.vey>:<line>:<column>

Say the type of the expression at that place of the file.

  -h, --help   this message
"#,
        "docs" => r#"usage: vey docs [file|dir] [-o <file>]

Write the documentation of the program, taken from its doc comments.

  -o, --out <file>   the file the documentation is written to
  -h, --help         this message
"#,
        "migrate" => r#"usage: vey migrate [file|dir] [--write]

Bring the program to this version of the language. The files are printed
when --write is not given.

  --write   write the files
  -h, --help  this message
"#,
        "deploy" => r#"usage: vey deploy [entry|dir] [--target <dir>]

Build the site and say where it is, so that a host may serve it.

  --target <dir>   the folder the site is copied to
  -h, --help         this message
"#,
        _ => {
            print_help();
            return;
        }
    };
    println!("{}", text);
}
