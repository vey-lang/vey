//! The reading of string literals.

use crate::lexer::{Lexer, Pos, StrPartToken, TokKind, Token};
use crate::VeyError;

impl Lexer {
    pub(crate) fn read_string(
        &mut self,
        start: usize,
        pos: Pos,
        quote: char,
    ) -> Result<Token, VeyError> {
        let mut parts: Vec<StrPartToken> = Vec::new();
        let mut text = String::new();
        self.bump();

        while let Some(c) = self.at(0) {
            if c == quote {
                self.bump();
                if !text.is_empty() || parts.is_empty() {
                    parts.push(StrPartToken { text, expr: None });
                }
                return Ok(Token {
                    kind: TokKind::Str,
                    value: String::new(),
                    number: 0.0,
                    parts,
                    pos,
                    start,
                    end: self.pos,
                    newline_before: false,
                });
            }

            if c == '\\' {
                self.bump();
                if let Some(e) = self.bump() {
                    match e {
                        'n' => text.push('\n'),
                        't' => text.push('\t'),
                        'r' => text.push('\r'),
                        '0' => text.push('\0'),
                        'b' => text.push('\u{8}'),
                        'f' => text.push('\u{c}'),
                        'v' => text.push('\u{b}'),
                        // A quote may be written as itself, so that a text
                        // with a quote of the other kind is easy to write.
                        '\'' => text.push('\''),
                        '"' => text.push('"'),
                        '\\' => text.push('\\'),
                        'u' => {
                            let value = self.read_hex(4)?;
                            match char::from_u32(value) {
                                Some(c) => text.push(c),
                                None => {
                                    return Err(self.err(format!(
                                        "\\u{:04x} is not a character",
                                        value
                                    )))
                                }
                            }
                        }
                        'x' => {
                            let value = self.read_hex(2)?;
                            match char::from_u32(value) {
                                Some(c) => text.push(c),
                                None => {
                                    return Err(self.err(format!(
                                        "\\x{:02x} is not a character",
                                        value
                                    )))
                                }
                            }
                        }
                        other => text.push(other),
                    }
                }
                continue;
            }

            if quote == '"' && c == '{' {
                // A brace only starts an interpolation when a matching brace
                // follows. Otherwise it is ordinary text, which keeps JSON
                // such as "{\"a\": 1}" writable.
                let Some(end) = self.match_brace(self.pos) else {
                    text.push('{');
                    self.bump();
                    continue;
                };
                let source: String = self.chars[self.pos + 1..end].iter().collect();
                if !text.is_empty() {
                    parts.push(StrPartToken {
                        text: std::mem::take(&mut text),
                        expr: None,
                    });
                }
                let expr_pos = Pos::new(self.line, self.col + 1);
                parts.push(StrPartToken {
                    text: String::new(),
                    expr: Some((source, expr_pos)),
                });
                while self.pos <= end {
                    self.bump();
                }
                continue;
            }

            text.push(c);
            self.bump();
        }

        Err(VeyError::new(
            "unterminated string",
            self.file.clone(),
            pos.line,
            pos.col,
        ))
    }

    /// Read a number of hexadecimal digits.
    ///
    /// The digits are the ones of a number that starts with `\\u` or `\\x`.
    fn read_hex(&mut self, count: usize) -> Result<u32, VeyError> {
        let mut value = 0u32;
        for _ in 0..count {
            let Some(c) = self.at(0) else {
                return Err(self.err("the escape is not complete"));
            };
            let Some(digit) = c.to_digit(16) else {
                return Err(self.err(format!("'{}' is not a hexadecimal digit", c)));
            };
            self.bump();
            value = value * 16 + digit;
        }
        Ok(value)
    }

    /// Consume a raw `{ ... }` section and return its inner text.
    ///
    /// The lexer must be positioned right after the token that introduces the
    /// block (for example `style`). Braces inside strings and `/* */` comments
    /// are ignored so CSS values survive intact.
    pub(crate) fn raw_brace_block(&mut self) -> Result<(String, Pos), VeyError> {
        self.skip_space()?;
        if self.at(0) != Some('{') {
            return Err(self.err("expected '{' to start a style block"));
        }
        let open = Pos::new(self.line, self.col);
        self.bump();
        let start = self.pos;
        let mut depth = 1usize;
        let mut i = self.pos;

        while let Some(&c) = self.chars.get(i) {
            if c == '"' || c == '\'' {
                let quote = c;
                i += 1;
                while let Some(&ch) = self.chars.get(i) {
                    if ch == '\\' {
                        i += 2;
                        continue;
                    }
                    i += 1;
                    if ch == quote {
                        break;
                    }
                }
            } else if c == '/' && self.chars.get(i + 1) == Some(&'*') {
                i += 2;
                while i < self.chars.len() {
                    if self.chars[i] == '*' && self.chars.get(i + 1) == Some(&'/') {
                        i += 2;
                        break;
                    }
                    i += 1;
                }
            } else if c == '{' {
                depth += 1;
                i += 1;
            } else if c == '}' {
                depth -= 1;
                if depth == 0 {
                    let css: String = self.chars[start..i].iter().collect();
                    while self.pos <= i {
                        self.bump();
                    }
                    return Ok((css, open));
                }
                i += 1;
            } else {
                i += 1;
            }
        }

        Err(VeyError::new(
            "unclosed style block",
            self.file.clone(),
            open.line,
            open.col,
        ))
    }

    pub(crate) fn match_brace(&self, start: usize) -> Option<usize> {
        let mut depth = 0i32;
        let mut i = start;
        let mut quote: Option<char> = None;
        while let Some(&c) = self.chars.get(i) {
            // Escapes are handled the same inside and outside strings, so a
            // backslash never leaves a quote open by accident.
            if c == '\\' {
                i += 2;
                continue;
            }
            if let Some(q) = quote {
                if c == q {
                    quote = None;
                }
                i += 1;
                continue;
            }
            match c {
                '"' | '\'' => {
                    quote = Some(c);
                    i += 1;
                }
                '{' => {
                    depth += 1;
                    i += 1;
                }
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(i);
                    }
                    i += 1;
                }
                _ => i += 1,
            }
        }
        None
    }
}

pub fn process_escapes(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => out.push('\0'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}
