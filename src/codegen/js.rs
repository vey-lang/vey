//! Helpers for writing JavaScript.

const JS_RESERVED: &[&str] = &[
    "let",
    "const",
    "var",
    "function",
    "class",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "new",
    "delete",
    "typeof",
    "instanceof",
    "in",
    "of",
    "this",
    "super",
    "yield",
    "await",
    "async",
    "static",
    "import",
    "export",
    "extends",
    "try",
    "catch",
    "finally",
    "throw",
    "void",
    "with",
    "debugger",
];

const EVENT_ATTRS: &[(&str, &str)] = &[
    ("click", "onclick"),
    ("dblclick", "ondblclick"),
    ("change", "onchange"),
    ("input", "oninput"),
    ("submit", "onsubmit"),
    ("load", "onload"),
    ("focus", "onfocus"),
    ("blur", "onblur"),
    ("keydown", "onkeydown"),
    ("keyup", "onkeyup"),
    ("keypress", "onkeypress"),
    ("key-press", "onkeypress"),
    ("mouseover", "onmouseover"),
    ("mouseout", "onmouseout"),
    ("wheel", "onwheel"),
    ("context-menu", "oncontextmenu"),
    ("pointer-down", "onpointerdown"),
    ("pointer-up", "onpointerup"),
    ("pointer-move", "onpointermove"),
    ("pointer-enter", "onpointerenter"),
    ("pointer-leave", "onpointerleave"),
    ("touch-start", "ontouchstart"),
    ("touch-end", "ontouchend"),
    ("touch-move", "ontouchmove"),
    ("drag-start", "ondragstart"),
    ("drag-end", "ondragend"),
    ("drag-over", "ondragover"),
    ("drop", "ondrop"),
];

use crate::parser::is_ident;

pub(crate) fn event_attr(name: &str) -> &str {
    for (from, to) in EVENT_ATTRS {
        if *from == name {
            return to;
        }
    }
    name
}

pub(crate) fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

pub(crate) fn js_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", other as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

pub(crate) fn js_key(key: &str) -> String {
    if is_ident(key) && !JS_RESERVED.contains(&key) {
        key.to_string()
    } else {
        js_string(key)
    }
}
