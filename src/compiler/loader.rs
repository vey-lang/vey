//! The loading of the modules of a program.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::Stmt;
use crate::compiler::{Module, StyleFile};
use crate::lexer::{Lexer, TokKind};
use crate::parser::Parser;
use crate::suggest::edit_distance;
use crate::VeyError;

pub(crate) fn load_module(
    path: &Path,
    modules: &mut Vec<Module>,
    ids: &mut HashMap<PathBuf, usize>,
    stack: &mut Vec<PathBuf>,
    styles: &mut Vec<StyleFile>,
    text: Option<&str>,
) -> Result<usize, VeyError> {
    if let Some(id) = ids.get(path) {
        return Ok(*id);
    }

    if stack.iter().any(|item| item == path) {
        let mut chain: Vec<String> = stack.iter().map(|item| file_name(item)).collect();
        chain.push(file_name(path));
        return Err(VeyError::new(
            format!("circular import: {}", chain.join(" -> ")),
            path.display().to_string(),
            1,
            1,
        ));
    }

    stack.push(path.to_path_buf());

    // The text of the file may be another one than the one on the disk,
    // which is what a language server needs.
    let source = match text {
        Some(text) => text.to_string(),
        None => fs::read_to_string(path).map_err(|err| {
            VeyError::new(
                format!("cannot read file: {}", err),
                path.display().to_string(),
                1,
                1,
            )
        })?,
    };

    let file_label = path.display().to_string();
    let mut parser = Parser::new(&source, &file_label)?;
    let ast = parser.parse_program()?;

    // A file may say which version of the language it was written for.
    if let Some(version) = &ast.version {
        check_version(version, &file_label)?;
    }

    let mut deps = Vec::new();
    let mut packages: Vec<(String, String)> = Vec::new();
    if let Some(parent) = path.parent() {
        for stmt in &ast.body {
            match stmt {
                Stmt::Style(style) => styles.push(StyleFile {
                    path: path.to_path_buf(),
                    content: style.css.clone(),
                }),
                Stmt::Use(use_stmt) => {
                    if deps.iter().any(|(existing, _)| existing == &use_stmt.alias)
                        || packages
                            .iter()
                            .any(|(existing, _)| existing == &use_stmt.alias)
                    {
                        return Err(VeyError::new(
                            format!("duplicate module alias '{}'", use_stmt.alias),
                            file_label.clone(),
                            use_stmt.pos.line,
                            use_stmt.pos.col,
                        ));
                    }
                    // A package of npm is not a file, it is loaded when the
                    // program runs.
                    if let Some(package) = use_stmt.path.strip_prefix("npm:") {
                        if package.is_empty() {
                            return Err(VeyError::new(
                                "the name of the package is missing",
                                file_label.clone(),
                                use_stmt.pos.line,
                                use_stmt.pos.col,
                            ));
                        }
                        packages.push((use_stmt.alias.clone(), package.to_string()));
                        continue;
                    }
                    let mut target = parent.join(&use_stmt.path);
                    if !target.exists() {
                        let with_extension = parent.join(format!("{}.vey", use_stmt.path));
                        if with_extension.exists() {
                            target = with_extension;
                        }
                    }
                    if !target.exists() {
                        let mut error = VeyError::new(
                            format!("module not found: {}", use_stmt.path),
                            file_label.clone(),
                            use_stmt.pos.line,
                            use_stmt.pos.col,
                        );
                        if let Some(hint) = module_hint(parent, &use_stmt.path) {
                            error = error.with_hint(hint);
                        }
                        return Err(error);
                    }
                    let target = target.canonicalize().map_err(|err| {
                        VeyError::new(
                            format!("cannot open module {}: {}", use_stmt.path, err),
                            file_label.clone(),
                            use_stmt.pos.line,
                            use_stmt.pos.col,
                        )
                    })?;
                    if target
                        .extension()
                        .is_some_and(|extension| extension == "css")
                    {
                        let content = fs::read_to_string(&target).map_err(|err| {
                            VeyError::new(
                                format!("cannot read stylesheet {}: {}", use_stmt.path, err),
                                file_label.clone(),
                                use_stmt.pos.line,
                                use_stmt.pos.col,
                            )
                        })?;
                        if !styles.iter().any(|style| style.path == target) {
                            styles.push(StyleFile {
                                path: target,
                                content,
                            });
                        }
                        continue;
                    }
                    let id = load_module(&target, modules, ids, stack, styles, None)?;
                    deps.push((use_stmt.alias.clone(), id));
                }
                _ => {}
            }
        }
    }

    let mut exports: Vec<String> = ast
        .body
        .iter()
        .filter_map(|stmt| stmt.exported_name())
        .filter(|(_, exported)| *exported)
        .map(|(name, _)| name.to_string())
        .collect();

    // The constructor of a type is exported when the type is, so that the
    // users of the module may make a value of the type.
    let exported_types: Vec<&str> = ast
        .body
        .iter()
        .filter_map(|stmt| match stmt {
            Stmt::Type(decl) if decl.exported => Some(decl.name.as_str()),
            Stmt::Interface(decl) if decl.exported => Some(decl.name.as_str()),
            _ => None,
        })
        .collect();
    for stmt in &ast.body {
        let Stmt::Impl(decl) = stmt else {
            continue;
        };
        if exported_types.contains(&decl.type_name.as_str()) {
            exports.push(decl.type_name.clone());
        }
    }

    let id = modules.len();
    ids.insert(path.to_path_buf(), id);
    stack.pop();
    modules.push(Module {
        id,
        path: path.to_path_buf(),
        source,
        ast,
        deps,
        packages,
        exports,
        unused: HashSet::new(),
    });

    Ok(id)
}

/// Mark the exported functions that nothing in the program reads.
///
/// A module writes every function it exports, but a program that reads one
/// of them has no use for the others. A function is in use when another
/// module reads it by the alias of the module, or when its own module names
/// it again. A module that is handed around as a value keeps all of its
/// names, because anything of it may be read.
pub(crate) fn mark_unused(modules: &mut [Module], entry: usize) {
    let words: Vec<Vec<String>> = modules
        .iter()
        .map(|module| words(&module.source))
        .collect();

    // The names that another module reads, and the modules that are given
    // away as a value.
    let mut read: Vec<HashSet<String>> = vec![HashSet::new(); modules.len()];
    let mut given_away = vec![false; modules.len()];
    for module in modules.iter() {
        let source = &words[module.id];
        for (alias, dep) in &module.deps {
            for (at, word) in source.iter().enumerate() {
                if word != alias {
                    continue;
                }
                // The alias itself is declared with `as`, and a property
                // named like the alias is not the module.
                if source.get(at.wrapping_sub(1)).map(String::as_str) == Some("as") {
                    continue;
                }
                if source.get(at + 1).map(String::as_str) == Some(".") {
                    if let Some(name) = source.get(at + 2) {
                        read[*dep].insert(name.clone());
                    }
                } else {
                    given_away[*dep] = true;
                }
            }
        }
    }

    for module in modules.iter_mut() {
        let mut used = read[module.id].clone();
        if module.id == entry || given_away[module.id] {
            used.extend(module.exports.iter().cloned());
        }
        // A name that the module writes more than once, its declaration and
        // a use of it, is in use inside the module.
        for name in &module.exports {
            if !used.contains(name)
                && words[module.id].iter().filter(|word| *word == name).count() > 1
            {
                used.insert(name.clone());
            }
        }
        for stmt in &module.ast.body {
            let Stmt::Fn(decl) = stmt else {
                continue;
            };
            if decl.exported && !used.contains(&decl.name) {
                module.unused.insert(decl.name.clone());
            }
        }
    }
}

/// The values of the tokens of a source, without the texts and the numbers.
///
/// A name that only appears in a text is not a use of it.
fn words(source: &str) -> Vec<String> {
    let mut lexer = Lexer::new(source, "");
    let mut out = Vec::new();
    loop {
        let Ok(token) = lexer.next_token() else {
            return Vec::new();
        };
        if token.kind == TokKind::Eof {
            break;
        }
        if token.kind == TokKind::Str {
            // A text may hold an expression, and that is a use of a name
            // like any other.
            for part in &token.parts {
                if let Some((source, _)) = &part.expr {
                    out.extend(words(source));
                }
            }
            continue;
        }
        if matches!(
            token.kind,
            TokKind::Ident | TokKind::Keyword | TokKind::Op | TokKind::Punct
        ) {
            out.push(token.value);
        }
    }
    out
}

/// Suggest a correct relative import when the requested module exists in an
/// ancestor directory. A common case is copying a page into `tests/` and
/// keeping `use "./style.css"` instead of `use "../style.css"`.
fn module_hint(parent: &Path, request: &str) -> Option<String> {
    let request_path = Path::new(request);
    if request.contains("..") || request_path.is_absolute() {
        return None;
    }

    let parent = parent.canonicalize().ok()?;
    let mut ancestor = Some(parent.as_path());
    for _ in 0..5 {
        let base = ancestor?;
        let mut candidates = vec![base.join(request)];
        if request_path.extension().is_none() {
            candidates.push(base.join(format!("{}.vey", request)));
        }
        for candidate in candidates {
            if !candidate.is_file() {
                continue;
            }
            let candidate = candidate.canonicalize().ok()?;
            let relative = relative_path(&parent, &candidate)?;
            return Some(format!("did you mean `use \"{}\"`?", relative.display()));
        }
        ancestor = base.parent();
    }

    sibling_hint(&parent, request)
}

/// Suggest a nearby file with a similar name, which catches the common
/// `use "./stylish.css"` typo when no ancestor directory has the file.
fn sibling_hint(parent: &Path, request: &str) -> Option<String> {
    let request_path = Path::new(request);
    if request_path
        .parent()
        .is_some_and(|parent| !parent.as_os_str().is_empty() && parent != Path::new("."))
    {
        return None;
    }
    let wanted = request_path.file_name()?.to_string_lossy().to_lowercase();
    let limit = (wanted.chars().count() / 3).max(2);

    let mut best: Option<(usize, String)> = None;
    for entry in fs::read_dir(parent).ok()?.flatten() {
        if !entry.path().is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.to_lowercase() == wanted {
            continue;
        }
        let distance = edit_distance(&wanted, &name.to_lowercase());
        if distance > limit {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(best_distance, _)| distance < *best_distance)
        {
            best = Some((distance, name));
        }
    }

    let (_, name) = best?;
    Some(format!("did you mean `use \"./{}\"`?", name))
}

fn relative_path(from: &Path, to: &Path) -> Option<PathBuf> {
    let from_parts: Vec<_> = from.components().collect();
    let to_parts: Vec<_> = to.components().collect();
    let mut common = 0;
    while common < from_parts.len()
        && common < to_parts.len()
        && from_parts[common] == to_parts[common]
    {
        common += 1;
    }
    if common == 0 {
        return None;
    }

    let mut relative = PathBuf::new();
    for _ in common..from_parts.len() {
        relative.push("..");
    }
    for part in &to_parts[common..] {
        relative.push(part.as_os_str());
    }
    Some(relative)
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string())
}

/// The version of the language a file was written for.
///
/// A file that is written for a newer version of the language cannot be
/// read by this compiler.
fn check_version(version: &str, file: &str) -> Result<(), VeyError> {
    let wanted = parse_version(version);
    let ours = parse_version(env!("CARGO_PKG_VERSION"));
    if wanted > ours {
        return Err(VeyError::new(
            format!(
                "this file is written for Vey {}, and this compiler is {}",
                version,
                env!("CARGO_PKG_VERSION")
            ),
            file,
            1,
            1,
        ));
    }
    Ok(())
}

/// The major and the minor part of a version.
fn parse_version(version: &str) -> (u32, u32) {
    let mut parts = version.split('.');
    let major = parts
        .next()
        .and_then(|part| part.trim().parse().ok())
        .unwrap_or(0);
    let minor = parts
        .next()
        .and_then(|part| part.trim().parse().ok())
        .unwrap_or(0);
    (major, minor)
}
