//! Tests for the runtime that ship with the compiler.
//!
//! The runtime is loaded straight into Node so that behaviour which is not
//! reachable from Vey source, such as the internal element factory, can be
//! checked as well.

mod common;

use std::fs;
use std::path::PathBuf;
use std::process::Command;

use common::temp_dir;

/// Load the runtime and evaluate `script`.
///
/// The script can use `runtime` for the Node flavoured runtime and `RUNTIME`
/// for the path of the runtime source.
fn run_runtime(label: &str, script: &str) -> String {
    let dir = temp_dir(label);
    // The runtime is split into parts, so it is put together here.
    let runtime = dir.join("runtime.js");
    fs::write(&runtime, vey::runtime::source()).unwrap();
    let harness = dir.join("harness.cjs");
    fs::write(
        &harness,
        format!(
            "const fs = require('node:fs');\n\
             const RUNTIME = process.argv[2];\n\
             const source = fs.readFileSync(RUNTIME, 'utf8');\n\
             global.RUNTIME = RUNTIME;\n\
             const createVeyRuntime = new Function(source + '\\nreturn createVeyRuntime;')();\n\
             const runtime = createVeyRuntime(globalThis, require);\n\
             {}\n",
            script
        ),
    )
    .unwrap();

    let output = Command::new("node")
        .arg(&harness)
        .arg(runtime)
        .output()
        .expect("failed to start node");
    assert!(
        output.status.success(),
        "runtime script failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

#[test]
fn inherited_names_are_not_treated_as_void_tags() {
    let text = run_runtime(
        "void-tags",
        r#"
console.log(runtime.html(runtime.h("constructor", null, [])));
console.log(runtime.html(runtime.h("br", null, [])));
"#,
    );
    assert!(text.contains("<constructor></constructor>"), "{text}");
    assert!(text.contains("<br>"), "{text}");
}

#[test]
fn raw_html_survives_the_dom_path() {
    let text = run_runtime(
        "raw-dom",
        r#"
const fakeDoc = {
  createElement(tag) {
    const el = {
      nodeType: 1,
      tagName: tag.toUpperCase(),
      children: [],
      innerHTML: "",
      appendChild(child) { this.children.push(child); return child; },
      setAttribute() {}
    };
    // A real <template> holds its parsed content in `.content`.
    el.content = { nodeType: 11, children: [], appendChild(c) { this.children.push(c); return c; } };
    return el;
  },
  createTextNode(text) { return { nodeType: 3, textContent: String(text) }; },
  createDocumentFragment() {
    return { nodeType: 11, children: [], appendChild(c) { this.children.push(c); return c; } };
  }
};
const browser = new Function(require('node:fs').readFileSync(RUNTIME, 'utf8') + '\nreturn createVeyRuntime;')()({ document: fakeDoc, console }, null);
const el = browser.h("p", null, [browser.raw("<strong>x</strong>")]);
console.log(el.children.length, el.children[0].nodeType);
"#,
    );
    // Raw markup becomes a fragment, never a text node with "[object Object]".
    assert!(text.contains("1 11"), "{text}");
    assert!(!text.contains("[object Object]"), "{text}");
}

#[test]
fn styles_use_the_same_key_format_on_both_paths() {
    let text = run_runtime(
        "style-keys",
        r#"
console.log(runtime.html(runtime.h("div", { style: { backgroundColor: "red", "font-size": "12px" } }, [])));
"#,
    );
    assert!(text.contains("background-color: red"), "{text}");
    assert!(text.contains("font-size: 12px"), "{text}");
}

#[test]
fn malformed_percent_encoding_does_not_throw() {
    let text = run_runtime(
        "safe-decode",
        r#"
console.log(runtime.url.decode("%"));
"#,
    );
    assert!(text.contains("%"), "{text}");
}

#[test]
fn node_only_helpers_report_a_clear_error_in_the_browser() {
    let text = run_runtime(
        "browser-guard",
        r#"
const browser = new Function(require('node:fs').readFileSync(RUNTIME, 'utf8') + '\nreturn createVeyRuntime;')()({ console }, null);
try { browser.fs.read("x"); } catch (err) { console.log(err.message); }
"#,
    );
    assert!(text.contains("only available in Node"), "{text}");
}

#[test]
fn every_helper_of_the_checker_is_in_the_runtime() {
    let runtime = vey::runtime::source();
    let builtins = fs::read_to_string("src/builtins.rs").expect("the list of helpers is missing");

    let mut names = Vec::new();
    for line in builtins.lines() {
        let line = line.trim();
        let Some(start) = line.find('"') else {
            continue;
        };
        let rest = &line[start + 1..];
        let Some(end) = rest.find('"') else {
            continue;
        };
        let name = &rest[..end];
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            names.push(name.to_string());
        }
    }
    assert!(!names.is_empty(), "no helpers were found");

    for name in names {
        assert!(
            runtime.contains(&format!("\n    {}:", name)),
            "the runtime does not have '{}'",
            name
        );
    }
}

#[test]
fn every_part_of_the_runtime_is_joined() {
    let source = vey::runtime::source();
    let parts = vey::runtime::parts();
    for part in &parts {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/runtime")
            .join(part);
        assert!(path.is_file(), "the part {} is missing", part);
    }
    // Every file of the folder is a part of the runtime.
    let folder = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/runtime");
    for entry in fs::read_dir(&folder).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".js") {
            continue;
        }
        assert!(
            parts.contains(&name.as_str()),
            "the part {} is not joined into the runtime",
            name
        );
    }
    // The runtime is one function that the program calls.
    assert!(
        source.contains("function createVeyRuntime("),
        "the runtime is broken"
    );
    assert!(
        source.trim_end().ends_with("})();"),
        "the runtime is broken"
    );
}
