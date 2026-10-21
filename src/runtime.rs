//! The runtime that every compiled program carries with it.
//!
//! The runtime lives in the `runtime/` folder, split into parts: one part
//! for the console and the conversions, one for the text, one for the lists,
//! and so on. The parts are joined here, in the order they need, and the
//! whole is wrapped so that nothing of it is in the scope of the page.

/// The parts of the runtime, in the order they are joined.
const PARTS: &[&str] = &[
    include_str!("../runtime/index.js"),
    include_str!("../runtime/shared.js"),
    include_str!("../runtime/node.js"),
    include_str!("../runtime/core.js"),
    include_str!("../runtime/text.js"),
    include_str!("../runtime/collections.js"),
    include_str!("../runtime/enum.js"),
    include_str!("../runtime/data.js"),
    include_str!("../runtime/dom.js"),
    include_str!("../runtime/http.js"),
    include_str!("../runtime/server.js"),
];

/// The source of the runtime, ready to be run.
pub fn source() -> String {
    let mut out = String::from("var createVeyRuntime = (function () {\n");
    for part in PARTS {
        out.push_str(part);
        out.push('\n');
    }
    out.push_str("return createVeyRuntime;\n})();\n");
    out
}

/// The names of the files the runtime is made of.
pub fn parts() -> Vec<&'static str> {
    vec![
        "index.js",
        "shared.js",
        "node.js",
        "core.js",
        "text.js",
        "collections.js",
        "enum.js",
        "data.js",
        "dom.js",
        "http.js",
        "server.js",
    ]
}
