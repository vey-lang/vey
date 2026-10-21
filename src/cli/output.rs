//! Turning a compiled bundle into runnable output.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use vey::compiler::Bundle;
use vey::sourcemap;
use vey::{format_error, VeyError};

/// The runtime that every compiled program carries with it.
pub(crate) const RUNTIME: &str = include_str!("../../runtime.js");

pub(crate) fn node_script(bundle: &Bundle) -> String {
    let prefix = format!(
        "{runtime}\n\nconst V = createVeyRuntime(globalThis, require);\n\n(async function () {{\n",
        runtime = RUNTIME
    );
    let prefix_lines = prefix.matches('\n').count();
    let map = sourcemap::inline(&bundle.sources, &bundle.mappings, prefix_lines);
    format!(
        "{prefix}{code}\n}})().catch(function (err) {{\n  console.error(err && err.stack ? err.stack : err);\n  process.exitCode = 1;\n}});\n{map}\n",
        code = bundle.code
    )
}

pub(crate) fn browser_script(bundle: &Bundle) -> String {
    let runtime = RUNTIME.replace("</script", "<\\/script");
    let code = bundle.code.replace("</script", "<\\/script");
    let prefix = format!(
        "{runtime}\n(function () {{\n  const V = createVeyRuntime(window, null);\n  (async function () {{\n"
    );
    let prefix_lines = prefix.matches('\n').count();
    let map = sourcemap::inline(&bundle.sources, &bundle.mappings, prefix_lines);
    format!(
        "{prefix}{code}\n  }})().catch(function (err) {{\n    console.error(err);\n    const box = document.createElement(\"pre\");\n    box.style.cssText = \"color:#b00020;white-space:pre-wrap;font:14px ui-monospace,monospace;padding:16px\";\n    box.textContent = \"Vey error: \" + ((err && err.stack) || err);\n    document.body.appendChild(box);\n  }});\n}})();\n{map}\n"
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

pub(crate) fn print_help() {
    println!(
        r#"Vey {version}

usage:
  vey run <file.vey>              compile and run with node
  vey dev [entry] [--port 4000]   start the live dev server
  vey test [file|dir]             run .vey test files
  vey build [entry|dir] [-o out]  build a deployable static site (.dist/)
  vey build <file.vey> --node     build a standalone node script
  vey web [entry] [-o out]        build a single html page
  vey new <directory>             create a project
  vey check [file|dir]           check a file or the whole project
  vey version                     print version
  vey help                        show this message

build options:
  -o, --out <dir>                 output directory (default: .dist)
  --title <text>                  page title
  --node                          emit a node script instead of a site

dev options:
  --port <n>                      port to listen on (default: 4000)
  --host <host>                   host to bind (default: 127.0.0.1)

example:
  vey new myapp
  cd myapp
  vey dev
  vey build
"#,
        version = env!("CARGO_PKG_VERSION")
    );
}
