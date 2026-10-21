//! The tokenizer of the language.
//!
//! The reading of string literals is in `strings`, because it needs to know
//! where an interpolation ends.

mod strings;

pub use strings::process_escapes;

use crate::VeyError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

impl Pos {
    pub fn new(line: usize, col: usize) -> Self {
        Self { line, col }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokKind {
    Eof,
    Number,
    Str,
    Ident,
    Keyword,
    Op,
    Punct,
}

#[derive(Clone, Debug)]
pub struct StrPartToken {
    pub text: String,
    pub expr: Option<(String, Pos)>,
}

#[derive(Clone, Debug)]
pub struct Token {
    pub kind: TokKind,
    pub value: String,
    pub number: f64,
    pub parts: Vec<StrPartToken>,
    pub pos: Pos,
    pub start: usize,
    pub end: usize,
}

impl Token {
    pub fn eof(pos: Pos, at: usize) -> Self {
        Self {
            kind: TokKind::Eof,
            value: String::new(),
            number: 0.0,
            parts: Vec::new(),
            pos,
            start: at,
            end: at,
        }
    }
}

#[derive(Clone)]
pub struct Lexer {
    chars: Vec<char>,
    file: String,
    pos: usize,
    line: usize,
    col: usize,
}

const OPS: &[&str] = &[
    "...", "?.", "|>", "=>", "->", "??", "**", "==", "!=", "<=", ">=", "..", "+=", "-=", "*=",
    "/=", "%=", "|", "+", "-", "*", "/", "%", "=", "<", ">", "?",
];

pub fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "let"
            | "const"
            | "fn"
            | "return"
            | "if"
            | "else"
            | "while"
            | "for"
            | "in"
            | "repeat"
            | "break"
            | "continue"
            | "try"
            | "catch"
            | "finally"
            | "use"
            | "as"
            | "export"
            | "component"
            | "await"
            | "async"
            | "true"
            | "false"
            | "null"
            | "and"
            | "or"
            | "not"
            | "match"
            | "enum"
    )
}

impl Lexer {
    pub fn new(src: &str, file: &str) -> Self {
        Self::with_pos(src, file, 1, 1)
    }

    pub fn with_pos(src: &str, file: &str, line: usize, col: usize) -> Self {
        Self {
            chars: src.chars().collect(),
            file: file.to_string(),
            pos: 0,
            line,
            col,
        }
    }

    fn err(&self, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file.clone(), self.line, self.col)
    }

    fn at(&self, n: usize) -> Option<char> {
        self.chars.get(self.pos + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = *self.chars.get(self.pos)?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    fn matches(&self, text: &str) -> bool {
        for (offset, c) in text.chars().enumerate() {
            if self.chars.get(self.pos + offset) != Some(&c) {
                return false;
            }
        }
        true
    }

    fn skip_space(&mut self) {
        loop {
            match self.at(0) {
                // A file that starts with a byte order mark is read as if it
                // was not there.
                Some(c) if c.is_whitespace() || c == '\u{feff}' => {
                    self.bump();
                }
                Some('/') if self.at(1) == Some('/') => {
                    while let Some(c) = self.at(0) {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                    }
                }
                _ => break,
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, VeyError> {
        self.skip_space();

        let start = self.pos;
        let pos = Pos::new(self.line, self.col);

        let Some(c) = self.at(0) else {
            return Ok(Token::eof(pos, start));
        };

        if c.is_ascii_digit() {
            return Ok(self.read_number(start, pos));
        }

        if c == '"' || c == '\'' {
            return self.read_string(start, pos, c);
        }

        if c.is_alphabetic() || c == '_' || c == '$' {
            let mut word = String::new();
            while let Some(c) = self.at(0) {
                if c.is_alphanumeric() || c == '_' || c == '$' {
                    word.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
            let kind = if is_keyword(&word) {
                TokKind::Keyword
            } else {
                TokKind::Ident
            };
            return Ok(Token {
                kind,
                value: word,
                number: 0.0,
                parts: Vec::new(),
                pos,
                start,
                end: self.pos,
            });
        }

        for op in OPS {
            if self.matches(op) {
                for _ in 0..op.chars().count() {
                    self.bump();
                }
                return Ok(Token {
                    kind: TokKind::Op,
                    value: (*op).to_string(),
                    number: 0.0,
                    parts: Vec::new(),
                    pos,
                    start,
                    end: self.pos,
                });
            }
        }

        if "()[]{},.:;#".contains(c) {
            self.bump();
            return Ok(Token {
                kind: TokKind::Punct,
                value: c.to_string(),
                number: 0.0,
                parts: Vec::new(),
                pos,
                start,
                end: self.pos,
            });
        }

        Err(self.err(format!("unexpected character '{}'", c)))
    }

    fn read_number(&mut self, start: usize, pos: Pos) -> Token {
        let mut raw = String::new();
        while let Some(c) = self.at(0) {
            if c.is_ascii_digit() {
                raw.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if self.at(0) == Some('.') && self.at(1).is_some_and(|c| c.is_ascii_digit()) {
            raw.push('.');
            self.bump();
            while let Some(c) = self.at(0) {
                if c.is_ascii_digit() {
                    raw.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
        }
        if matches!(self.at(0), Some('e') | Some('E')) {
            let mut look = 1;
            if matches!(self.at(look), Some('+') | Some('-')) {
                look += 1;
            }
            if self.at(look).is_some_and(|c| c.is_ascii_digit()) {
                if let Some(c) = self.bump() {
                    raw.push(c);
                }
                if matches!(self.at(0), Some('+') | Some('-')) {
                    if let Some(c) = self.bump() {
                        raw.push(c);
                    }
                }
                while let Some(c) = self.at(0) {
                    if c.is_ascii_digit() {
                        raw.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        Token {
            kind: TokKind::Number,
            value: raw.clone(),
            number: raw.parse().unwrap_or(0.0),
            parts: Vec::new(),
            pos,
            start,
            end: self.pos,
        }
    }
}
