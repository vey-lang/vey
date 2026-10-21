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

fn is_keyword(word: &str) -> bool {
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
                Some(c) if c.is_whitespace() => {
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
                raw.push(self.bump().unwrap());
                if matches!(self.at(0), Some('+') | Some('-')) {
                    raw.push(self.bump().unwrap());
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

    fn read_string(&mut self, start: usize, pos: Pos, quote: char) -> Result<Token, VeyError> {
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
                });
            }

            if c == '\\' {
                self.bump();
                if let Some(e) = self.bump() {
                    text.push(match e {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        other => other,
                    });
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

    /// Consume a raw `{ ... }` section and return its inner text.
    ///
    /// The lexer must be positioned right after the token that introduces the
    /// block (for example `style`). Braces inside strings and `/* */` comments
    /// are ignored so CSS values survive intact.
    pub fn raw_brace_block(&mut self) -> Result<(String, Pos), VeyError> {
        self.skip_space();
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

    fn match_brace(&self, start: usize) -> Option<usize> {
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

/// Resolve the escape sequences of a string literal.
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
