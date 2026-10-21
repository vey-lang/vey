//! The colouring of Vey code, used by the error page.

/// The words of the language.
const KEYWORDS: &[&str] = &[
    "let",
    "const",
    "fn",
    "return",
    "if",
    "else",
    "while",
    "for",
    "in",
    "repeat",
    "break",
    "continue",
    "try",
    "catch",
    "use",
    "as",
    "export",
    "component",
    "await",
    "true",
    "false",
    "null",
    "and",
    "or",
    "not",
    "match",
    "enum",
    "type",
];

/// The code of a line as HTML, with the parts of the language coloured.
pub(crate) fn highlight(line: &str) -> String {
    let mut out = String::new();
    let mut chars = line.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '/' if chars.peek() == Some(&'/') => {
                // The rest of the line is a comment.
                let comment: String = chars.collect();
                out.push_str(&format!("<span class=\"cm\">//{}</span>", escape(&comment)));
                break;
            }
            '"' | '\'' => {
                // A text, with the interpolations inside it.
                let quote = c;
                let mut text = String::from(quote);
                let mut done = false;
                while let Some(next) = chars.next() {
                    text.push(next);
                    if next == '\\' {
                        if let Some(escaped) = chars.next() {
                            text.push(escaped);
                        }
                        continue;
                    }
                    if next == quote {
                        done = true;
                        break;
                    }
                }
                let _ = done;
                out.push_str(&format!("<span class=\"str\">{}</span>", escape(&text)));
            }
            c if c.is_ascii_digit() => {
                let mut number = String::from(c);
                while let Some(next) = chars.peek() {
                    if next.is_ascii_digit() || *next == '.' {
                        number.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                out.push_str(&format!("<span class=\"num\">{}</span>", escape(&number)));
            }
            c if c.is_alphabetic() || c == '_' || c == '$' => {
                let mut word = String::from(c);
                while let Some(next) = chars.peek() {
                    if next.is_alphanumeric() || *next == '_' || *next == '$' {
                        word.push(*next);
                        chars.next();
                    } else {
                        break;
                    }
                }
                if KEYWORDS.contains(&word.as_str()) {
                    out.push_str(&format!("<span class=\"kw\">{}</span>", escape(&word)));
                } else if chars.peek() == Some(&'(') {
                    out.push_str(&format!("<span class=\"fn\">{}</span>", escape(&word)));
                } else {
                    out.push_str(&escape(&word));
                }
            }
            c if c.is_whitespace() => {
                out.push(c);
            }
            c => out.push_str(&escape(&c.to_string())),
        }
    }
    out
}

/// The text of a part of the code, safe to put in HTML.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
