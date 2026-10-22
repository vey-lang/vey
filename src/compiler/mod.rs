//! The module graph of a program and the bundle it becomes.

mod cache;
mod loader;

use std::collections::{HashMap, HashSet};
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
    /// The exported functions that nothing in the program reads, so they
    /// are not written into the bundle.
    pub unused: HashSet<String>,
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

    let entry_id = load_module(&entry, &mut modules, &mut ids, &mut stack, &mut styles, None)?;

    // An exported function that nothing reads is not written.
    loader::mark_unused(&mut modules, entry_id);

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
    let key = cache::key(&sources, &styles, env!("CARGO_PKG_VERSION"));
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
            unused: module.unused.clone(),
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
        // generator uses the same description. The enums of the modules that
        // are used are there too, under the name of their alias, so that a
        // match on an enum of another module knows the fields of its
        // variant.
        let mut enums = env.enums.clone();
        for (alias, import) in &imports {
            for (name, info) in &import.enums {
                enums.insert(format!("{}.{}", alias, name), info.clone());
            }
        }
        options.push(Options {
            enums,
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
        chunks.push(wrap_module(module.id, &generated.code));
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

/// The problems of one file of a project.
///
/// The text of the file may be another one than the one on the disk, which
/// is what a language server needs. The modules that the file uses are read
/// from the disk.
pub fn check_file(path: &Path, text: Option<&str>) -> Result<FileProblems, VeyError> {
    let mut modules: Vec<Module> = Vec::new();
    let mut ids: HashMap<PathBuf, usize> = HashMap::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut styles: Vec<StyleFile> = Vec::new();
    let entry_id = load_module(path, &mut modules, &mut ids, &mut stack, &mut styles, text)?;

    let mut warnings: Vec<Warning> = Vec::new();
    for module in &modules {
        let label = module.path.display().to_string();
        match check::check(&module.ast, &label) {
            Ok(found) => warnings.extend(found),
            Err(err) => {
                // The mistake is said as a warning, so that the editor may
                // show it next to the other ones.
                warnings.push(Warning::new(
                    err.message.clone(),
                    label.clone(),
                    err.line,
                    err.col,
                ));
            }
        }
    }

    // The types of the modules, in the order they are loaded.
    let (_, errors) = type_environments(&modules, entry_id);

    Ok(FileProblems { warnings, errors })
}

/// The type environment of every module, in the order they are loaded.
///
/// The modules that a file uses are checked as well, so that the types of
/// another module are known. The mistake of the entry file is given back,
/// so that the editor may show it.
fn type_environments(
    modules: &[Module],
    _entry_id: usize,
) -> (Vec<types::ModuleTypeEnv>, Vec<VeyError>) {
    let mut envs: Vec<types::ModuleTypeEnv> = Vec::with_capacity(modules.len());
    let mut errors: Vec<VeyError> = Vec::new();
    for module in modules {
        let mut imports: HashMap<String, types::ModuleTypeEnv> = HashMap::new();
        for (alias, id) in &module.deps {
            if let Some(env) = envs.get(*id) {
                imports.insert(alias.clone(), env.clone());
            }
        }
        let label = module.path.display().to_string();
        // The mistake of a module is said even when the module is not the
        // one that was asked about: the editor shows the mistakes of the
        // whole project.
        let env = match types::check(&module.ast, &label, &imports) {
            Ok(env) => env,
            Err(err) => {
                errors.push(err);
                // The module is not known to the ones that use it, so its
                // members are taken as they are and are not checked again.
                let mut empty = types::ModuleTypeEnv::default();
                empty.unchecked = true;
                empty
            }
        };
        while envs.len() < module.id {
            envs.push(types::ModuleTypeEnv::default());
        }
        if envs.len() == module.id {
            envs.push(env);
        } else {
            envs[module.id] = env;
        }
    }
    (envs, errors)
}

/// The type of the expression at a place of a file.
///
/// The modules that the file uses are checked as well, so that the types of
/// another module are known and the answer is the real one.
pub fn explain_file(path: &Path, line: usize, col: usize) -> Result<String, VeyError> {
    explain_source(path, None, line, col)
}

/// The type of the expression at a place of a file.
///
/// The text of the file may be another one than the one on the disk, which
/// is what a language server needs.
pub fn explain_source(
    path: &Path,
    text: Option<&str>,
    line: usize,
    col: usize,
) -> Result<String, VeyError> {
    let mut modules: Vec<Module> = Vec::new();
    let mut ids: HashMap<PathBuf, usize> = HashMap::new();
    let mut stack: Vec<PathBuf> = Vec::new();
    let mut styles: Vec<StyleFile> = Vec::new();
    let entry_id = load_module(path, &mut modules, &mut ids, &mut stack, &mut styles, text)?;

    let (envs, _) = type_environments(&modules, entry_id);

    // The types of the modules that the file uses.
    let mut imports: HashMap<String, types::ModuleTypeEnv> = HashMap::new();
    let entry = &modules[entry_id];
    for (alias, id) in &entry.deps {
        if let Some(env) = envs.get(*id) {
            imports.insert(alias.clone(), env.clone());
        }
    }

    let label = entry.path.display().to_string();
    types::explain(&entry.ast, &label, line, col, &imports)
}

/// The problems of a file: the warnings and the mistakes of the file and
/// of the modules that it uses.
pub struct FileProblems {
    pub warnings: Vec<Warning>,
    pub errors: Vec<VeyError>,
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

fn wrap_module(id: usize, code: &str) -> String {
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
    format!(
        "const __m{} = await (async function () {{\n{}\n}})();",
        id, body
    )
}
