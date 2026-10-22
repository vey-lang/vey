//! The source map of a compiled program.
//!
//! The map tells a debugger which line of the original file a piece of the
//! output came from. The compiler leaves a mark in the generated code for
//! every statement and every expression; the marks are taken out at the end
//! and become the segments of the map.

mod vlq;

use std::collections::BTreeMap;

use vlq::{base64, encode_vlq};

use crate::compiler::SourceFile;
use crate::lexer::Pos;

/// The character that opens and closes a mark.
///
/// It is a control character, so it cannot be part of a text of the program:
/// the quotes of the code escape every character below the space.
const MARK: char = '\u{1}';

/// The mark that says where a piece of the generated code came from.
pub fn marker(source: usize, pos: Pos) -> String {
    format!("{}{},{},{}{}", MARK, source, pos.line, pos.col, MARK)
}

/// A piece of the generated code that came from a place in a source file.
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    /// The line of the generated code, counted from zero.
    pub generated_line: usize,
    /// The column of the generated code, counted from zero.
    pub generated_column: usize,
    /// The file the code came from.
    pub source: usize,
    /// The line of the file, counted from zero.
    pub source_line: usize,
    /// The column of the file, counted from zero.
    pub source_column: usize,
}

/// Take the marks out of the generated code and return the places they name.
///
/// The code that comes back has no marks in it, so it can be run as it is.
pub fn extract(code: &str) -> (String, Vec<Segment>) {
    let mut out = String::with_capacity(code.len());
    let mut segments = Vec::new();
    let mut line = 0usize;
    let mut column = 0usize;
    let mut chars = code.chars().peekable();

    while let Some(c) = chars.next() {
        if c == MARK {
            let mut text = String::new();
            for c in chars.by_ref() {
                if c == MARK {
                    break;
                }
                text.push(c);
            }
            if let Some(segment) = parse_mark(&text, line, column) {
                segments.push(segment);
            }
            continue;
        }
        out.push(c);
        if c == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    (out, segments)
}

/// Read a mark and turn it into a segment.
fn parse_mark(text: &str, generated_line: usize, generated_column: usize) -> Option<Segment> {
    let mut parts = text.split(',');
    let source: usize = parts.next()?.parse().ok()?;
    let source_line: usize = parts.next()?.parse().ok()?;
    let source_column: usize = parts.next()?.parse().ok()?;
    Some(Segment {
        generated_line,
        generated_column,
        source,
        source_line: source_line.saturating_sub(1),
        source_column: source_column.saturating_sub(1),
    })
}

/// The source map, ready to be put at the end of a file.
pub fn inline(sources: &[SourceFile], segments: &[Segment], prefix_lines: usize) -> String {
    // The segments of every line, in the order they appear.
    let mut lines: BTreeMap<usize, Vec<Segment>> = BTreeMap::new();
    for segment in segments {
        lines.entry(segment.generated_line).or_default().push(*segment);
    }

    let last = lines.keys().next_back().copied().unwrap_or(0);
    let total_lines = prefix_lines + last + 1;

    let mut encoded = String::new();
    let mut previous_source = 0i64;
    let mut previous_line = 0i64;
    let mut previous_column = 0i64;

    for line in 0..total_lines {
        if line > 0 {
            encoded.push(';');
        }
        let Some(index) = line.checked_sub(prefix_lines) else {
            continue;
        };
        let Some(list) = lines.get(&index) else {
            continue;
        };

        let mut previous_generated_column = 0i64;
        let mut list = list.clone();
        list.sort_by_key(|segment| segment.generated_column);
        for (index, segment) in list.iter().enumerate() {
            if index > 0 {
                // The segments of a line are separated by a comma.
                encoded.push(',');
            }
            encode_vlq(
                segment.generated_column as i64 - previous_generated_column,
                &mut encoded,
            );
            previous_generated_column = segment.generated_column as i64;
            encode_vlq(segment.source as i64 - previous_source, &mut encoded);
            encode_vlq(segment.source_line as i64 - previous_line, &mut encoded);
            encode_vlq(segment.source_column as i64 - previous_column, &mut encoded);
            previous_source = segment.source as i64;
            previous_line = segment.source_line as i64;
            previous_column = segment.source_column as i64;
        }
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
