//! The module graph of a program and the bundle it becomes.

mod cache;
mod loader;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ast::Program;
use crate::check;
use crate::codegen::{generate, Options};
use crate::sourcemap;
use crate::types;
use crate::warn::Warning;
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

#[derive(Clone)]
pub struct SourceFile {
    pub path: String,
    pub content: String,
}

#[derive(Clone)]
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
    /// The warnings of the program.
    pub warnings: Vec<Warning>,
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

    // The files of the project are known, so the result of the last build may
    // be taken from the cache when nothing has changed.
    let sources: Vec<SourceFile> = modules
        .iter()
        .map(|module| SourceFile {
            path: module.path.display().to_string(),
            content: module.source.clone(),
        })
        .collect();
    let root = project_root(&entry);
    let key = cache::key(&sources, env!("CARGO_PKG_VERSION"));
    if let Some(cached) = cache::read(&root, key) {
        return Ok(Bundle {
            code: cached.code,
            modules,
            entry: entry_id,
            entry_path: entry,
            sources: cached.sources,
            mappings: cached.mappings,
            styles: cached.styles,
            warnings: cached.warnings,
        });
    }

    // type environments for each module, built in dependency order
    let mut type_envs: Vec<types::ModuleTypeEnv> = Vec::with_capacity(modules.len());

    let mut warnings: Vec<Warning> = Vec::new();
    // The options of every module, so that the code may be written in
    // parallel.
    let mut options: Vec<Options> = Vec::with_capacity(modules.len());
    for (index, module) in modules.iter().enumerate() {
        let deps = module
            .deps
            .iter()
            .map(|(alias, id)| (alias.clone(), format!("__m{}", id)))
            .collect();
        let module_options = Options {
            file: module.path.display().to_string(),
            module: true,
            deps,
            packages: module.packages.clone(),
            exports: module.exports.clone(),
            source: index,
            enums: std::collections::HashMap::new(),
            resolved: std::collections::HashMap::new(),
        };
        let found = check::check(&module.ast, &module.path.display().to_string())?;
        warnings.extend(found);
        // build imports map for this module from its deps
        let mut imports: HashMap<String, types::ModuleTypeEnv> = HashMap::new();
        for (alias, dep_id) in &module.deps {
            if let Some(env) = type_envs.get(*dep_id) {
                imports.insert(alias.clone(), env.clone());
            }
        }
        // A package of npm is not checked, so it is an empty environment.
        for (alias, _) in &module.packages {
            let mut env = types::ModuleTypeEnv::default();
            env.unchecked = true;
            imports.insert(alias.clone(), env);
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
        options.push(Options {
            enums: env.enums,
            resolved: env.resolved,
            ..module_options
        });
    }

    // The code of the modules is written in parallel, because every module
    // is a piece of its own.
    let generated: Vec<Result<crate::codegen::Generated, VeyError>> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = modules
                .iter()
                .zip(options.iter())
                .map(|(module, options)| scope.spawn(move || generate(&module.ast, options)))
                .collect();
            handles
                .into_iter()
                .map(|handle| {
                    handle.join().unwrap_or_else(|_| {
                        Err(VeyError::new("the compiler stopped", "the project", 1, 1))
                    })
                })
                .collect()
        });

    let mut chunks = Vec::new();
    for (module, result) in modules.iter().zip(generated.into_iter()) {
        let generated = result?;
        let (wrapped, _) = wrap_module(module.id, &generated.code);
        chunks.push(wrapped);
    }

    // The marks of the code say where every piece came from. They are taken
    // out here, so that the bundle has no trace of them.
    let (code, mappings) = sourcemap::extract(&chunks.join("\n"));

    let bundle = Bundle {
        code,
        modules,
        entry: entry_id,
        entry_path: entry,
        sources,
        mappings,
        styles,
        warnings,
    };
    cache::store(&root, &bundle);
    Ok(bundle)
}

/// The folder the cache of a project is written in.
///
/// A folder that is called `src` belongs to the project above it.
fn project_root(entry: &Path) -> PathBuf {
    let parent = entry
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if parent.file_name().is_some_and(|name| name == "src") {
        return parent
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
    }
    parent.to_path_buf()
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
