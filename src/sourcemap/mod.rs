//! The source map of a compiled program.
//!
//! The map tells a debugger which line of the original file a line of the
//! output came from.

mod vlq;

use vlq::{base64, encode_vlq};

use crate::compiler::SourceFile;
pub fn inline(
    sources: &[SourceFile],
    mappings: &[Option<(usize, usize)>],
    prefix_lines: usize,
) -> String {
    let total_lines = prefix_lines + mappings.len();
    let mut encoded = String::new();
    let mut previous_source = 0i64;
    let mut previous_line = 0i64;
    let mut previous_column = 0i64;

    for line in 0..total_lines {
        if line > 0 {
            encoded.push(';');
        }
        let mapping = if line >= prefix_lines {
            mappings[line - prefix_lines]
        } else {
            None
        };
        let Some((source, source_line)) = mapping else {
            continue;
        };

        encode_vlq(0, &mut encoded);
        encode_vlq(source as i64 - previous_source, &mut encoded);
        encode_vlq(source_line as i64 - previous_line, &mut encoded);
        encode_vlq(0 - previous_column, &mut encoded);

        previous_source = source as i64;
        previous_line = source_line as i64;
        previous_column = 0;
    }

    let mut json = String::new();
    json.push_str("{\"version\":3,\"sources\":[");
    for (index, source) in sources.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&json_string(&source.path));
    }
    json.push_str("],\"sourcesContent\":[");
    for (index, source) in sources.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str(&json_string(&source.content));
    }
    json.push_str("],\"names\":[],\"mappings\":");
    json.push_str(&json_string(&encoded));
    json.push('}');

    format!(
        "//# sourceMappingURL=data:application/json;base64,{}",
        base64(json.as_bytes())
    )
}
fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
