//! The module graph of a program and the bundle it becomes.

mod loader;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::check;
use crate::codegen::{generate, Options};
use crate::sourcemap;
use crate::types;
use crate::VeyError;

use loader::load_module;
pub struct Module {
    pub id: usize,
    pub path: PathBuf,
    pub source: String,
    pub ast: Program,
    pub deps: Vec<(String, usize)>,
    /// The packages of npm that the module uses, by their alias.
    pub packages: Vec<(String, String)>,
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
    /// The places of the generated code that came from a source file.
    pub mappings: Vec<sourcemap::Segment>,
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
            packages: module.packages.clone(),
            exports: module.exports.clone(),
            source: sources.len(),
            enums: std::collections::HashMap::new(),
        };
        check::check(&module.ast, &module.path.display().to_string())?;
        // build imports map for this module from its deps
        let mut imports: HashMap<String, types::ModuleTypeEnv> = HashMap::new();
        for (alias, dep_id) in &module.deps {
            if let Some(env) = type_envs.get(*dep_id) {
                imports.insert(alias.clone(), env.clone());
            }
        }
        // A package of npm is not checked, so it is an empty environment.
        for (alias, _) in &module.packages {
            imports.insert(alias.clone(), types::ModuleTypeEnv::default());
        }
        let env = types::check(&module.ast, &module.path.display().to_string(), &imports)?;
        while type_envs.len() < module.id {
            type_envs.push(types::ModuleTypeEnv::default());
        }
        if type_envs.len() == module.id {
            type_envs.push(env.clone());
        } else {
            type_envs[module.id] = env.clone();
        }
        // The enums of the module are described by the checker, and the code
        // generator uses the same description.
        let options = Options { enums: env.enums, ..options };
        let generated = generate(&module.ast, &options)?;
        sources.push(SourceFile {
            path: module.path.display().to_string(),
            content: module.source.clone(),
        });

        let (wrapped, _) = wrap_module(module.id, &generated.code);
        chunks.push(wrapped);
    }

    // The marks of the code say where every piece came from. They are taken
    // out here, so that the bundle has no trace of them.
    let (code, mappings) = sourcemap::extract(&chunks.join("\n"));

    Ok(Bundle {
        code,
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
