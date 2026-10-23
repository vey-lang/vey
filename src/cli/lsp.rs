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
            "textDocument/documentSymbol" => {
                let uri = message
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.text("uri"))?;
                Some(answer(id, self.document_symbols(&uri)))
            }
            "textDocument/definition" => {
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.definition(&uri, line, col)))
            }
            "textDocument/references" => {
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.references(&uri, line, col)))
            }
            "textDocument/rename" => {
                let (uri, line, col) = self.place(message)?;
                let new_name = match message.get("params").and_then(|p| p.get("newName")) {
                    Some(Value::Text(text)) => text.clone(),
                    _ => return None,
                };
                Some(answer(id, self.rename(&uri, line, col, &new_name)))
            }
            "textDocument/didClose" => {
                if let Some(uri) = message
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.text("uri"))
                {
                    self.files.remove(uri);
                }
                None
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
        let character = position.number("character")? as usize;
        let source_line = self.files.get(&uri)?.lines().nth(line.saturating_sub(1))?;
        let mut utf16_column = 0;
        let mut character_column = 0;
        for ch in source_line.chars() {
            if character <= utf16_column {
                break;
            }
            let next_utf16_column = utf16_column + ch.len_utf16();
            if character < next_utf16_column {
                break;
            }
            utf16_column = next_utf16_column;
            character_column += 1;
        }
        Some((uri, line, character_column + 1))
    }

    /// Say what is wrong with a file.
    fn publish(&mut self, uri: &str) {
        let problems = self.problems(uri);
        // A mistake of another file is said for that file, so that the
        // editor may show it where it belongs.
        let mut groups: Vec<(String, Vec<Value>)> = vec![(uri.to_string(), Vec::new())];
        for (file, problem) in problems {
            let target = match file {
                Some(file) if !file.is_empty() => format!("file://{}", file),
                _ => uri.to_string(),
            };
            match groups.iter_mut().find(|(group, _)| *group == target) {
                Some((_, list)) => list.push(problem),
                None => groups.push((target, vec![problem])),
            }
        }
        for (target, problems) in groups {
            self.send_problems(&target, problems);
        }
    }

    /// Say the problems of one file.
    fn send_problems(&self, uri: &str, problems: Vec<Value>) {
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
    ///
    /// Every problem says which file it belongs to, because the file that is
    /// opened may use a module that is wrong.
    fn problems(&self, uri: &str) -> Vec<(Option<String>, Value)> {
        let Some(text) = self.files.get(uri) else {
            return Vec::new();
        };
        let path = path_of(uri);
        let label = path.to_string_lossy().to_string();
        // A problem of the file that is opened belongs to the buffer of the
        // editor, which may not be written on the disk yet. The places of
        // the other files are the ones on the disk.
        let opened = path.canonicalize().ok();
        let file_of = |file: &str| -> Option<String> {
            if opened.as_deref() == Some(std::path::Path::new(file)) {
                None
            } else {
                Some(file.to_string())
            }
        };
        let mut parser = match Parser::new(text, &label) {
            Ok(parser) => parser,
            Err(err) => return vec![(None, problem(&err, 1, 1))],
        };
        if let Err(err) = parser.parse_program() {
            return vec![(None, problem(&err, err.line, err.col))];
        }

        // The file is checked with the modules it uses, so that the
        // mistakes of the types and the warnings are said as well.
        let mut out = Vec::new();
        match vey::compiler::check_file(&path, Some(text)) {
            Ok(problems) => {
                for warning in &problems.warnings {
                    out.push((file_of(&warning.file), warning_problem(warning)));
                }
                for err in &problems.errors {
                    out.push((file_of(&err.file), problem(err, err.line, err.col)));
                }
            }
            Err(err) => out.push((file_of(&err.file), problem(&err, err.line, err.col))),
        }
        out
    }

    /// The names that are declared in a file.
    fn names(&self, uri: &str) -> Option<Names> {
        let text = self.files.get(uri)?;
        let path = path_of(uri);
        let label = path.to_string_lossy().to_string();
        let mut parser = Parser::new(text, &label).ok()?;
        let program = parser.parse_program().ok()?;
        Some(Names::of(&program, text))
    }

    /// The names of a file, in the shape the protocol wants.
    fn document_symbols(&self, uri: &str) -> Value {
        let Some(names) = self.names(uri) else {
            return Value::List(Vec::new());
        };
        let mut out = Vec::new();
        for (name, pos, kind) in &names.defs {
            out.push(json::object(vec![
                ("name", json::text(name)),
                ("kind", json::number(symbol_kind(kind))),
                ("range", name_range(&names.source, pos, name)),
                ("selectionRange", name_range(&names.source, pos, name)),
            ]));
        }
        Value::List(out)
    }

    /// Where a name is declared.
    fn definition(&self, uri: &str, line: usize, col: usize) -> Value {
        let Some(names) = self.names(uri) else {
            return Value::Null;
        };
        let Some(symbol) = names.symbol_at(line, col) else {
            return Value::Null;
        };
        let Some(definition) = names.symbols[symbol].declaration else {
            return Value::Null;
        };
        let (name, pos, _) = &names.defs[definition];
        location(uri, &names.source, pos, name)
    }

    /// Where a name is used.
    fn references(&self, uri: &str, line: usize, col: usize) -> Value {
        let Some(names) = self.names(uri) else {
            return Value::List(Vec::new());
        };
        let Some(symbol) = names.symbol_at(line, col) else {
            return Value::List(Vec::new());
        };
        let mut out = Vec::new();
        if let Some(definition) = names.symbols[symbol].declaration {
            let (name, pos, _) = &names.defs[definition];
            out.push(location(uri, &names.source, pos, name));
        }
        for (index, (name, pos)) in names.uses.iter().enumerate() {
            if names.use_symbols[index] == Some(symbol) {
                out.push(location(uri, &names.source, pos, name));
            }
        }
        Value::List(out)
    }

    /// The places where a name is written, so that it may be changed.
    fn rename(&self, uri: &str, line: usize, col: usize, new_name: &str) -> Value {
        let Some(names) = self.names(uri) else {
            return Value::Null;
        };
        let Some(symbol) = names.symbol_at(line, col) else {
            return Value::Null;
        };
        let Some(definition) = names.symbols[symbol].declaration else {
            return Value::Null;
        };
        let mut changes = Vec::new();
        let mut places = vec![(names.defs[definition].0.clone(), names.defs[definition].1)];
        for (index, (name, pos)) in names.uses.iter().enumerate() {
            if names.use_symbols[index] == Some(symbol) {
                places.push((name.clone(), *pos));
            }
        }
        for (name, pos) in places {
            changes.push(json::object(vec![
                ("range", name_range(&names.source, &pos, &name)),
                ("newText", json::text(new_name)),
            ]));
        }
        json::object(vec![(
            "changes",
            json::object(vec![(uri, Value::List(changes))]),
        )])
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
        if parser.parse_program().is_err() {
            return Value::Null;
        }
        // The modules that the file uses are known, so that the type of a
        // value of another module is the real one.
        let ty = match vey::compiler::explain_source(&path, Some(text), line, col) {
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
            ("documentSymbolProvider", Value::Bool(true)),
            ("definitionProvider", Value::Bool(true)),
            ("referencesProvider", Value::Bool(true)),
            ("renameProvider", Value::Bool(true)),
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

/// The names of a program, and the places they are written.
///
/// The names are found by walking the tree of the program, so that the
/// editor may say where a name is declared and where it is used.
struct Names {
    source: String,
    /// The names that are declared.
    defs: Vec<(String, vey::lexer::Pos, &'static str)>,
    /// The binding identity for each declaration (including non-value names).
    def_symbols: Vec<Option<usize>>,
    /// The names that are used.
    uses: Vec<(String, vey::lexer::Pos)>,
    /// The binding identity for each use, when it resolves.
    use_symbols: Vec<Option<usize>>,
    /// The value names in each lexical scope.
    scopes: Vec<HashMap<String, usize>>,
    /// A binding's declaration, indexed by its stable identity.
    symbols: Vec<NameSymbol>,
    /// Top-level bindings are visible to function bodies before their text.
    predeclared: Vec<(String, vey::lexer::Pos, usize)>,
}

struct NameSymbol {
    declaration: Option<usize>,
}

impl Names {
    fn new(source: &str) -> Names {
        Names {
            source: source.to_string(),
            defs: Vec::new(),
            def_symbols: Vec::new(),
            uses: Vec::new(),
            use_symbols: Vec::new(),
            scopes: vec![HashMap::new()],
            symbols: Vec::new(),
            predeclared: Vec::new(),
        }
    }

    /// The names of a program.
    fn of(program: &vey::ast::Program, source: &str) -> Names {
        let mut names = Names::new(source);
        names.predeclare_top_level(&program.body);
        names.body(&program.body);
        names
    }

    fn predeclare_top_level(&mut self, body: &[vey::ast::Stmt]) {
        use vey::ast::Stmt;
        for stmt in body {
            match stmt {
                Stmt::Var(decl) => {
                    for (name, pos) in decl.pattern.bindings() {
                        self.reserve(&name, pos);
                    }
                }
                Stmt::Fn(decl) => {
                    let pos = self.identifier_pos(decl.pos, &decl.name);
                    self.reserve(&decl.name, pos);
                }
                Stmt::Component(decl) => {
                    let pos = self.identifier_pos(decl.pos, &decl.name);
                    self.reserve(&decl.name, pos);
                }
                Stmt::Enum(decl) => {
                    let pos = self.identifier_pos(decl.pos, &decl.name);
                    self.reserve(&decl.name, pos);
                }
                Stmt::Use(decl) if !decl.path.ends_with(".css") => {
                    if let Some(pos) = decl.alias_pos {
                        self.reserve(&decl.alias, pos);
                    } else {
                        self.define_hidden(&decl.alias);
                    }
                }
                _ => {}
            }
        }
    }

    fn reserve(&mut self, name: &str, pos: vey::lexer::Pos) {
        let Some(scope) = self.scopes.last_mut() else {
            return;
        };
        if scope.contains_key(name) {
            return;
        }
        let symbol = self.symbols.len();
        self.symbols.push(NameSymbol { declaration: None });
        scope.insert(name.to_string(), symbol);
        self.predeclared.push((name.to_string(), pos, symbol));
    }

    fn identifier_pos(&self, base: vey::lexer::Pos, name: &str) -> vey::lexer::Pos {
        let Some(line) = self.source.lines().nth(base.line.saturating_sub(1)) else {
            return base;
        };
        let start_byte = line
            .char_indices()
            .nth(base.col.saturating_sub(1))
            .map(|(index, _)| index)
            .unwrap_or(line.len());
        let remaining = &line[start_byte..];
        for (offset, _) in remaining.match_indices(name) {
            let byte = start_byte + offset;
            let before = line[..byte].chars().next_back();
            let after = line[byte + name.len()..].chars().next();
            let is_ident = |c: char| c == '_' || c.is_alphanumeric();
            if before.is_some_and(is_ident) || after.is_some_and(is_ident) {
                continue;
            }
            let column = line[..byte].chars().count() + 1;
            return vey::lexer::Pos::new(base.line, column);
        }
        base
    }

    fn body(&mut self, body: &[vey::ast::Stmt]) {
        for stmt in body {
            self.stmt(stmt);
        }
    }

    fn define(&mut self, name: &str, pos: vey::lexer::Pos, kind: &'static str) {
        let definition = self.defs.len();
        let symbol = if matches!(
            kind,
            "variable" | "parameter" | "function" | "component" | "enum" | "module"
        ) {
            if let Some((_, _, symbol)) = self
                .predeclared
                .iter()
                .find(|(reserved, reserved_pos, _)| reserved == name && *reserved_pos == pos)
            {
                Some(*symbol)
            } else if self
                .scopes
                .last()
                .is_some_and(|scope| scope.contains_key(name))
            {
                None
            } else {
                let symbol = self.symbols.len();
                self.symbols.push(NameSymbol {
                    declaration: Some(definition),
                });
                if let Some(scope) = self.scopes.last_mut() {
                    scope.insert(name.to_string(), symbol);
                }
                Some(symbol)
            }
        } else {
            let symbol = self.symbols.len();
            self.symbols.push(NameSymbol {
                declaration: Some(definition),
            });
            Some(symbol)
        };
        if let Some(symbol) = symbol {
            self.symbols[symbol].declaration = Some(definition);
        }
        self.defs.push((name.to_string(), pos, kind));
        self.def_symbols.push(symbol);
    }

    fn use_name(&mut self, name: &str, pos: vey::lexer::Pos) {
        self.uses.push((name.to_string(), pos));
        self.use_symbols.push(self.resolve(name));
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn define_hidden(&mut self, name: &str) {
        if self
            .scopes
            .last()
            .is_some_and(|scope| scope.contains_key(name))
        {
            return;
        }
        let symbol = self.symbols.len();
        self.symbols.push(NameSymbol { declaration: None });
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), symbol);
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn block(&mut self, body: &[vey::ast::Stmt]) {
        self.push_scope();
        self.body(body);
        self.pop_scope();
    }

    fn stmt(&mut self, stmt: &vey::ast::Stmt) {
        use vey::ast::Stmt;
        match stmt {
            Stmt::Var(decl) => {
                if let Some(init) = &decl.init {
                    self.expr(init);
                }
                self.pattern(&decl.pattern, "variable");
            }
            Stmt::Fn(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "function");
                self.push_scope();
                for param in &decl.params {
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::Component(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "component");
                self.push_scope();
                for param in &decl.params {
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                for inner in &decl.body {
                    self.stmt(inner);
                }
                self.markup(&decl.children);
                self.pop_scope();
            }
            Stmt::Enum(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "enum");
                for variant in &decl.variants {
                    self.define(&variant.name, variant.pos, "variant");
                }
            }
            Stmt::Type(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "type");
            }
            Stmt::Interface(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "interface");
            }
            Stmt::Impl(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.type_name);
                self.define(&decl.type_name, pos, "type");
                for method in &decl.methods {
                    self.define(&method.name, method.pos, "method");
                    self.push_scope();
                    self.define_hidden("self");
                    for param in &method.params {
                        if let Some(default) = &param.default {
                            self.expr(default);
                        }
                        self.pattern(&param.pattern, "parameter");
                    }
                    self.body(&method.body.body);
                    self.pop_scope();
                }
            }
            Stmt::If(decl) => {
                self.expr(&decl.test);
                self.block(&decl.then_block.body);
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::IfLet(decl) => {
                self.expr(&decl.value);
                self.push_scope();
                self.pattern_match(&decl.pattern);
                self.block(&decl.then_block.body);
                self.pop_scope();
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::While(decl) => {
                self.expr(&decl.test);
                self.block(&decl.body.body);
            }
            Stmt::WhileLet(decl) => {
                self.expr(&decl.value);
                self.push_scope();
                self.pattern_match(&decl.pattern);
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::LetElse(decl) => {
                self.expr(&decl.value);
                self.pattern(&decl.pattern, "variable");
                self.block(&decl.otherwise.body);
            }
            Stmt::Defer(decl) => self.expr(&decl.value),
            Stmt::Throw(decl) => self.expr(&decl.value),
            Stmt::For(decl) => {
                if let Some(step) = &decl.step {
                    self.expr(step);
                }
                self.expr(&decl.iter);
                self.push_scope();
                self.pattern(&decl.pattern, "variable");
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::Repeat(decl) => {
                self.expr(&decl.count);
                self.block(&decl.body.body);
            }
            Stmt::Return(decl) => {
                if let Some(value) = &decl.value {
                    self.expr(value);
                }
            }
            Stmt::Try(decl) => {
                self.block(&decl.body.body);
                if let Some(catch) = &decl.catch {
                    self.push_scope();
                    if let Some(name) = &decl.name {
                        self.define_hidden(name);
                    }
                    self.body(&catch.body);
                    self.pop_scope();
                }
                if let Some(finally) = &decl.finally {
                    self.block(&finally.body);
                }
            }
            Stmt::Expr(decl) => self.expr(&decl.expr),
            Stmt::Assign(decl) => {
                self.expr(&decl.target);
                self.expr(&decl.value);
            }
            Stmt::Block(block, _) => self.block(&block.body),
            Stmt::Use(decl) if !decl.path.ends_with(".css") => {
                if let Some(pos) = decl.alias_pos {
                    self.define(&decl.alias, pos, "module");
                } else {
                    self.define_hidden(&decl.alias);
                }
            }
            Stmt::Use(_) => {}
            Stmt::Break(_, _) | Stmt::Continue(_, _) | Stmt::Style(_) => {}
        }
    }

    /// The names of a pattern are the ones that are bound.
    fn pattern(&mut self, pattern: &vey::ast::BindingPattern, kind: &'static str) {
        use vey::ast::BindingPattern;
        match pattern {
            BindingPattern::Name { name, pos, default } => {
                if let Some(default) = default {
                    self.expr(default);
                }
                self.define(name, *pos, kind);
            }
            BindingPattern::Array { items, rest, .. } => {
                for item in items {
                    self.pattern(item, kind);
                }
                if let Some(rest) = rest {
                    self.define_hidden(rest);
                }
            }
            BindingPattern::Object { fields, rest, .. } => {
                for field in fields {
                    self.pattern(&field.pattern, kind);
                }
                if let Some(rest) = rest {
                    self.define_hidden(rest);
                }
            }
        }
    }

    fn markup(&mut self, nodes: &[vey::ast::MarkupNode]) {
        use vey::ast::MarkupNode;
        for node in nodes {
            match node {
                MarkupNode::Expr(expr) => self.expr(expr),
                MarkupNode::Element(element) => {
                    if element.component {
                        if let Some((module, _)) = element.tag.rsplit_once('.') {
                            self.use_name(module, element.pos);
                        } else {
                            self.use_name(&element.tag, element.pos);
                        }
                    }
                    for (_, value) in &element.attrs {
                        self.expr(value);
                    }
                    for value in &element.args {
                        self.expr(value);
                    }
                    self.markup(&element.children);
                }
                MarkupNode::If(branch) => {
                    self.expr(&branch.test);
                    self.markup(&branch.then_nodes);
                    self.markup(&branch.else_nodes);
                }
                MarkupNode::For(loop_node) => {
                    self.expr(&loop_node.iter);
                    self.push_scope();
                    self.define_hidden(&loop_node.name);
                    if let Some(index) = &loop_node.index {
                        self.define_hidden(index);
                    }
                    if let Some(key) = &loop_node.key {
                        self.expr(key);
                    }
                    self.markup(&loop_node.body);
                    self.pop_scope();
                }
                MarkupNode::Match(m) => {
                    self.expr(&m.subject);
                    for arm in &m.arms {
                        self.push_scope();
                        for pattern in &arm.patterns {
                            self.pattern_match(pattern);
                        }
                        if let Some(guard) = &arm.guard {
                            self.expr(guard);
                        }
                        self.markup(&arm.body);
                        self.pop_scope();
                    }
                }
            }
        }
    }

    /// The names of a pattern of a match are the ones that are bound.
    fn pattern_match(&mut self, pattern: &vey::ast::Pattern) {
        use vey::ast::Pattern;
        match pattern {
            Pattern::Binding { name, pos } => self.define(name, *pos, "variable"),
            Pattern::Enum { bindings, .. } => {
                for binding in bindings {
                    self.define(&binding.name, binding.pos, "variable");
                }
            }
            Pattern::List(items, _) => {
                for item in items {
                    self.pattern_match(item);
                }
            }
            Pattern::Object(fields, _) => {
                for (_, item) in fields {
                    self.pattern_match(item);
                }
            }
            other => {
                let _ = other;
            }
        }
    }

    fn expr(&mut self, expr: &vey::ast::Expr) {
        use vey::ast::Expr;
        match expr {
            Expr::Ident(name, pos) => self.use_name(name, *pos),
            Expr::String(parts, _) => {
                for part in parts {
                    if let Some(value) = &part.expr {
                        self.expr(value);
                    }
                }
            }
            Expr::Array(items, _) => {
                for item in items {
                    self.expr(item);
                }
            }
            Expr::Object(props, _) => {
                for prop in props {
                    match prop {
                        vey::ast::ObjectProp::Field(_, value) => self.expr(value),
                        vey::ast::ObjectProp::Spread(value) => self.expr(value),
                    }
                }
            }
            Expr::Unary(_, value, _) => self.expr(value),
            Expr::Binary(_, left, right, _)
            | Expr::Logical(_, left, right, _)
            | Expr::Nullish(left, right, _)
            | Expr::Range(left, right, _) => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Ternary(test, then_value, else_value, _) => {
                self.expr(test);
                self.expr(then_value);
                self.expr(else_value);
            }
            Expr::Call(callee, args, _, _) | Expr::CallGeneric(callee, _, args, _, _) => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Member(object, prop, _, _) => {
                self.expr(object);
                if let vey::ast::MemberProp::Computed(index) = prop {
                    self.expr(index);
                }
            }
            Expr::Fn(params, body, _, _) => {
                self.push_scope();
                for param in params {
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                self.body(&body.body);
                self.pop_scope();
            }
            Expr::Arrow(_, params, body, _, _) => {
                self.push_scope();
                for param in params {
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                match body {
                    vey::ast::ArrowBody::Expr(value) => self.expr(value),
                    vey::ast::ArrowBody::Block(block) => self.body(&block.body),
                }
                self.pop_scope();
            }
            Expr::Match(subject, arms, _) => {
                self.expr(subject);
                for arm in arms {
                    self.push_scope();
                    for pattern in &arm.patterns {
                        self.pattern_match(pattern);
                    }
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    self.expr(&arm.body);
                    self.pop_scope();
                }
            }
            Expr::Await(value, _) | Expr::Try(value, _) | Expr::Spread(value, _) => {
                self.expr(value)
            }
            Expr::As(value, _, _) | Expr::Is(value, _, _) => self.expr(value),
            Expr::Yield(value, _) => self.expr(value),
            _ => {}
        }
    }

    /// The name that is at a place, if there is one there.
    /// The identity of the name that is at a place, if it is bound.
    fn symbol_at(&self, line: usize, col: usize) -> Option<usize> {
        for (index, (name, pos)) in self.uses.iter().enumerate() {
            if pos.line == line && col >= pos.col && col < pos.col + name.chars().count() {
                return self.use_symbols[index];
            }
        }
        for (index, (name, pos, _)) in self.defs.iter().enumerate() {
            if pos.line == line && col >= pos.col && col < pos.col + name.chars().count() {
                return self.def_symbols[index];
            }
        }
        None
    }
}

/// The number of a kind of name, in the shape the protocol wants.
fn symbol_kind(kind: &str) -> f64 {
    match kind {
        "function" => 12.0,
        "component" => 12.0,
        "method" => 6.0,
        "type" => 5.0,
        "interface" => 11.0,
        "enum" => 10.0,
        "variant" => 22.0,
        "parameter" => 13.0,
        "module" => 2.0,
        _ => 13.0,
    }
}

/// The range of a name in the UTF-16 positions required by LSP.
fn name_range(source: &str, pos: &vey::lexer::Pos, name: &str) -> Value {
    let line = source.lines().nth(pos.line.saturating_sub(1)).unwrap_or("");
    let start = line
        .chars()
        .take(pos.col.saturating_sub(1))
        .map(char::len_utf16)
        .sum::<usize>();
    let end = start + name.encode_utf16().count();
    json::object(vec![
        (
            "start",
            json::object(vec![
                ("line", json::number(pos.line as f64 - 1.0)),
                ("character", json::number(start as f64)),
            ]),
        ),
        (
            "end",
            json::object(vec![
                ("line", json::number(pos.line as f64 - 1.0)),
                ("character", json::number(end as f64)),
            ]),
        ),
    ])
}

/// The place of a name in a file, in the shape the protocol wants.
fn location(uri: &str, source: &str, pos: &vey::lexer::Pos, name: &str) -> Value {
    json::object(vec![
        ("uri", json::text(uri)),
        ("range", name_range(source, pos, name)),
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
        (
            "message",
            json::text(&message_with_hint(&err.message, err.hint.as_deref())),
        ),
    ];
    items.push(("code", json::text("vey")));
    json::object(items)
}

/// A warning of the compiler, as the protocol wants it.
fn warning_problem(warning: &vey::warn::Warning) -> Value {
    let items = vec![
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
        (
            "message",
            json::text(&message_with_hint(&warning.message, warning.hint.as_deref())),
        ),
    ];
    json::object(items)
}

/// The text of a problem, with the hint of the compiler on a line of its
/// own.
///
/// The protocol has no field of its own for a hint, so it is part of the
/// message.
fn message_with_hint(message: &str, hint: Option<&str>) -> String {
    match hint {
        Some(hint) if !hint.is_empty() => format!("{}\n\nhint: {}", message, hint),
        _ => message.to_string(),
    }
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
