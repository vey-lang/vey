//! The loading of the modules of a program.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::ast::Stmt;
use crate::compiler::{Module, StyleFile};
use crate::parser::Parser;
use crate::suggest::edit_distance;
use crate::VeyError;

pub(crate) fn load_module(
    path: &Path,
    modules: &mut Vec<Module>,
    ids: &mut HashMap<PathBuf, usize>,
    stack: &mut Vec<PathBuf>,
    styles: &mut Vec<StyleFile>,
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

    let source = fs::read_to_string(path).map_err(|err| {
        VeyError::new(
            format!("cannot read file: {}", err),
            path.display().to_string(),
            1,
            1,
        )
    })?;

    let file_label = path.display().to_string();
    let mut parser = Parser::new(&source, &file_label)?;
    let ast = parser.parse_program()?;

    let mut deps = Vec::new();
    if let Some(parent) = path.parent() {
        for stmt in &ast.body {
            match stmt {
                Stmt::Style(style) => styles.push(StyleFile {
                    path: path.to_path_buf(),
                    content: style.css.clone(),
                }),
                Stmt::Use(use_stmt) => {
                    if deps.iter().any(|(existing, _)| existing == &use_stmt.alias) {
                        return Err(VeyError::new(
                            format!("duplicate module alias '{}'", use_stmt.alias),
                            file_label.clone(),
                            use_stmt.pos.line,
                            use_stmt.pos.col,
                        ));
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
                    let id = load_module(&target, modules, ids, stack, styles)?;
                    deps.push((use_stmt.alias.clone(), id));
                }
                _ => {}
            }
        }
    }

    let exports = ast
        .body
        .iter()
        .filter_map(|stmt| stmt.exported_name())
        .filter(|(_, exported)| *exported)
        .map(|(name, _)| name.to_string())
        .collect();

    let id = modules.len();
    ids.insert(path.to_path_buf(), id);
    stack.pop();
    modules.push(Module {
        id,
        path: path.to_path_buf(),
        source,
        ast,
        deps,
        exports,
    });

    Ok(id)
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
