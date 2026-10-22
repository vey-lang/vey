//! The language server.
//!
//! The server speaks the protocol of the language servers over the standard
//! input and the standard output. It says what is wrong with the program,
//! the type of the expression under the cursor and the names that may be
//! written there.

use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use vey::parser::Parser;
use vey::types;

use crate::cli::args::parse_args;
use crate::cli::json::{self, Value};

pub(crate) fn cmd_lsp(args: &[String]) -> i32 {
    let parsed = match parse_args(args) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("{}", err);
            return 1;
        }
    };
    if parsed.help {
        println!("usage: vey lsp");
        println!();
        println!("The language server of Vey. It speaks the protocol of the");
        println!("language servers over the standard input and the standard output.");
        return 0;
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut server = Server {
        files: HashMap::new(),
    };

    loop {
        let Some(message) = read_message(&mut stdin.lock()) else {
            break;
        };
        let Some(answer) = server.handle(&message) else {
            continue;
        };
        if write_message(&mut stdout, &answer).is_err() {
            break;
        }
    }
    0
}

/// Read a message of the protocol.
fn read_message(input: &mut impl BufRead) -> Option<Value> {
    let mut length = 0usize;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let line = line.trim_end();
        if line.is_empty() {
            break;
        }
        if let Some(rest) = line.strip_prefix("Content-Length:") {
            length = rest.trim().parse().ok()?;
        }
    }
    let mut body = vec![0u8; length];
    input.read_exact(&mut body).ok()?;
    let text = String::from_utf8(body).ok()?;
    json::read(&text).map(|(value, _)| value)
}

/// Write a message of the protocol.
fn write_message(output: &mut impl Write, value: &Value) -> io::Result<()> {
    let body = value.to_json();
    write!(output, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    output.flush()
}

struct Server {
    /// The files that are open, with the text of them.
    files: HashMap<String, String>,
}

impl Server {
    /// The answer of a message, when there is one.
    fn handle(&mut self, message: &Value) -> Option<Value> {
        let method = message.text("method")?;
        let id = message.get("id").cloned();

        match method {
            "initialize" => Some(answer(id, initialize())),
            "initialized" => None,
            "shutdown" => Some(answer(id, Value::Null)),
            "exit" => None,
            "textDocument/didOpen" => {
                if let Some(document) = message.get("params").and_then(|p| p.get("textDocument")) {
                    if let (Some(uri), Some(text)) = (document.text("uri"), document.text("text")) {
                        self.files.insert(uri.to_string(), text.to_string());
                        self.publish(uri);
                    }
                }
                None
            }
            "textDocument/didChange" => {
                if let Some(document) = message.get("params").and_then(|p| p.get("textDocument")) {
                    if let Some(uri) = document.text("uri") {
                        // The whole text is sent, so the last change holds it.
                        if let Some(changes) = message
                            .get("params")
                            .and_then(|p| p.list("contentChanges"))
                        {
                            if let Some(Value::Text(text)) = changes.last() {
                                self.files.insert(uri.to_string(), text.clone());
                            }
                        }
                        self.publish(uri);
                    }
                }
                None
            }
            "textDocument/didSave" => {
                if let Some(uri) = message
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.text("uri"))
                {
                    self.publish(uri);
                }
                None
            }
            "textDocument/hover" => {
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.hover(&uri, line, col)))
            }
            "textDocument/completion" => {
                let (uri, _line, _col) = self.place(message)?;
                Some(answer(id, self.completion(&uri)))
            }
            _ => Some(answer(id, Value::Null)),
        }
    }

    /// The file, the line and the column of a message.
    fn place(&self, message: &Value) -> Option<(String, usize, usize)> {
        let document = message.get("params")?.get("textDocument")?;
        let uri = document.text("uri")?.to_string();
        let position = message.get("params")?.get("position")?;
        let line = position.number("line")? as usize + 1;
        let character = position.number("character")? as usize + 1;
        Some((uri, line, character))
    }

    /// Say what is wrong with a file.
    fn publish(&mut self, uri: &str) {
        let problems = self.problems(uri);
        let message = json::object(vec![
            ("jsonrpc", json::text("2.0")),
            ("method", json::text("textDocument/publishDiagnostics")),
            (
                "params",
                json::object(vec![
                    ("uri", json::text(uri)),
                    ("diagnostics", Value::List(problems)),
                ]),
            ),
        ]);
        let stdout = io::stdout();
        let mut lock = stdout.lock();
        let _ = write_message(&mut lock, &message);
    }

    /// The mistakes of a file, in the shape the protocol wants.
    fn problems(&self, uri: &str) -> Vec<Value> {
        let Some(text) = self.files.get(uri) else {
            return Vec::new();
        };
        let path = path_of(uri);
        let label = path.to_string_lossy().to_string();
        let mut parser = match Parser::new(text, &label) {
            Ok(parser) => parser,
            Err(err) => return vec![problem(&err, 1, 1)],
        };
        if let Err(err) = parser.parse_program() {
            return vec![problem(&err, err.line, err.col)];
        }

        // The file is checked with the modules it uses, so that the
        // mistakes of the types and the warnings are said as well.
        let mut out = Vec::new();
        match vey::compiler::check_file(&path, Some(text)) {
            Ok(problems) => {
                for warning in &problems.warnings {
                    out.push(warning_problem(warning));
                }
                if let Some(err) = &problems.error {
                    out.push(problem(err, err.line, err.col));
                }
            }
            Err(err) => out.push(problem(&err, err.line, err.col)),
        }
        out
    }

    /// The type of the expression under the cursor.
    fn hover(&self, uri: &str, line: usize, col: usize) -> Value {
        let Some(text) = self.files.get(uri) else {
            return Value::Null;
        };
        let path = path_of(uri);
        let label = path.to_string_lossy().to_string();
        let mut parser = match Parser::new(text, &label) {
            Ok(parser) => parser,
            Err(_) => return Value::Null,
        };
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(_) => return Value::Null,
        };
        let ty = match types::explain(&program, &label, line, col) {
            Ok(ty) => ty,
            Err(_) => return Value::Null,
        };
        json::object(vec![
            (
                "contents",
                json::object(vec![
                    ("kind", json::text("markdown")),
                    ("value", json::text(&format!("```vey\n{}\n```", ty))),
                ]),
            ),
        ])
    }

    /// The names that may be written at a place.
    fn completion(&self, uri: &str) -> Value {
        let Some(text) = self.files.get(uri) else {
            return Value::List(Vec::new());
        };
        let path = path_of(uri);
        let label = path.to_string_lossy().to_string();
        let mut parser = match Parser::new(text, &label) {
            Ok(parser) => parser,
            Err(_) => return Value::List(Vec::new()),
        };
        let program = match parser.parse_program() {
            Ok(program) => program,
            Err(_) => return Value::List(Vec::new()),
        };

        let mut names: Vec<Value> = Vec::new();
        for stmt in &program.body {
            if let Some((name, _)) = stmt.exported_name() {
                names.push(json::object(vec![("label", json::text(name))]));
            }
        }
        for name in vey::builtins::BUILTINS {
            names.push(json::object(vec![("label", json::text(name))]));
        }
        Value::List(names)
    }
}

/// The answer of a message.
fn answer(id: Option<Value>, result: Value) -> Value {
    json::object(vec![
        ("jsonrpc", json::text("2.0")),
        ("id", id.unwrap_or(Value::Null)),
        ("result", result),
    ])
}

/// What the server can do.
fn initialize() -> Value {
    json::object(vec![
        ("capabilities", json::object(vec![
            ("textDocumentSync", json::number(1.0)),
            ("hoverProvider", Value::Bool(true)),
            ("completionProvider", json::object(vec![
                ("triggerCharacters", Value::List(vec![json::text(".")])),
            ])),
        ])),
        (
            "serverInfo",
            json::object(vec![
                ("name", json::text("vey")),
                ("version", json::text(env!("CARGO_PKG_VERSION"))),
            ]),
        ),
    ])
}

/// A mistake, in the shape the protocol wants.
fn problem(err: &vey::VeyError, line: usize, col: usize) -> Value {
    let mut items = vec![
        (
            "range",
            json::object(vec![
                (
                    "start",
                    json::object(vec![
                        ("line", json::number((line.max(1) - 1) as f64)),
                        ("character", json::number((col.max(1) - 1) as f64)),
                    ]),
                ),
                (
                    "end",
                    json::object(vec![
                        ("line", json::number((line.max(1) - 1) as f64)),
                        ("character", json::number(col.max(1) as f64)),
                    ]),
                ),
            ]),
        ),
        ("severity", json::number(1.0)),
        ("source", json::text("vey")),
        ("message", json::text(&err.message)),
    ];
    if let Some(hint) = &err.hint {
        items.push(("hint", json::text(hint)));
    }
    json::object(items)
}

/// A warning of the compiler, as the protocol wants it.
fn warning_problem(warning: &vey::warn::Warning) -> Value {
    let mut items = vec![
        (
            "range",
            json::object(vec![
                (
                    "start",
                    json::object(vec![
                        ("line", json::number((warning.line.max(1) - 1) as f64)),
                        ("character", json::number((warning.col.max(1) - 1) as f64)),
                    ]),
                ),
                (
                    "end",
                    json::object(vec![
                        ("line", json::number((warning.line.max(1) - 1) as f64)),
                        ("character", json::number(warning.col.max(1) as f64)),
                    ]),
                ),
            ]),
        ),
        ("severity", json::number(2.0)),
        ("source", json::text("vey")),
        ("message", json::text(&warning.message)),
    ];
    if let Some(hint) = &warning.hint {
        items.push(("hint", json::text(hint)));
    }
    json::object(items)
}

/// The path of a file, from the name the protocol uses.
fn path_of(uri: &str) -> PathBuf {
    let path = uri.strip_prefix("file://").unwrap_or(uri);
    PathBuf::from(percent_decode(path))
}

/// The text of a name, with the marks that are written as `%xx`.
fn percent_decode(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut code = String::new();
        for _ in 0..2 {
            match chars.next() {
                Some(c) => code.push(c),
                None => break,
            }
        }
        match u8::from_str_radix(&code, 16) {
            Ok(byte) => out.push(byte as char),
            Err(_) => out.push('%'),
        }
    }
    out
}
