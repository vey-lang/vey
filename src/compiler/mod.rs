//! The module graph of a program and the bundle it becomes.

mod loader;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::check;
use crate::codegen::{generate, Options};
use crate::types;
use crate::VeyError;

use loader::load_module;
pub struct Module {
    pub id: usize,
    pub path: PathBuf,
    pub source: String,
    pub ast: Program,
    pub deps: Vec<(String, usize)>,
    pub exports: Vec<String>,
}

pub struct SourceFile {
    pub path: String,
    pub content: String,
}

pub struct StyleFile {
    pub path: PathBuf,
    pub content: String,
}

pub struct Bundle {
    pub code: String,
    pub modules: Vec<Module>,
    pub entry: usize,
    pub entry_path: PathBuf,
    pub sources: Vec<SourceFile>,
    pub mappings: Vec<Option<(usize, usize)>>,
    /// CSS collected from `style { ... }` blocks and `.css` imports, in
    /// dependency order.
    pub styles: Vec<StyleFile>,
}

pub fn compile_project(entry: &Path) -> Result<Bundle, VeyError> {
    let entry = entry.canonicalize().map_err(|err| {
        VeyError::new(
            format!("cannot open {}: {}", entry.display(), err),
            entry.display().to_string(),
            1,
            1,
        )
    })?;

    // A file that is not a Vey file is a mistake worth naming.
    if entry
        .extension()
        .is_some_and(|extension| extension != "vey")
    {
        return Err(VeyError::new(
            format!("{} is not a Vey file", entry.display()),
            entry.display().to_string(),
            1,
            1,
        ));
    }

    let mut modules: Vec<Module> = Vec::new();
    let mut ids: HashMap<PathBuf, usize> = HashMap::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut styles: Vec<StyleFile> = Vec::new();

    let entry_id = load_module(&entry, &mut modules, &mut ids, &mut stack, &mut styles)?;

    // type environments for each module, built in dependency order
    let mut type_envs: Vec<types::ModuleTypeEnv> = Vec::with_capacity(modules.len());

    let mut chunks = Vec::new();
    let mut sources = Vec::new();
    let mut mappings: Vec<Option<(usize, usize)>> = Vec::new();
    let mut bundle_line = 0usize;

    for module in &modules {
        let deps = module
            .deps
            .iter()
            .map(|(alias, id)| (alias.clone(), format!("__m{}", id)))
            .collect();
        let options = Options {
            file: module.path.display().to_string(),
            module: true,
            deps,
            exports: module.exports.clone(),
        };
        check::check(&module.ast, &module.path.display().to_string())?;
        // build imports map for this module from its deps
        let mut imports: HashMap<String, types::ModuleTypeEnv> = HashMap::new();
        for (alias, dep_id) in &module.deps {
            if let Some(env) = type_envs.get(*dep_id) {
                imports.insert(alias.clone(), env.clone());
            }
        }
        let env = types::check(&module.ast, &module.path.display().to_string(), &imports)?;
        while type_envs.len() < module.id {
            type_envs.push(types::ModuleTypeEnv::default());
        }
        if type_envs.len() == module.id {
            type_envs.push(env);
        } else {
            type_envs[module.id] = env;
        }
        let generated = generate(&module.ast, &options)?;
        let source_index = sources.len();
        sources.push(SourceFile {
            path: module.path.display().to_string(),
            content: module.source.clone(),
        });

        for (index, source_line) in generated.lines.iter().enumerate() {
            let generated_line = bundle_line + 1 + index;
            if generated_line >= mappings.len() {
                mappings.resize(generated_line + 1, None);
            }
            if let Some(source_line) = source_line {
                mappings[generated_line] = Some((source_index, source_line.saturating_sub(1)));
            }
        }

        let (wrapped, line_count) = wrap_module(module.id, &generated.code);
        chunks.push(wrapped);
        bundle_line += line_count;
    }

    mappings.resize(bundle_line, None);

    Ok(Bundle {
        code: chunks.join("\n"),
        modules,
        entry: entry_id,
        entry_path: entry,
        sources,
        mappings,
        styles,
    })
}
fn wrap_module(id: usize, code: &str) -> (String, usize) {
    let body = code
        .lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("  {}", line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let text = format!(
        "const __m{} = await (async function () {{\n{}\n}})();",
        id, body
    );
    let line_count = text.lines().count();
    (text, line_count)
}
