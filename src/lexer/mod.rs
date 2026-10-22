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

    /// Whether the place comes after the other one.
    pub fn after(&self, other: &Pos) -> bool {
        self.line > other.line || (self.line == other.line && self.col >= other.col)
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
    /// Whether the token is the first one of its line.
    ///
    /// The end of a line may end a statement, so the parser needs to know.
    pub newline_before: bool,
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
            newline_before: false,
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
    /// The doc comments of the file, in the order they were written.
    docs: Vec<(String, Pos)>,
    /// The version of the language the file was written for.
    version: Option<String>,
    /// Whether a newline was crossed before the token that is read.
    newline: bool,
}

const OPS: &[&str] = &[
    "...", "?.", "|>", "=>", "->", "**=", "??=", "<<=", ">>=", "&=", "|=", "^=", "??", "**", "==",
    "!=", "<=", ">=", "<<", ">>", "..", "+=", "-=", "*=", "/=", "%=", "&", "|", "^", "!", "+", "-",
    "*", "/", "%", "=", "<", ">", "?",
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
            | "pub"
            | "priv"
            | "throw"
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
            docs: Vec::new(),
            version: None,
            newline: false,
        }
    }

    /// The doc comments of the file.
    pub fn take_docs(&mut self) -> Vec<(String, Pos)> {
        std::mem::take(&mut self.docs)
    }

    /// The version of the language the file was written for.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
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

    fn skip_space(&mut self) -> Result<(), VeyError> {
        self.newline = false;
        loop {
            match self.at(0) {
                // A file that starts with a byte order mark is read as if it
                // was not there.
                Some(c) if c.is_whitespace() || c == '\u{feff}' => {
                    if c == '\n' {
                        self.newline = true;
                    }
                    self.bump();
                }
                Some('/') if self.at(1) == Some('/') => {
                    let pos = Pos::new(self.line, self.col);
                    let mut text = String::new();
                    while let Some(c) = self.at(0) {
                        if c == '\n' {
                            break;
                        }
                        self.bump();
                        text.push(c);
                    }
                    self.read_comment(&text, pos);
                }
                Some('/') if self.at(1) == Some('*') => {
                    // A comment that may go over several lines.
                    self.bump();
                    self.bump();
                    loop {
                        match self.at(0) {
                            None => {
                                return Err(self.err("the comment is not closed"));
                            }
                            Some('*') if self.at(1) == Some('/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            Some(_) => {
                                self.bump();
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        Ok(())
    }

    /// Read a comment.
    ///
    /// A comment of three slashes is a doc comment, and a comment that
    /// names the version of the language says which one the file was
    /// written for.
    fn read_comment(&mut self, text: &str, pos: Pos) {
        if let Some(rest) = text.strip_prefix("///") {
            let mut line = rest.to_string();
            if let Some(hash) = line.find('#') {
                line.truncate(hash);
            }
            self.docs.push((line.trim().to_string(), pos));
            return;
        }
        let trimmed = text.trim_start();
        let Some(rest) = trimmed.strip_prefix("//") else {
            return;
        };
        let trimmed = rest.trim_start();
        if let Some(rest) = trimmed.strip_prefix("vey:") {
            let version = rest.trim();
            if !version.is_empty() && self.version.is_none() {
                self.version = Some(version.to_string());
            }
        }
    }

    pub fn next_token(&mut self) -> Result<Token, VeyError> {
        self.skip_space()?;

        let start = self.pos;
        let pos = Pos::new(self.line, self.col);
        let newline_before = self.newline;

        let Some(c) = self.at(0) else {
            return Ok(Token::eof(pos, start));
        };

        if c.is_ascii_digit() || (c == '.' && self.at(1).is_some_and(|c| c.is_ascii_digit())) {
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
                newline_before,
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
                    newline_before,
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
                newline_before,
            });
        }

        Err(self.err(format!("unexpected character '{}'", c)))
    }

    fn read_number(&mut self, start: usize, pos: Pos) -> Token {
        let newline_before = self.newline;
        let mut raw = String::new();

        // A number may be written in another base, as in `0xff`.
        if self.at(0) == Some('0') {
            if let Some(mark) = self.at(1) {
                let radix = match mark {
                    'x' | 'X' => Some(16),
                    'b' | 'B' => Some(2),
                    'o' | 'O' => Some(8),
                    _ => None,
                };
                if let Some(radix) = radix {
                    self.bump();
                    self.bump();
                    let mut digits = String::new();
                    while let Some(c) = self.at(0) {
                        if c == '_' {
                            self.bump();
                            continue;
                        }
                        if !c.is_digit(radix) {
                            break;
                        }
                        digits.push(c);
                        self.bump();
                    }
                    if digits.is_empty() {
                        return Token {
                            kind: TokKind::Number,
                            value: "0".to_string(),
                            number: 0.0,
                            parts: Vec::new(),
                            pos,
                            start,
                            end: self.pos,
                            newline_before,
                        };
                    }
                    let number = u64::from_str_radix(&digits, radix)
                        .map(|value| value as f64)
                        .unwrap_or(0.0);
                    return Token {
                        kind: TokKind::Number,
                        value: digits,
                        number,
                        parts: Vec::new(),
                        pos,
                        start,
                        end: self.pos,
                        newline_before,
                    };
                }
            }
        }

        while let Some(c) = self.at(0) {
            if c.is_ascii_digit() {
                raw.push(c);
                self.bump();
            } else if c == '_' {
                // The digits of a number may be separated by a mark.
                self.bump();
            } else {
                break;
            }
        }
        if raw.is_empty() && self.at(0) == Some('.') {
            // A number may begin with a point, as in `.5`.
            raw.push('.');
            self.bump();
            while let Some(c) = self.at(0) {
                if c.is_ascii_digit() {
                    raw.push(c);
                    self.bump();
                } else if c == '_' {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        if self.at(0) == Some('.') && self.at(1).is_some_and(|c| c.is_ascii_digit()) {
            raw.push('.');
            self.bump();
            while let Some(c) = self.at(0) {
                if c.is_ascii_digit() {
                    raw.push(c);
                    self.bump();
                } else if c == '_' {
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
                    } else if c == '_' {
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
        }
        let number = raw.parse().unwrap_or(0.0);
        Token {
            kind: TokKind::Number,
            value: raw.clone(),
            number,
            parts: Vec::new(),
            pos,
            start,
            end: self.pos,
            newline_before,
        }
    }
}
