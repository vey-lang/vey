//! The language server.
//!
//! The server speaks the protocol of the language servers over the standard
//! input and the standard output. It says what is wrong with the program,
//! the type of the expression under the cursor and the names that may be
//! written there.

use std::collections::{HashMap, HashSet};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use vey::parser::Parser;

use crate::cli::args::parse_args;
use crate::cli::json::{self, Value};

pub(crate) fn cmd_lsp(args: &[String]) -> i32 {
    let parsed = match parse_args("lsp", args) {
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
        workspace_roots: Vec::new(),
    };

    let mut input = stdin.lock();
    let mut shutdown_received = false;
    let mut exit_code = 0;
    loop {
        let message = match read_message(&mut input) {
            Ok(Some(message)) => message,
            Ok(None) => break,
            Err(ReadMessageError::Malformed) => {
                let error = error_answer(Value::Null, -32700, "Parse error");
                let _ = write_message(&mut stdout, &error);
                exit_code = 1;
                break;
            }
            Err(ReadMessageError::Io(error)) => {
                eprintln!("failed to read an LSP message: {error}");
                exit_code = 1;
                break;
            }
        };
        if message.text("method") == Some("exit") {
            exit_code = if shutdown_received { 0 } else { 1 };
            break;
        }
        if message.text("method") == Some("shutdown") && message.get("id").is_some() {
            shutdown_received = true;
        }
        let Some(answer) = server.handle(&message) else {
            continue;
        };
        if write_message(&mut stdout, &answer).is_err() {
            exit_code = 1;
            break;
        }
    }
    exit_code
}

/// Read a message of the protocol.
fn read_message(input: &mut impl BufRead) -> Result<Option<Value>, ReadMessageError> {
    let mut length = None;
    let mut saw_header = false;
    loop {
        let mut line = String::new();
        let count = input.read_line(&mut line).map_err(ReadMessageError::Io)?;
        if count == 0 {
            return if saw_header {
                Err(ReadMessageError::Malformed)
            } else {
                Ok(None)
            };
        }
        saw_header = true;
        let line = line.trim_end_matches(['\r', '\n']);
        if line.is_empty() {
            break;
        }
        let Some((name, value)) = line.split_once(':') else {
            return Err(ReadMessageError::Malformed);
        };
        if name.eq_ignore_ascii_case("Content-Length") {
            if length.is_some() {
                return Err(ReadMessageError::Malformed);
            }
            length = Some(
                value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| ReadMessageError::Malformed)?,
            );
        }
    }
    let length = length.ok_or(ReadMessageError::Malformed)?;
    if length > MAX_MESSAGE_BYTES {
        return Err(ReadMessageError::Malformed);
    }
    let mut body = vec![0u8; length];
    input.read_exact(&mut body).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            ReadMessageError::Malformed
        } else {
            ReadMessageError::Io(error)
        }
    })?;
    let text = String::from_utf8(body).map_err(|_| ReadMessageError::Malformed)?;
    let (value, rest) = json::read(&text).ok_or(ReadMessageError::Malformed)?;
    if !rest.trim().is_empty() {
        return Err(ReadMessageError::Malformed);
    }
    Ok(Some(value))
}

const MAX_MESSAGE_BYTES: usize = 64 * 1024 * 1024;

enum ReadMessageError {
    Io(io::Error),
    Malformed,
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
    /// The workspace folders that may contain unopened references.
    workspace_roots: Vec<PathBuf>,
}

impl Server {
    /// The answer of a message, when there is one.
    fn handle(&mut self, message: &Value) -> Option<Value> {
        let id = message.get("id").cloned();
        let Some(method) = message.text("method") else {
            return Some(error_answer(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request",
            ));
        };
        if message.text("jsonrpc") != Some("2.0") {
            return Some(error_answer(
                id.unwrap_or(Value::Null),
                -32600,
                "Invalid Request",
            ));
        }

        match method {
            "initialize" => {
                self.workspace_roots.clear();
                if let Some(params) = message.get("params") {
                    if let Some(folders) = params.list("workspaceFolders") {
                        for folder in folders {
                            if let Some(root) = existing_workspace_folder_path(folder) {
                                if !self.workspace_roots.contains(&root) {
                                    self.workspace_roots.push(root);
                                }
                            }
                        }
                    }
                    if self.workspace_roots.is_empty() {
                        if let Some(root) = params
                            .text("rootUri")
                            .and_then(path_of)
                            .or_else(|| params.text("rootPath").map(PathBuf::from))
                            .filter(|path| path.is_dir())
                            .map(normalize_workspace_root)
                        {
                            self.workspace_roots.push(root);
                        }
                    }
                }
                id.map(|id| answer(id, initialize()))
            }
            "initialized" => None,
            "workspace/didChangeWorkspaceFolders" => {
                if let Some(event) = message.get("params").and_then(|params| params.get("event")) {
                    let removed: HashSet<PathBuf> = event
                        .list("removed")
                        .into_iter()
                        .flatten()
                        .filter_map(workspace_folder_path)
                        .collect();
                    self.workspace_roots.retain(|root| !removed.contains(root));
                    if let Some(added) = event.list("added") {
                        for folder in added {
                            if let Some(root) = existing_workspace_folder_path(folder) {
                                if !self.workspace_roots.contains(&root) {
                                    self.workspace_roots.push(root);
                                }
                            }
                        }
                    }
                }
                None
            }
            "shutdown" => id.map(|id| answer(id, Value::Null)),
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
                let id = id?;
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.hover(&uri, line, col)))
            }
            "textDocument/completion" => {
                let id = id?;
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.completion(&uri, line, col)))
            }
            "textDocument/documentSymbol" => {
                let id = id?;
                let uri = message
                    .get("params")
                    .and_then(|p| p.get("textDocument"))
                    .and_then(|d| d.text("uri"))?;
                Some(answer(id, self.document_symbols(&uri)))
            }
            "textDocument/definition" => {
                let id = id?;
                let (uri, line, col) = self.place(message)?;
                Some(answer(id, self.definition(&uri, line, col)))
            }
            "textDocument/references" => {
                let id = id?;
                let (uri, line, col) = self.place(message)?;
                let include_declaration = message
                    .get("params")
                    .and_then(|params| params.get("context"))
                    .and_then(|context| context.get("includeDeclaration"))
                    != Some(&Value::Bool(false));
                Some(answer(
                    id,
                    self.references(&uri, line, col, include_declaration),
                ))
            }
            "textDocument/rename" => {
                let id = id?;
                let (uri, line, col) = self.place(message)?;
                let new_name = match message.get("params").and_then(|p| p.get("newName")) {
                    Some(Value::Text(text)) => text.clone(),
                    _ => return None,
                };
                if !is_rename_identifier(&new_name) {
                    return Some(answer(id, Value::Null));
                }
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
            _ => id.map(|id| error_answer(id, -32601, &format!("Method not found: {method}"))),
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
                Some(file) if !file.is_empty() => file_uri(&file),
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
        let Some(path) = path_of(uri) else {
            return Vec::new();
        };
        let label = path.to_string_lossy().to_string();
        // A problem of the file that is opened belongs to the buffer of the
        // editor, which may not be written on the disk yet. The places of
        // the other files are the ones on the disk.
        let opened = path.canonicalize().ok();
        let opened_parent = path.parent();
        let canonical_parent = opened.as_deref().and_then(std::path::Path::parent);
        let file_uri_of = |file: &str| -> Option<String> {
            let canonical = std::path::Path::new(file);
            // A problem of the file that is opened belongs to the buffer of
            // the editor, which may not be written on the disk yet.
            if opened.as_deref() == Some(canonical) {
                return None;
            }
            // The editor may have opened a file through a path that is not
            // the canonical one, as a symbolic link makes possible. The URI
            // of the document that is open is the name the editor knows.
            for (open_uri, _) in &self.files {
                let Some(open_path) = path_of(open_uri) else {
                    continue;
                };
                if open_path.canonicalize().ok().as_deref() == Some(canonical) {
                    return Some(open_path.to_string_lossy().to_string());
                }
            }
            // A module next to the opened file keeps the spelling of the
            // folder that the editor used, even when the compiler resolved
            // the real path behind a symbolic link.
            if let (Some(parent), Some(real_parent)) = (opened_parent, canonical_parent) {
                if let Ok(relative) = canonical.strip_prefix(real_parent) {
                    return Some(parent.join(relative).to_string_lossy().to_string());
                }
            }
            Some(file.to_string())
        };
        let diagnostic_source = |file: Option<&str>| -> String {
            match file {
                Some(file) => match file_uri_of(file) {
                    Some(file) => self.source(&file_uri(&file)).unwrap_or_else(|| text.clone()),
                    None => text.clone(),
                },
                None => text.clone(),
            }
        };
        let mut parser = match Parser::new(text, &label) {
            Ok(parser) => parser,
            Err(err) => return vec![(None, problem(&err, 1, 1, text))],
        };
        if let Err(err) = parser.parse_program() {
            return vec![(None, problem(&err, err.line, err.col, text))];
        }

        // The file is checked with the modules it uses, so that the
        // mistakes of the types and the warnings are said as well.
        let mut out = Vec::new();
        match vey::compiler::check_file(&path, Some(text)) {
            Ok(problems) => {
                for warning in &problems.warnings {
                    let source = diagnostic_source(Some(&warning.file));
                    out.push((
                        file_uri_of(&warning.file),
                        warning_problem(warning, &source),
                    ));
                }
                for err in &problems.errors {
                    let source = diagnostic_source(Some(&err.file));
                    out.push((
                        file_uri_of(&err.file),
                        problem(err, err.line, err.col, &source),
                    ));
                }
            }
            Err(err) => {
                let source = diagnostic_source(Some(&err.file));
                out.push((
                    file_uri_of(&err.file),
                    problem(&err, err.line, err.col, &source),
                ));
            }
        }
        out
    }

    /// The names that are declared in a file.
    fn names(&self, uri: &str) -> Option<Names> {
        let path = path_of(uri)?;
        let text = self.source(uri)?;
        let label = path.to_string_lossy().to_string();
        let mut parser = Parser::new(&text, &label).ok()?;
        let program = parser.parse_program().ok()?;
        Some(Names::of(&program, &text))
    }

    fn source(&self, uri: &str) -> Option<String> {
        self.files
            .get(uri)
            .cloned()
            .or_else(|| path_of(uri).and_then(|path| std::fs::read_to_string(path).ok()))
    }

    fn workspace_files(&self) -> Vec<String> {
        let mut files: Vec<String> = self.files.keys().cloned().collect();
        let mut seen = HashSet::new();
        for uri in self.files.keys() {
            if let Some(path) = path_of(uri) {
                seen.insert(path.canonicalize().unwrap_or(path));
            }
        }
        let mut paths = Vec::new();
        for root in &self.workspace_roots {
            collect_workspace_sources(root, &mut paths);
        }
        for path in paths {
            let path = path.canonicalize().unwrap_or(path);
            if seen.insert(path.clone()) {
                files.push(file_uri(&path.to_string_lossy()));
            }
        }
        files
    }

    fn imported_module_uri(
        &self,
        uri: &str,
        names: &Names,
        alias: &str,
        receiver_symbol: Option<usize>,
    ) -> Option<String> {
        let (_, import_path, _) =
            names
                .module_imports
                .iter()
                .find(|(module_alias, _, symbol)| {
                    module_alias == alias && Some(*symbol) == receiver_symbol
                })?;
        if import_path.starts_with("npm:") {
            return None;
        }
        let source = path_of(uri)?;
        let parent = source.parent()?;
        let mut target = parent.join(import_path);
        if !target.exists() {
            let with_extension = parent.join(format!("{}.vey", import_path));
            if with_extension.exists() {
                target = with_extension;
            }
        }
        let target = target.canonicalize().ok()?;
        if let Some(open_uri) = self.files.keys().find_map(|candidate_uri| {
            path_of(candidate_uri)
                .and_then(|path| path.canonicalize().ok())
                .filter(|path| path == &target)
                .map(|_| candidate_uri.clone())
        }) {
            return Some(open_uri);
        }
        Some(file_uri(&target.to_string_lossy()))
    }

    fn symbol_target_at(
        &self,
        uri: &str,
        line: usize,
        col: usize,
    ) -> Option<(String, String, vey::lexer::Pos)> {
        let names = self.names(uri)?;
        if let Some(symbol) = names.symbol_at(line, col) {
            if let Some(definition) = names.symbols.get(symbol)?.declaration {
                let (name, pos, _) = names.defs.get(definition)?;
                return Some((uri.to_string(), name.clone(), *pos));
            }
        }
        if let Some(member) = names.module_member_uses.iter().find(|member| {
            member.pos.line == line
                && col >= member.pos.col
                && col < member.pos.col + member.range_len
        }) {
            if let Some(target_uri) =
                self.imported_module_uri(uri, &names, &member.alias, member.receiver_symbol)
            {
                if let Some(target_names) = self.names(&target_uri) {
                    if target_names.exports.contains(&member.name) {
                        let export_symbol = target_names
                            .export_symbols
                            .get(&(member.name.clone(), member.type_namespace))?;
                        if let Some((_, (name, pos, _))) = target_names
                            .defs
                            .iter()
                            .enumerate()
                            .find(|(definition, (name, _, _))| {
                                name == &member.name
                                    && target_names.def_symbols.get(*definition).copied().flatten()
                                        == Some(*export_symbol)
                            })
                        {
                            return Some((target_uri, name.clone(), *pos));
                        }
                    }
                }
            }
        }
        let variant = names.enum_variant_uses.iter().find(|variant| {
            variant.pos.line == line
                && col >= variant.pos.col
                && col < variant.pos.col + variant.name.chars().count()
        })?;
        if let Some(module) = &variant.module_alias {
            let target_uri =
                self.imported_module_uri(uri, &names, module, variant.receiver_symbol)?;
            let target_names = self.names(&target_uri)?;
            if !target_names.exports.contains(&variant.enum_name) {
                return None;
            }
            let enum_symbol = target_names
                .export_symbols
                .get(&(variant.enum_name.clone(), true))?;
            let target = target_names.enum_variant_defs.iter().find(|definition| {
                definition.enum_symbol == *enum_symbol
                    && definition.enum_name == variant.enum_name
                    && definition.name == variant.name
            })?;
            return Some((target_uri, target.name.clone(), target.pos));
        }
        let enum_symbol = variant.enum_symbol?;
        let target = names.enum_variant_defs.iter().find(|definition| {
            definition.enum_symbol == enum_symbol && definition.name == variant.name
        })?;
        Some((uri.to_string(), target.name.clone(), target.pos))
    }

    fn symbol_occurrences(
        &self,
        target_uri: &str,
        target_name: &str,
        target_pos: vey::lexer::Pos,
    ) -> Option<Vec<(String, String, vey::lexer::Pos, usize)>> {
        let target_names = self.names(target_uri)?;
        let definition = target_names
            .defs
            .iter()
            .position(|(name, pos, _)| name == target_name && *pos == target_pos)?;
        let symbol = target_names.def_symbols.get(definition).copied().flatten()?;
        let mut occurrences = vec![(
            target_uri.to_string(),
            target_name.to_string(),
            target_names.defs[definition].1,
            target_name.chars().count(),
        )];
        for (index, (name, pos)) in target_names.uses.iter().enumerate() {
            if target_names.use_symbols[index] == Some(symbol) {
                occurrences.push((
                    target_uri.to_string(),
                    name.clone(),
                    *pos,
                    name.chars().count(),
                ));
            }
        }
        let exported_symbol = target_names
            .export_symbols
            .values()
            .any(|exported| *exported == symbol);
        let exported_variant = target_names.enum_variant_defs.iter().any(|variant| {
            variant.symbol == symbol
                && target_names
                    .export_symbols
                    .values()
                    .any(|exported_enum| *exported_enum == variant.enum_symbol)
        });
        let workspace_files = if exported_symbol || exported_variant {
            self.workspace_files()
        } else {
            vec![target_uri.to_string()]
        };
        for candidate_uri in &workspace_files {
            let Some(candidate_names) = self.names(candidate_uri) else {
                continue;
            };
            for member in &candidate_names.module_member_uses {
                if target_names.exports.contains(target_name)
                    && target_names
                        .export_symbols
                        .get(&(target_name.to_string(), member.type_namespace))
                        .copied()
                        == Some(symbol)
                    && member.name == target_name
                    && self
                        .imported_module_uri(
                            candidate_uri,
                            &candidate_names,
                            &member.alias,
                            member.receiver_symbol,
                        )
                        .as_deref()
                        == Some(target_uri)
                {
                    occurrences.push((
                        candidate_uri.clone(),
                        member.name.clone(),
                        member.pos,
                        member.range_len,
                    ));
                }
            }
        }
        if let Some(target_variant) = target_names
            .enum_variant_defs
            .iter()
            .find(|variant| variant.name == target_name && variant.pos == target_pos)
        {
            if target_variant.symbol == symbol {
                for candidate_uri in &workspace_files {
                    let Some(candidate_names) = self.names(candidate_uri) else {
                        continue;
                    };
                    for variant in &candidate_names.enum_variant_uses {
                        if variant.enum_name != target_variant.enum_name
                            || variant.name != target_variant.name
                        {
                            continue;
                        }
                        let same_enum = if let Some(module) = &variant.module_alias {
                            self.imported_module_uri(
                                candidate_uri,
                                &candidate_names,
                                module,
                                variant.receiver_symbol,
                            )
                            .as_deref()
                                == Some(target_uri)
                        } else {
                            candidate_uri == target_uri
                                && variant.enum_symbol == Some(target_variant.enum_symbol)
                        };
                        if same_enum {
                            occurrences.push((
                                candidate_uri.clone(),
                                variant.name.clone(),
                                variant.pos,
                                variant.name.chars().count(),
                            ));
                        }
                    }
                }
            }
        }
        Some(occurrences)
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
        let Some((target_uri, target_name, target_pos)) = self.symbol_target_at(uri, line, col)
        else {
            return Value::Null;
        };
        let Some(names) = self.names(&target_uri) else {
            return Value::Null;
        };
        let Some((name, pos, _)) = names
            .defs
            .iter()
            .find(|(name, pos, _)| name == &target_name && *pos == target_pos)
        else {
            return Value::Null;
        };
        location(&target_uri, &names.source, pos, name)
    }

    /// Where a name is used.
    fn references(&self, uri: &str, line: usize, col: usize, include_declaration: bool) -> Value {
        let Some((target_uri, target_name, target_pos)) = self.symbol_target_at(uri, line, col)
        else {
            return Value::List(Vec::new());
        };
        let Some(occurrences) = self.symbol_occurrences(&target_uri, &target_name, target_pos) else {
            return Value::List(Vec::new());
        };
        Value::List(
            occurrences
                .iter()
                .filter_map(|(file_uri, _name, pos, range_len)| {
                    if !include_declaration && file_uri == &target_uri && *pos == target_pos {
                        return None;
                    }
                    let source = self.source(file_uri)?;
                    Some(location_with_width(file_uri, &source, pos, *range_len))
                })
                .collect(),
        )
    }

    /// The places where a name is written, so that it may be changed.
    fn rename(&self, uri: &str, line: usize, col: usize, new_name: &str) -> Value {
        let Some((target_uri, target_name, target_pos)) = self.symbol_target_at(uri, line, col)
        else {
            return Value::Null;
        };
        let Some(occurrences) = self.symbol_occurrences(&target_uri, &target_name, target_pos) else {
            return Value::Null;
        };
        let mut changes: Vec<(String, Vec<Value>)> = Vec::new();
        for (file_uri, _name, pos, range_len) in occurrences {
            let Some(source) = self.source(&file_uri) else {
                continue;
            };
            let edit = json::object(vec![
                ("range", name_range_with_width(&source, &pos, range_len)),
                ("newText", json::text(new_name)),
            ]);
            if let Some((_, edits)) = changes.iter_mut().find(|(uri, _)| uri == &file_uri) {
                edits.push(edit);
            } else {
                changes.push((file_uri, vec![edit]));
            }
        }
        let document_changes: Vec<(&str, Value)> = changes
            .iter()
            .map(|(uri, edits)| (uri.as_str(), Value::List(edits.clone())))
            .collect();
        json::object(vec![("changes", json::object(document_changes))])
    }

    /// The type of the expression under the cursor.
    fn hover(&self, uri: &str, line: usize, col: usize) -> Value {
        let Some(text) = self.files.get(uri) else {
            return Value::Null;
        };
        let Some(path) = path_of(uri) else {
            return Value::Null;
        };
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
    fn completion(&self, uri: &str, line: usize, col: usize) -> Value {
        let Some(text) = self.files.get(uri) else {
            return Value::List(Vec::new());
        };
        let Some(path) = path_of(uri) else {
            return Value::List(Vec::new());
        };
        let label = path.to_string_lossy().to_string();

        // The names of the scopes that hold the cursor, from the nearest
        // one out. A local name is only offered where it may be written.
        let mut names: Vec<Value> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Ok(mut parser) = Parser::new(text, &label) {
            if let Ok(program) = parser.parse_program() {
                for (name, kind) in Names::of(&program, text).visible_values(line, col) {
                    if seen.insert(name.clone()) {
                        names.push(json::object(vec![
                            ("label", json::text(&name)),
                            ("kind", json::number(completion_kind(kind))),
                        ]));
                    }
                }
            }
        }
        for name in vey::builtins::BUILTINS {
            if seen.insert((*name).to_string()) {
                names.push(json::object(vec![
                    ("label", json::text(name)),
                    ("kind", json::number(3.0)),
                ]));
            }
        }
        Value::List(names)
    }
}

/// The shape of a completion item, in the numbers of the protocol.
fn completion_kind(kind: &str) -> f64 {
    match kind {
        "function" | "component" => 3.0,
        "module" => 9.0,
        "enum" => 13.0,
        "interface" => 8.0,
        "type" => 25.0,
        "method" => 2.0,
        _ => 6.0,
    }
}

/// The answer of a message.
fn answer(id: Value, result: Value) -> Value {
    json::object(vec![
        ("jsonrpc", json::text("2.0")),
        ("id", id),
        ("result", result),
    ])
}

fn error_answer(id: Value, code: i64, message: &str) -> Value {
    json::object(vec![
        ("jsonrpc", json::text("2.0")),
        ("id", id),
        (
            "error",
            json::object(vec![
                ("code", json::number(code as f64)),
                ("message", json::text(message)),
            ]),
        ),
    ])
}

fn is_rename_identifier(name: &str) -> bool {
    let mut lexer = vey::lexer::Lexer::new(name, "<rename>");
    matches!(
        lexer.next_token(),
        Ok(token) if token.kind == vey::lexer::TokKind::Ident && token.value == name
    ) && matches!(lexer.next_token(), Ok(token) if token.kind == vey::lexer::TokKind::Eof)
}

/// What the server can do.
fn initialize() -> Value {
    json::object(vec![
        (
            "capabilities",
            json::object(vec![
                ("textDocumentSync", json::number(1.0)),
                ("hoverProvider", Value::Bool(true)),
                (
                    "completionProvider",
                    json::object(vec![(
                        "triggerCharacters",
                        Value::List(vec![json::text(".")]),
                    )]),
                ),
                ("documentSymbolProvider", Value::Bool(true)),
                ("definitionProvider", Value::Bool(true)),
                ("referencesProvider", Value::Bool(true)),
                ("renameProvider", Value::Bool(true)),
                (
                    "workspace",
                    json::object(vec![(
                        "workspaceFolders",
                        json::object(vec![
                            ("supported", Value::Bool(true)),
                            ("changeNotifications", Value::Bool(true)),
                        ]),
                    )]),
                ),
            ]),
        ),
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
    /// File imports, by their local module alias.
    module_imports: Vec<(String, String, usize)>,
    /// Direct static member accesses on imported module aliases.
    module_member_uses: Vec<ModuleMemberUse>,
    /// Variant declarations keyed by their containing enum's binding identity.
    enum_variant_defs: Vec<EnumVariantDef>,
    /// Local and imported enum-variant occurrences.
    enum_variant_uses: Vec<EnumVariantUse>,
    /// Top-level names visible to another module.
    exports: HashSet<String>,
    /// The binding identity behind each exported name.
    export_symbols: HashMap<(String, bool), usize>,
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
    /// Type names in each lexical scope, separate from value bindings.
    type_scopes: Vec<HashMap<String, usize>>,
    /// The stable identity of each scope on the stack.
    scope_ids: Vec<usize>,
    /// The source range of each lexical scope, by its stable identity. The
    /// top level has no range, because it is everywhere.
    scope_regions: Vec<Option<(vey::lexer::Pos, vey::lexer::Pos)>>,
    next_scope_id: usize,
    /// The names that a completion may offer, with the scope they live in.
    values: Vec<ScopedName>,
    /// A binding's declaration, indexed by its stable identity.
    symbols: Vec<NameSymbol>,
    /// Top-level bindings are visible to function bodies before their text.
    predeclared: Vec<(String, vey::lexer::Pos, usize)>,
    /// Top-level type bindings are visible before their declarations.
    type_predeclared: Vec<(String, vey::lexer::Pos, usize)>,
}

/// A name that may be written at a place of the program.
struct ScopedName {
    name: String,
    pos: vey::lexer::Pos,
    kind: &'static str,
    scope: usize,
}

struct NameSymbol {
    declaration: Option<usize>,
}

struct ModuleMemberUse {
    alias: String,
    receiver_symbol: Option<usize>,
    name: String,
    type_namespace: bool,
    pos: vey::lexer::Pos,
    range_len: usize,
}
struct EnumVariantDef {
    enum_name: String,
    enum_symbol: usize,
    name: String,
    symbol: usize,
    pos: vey::lexer::Pos,
}
struct EnumVariantUse {
    enum_name: String,
    enum_symbol: Option<usize>,
    module_alias: Option<String>,
    receiver_symbol: Option<usize>,
    name: String,
    pos: vey::lexer::Pos,
}

fn member_name_pos(source: &str, operator: vey::lexer::Pos, name: &str) -> vey::lexer::Pos {
    let Some(line) = source.lines().nth(operator.line.saturating_sub(1)) else {
        return operator;
    };
    let chars: Vec<char> = line.chars().collect();
    let mut index = operator.col.saturating_sub(1);
    while matches!(chars.get(index), Some('?') | Some('.')) {
        index += 1;
    }
    loop {
        while chars.get(index).is_some_and(|ch| ch.is_whitespace()) {
            index += 1;
        }
        if chars.get(index) == Some(&'/') && chars.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < chars.len() && !(chars[index] == '*' && chars[index + 1] == '/') {
                index += 1;
            }
            index = (index + 2).min(chars.len());
            continue;
        }
        break;
    }
    let expected: Vec<char> = name.chars().collect();
    if chars.get(index..index + expected.len()) == Some(expected.as_slice()) {
        vey::lexer::Pos::new(operator.line, index + 1)
    } else {
        operator
    }
}

fn literal_member_name(
    source: &str,
    index: &vey::ast::Expr,
) -> Option<(String, vey::lexer::Pos, usize)> {
    let vey::ast::Expr::String(parts, pos) = index else {
        return None;
    };
    if parts.len() != 1 || parts[0].expr.is_some() {
        return None;
    }
    let name = &parts[0].text;
    let line = source.lines().nth(pos.line.saturating_sub(1))?;
    let chars: Vec<char> = line.chars().collect();
    let quote_index = pos.col.saturating_sub(1);
    let quote = *chars.get(quote_index)?;
    if !matches!(quote, '\'' | '"' | '`') {
        return None;
    }
    let start = quote_index + 1;
    let mut end = start;
    while let Some(ch) = chars.get(end) {
        if *ch == '\\' {
            end = (end + 2).min(chars.len());
        } else if *ch == quote {
            break;
        } else {
            end += 1;
        }
    }
    if chars.get(end) != Some(&quote) {
        return None;
    }
    Some((
        name.clone(),
        vey::lexer::Pos::new(pos.line, start + 1),
        end - start,
    ))
}

impl Names {
    fn new(source: &str) -> Names {
        Names {
            source: source.to_string(),
            module_imports: Vec::new(),
            module_member_uses: Vec::new(),
            enum_variant_defs: Vec::new(),
            enum_variant_uses: Vec::new(),
            exports: HashSet::new(),
            export_symbols: HashMap::new(),
            defs: Vec::new(),
            def_symbols: Vec::new(),
            uses: Vec::new(),
            use_symbols: Vec::new(),
            scopes: vec![HashMap::new()],
            type_scopes: vec![HashMap::new()],
            scope_ids: vec![0],
            scope_regions: vec![None],
            next_scope_id: 1,
            values: Vec::new(),
            symbols: Vec::new(),
            predeclared: Vec::new(),
            type_predeclared: Vec::new(),
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
                    if let Some(symbol) = self.scopes.last().and_then(|scope| scope.get(&decl.name))
                    {
                        if let Some(scope) = self.type_scopes.last_mut() {
                            scope.insert(decl.name.clone(), *symbol);
                        }
                        self.type_predeclared
                            .push((decl.name.clone(), pos, *symbol));
                    }
                }
                Stmt::Type(decl) => {
                    let pos = self.identifier_pos(decl.pos, &decl.name);
                    self.reserve_type(&decl.name, pos);
                }
                Stmt::Interface(decl) => {
                    let pos = self.identifier_pos(decl.pos, &decl.name);
                    self.reserve_type(&decl.name, pos);
                }
                Stmt::Use(decl) if !decl.path.ends_with(".css") => {
                    if let Some(pos) = decl.alias_pos {
                        self.reserve(&decl.alias, pos);
                    } else {
                        self.define_hidden(&decl.alias, "module");
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

    fn reserve_type(&mut self, name: &str, pos: vey::lexer::Pos) {
        let Some(scope) = self.type_scopes.last_mut() else {
            return;
        };
        if scope.contains_key(name) {
            return;
        }
        let symbol = self.symbols.len();
        self.symbols.push(NameSymbol { declaration: None });
        scope.insert(name.to_string(), symbol);
        self.type_predeclared.push((name.to_string(), pos, symbol));
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
        let symbol = if matches!(kind, "type" | "interface") {
            if let Some((_, _, symbol)) = self
                .type_predeclared
                .iter()
                .find(|(reserved, reserved_pos, _)| reserved == name && *reserved_pos == pos)
            {
                Some(*symbol)
            } else {
                let symbol = self.symbols.len();
                self.symbols.push(NameSymbol {
                    declaration: Some(definition),
                });
                Some(symbol)
            }
        } else if matches!(
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
        if self.scope_ids.last() == Some(&0) && self.exports.contains(name) {
            if let Some(symbol) = symbol {
                match kind {
                    "type" | "interface" => {
                        self.export_symbols.insert((name.to_string(), true), symbol);
                    }
                    "enum" => {
                        self.export_symbols.insert((name.to_string(), true), symbol);
                        self.export_symbols
                            .insert((name.to_string(), false), symbol);
                    }
                    "variable" | "function" | "component" => {
                        self.export_symbols
                            .insert((name.to_string(), false), symbol);
                    }
                    _ => {}
                }
            }
        }
        if matches!(kind, "type" | "interface" | "enum") {
            if let (Some(symbol), Some(scope)) = (symbol, self.type_scopes.last_mut()) {
                scope.insert(name.to_string(), symbol);
            }
        }
        // A variant and a method are written after a dot, not on their own,
        // so they are not offered as bare names.
        if kind != "variant" && kind != "method" {
            self.values.push(ScopedName {
                name: name.to_string(),
                pos,
                kind,
                scope: *self.scope_ids.last().unwrap_or(&0),
            });
        }
    }

    fn use_name(&mut self, name: &str, pos: vey::lexer::Pos) {
        self.uses.push((name.to_string(), pos));
        self.use_symbols.push(self.resolve(name));
    }

    fn define_type_parameter(&mut self, name: &str, pos: vey::lexer::Pos) {
        let definition = self.defs.len();
        let symbol = self.symbols.len();
        self.symbols.push(NameSymbol {
            declaration: Some(definition),
        });
        self.defs.push((name.to_string(), pos, "type_parameter"));
        self.def_symbols.push(Some(symbol));
        if let Some(scope) = self.type_scopes.last_mut() {
            scope.insert(name.to_string(), symbol);
        }
    }

    fn use_type_name(&mut self, name: &str, pos: vey::lexer::Pos) {
        self.uses.push((name.to_string(), pos));
        let symbol = self
            .type_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied());
        let built_in = matches!(
            name,
            "any"
                | "unknown"
                | "number"
                | "string"
                | "bool"
                | "null"
                | "undefined"
                | "void"
                | "never"
                | "symbol"
                | "bigint"
                | "object"
                | "Promise"
                | "Option"
                | "Signal"
                | "Store"
                | "Result"
        );
        let symbol = symbol.filter(|symbol| {
            let is_type_parameter = self
                .symbols
                .get(*symbol)
                .and_then(|symbol| symbol.declaration)
                .and_then(|definition| self.defs.get(definition))
                .is_some_and(|(_, _, kind)| *kind == "type_parameter");
            is_type_parameter || !built_in
        });
        self.use_symbols.push(symbol);
    }

    fn resolve(&self, name: &str) -> Option<usize> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn define_hidden(&mut self, name: &str, kind: &'static str) {
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
        // A name that the compiler makes has no token of its own. It is
        // written before any cursor in its scope, so a completion may still
        // offer it.
        self.values.push(ScopedName {
            name: name.to_string(),
            pos: vey::lexer::Pos::new(0, 0),
            kind,
            scope: *self.scope_ids.last().unwrap_or(&0),
        });
    }

    fn push_scope(&mut self, region: Option<(vey::lexer::Pos, vey::lexer::Pos)>) {
        self.scopes.push(HashMap::new());
        self.type_scopes.push(HashMap::new());
        self.scope_ids.push(self.next_scope_id);
        self.scope_regions.push(region);
        self.next_scope_id += 1;
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
            self.type_scopes.pop();
            self.scope_ids.pop();
        }
    }

    fn block(&mut self, block: &vey::ast::Block) {
        self.push_scope(Some((block.start, block.end)));
        self.body(&block.body);
        self.pop_scope();
    }

    /// The names that may be written at a place of the program.
    ///
    /// The scopes are walked from the innermost one out, so that a name that
    /// hides another one is offered once, and with the nearest meaning.
    fn visible_values(&self, line: usize, col: usize) -> Vec<(String, &'static str)> {
        let cursor = vey::lexer::Pos::new(line, col);
        let mut found: Vec<(String, &'static str)> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for scope in (0..self.scope_regions.len()).rev() {
            if scope != 0 {
                let Some((start, end)) = self.scope_regions.get(scope).copied().flatten() else {
                    continue;
                };
                // The cursor may be at either end of the range, because a
                // position is the place before the character that it names.
                let before_start = cursor.line < start.line
                    || (cursor.line == start.line && cursor.col < start.col);
                let after_end =
                    cursor.line > end.line || (cursor.line == end.line && cursor.col > end.col);
                if before_start || after_end {
                    continue;
                }
            }
            for value in self.values.iter().filter(|value| value.scope == scope) {
                // A local name is not there before its declaration. The
                // top level is known before the program runs.
                if scope != 0 && value.pos.after(&cursor) {
                    continue;
                }
                if seen.insert(value.name.clone()) {
                    found.push((value.name.clone(), value.kind));
                }
            }
        }
        found
    }

    fn stmt(&mut self, stmt: &vey::ast::Stmt) {
        use vey::ast::Stmt;
        match stmt {
            Stmt::Var(decl) => {
                if let Some(ty) = &decl.ty {
                    self.type_expr(ty);
                }
                if let Some(init) = &decl.init {
                    self.expr(init);
                }
                if decl.exported {
                    for (name, _) in decl.pattern.bindings() {
                        self.exports.insert(name.to_string());
                    }
                }
                self.pattern(&decl.pattern, "variable");
            }
            Stmt::Fn(decl) => {
                if decl.exported {
                    self.exports.insert(decl.name.clone());
                }
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "function");
                self.push_scope(Some((decl.body.start, decl.body.end)));
                for param in &decl.type_params {
                    self.define_type_parameter(&param.name, param.pos);
                }
                for param in &decl.type_params {
                    if let Some(constraint) = &param.constraint {
                        self.type_expr(constraint);
                    }
                }
                if let Some(ret) = &decl.ret {
                    self.type_expr(ret);
                }
                for param in &decl.params {
                    if let Some(ty) = &param.ty {
                        self.type_expr(ty);
                    }
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::Component(decl) => {
                if decl.exported {
                    self.exports.insert(decl.name.clone());
                }
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "component");
                self.push_scope(Some((decl.body_start, decl.body_end)));
                for param in &decl.params {
                    if let Some(ty) = &param.ty {
                        self.type_expr(ty);
                    }
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
                if decl.exported {
                    self.exports.insert(decl.name.clone());
                }
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "enum");
                let enum_symbol = self.def_symbols.last().copied().flatten();
                for variant in &decl.variants {
                    self.define(&variant.name, variant.pos, "variant");
                    if let (Some(enum_symbol), Some(symbol)) =
                        (enum_symbol, self.def_symbols.last().copied().flatten())
                    {
                        self.enum_variant_defs.push(EnumVariantDef {
                            enum_name: decl.name.clone(),
                            enum_symbol,
                            name: variant.name.clone(),
                            symbol,
                            pos: variant.pos,
                        });
                    }
                    for field in &variant.fields {
                        if let Some(ty) = &field.ty {
                            self.type_expr(ty);
                        }
                    }
                }
            }
            Stmt::Type(decl) => {
                if decl.exported {
                    self.exports.insert(decl.name.clone());
                }
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "type");
                self.push_scope(None);
                for param in &decl.type_params {
                    self.define_type_parameter(&param.name, param.pos);
                }
                for param in &decl.type_params {
                    if let Some(constraint) = &param.constraint {
                        self.type_expr(constraint);
                    }
                }
                self.type_expr(&decl.ty);
                self.pop_scope();
            }
            Stmt::Interface(decl) => {
                if decl.exported {
                    self.exports.insert(decl.name.clone());
                }
                let pos = self.identifier_pos(decl.pos, &decl.name);
                self.define(&decl.name, pos, "interface");
                self.push_scope(None);
                for param in &decl.type_params {
                    self.define_type_parameter(&param.name, param.pos);
                }
                for param in &decl.type_params {
                    if let Some(constraint) = &param.constraint {
                        self.type_expr(constraint);
                    }
                }
                for member in &decl.members {
                    match member {
                        vey::ast::InterfaceMember::Field(field) => self.type_expr(&field.ty),
                        vey::ast::InterfaceMember::Method(method) => {
                            for param in &method.params {
                                if let Some(ty) = &param.ty {
                                    self.type_expr(ty);
                                }
                            }
                            self.type_expr(&method.ret);
                        }
                    }
                }
                self.pop_scope();
            }
            Stmt::Impl(decl) => {
                let pos = self.identifier_pos(decl.pos, &decl.type_name);
                self.use_type_name(&decl.type_name, pos);
                for method in &decl.methods {
                    self.define(&method.name, method.pos, "method");
                    if let Some(ret) = &method.ret {
                        self.type_expr(ret);
                    }
                    self.push_scope(Some((method.body.start, method.body.end)));
                    self.define_hidden("self", "variable");
                    for param in &method.params {
                        if let Some(ty) = &param.ty {
                            self.type_expr(ty);
                        }
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
                self.block(&decl.then_block);
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::IfLet(decl) => {
                self.expr(&decl.value);
                self.push_scope(Some((decl.then_block.start, decl.then_block.end)));
                self.pattern_match(&decl.pattern);
                self.block(&decl.then_block);
                self.pop_scope();
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::While(decl) => {
                self.expr(&decl.test);
                self.block(&decl.body);
            }
            Stmt::WhileLet(decl) => {
                self.expr(&decl.value);
                self.push_scope(Some((decl.body.start, decl.body.end)));
                self.pattern_match(&decl.pattern);
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::LetElse(decl) => {
                self.expr(&decl.value);
                self.pattern(&decl.pattern, "variable");
                self.block(&decl.otherwise);
            }
            Stmt::Defer(decl) => self.expr(&decl.value),
            Stmt::Throw(decl) => self.expr(&decl.value),
            Stmt::For(decl) => {
                if let Some(ty) = &decl.ty {
                    self.type_expr(ty);
                }
                if let Some(step) = &decl.step {
                    self.expr(step);
                }
                self.expr(&decl.iter);
                self.push_scope(Some((decl.body.start, decl.body.end)));
                self.pattern(&decl.pattern, "variable");
                self.body(&decl.body.body);
                self.pop_scope();
            }
            Stmt::Repeat(decl) => {
                self.expr(&decl.count);
                self.block(&decl.body);
            }
            Stmt::Return(decl) => {
                if let Some(value) = &decl.value {
                    self.expr(value);
                }
            }
            Stmt::Try(decl) => {
                self.block(&decl.body);
                if let Some(catch) = &decl.catch {
                    self.push_scope(Some((catch.start, catch.end)));
                    if let Some(name) = &decl.name {
                        self.define_hidden(name, "variable");
                    }
                    self.body(&catch.body);
                    self.pop_scope();
                }
                if let Some(finally) = &decl.finally {
                    self.block(finally);
                }
            }
            Stmt::Expr(decl) => self.expr(&decl.expr),
            Stmt::Assign(decl) => {
                self.expr(&decl.target);
                self.expr(&decl.value);
            }
            Stmt::Block(block, _) => self.block(block),
            Stmt::Use(decl) if !decl.path.ends_with(".css") => {
                if let Some(pos) = decl.alias_pos {
                    self.define(&decl.alias, pos, "module");
                } else {
                    self.define_hidden(&decl.alias, "module");
                }
                if let Some(symbol) = self.resolve(&decl.alias) {
                    self.module_imports
                        .push((decl.alias.clone(), decl.path.clone(), symbol));
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
                    self.define_hidden(rest, kind);
                }
            }
            BindingPattern::Object { fields, rest, .. } => {
                for field in fields {
                    self.pattern(&field.pattern, kind);
                }
                if let Some(rest) = rest {
                    self.define_hidden(rest, kind);
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
                        if let Some((module, member)) = element.tag.rsplit_once('.') {
                            self.module_member_uses.push(ModuleMemberUse {
                                alias: module.to_string(),
                                receiver_symbol: self.resolve(module),
                                name: member.to_string(),
                                type_namespace: false,
                                pos: vey::lexer::Pos::new(
                                    element.pos.line,
                                    element.pos.col + module.chars().count() + 1,
                                ),
                                range_len: member.chars().count(),
                            });
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
                    self.push_scope(Some((loop_node.body_start, loop_node.body_end)));
                    self.define_hidden(&loop_node.name, "variable");
                    if let Some(index) = &loop_node.index {
                        self.define_hidden(index, "variable");
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
                        self.push_scope(Some((arm.body_start, arm.body_end)));
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
            Pattern::Enum {
                enum_name,
                variant,
                bindings,
                pos,
            } => {
                let variant_pos = self.identifier_pos(*pos, variant);
                if let Some((module, member)) = enum_name.split_once('.') {
                    let receiver_symbol = self.resolve(module);
                    self.module_member_uses.push(ModuleMemberUse {
                        alias: module.to_string(),
                        receiver_symbol,
                        name: member.to_string(),
                        type_namespace: true,
                        pos: vey::lexer::Pos::new(pos.line, pos.col + module.chars().count() + 1),
                        range_len: member.chars().count(),
                    });
                    self.use_name(module, *pos);
                    self.enum_variant_uses.push(EnumVariantUse {
                        enum_name: member.to_string(),
                        enum_symbol: None,
                        module_alias: Some(module.to_string()),
                        receiver_symbol,
                        name: variant.clone(),
                        pos: variant_pos,
                    });
                } else {
                    let enum_symbol = self.resolve(enum_name);
                    self.use_name(enum_name, *pos);
                    self.enum_variant_uses.push(EnumVariantUse {
                        enum_name: enum_name.clone(),
                        enum_symbol,
                        module_alias: None,
                        receiver_symbol: None,
                        name: variant.clone(),
                        pos: variant_pos,
                    });
                }
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

    fn type_expr(&mut self, ty: &vey::ast::TypeExpr) {
        use vey::ast::TypeExpr;
        match ty {
            TypeExpr::Qualified(module, member, pos) => {
                self.module_member_uses.push(ModuleMemberUse {
                    alias: module.clone(),
                    receiver_symbol: self.resolve(module),
                    name: member.clone(),
                    type_namespace: true,
                    pos: vey::lexer::Pos::new(pos.line, pos.col + module.chars().count() + 1),
                    range_len: member.chars().count(),
                });
                self.use_name(module, *pos);
            }
            TypeExpr::Generic(name, args, pos) => {
                if let Some((module, member)) = name.rsplit_once('.') {
                    self.module_member_uses.push(ModuleMemberUse {
                        alias: module.to_string(),
                        receiver_symbol: self.resolve(module),
                        name: member.to_string(),
                        type_namespace: true,
                        pos: vey::lexer::Pos::new(pos.line, pos.col + module.chars().count() + 1),
                        range_len: member.chars().count(),
                    });
                    self.use_name(module, *pos);
                } else {
                    self.use_type_name(name, *pos);
                }
                for arg in args {
                    self.type_expr(arg);
                }
            }
            TypeExpr::Array(inner, _) | TypeExpr::Optional(inner, _) => self.type_expr(inner),
            TypeExpr::Tuple(items, _) | TypeExpr::Union(items, _) => {
                for item in items {
                    self.type_expr(item);
                }
            }
            TypeExpr::Dict(key, value, _) => {
                self.type_expr(key);
                self.type_expr(value);
            }
            TypeExpr::Object(fields, _) => {
                for field in fields {
                    self.type_expr(&field.ty);
                }
            }
            TypeExpr::Function(params, ret, _, _) => {
                for param in params {
                    self.type_expr(param);
                }
                self.type_expr(ret);
            }
            TypeExpr::Name(name, pos) => self.use_type_name(name, *pos),
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
            Expr::Call(callee, args, _, _) => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::CallGeneric(callee, types, args, _, _) => {
                self.expr(callee);
                for ty in types {
                    self.type_expr(ty);
                }
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Member(object, prop, _, pos) => {
                if let (Expr::Ident(alias, _), vey::ast::MemberProp::Computed(index)) =
                    (object.as_ref(), prop)
                {
                    if let Some((name, pos, range_len)) = literal_member_name(&self.source, index) {
                        self.module_member_uses.push(ModuleMemberUse {
                            alias: alias.clone(),
                            receiver_symbol: self.resolve(alias),
                            name,
                            type_namespace: false,
                            pos,
                            range_len,
                        });
                    }
                }
                if let vey::ast::MemberProp::Name(name) = prop {
                    let variant_pos = member_name_pos(&self.source, *pos, name);
                    match object.as_ref() {
                        Expr::Ident(enum_name, _) => {
                            self.enum_variant_uses.push(EnumVariantUse {
                                enum_name: enum_name.clone(),
                                enum_symbol: self.resolve(enum_name),
                                module_alias: None,
                                receiver_symbol: None,
                                name: name.clone(),
                                pos: variant_pos,
                            });
                        }
                        Expr::Member(base, vey::ast::MemberProp::Name(enum_name), _, _)
                            if let Expr::Ident(module, _) = base.as_ref() =>
                        {
                            self.enum_variant_uses.push(EnumVariantUse {
                                enum_name: enum_name.clone(),
                                enum_symbol: None,
                                module_alias: Some(module.clone()),
                                receiver_symbol: self.resolve(module),
                                name: name.clone(),
                                pos: variant_pos,
                            });
                        }
                        _ => {}
                    }
                }
                if let (Expr::Ident(alias, _), vey::ast::MemberProp::Name(name)) =
                    (object.as_ref(), prop)
                {
                    self.module_member_uses.push(ModuleMemberUse {
                        alias: alias.clone(),
                        receiver_symbol: self.resolve(alias),
                        name: name.clone(),
                        type_namespace: false,
                        pos: member_name_pos(&self.source, *pos, name),
                        range_len: name.chars().count(),
                    });
                }
                self.expr(object);
                if let vey::ast::MemberProp::Computed(index) = prop {
                    self.expr(index);
                }
            }
            Expr::Fn(params, body, ret, _) => {
                if let Some(ret) = ret {
                    self.type_expr(ret);
                }
                self.push_scope(Some((body.start, body.end)));
                for param in params {
                    if let Some(ty) = &param.ty {
                        self.type_expr(ty);
                    }
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                self.body(&body.body);
                self.pop_scope();
            }
            Expr::Arrow(type_params, params, body, ret, _) => {
                let region = match body {
                    vey::ast::ArrowBody::Block(block) => Some((block.start, block.end)),
                    vey::ast::ArrowBody::Expr(_, start, end) => Some((*start, *end)),
                };
                self.push_scope(region);
                for param in type_params {
                    self.define_type_parameter(&param.name, param.pos);
                }
                for param in type_params {
                    if let Some(constraint) = &param.constraint {
                        self.type_expr(constraint);
                    }
                }
                if let Some(ret) = ret {
                    self.type_expr(ret);
                }
                for param in params {
                    if let Some(ty) = &param.ty {
                        self.type_expr(ty);
                    }
                    if let Some(default) = &param.default {
                        self.expr(default);
                    }
                    self.pattern(&param.pattern, "parameter");
                }
                match body {
                    vey::ast::ArrowBody::Expr(value, _, _) => self.expr(value),
                    vey::ast::ArrowBody::Block(block) => self.body(&block.body),
                }
                self.pop_scope();
            }
            Expr::Match(subject, arms, _) => {
                self.expr(subject);
                for arm in arms {
                    self.push_scope(Some((arm.body_start, arm.body_end)));
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
            Expr::As(value, ty, _) | Expr::Is(value, ty, _) => {
                self.expr(value);
                self.type_expr(ty);
            }
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
        "type_parameter" => 26.0,
        "module" => 2.0,
        _ => 13.0,
    }
}

/// The range of a name in the UTF-16 positions required by LSP.
fn name_range(source: &str, pos: &vey::lexer::Pos, name: &str) -> Value {
    name_range_with_width(source, pos, name.chars().count())
}

fn name_range_with_width(source: &str, pos: &vey::lexer::Pos, width: usize) -> Value {
    let line = source.lines().nth(pos.line.saturating_sub(1)).unwrap_or("");
    let chars: Vec<char> = line.chars().collect();
    let start_index = pos.col.saturating_sub(1);
    let start = chars
        .iter()
        .take(start_index)
        .map(|ch| ch.len_utf16())
        .sum::<usize>();
    let end = start
        + chars
            .iter()
            .skip(start_index)
            .take(width)
            .map(|ch| ch.len_utf16())
            .sum::<usize>();
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

fn location_with_width(uri: &str, source: &str, pos: &vey::lexer::Pos, width: usize) -> Value {
    json::object(vec![
        ("uri", json::text(uri)),
        ("range", name_range_with_width(source, pos, width)),
    ])
}

/// A character column of the compiler in the UTF-16 units of the protocol.
fn utf16_column(source: &str, line: usize, character_column: usize) -> usize {
    source
        .lines()
        .nth(line.saturating_sub(1))
        .unwrap_or("")
        .chars()
        .take(character_column)
        .map(char::len_utf16)
        .sum()
}

/// A mistake, in the shape the protocol wants.
fn problem(err: &vey::VeyError, line: usize, col: usize, source: &str) -> Value {
    let start_col = utf16_column(source, line, col.max(1) - 1);
    let end_col = utf16_column(source, line, col.max(1));
    let mut items = vec![
        (
            "range",
            json::object(vec![
                (
                    "start",
                    json::object(vec![
                        ("line", json::number((line.max(1) - 1) as f64)),
                        ("character", json::number(start_col as f64)),
                    ]),
                ),
                (
                    "end",
                    json::object(vec![
                        ("line", json::number((line.max(1) - 1) as f64)),
                        ("character", json::number(end_col as f64)),
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
fn warning_problem(warning: &vey::warn::Warning, source: &str) -> Value {
    let start_col = utf16_column(source, warning.line, warning.col.max(1) - 1);
    let end_col = utf16_column(source, warning.line, warning.col.max(1));
    let items = vec![
        (
            "range",
            json::object(vec![
                (
                    "start",
                    json::object(vec![
                        ("line", json::number((warning.line.max(1) - 1) as f64)),
                        ("character", json::number(start_col as f64)),
                    ]),
                ),
                (
                    "end",
                    json::object(vec![
                        ("line", json::number((warning.line.max(1) - 1) as f64)),
                        ("character", json::number(end_col as f64)),
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
fn path_of(uri: &str) -> Option<PathBuf> {
    let (scheme, rest) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case("file") {
        return None;
    }
    let uri_path = rest.strip_prefix("//").unwrap_or(rest);
    if !uri_path.starts_with('/') && !uri_path.contains('/') {
        return None;
    }
    if uri_path.contains('?') || uri_path.contains('#') {
        return None;
    }
    let path = if uri_path.starts_with('/') {
        uri_path.to_string()
    } else {
        let (authority, path) = uri_path.split_once('/')?;
        if !authority.eq_ignore_ascii_case("localhost") {
            return None;
        }
        format!("/{path}")
    };
    let path = percent_decode(&path)?;
    if path.contains('\0') {
        return None;
    }
    #[cfg(windows)]
    let path = {
        let drive_path = path
            .strip_prefix('/')
            .filter(|candidate| {
                candidate.as_bytes().get(1) == Some(&b':')
                    && candidate.as_bytes()[0].is_ascii_alphabetic()
            })
            .unwrap_or(&path);
        drive_path.replace('/', "\\")
    };
    #[cfg(not(windows))]
    let path = path;
    Some(PathBuf::from(path))
}

fn normalize_workspace_root(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

fn workspace_folder_path(folder: &Value) -> Option<PathBuf> {
    Some(normalize_workspace_root(path_of(folder.text("uri")?)?))
}

fn existing_workspace_folder_path(folder: &Value) -> Option<PathBuf> {
    let path = workspace_folder_path(folder)?;
    path.is_dir().then_some(path)
}

fn collect_workspace_sources(root: &Path, out: &mut Vec<PathBuf>) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                let ignored = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| {
                        matches!(
                            name,
                            ".git" | ".vey" | "build" | "dist" | "node_modules" | "target"
                        )
                    });
                if !ignored {
                    pending.push(path);
                }
            } else if kind.is_file() && path.extension().is_some_and(|extension| extension == "vey")
            {
                out.push(path);
            }
        }
    }
}

fn file_uri(path: &str) -> String {
    #[cfg(windows)]
    let path = path.replace('\\', "/");
    #[cfg(not(windows))]
    let path = path.to_string();
    let path = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    let mut uri = String::from("file://");
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[(byte >> 4) as usize]));
            uri.push(char::from(HEX[(byte & 0x0f) as usize]));
        }
    }
    uri
}

/// Decode percent escapes as URI octets, then validate the UTF-8 path.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            if index + 2 >= bytes.len() {
                return None;
            }
            let high = hex_value(bytes[index + 1])?;
            let low = hex_value(bytes[index + 2])?;
            decoded.push((high << 4) | low);
            index += 3;
        } else {
            let character = text[index..].chars().next()?;
            let mut encoded = [0; 4];
            decoded.extend_from_slice(character.encode_utf8(&mut encoded).as_bytes());
            index += character.len_utf8();
        }
    }
    String::from_utf8(decoded).ok()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod uri_tests {
    use super::path_of;
    use std::path::PathBuf;

    #[test]
    fn file_uri_decodes_utf8_octets_without_changing_plus() {
        assert_eq!(
            path_of("file:///tmp/caf%C3%A9+a%20b.vey"),
            Some(PathBuf::from("/tmp/café+a b.vey"))
        );
        assert_eq!(
            path_of("file://localhost/tmp/caf%C3%A9.vey"),
            Some(PathBuf::from("/tmp/café.vey"))
        );
        assert_eq!(
            path_of("FILE:/tmp/plain.vey"),
            Some(PathBuf::from("/tmp/plain.vey"))
        );
    }

    #[test]
    fn malformed_and_non_local_file_uris_are_rejected() {
        for uri in [
            "file:///tmp/%",
            "file:///tmp/%2",
            "file:///tmp/%GG",
            "file:///tmp/%FF",
            "file:///tmp/%00",
            "file:///tmp/a?query",
            "file:///tmp/a#fragment",
            "file://example.test/tmp/a",
        ] {
            assert_eq!(path_of(uri), None, "{uri}");
        }
    }
}

#[cfg(test)]
mod protocol_tests {
    use super::{json, read_message, ReadMessageError, Server, Value, MAX_MESSAGE_BYTES};
    use std::collections::HashMap;
    use std::io::Cursor;

    #[test]
    fn clean_eof_is_distinct_from_a_valid_message() {
        let mut eof = Cursor::new(Vec::<u8>::new());
        assert!(matches!(read_message(&mut eof), Ok(None)));

        let mut message = Cursor::new(&b"content-length: 2\r\n\r\n{}"[..]);
        assert!(matches!(
            read_message(&mut message),
            Ok(Some(Value::Object(_)))
        ));
    }

    #[test]
    fn invalid_duplicate_and_oversized_frame_headers_are_rejected() {
        for frame in [
            "Content-Length: nope\r\n\r\n",
            "Content-Length: 2\r\ncontent-length: 2\r\n\r\n{}",
            "X-Test: one\r\n\r\n",
        ] {
            let mut input = Cursor::new(frame.as_bytes());
            assert!(matches!(
                read_message(&mut input),
                Err(ReadMessageError::Malformed)
            ));
        }

        let frame = format!("Content-Length: {}\r\n\r\n", MAX_MESSAGE_BYTES + 1);
        let mut input = Cursor::new(frame.as_bytes());
        assert!(matches!(
            read_message(&mut input),
            Err(ReadMessageError::Malformed)
        ));
    }

    #[test]
    fn requests_without_a_method_get_invalid_request() {
        let (message, rest) = json::read(r#"{"jsonrpc":"2.0","id":7}"#).unwrap();
        assert!(rest.is_empty());
        let mut server = Server {
            files: HashMap::new(),
            workspace_roots: Vec::new(),
        };
        let response = server.handle(&message).unwrap();
        assert_eq!(response.get("id"), Some(&Value::Number(7.0)));
        assert_eq!(
            response.get("error").and_then(|error| error.number("code")),
            Some(-32600.0)
        );
    }
}

#[cfg(test)]
mod diagnostic_position_tests {
    use super::{problem, utf16_column, warning_problem};
    use vey::{warn::Warning, VeyError};

    #[test]
    fn diagnostic_ranges_use_utf16_code_units() {
        let source = "a😀b";
        assert_eq!(utf16_column(source, 1, 1), 1);
        assert_eq!(utf16_column(source, 1, 2), 3);

        let error = VeyError::new("bad value", "main.vey", 1, 2);
        let diagnostic = problem(&error, 1, 2, source);
        let range = diagnostic.get("range").unwrap();
        assert_eq!(range.get("start").unwrap().number("character"), Some(1.0));
        assert_eq!(range.get("end").unwrap().number("character"), Some(3.0));

        let warning = Warning::new("unused", "main.vey", 1, 2);
        let diagnostic = warning_problem(&warning, source);
        let range = diagnostic.get("range").unwrap();
        assert_eq!(range.get("start").unwrap().number("character"), Some(1.0));
        assert_eq!(range.get("end").unwrap().number("character"), Some(3.0));
    }
}
