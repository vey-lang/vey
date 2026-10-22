use std::collections::HashSet;

mod awaits;
mod exprs;
mod js;
mod markup;

use crate::ast::*;
use crate::builtins::BUILTINS;
use crate::lexer::Pos;
use crate::sourcemap::marker;
use crate::types::core::EnumInfo;
use crate::VeyError;

pub(crate) use awaits::{
    block_has_await, block_has_defer, block_has_try, block_has_yield, markup_has_await,
};
pub(crate) use js::{js_key, js_string};

pub struct Options {
    pub file: String,
    pub module: bool,
    pub deps: Vec<(String, String)>,
    /// The packages of npm that the program uses, by their alias.
    pub packages: Vec<(String, String)>,
    pub exports: Vec<String>,
    /// The exported functions that nothing reads, so they are not written.
    pub unused: HashSet<String>,
    /// The file of the program among the files of the bundle.
    ///
    /// The mark of every piece of code says which file it came from, so that
    /// a runtime error points at the right one.
    pub source: usize,
    /// The enums of the program, described by the checker.
    pub enums: std::collections::HashMap<String, EnumInfo>,
    /// The calls whose arguments are given by name, and the order of the
    /// arguments.
    pub resolved: crate::types::core::ResolvedCalls,
}

impl Options {
    pub fn new(file: &str) -> Self {
        Self {
            file: file.to_string(),
            module: false,
            deps: Vec::new(),
            packages: Vec::new(),
            exports: Vec::new(),
            unused: HashSet::new(),
            source: 0,
            enums: std::collections::HashMap::new(),
            resolved: std::collections::HashMap::new(),
        }
    }
}

pub struct Generated {
    pub code: String,
}

pub fn generate(program: &Program, opts: &Options) -> Result<Generated, VeyError> {
    let mut codegen = Codegen {
        file: opts.file.clone(),
        out: String::new(),
        source: opts.source,
        pending: None,
        current_defer: None,
        indent: 0,
        scopes: vec![HashSet::new()],
        temp: 0,
        deps: opts.deps.clone(),
        packages: opts.packages.clone(),
        module: opts.module,
        exports: opts.exports.clone(),
        unused: opts.unused.clone(),
        enums: opts.enums.clone(),
        resolved: opts.resolved.clone(),
    };
    codegen.run(program)?;
    Ok(Generated {
        code: codegen.out,
    })
}

struct Codegen {
    file: String,
    out: String,
    /// The file of the program among the files of the bundle.
    source: usize,
    /// The mark of the statement that is being written.
    pending: Option<Pos>,
    /// The list of the pieces of code that run when the function is left.
    current_defer: Option<String>,
    indent: usize,
    scopes: Vec<HashSet<String>>,
    temp: usize,
    deps: Vec<(String, String)>,
    packages: Vec<(String, String)>,
    module: bool,
    exports: Vec<String>,
    unused: HashSet<String>,
    enums: std::collections::HashMap<String, EnumInfo>,
    /// The calls whose arguments are given by name, and the order of the
    /// arguments.
    resolved: crate::types::core::ResolvedCalls,
}

impl Codegen {
    /// An error that points at a place of the program.
    fn err_at(&self, pos: Pos, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file.clone(), pos.line, pos.col)
    }

    /// The mark that says where a piece of the code came from.
    fn mark(&self, pos: Pos) -> String {
        marker(self.source, pos)
    }

    fn emit(&mut self, text: &str) {
        let pos = self.pending.take();
        self.emit_marked(pos, text);
    }

    /// Write a line of code, with a mark that says where it came from.
    fn emit_marked(&mut self, pos: Option<Pos>, text: &str) {
        let pad = "  ".repeat(self.indent);
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.out.push('\n');
            } else {
                self.out.push_str(&pad);
            }
            if i == 0 {
                if let Some(pos) = pos {
                    self.out.push_str(&self.mark(pos));
                }
            }
            self.out.push_str(line);
        }
        self.out.push('\n');
    }
    fn push_scope(&mut self) {
        self.scopes.push(HashSet::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string());
        }
    }

    fn lookup(&self, name: &str) -> bool {
        self.scopes.iter().rev().any(|scope| scope.contains(name))
    }

    fn resolve(&self, name: &str) -> String {
        if self.lookup(name) {
            return format!("_{}", name);
        }
        if BUILTINS.contains(&name) {
            return format!("V.{}", name);
        }
        format!("_{}", name)
    }

    fn run(&mut self, program: &Program) -> Result<(), VeyError> {
        if self.module {
            self.emit("const __exports = {};");
        }
        for stmt in &program.body {
            if matches!(stmt, Stmt::Use(_)) {
                self.gen_stmt(stmt)?;
            }
        }
        let rest: Vec<Stmt> = program
            .body
            .iter()
            .filter(|stmt| !matches!(stmt, Stmt::Use(_)))
            .filter(|stmt| self.is_written(stmt))
            .cloned()
            .collect();
        self.gen_body(&Block { body: rest })?;
        if self.module {
            for name in self.exports.clone() {
                if self.unused.contains(&name) {
                    continue;
                }
                self.emit(&format!(
                    "Object.defineProperty(__exports, {}, {{ get: () => _{} , enumerable: true }});",
                    js_string(&name),
                    name
                ));
            }
            self.emit("return __exports;");
        }
        Ok(())
    }

    /// Whether a declaration is written into the bundle.
    ///
    /// An exported function that nothing in the program reads is left out,
    /// so that a module that has many of them does not weigh on every page.
    fn is_written(&self, stmt: &Stmt) -> bool {
        match stmt {
            Stmt::Fn(decl) if decl.exported => !self.unused.contains(&decl.name),
            _ => true,
        }
    }

    fn capture<F>(&mut self, body: F) -> Result<String, VeyError>
    where
        F: FnOnce(&mut Self) -> Result<(), VeyError>,
    {
        let outer = std::mem::take(&mut self.out);
        let outer_indent = self.indent;
        self.indent += 1;
        let result = body(self);
        let captured = std::mem::replace(&mut self.out, outer);
        self.indent = outer_indent;
        result?;
        Ok(captured)
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
        // The mark of the statement is written with the first line of it.
        self.pending = Some(stmt.pos());
        let result = self.gen_stmt_inner(stmt);
        self.pending = None;
        result
    }

    fn gen_stmt_inner(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
        match stmt {
            Stmt::Var(decl) => {
                let init = match &decl.init {
                    Some(expr) => format!(" = {}", self.gen_expr(expr)?),
                    None => String::new(),
                };
                let keyword = if decl.kind == VarKind::Let {
                    "let"
                } else {
                    "const"
                };
                let pattern = self.gen_binding_pattern(&decl.pattern)?;
                self.emit(&format!("{} {}{};", keyword, pattern, init));
                for (name, _) in decl.pattern.bindings() {
                    self.declare(name);
                }
            }
            Stmt::Fn(decl) => {
                self.declare(&decl.name);
                // The scope of the function is open before the parameters
                // are written, so that a value of a parameter may name the
                // parameter in front of it.
                self.push_scope();
                let params = self.gen_params(&decl.params)?;
                let async_kw = if block_has_await(&decl.body) {
                    "async "
                } else {
                    ""
                };
                // A function that hands values back one by one is a
                // generator.
                let generator = if block_has_yield(&decl.body) { "*" } else { "" };
                self.emit(&format!(
                    "{}function{} _{} ({}) {{",
                    async_kw, generator, decl.name, params
                ));
                self.indent += 1;
                self.gen_body(&decl.body)?;
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::Enum(decl) => {
                self.declare(&decl.name);
                let mut entries = Vec::new();
                for variant in &decl.variants {
                    if variant.fields.is_empty() {
                        entries.push(format!(
                            "{}: V.enumValue({}, {}, {})",
                            js_key(&variant.name),
                            js_string(&self.enum_runtime_name(&decl.name)),
                            js_string(&variant.name),
                            js_string(&decl.name)
                        ));
                    } else {
                        let names = variant
                            .fields
                            .iter()
                            .map(|f| js_string(&f.name))
                            .collect::<Vec<_>>()
                            .join(", ");
                        entries.push(format!(
                            "{}: V.enumCtor({}, {}, [{}])",
                            js_key(&variant.name),
                            js_string(&self.enum_runtime_name(&decl.name)),
                            js_string(&variant.name),
                            names
                        ));
                    }
                }
                self.emit(&format!(
                    "const _{} = Object.freeze({{ {} }});",
                    decl.name,
                    entries.join(", ")
                ));
            }
            Stmt::Component(decl) => {
                self.declare(&decl.name);
                // A component accepts either a marked props object (used by
                // markup) or plain positional arguments. `__veyValues` holds
                // the positional arguments, `__veyProps` the named ones.
                // A component whose markup waits is async.
                let async_kw = if markup_has_await(&decl.children) { "async " } else { "" };
                self.emit(&format!(
                    "{}function _{} (__veyFirst, ...__veyRest) {{",
                    async_kw, decl.name
                ));
                self.indent += 1;
                self.push_scope();
                self.emit(
                    "const __veyProps = (__veyFirst !== null && typeof __veyFirst === \"object\" \
                     && __veyFirst.__veyProps === true) ? __veyFirst : {};",
                );
                self.emit(
                    "const __veyValues = __veyFirst !== null \
                     && typeof __veyFirst === \"object\" \
                     && __veyFirst.__veyProps === true \
                     ? __veyRest \
                     : (__veyFirst === undefined ? [] : [__veyFirst, ...__veyRest]);",
                );
                for (index, param) in decl.params.iter().enumerate() {
                    match &param.pattern {
                        BindingPattern::Name { name, .. } => {
                            self.declare(name);
                            let key = js_string(name);
                            let named = format!("__veyProps[{}]", key);
                            let value = match &param.default {
                                Some(default) => format!(
                                    "__veyValues.length > {index} ? __veyValues[{index}] \
                                     : ({named} !== undefined ? {named} : ({}))",
                                    self.gen_expr(default)?
                                ),
                                None => {
                                    format!("__veyValues.length > {index} ? __veyValues[{index}] : {named}")
                                }
                            };
                            self.emit(&format!("const _{} = {};", name, value));
                        }
                        // A pattern takes its values from the named props.
                        pattern => {
                            for (name, expression) in self.prop_bindings(pattern)? {
                                self.declare(&name);
                                self.emit(&format!("const _{} = {};", name, expression));
                            }
                        }
                    }
                }
                let markup = self.gen_markup_nodes(&decl.children)?;
                self.emit(&format!("return {};", markup));
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::If(decl) => {
                let test = self.gen_expr(&decl.test)?;
                self.emit(&format!("if ({}) {{", test));
                self.indent += 1;
                self.gen_block(&decl.then_block)?;
                self.indent -= 1;
                if let Some(branch) = &decl.else_branch {
                    self.emit("} else {");
                    self.indent += 1;
                    self.gen_stmt(branch)?;
                    self.indent -= 1;
                }
                self.emit("}");
            }
            Stmt::IfLet(decl) => {
                // The value is taken once, and the names of the pattern take
                // it apart when it fits.
                let slot = self.temp;
                self.temp += 1;
                let value = self.gen_expr(&decl.value)?;
                self.emit(&format!("const __let{} = {};", slot, value));
                let (condition, bindings) = self.gen_pattern(&decl.pattern, &format!("__let{}", slot))?;
                self.emit(&format!("if ({}) {{", condition));
                self.indent += 1;
                self.push_scope();
                for (name, expression) in &bindings {
                    self.declare(name);
                    self.emit(&format!("const _{} = {};", name, expression));
                }
                self.gen_block(&decl.then_block)?;
                self.pop_scope();
                self.indent -= 1;
                if let Some(branch) = &decl.else_branch {
                    self.emit("} else {");
                    self.indent += 1;
                    self.gen_stmt(branch)?;
                    self.indent -= 1;
                }
                self.emit("}");
            }
            Stmt::While(decl) => {
                let test = self.gen_expr(&decl.test)?;
                let label = decl.label.as_ref().map(|name| format!("{}: ", name)).unwrap_or_default();
                self.emit(&format!("{}while ({}) {{", label, test));
                self.indent += 1;
                self.gen_block(&decl.body)?;
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::WhileLet(decl) => {
                // The value is taken again on every turn of the loop.
                let slot = self.temp;
                self.temp += 1;
                let value = self.gen_expr(&decl.value)?;
                self.emit("while (true) {");
                self.indent += 1;
                self.emit(&format!("const __let{} = {};", slot, value));
                let (condition, bindings) = self.gen_pattern(&decl.pattern, &format!("__let{}", slot))?;
                self.emit(&format!("if (!({})) break;", condition));
                self.push_scope();
                for (name, expression) in &bindings {
                    self.declare(name);
                    self.emit(&format!("const _{} = {};", name, expression));
                }
                self.gen_block(&decl.body)?;
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::LetElse(decl) => {
                // The value has to be there, or the block runs.
                let slot = self.temp;
                self.temp += 1;
                let value = self.gen_expr(&decl.value)?;
                self.emit(&format!("const __let{} = {};", slot, value));
                self.emit(&format!(
                    "if (__let{} === null || __let{} === undefined) {{",
                    slot, slot
                ));
                self.indent += 1;
                self.gen_block(&decl.otherwise)?;
                self.indent -= 1;
                self.emit("}");
                self.push_scope();
                let pattern = self.gen_binding_pattern(&decl.pattern)?;
                self.emit(&format!(
                    "const {} = __let{};",
                    pattern,
                    slot
                ));
                for (name, _) in decl.pattern.bindings() {
                    self.declare(name);
                }
                self.pop_scope();
            }
            Stmt::Defer(decl) => {
                // The piece of code runs when the block is left. The top
                // level of a file is a block of its own, so a `defer` there
                // runs at the end of the program.
                let value = self.gen_expr(&decl.value)?;
                let Some(name) = self.current_defer.clone() else {
                    // Every block that holds a `defer` keeps a list of them,
                    // so this is only a guard.
                    return Err(self.err_at(decl.pos, "this defer has no block to run in"));
                };
                self.emit(&format!("{}.push(() => ({}));", name, value));
            }
            Stmt::For(decl) => {
                let label = decl
                    .label
                    .as_ref()
                    .map(|name| format!("{}: ", name))
                    .unwrap_or_default();
                // A loop over a range walks it with a counter, so that it may
                // have a step.
                let range = match &decl.step {
                    Some(_) => match &decl.iter {
                        Expr::Range(start, end, _) => Some((start, end)),
                        _ => {
                            return Err(self.err_at(
                                decl.pos,
                                "a loop with a step must walk a range",
                            ))
                        }
                    },
                    None => None,
                };
                let pattern = self.gen_binding_pattern(&decl.pattern)?;
                if let Some((start_expr, end_expr)) = range {
                    let mut start = self.gen_expr(start_expr)?;
                    let mut end = self.gen_expr(end_expr)?;
                    let step_expr = decl.step.as_ref().expect("the step");
                    let step = self.gen_expr(step_expr)?;
                    // A loop that counts down is written the other way round.
                    let down = is_negative(step_expr);
                    // A range that counts down begins at its end, so that
                    // the range may be written the way it is read.
                    if down {
                        if let (Some(from), Some(to)) =
                            (number_value(start_expr), number_value(end_expr))
                        {
                            if from < to {
                                std::mem::swap(&mut start, &mut end);
                            }
                        }
                    }
                    let test = if down { ">=" } else { "<" };
                    // The counter is a name of its own, so that a pattern
                    // may take it apart.
                    let counter = format!("__veyI{}", self.temp);
                    self.temp += 1;
                    self.emit(&format!(
                        "{}for (let {} = {}; {} {} {}; {} += {}) {{",
                        label, counter, start, counter, test, end, counter, step
                    ));
                    self.indent += 1;
                    self.push_scope();
                    for (name, _) in decl.pattern.bindings() {
                        self.declare(name);
                    }
                    self.emit(&format!("const {} = {};", pattern, counter));
                    // The body of the loop has a block of its own, so that the
                    // pieces of code that run when it is left belong to it.
                    self.gen_block(&decl.body)?;
                    self.pop_scope();
                    self.indent -= 1;
                    self.emit("}");
                    return Ok(());
                }
                let iter = self.gen_expr(&decl.iter)?;
                self.emit(&format!(
                    "{}for (let {} of V.iterate({})) {{",
                    label, pattern, iter
                ));
                self.indent += 1;
                self.push_scope();
                for (name, _) in decl.pattern.bindings() {
                    self.declare(name);
                }
                // The body of the loop has a block of its own, so that the
                // pieces of code that run when it is left belong to it.
                self.gen_block(&decl.body)?;
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::Repeat(decl) => {
                let slot = self.temp;
                self.temp += 1;
                let index = format!("__i{}", slot);
                let limit = format!("__n{}", slot);
                let count = self.gen_expr(&decl.count)?;
                let label = decl
                    .label
                    .as_ref()
                    .map(|name| format!("{}: ", name))
                    .unwrap_or_default();
                self.emit(&format!("const {} = ({});", limit, count));
                self.emit(&format!(
                    "{}for (let {} = 0; {} < {}; {}++) {{",
                    label, index, index, limit, index
                ));
                self.indent += 1;
                self.gen_block(&decl.body)?;
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::Return(decl) => match &decl.value {
                Some(value) => {
                    let value = self.gen_expr(value)?;
                    self.emit(&format!("return {};", value));
                }
                None => self.emit("return;"),
            },
            Stmt::Break(label, _) => match label {
                Some(label) => self.emit(&format!("break {};", label)),
                None => self.emit("break;"),
            },
            Stmt::Continue(label, _) => match label {
                Some(label) => self.emit(&format!("continue {};", label)),
                None => self.emit("continue;"),
            },
            Stmt::Throw(decl) => {
                let value = self.gen_expr(&decl.value)?;
                self.emit(&format!("throw {};", value));
            }
            Stmt::Try(decl) => {
                self.emit("try {");
                self.indent += 1;
                self.gen_block(&decl.body)?;
                self.indent -= 1;
                if let Some(catch) = &decl.catch {
                    let name = decl
                        .name
                        .as_ref()
                        .map(|n| format!("_{}", n))
                        .unwrap_or_else(|| "__err".to_string());
                    self.emit(&format!("}} catch ({}) {{", name));
                    self.indent += 1;
                    self.push_scope();
                    if let Some(n) = &decl.name {
                        self.declare(n);
                    }
                    for inner in &catch.body {
                        self.gen_stmt(inner)?;
                    }
                    self.pop_scope();
                    self.indent -= 1;
                    self.emit("}");
                }
                if let Some(finally) = &decl.finally {
                    // The catch block has already closed the try.
                    if decl.catch.is_some() {
                        self.emit(" finally {");
                    } else {
                        self.emit("} finally {");
                    }
                    self.indent += 1;
                    self.gen_block(finally)?;
                    self.indent -= 1;
                    self.emit("}");
                } else if decl.catch.is_none() {
                    // Without a catch and without a finally the block of the
                    // try still has to be closed.
                    self.emit("}");
                }
            }
            Stmt::Type(_) => {}
            Stmt::Interface(_) => {}
            Stmt::Impl(decl) => {
                // The block becomes a function that makes a value of the
                // type and gives it the functions of the type.
                let name = format!("_{}", decl.type_name);
                self.emit(&format!("function {}(__veyValue) {{", name));
                self.indent += 1;
                self.emit("const __veySelf = __veyValue;");
                for method in &decl.methods {
                    // The scope of the method is open before the parameters
                    // are written, so that a value of a parameter may name
                    // the parameter in front of it.
                    self.push_scope();
                    self.declare("self");
                    let params = self.gen_params(&method.params)?;
                    let async_kw = if block_has_await(&method.body) {
                        "async "
                    } else {
                        ""
                    };
                    let generator = if block_has_yield(&method.body) { "*" } else { "" };
                    self.emit(&format!(
                        "__veySelf.{} = {}function{}({}) {{",
                        js_key(&method.name),
                        async_kw,
                        generator,
                        params
                    ));
                    self.indent += 1;
                    self.emit("const _self = __veySelf;");
                    self.gen_body(&method.body)?;
                    self.pop_scope();
                    self.indent -= 1;
                    self.emit("};");
                }
                self.emit("return __veySelf;");
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::Style(_) => {}
            Stmt::Use(decl) => {
                if decl.path.ends_with(".css") {
                    return Ok(());
                }
                // A package of npm is loaded when the program runs.
                if decl.path.starts_with("npm:") {
                    let name = self
                        .packages
                        .iter()
                        .find(|(alias, _)| alias == &decl.alias)
                        .map(|(_, package)| package.clone())
                        .ok_or_else(|| {
                            self.err_at(
                                decl.pos,
                                format!("package '{}' was not resolved", decl.path),
                            )
                        })?;
                    self.emit(&format!(
                        "const _{} = V.require({});",
                        decl.alias,
                        js_string(&name)
                    ));
                    self.declare(&decl.alias);
                    return Ok(());
                }
                let target = self
                    .deps
                    .iter()
                    .find(|(alias, _)| alias == &decl.alias)
                    .map(|(_, module)| module.clone())
                    .ok_or_else(|| {
                        self.err_at(
                            decl.pos,
                            format!("module '{}' was not resolved", decl.path),
                        )
                    })?;
                self.emit(&format!("const _{} = {};", decl.alias, target));
                self.declare(&decl.alias);
            }
            Stmt::Expr(decl) => {
                let expr = self.gen_expr(&decl.expr)?;
                self.emit(&format!("{};", expr));
            }
            Stmt::Assign(decl) => {
                let target = self.gen_expr(&decl.target)?;
                let value = self.gen_expr(&decl.value)?;
                self.emit(&format!("{} {} {};", target, decl.op, value));
            }
            Stmt::Block(block, _) => {
                self.emit("{");
                self.indent += 1;
                self.gen_block(block)?;
                self.indent -= 1;
                self.emit("}");
            }
        }
        Ok(())
    }

    fn gen_block(&mut self, block: &Block) -> Result<(), VeyError> {
        self.push_scope();
        // A block that has something to do when it is left needs a block of
        // its own in the JavaScript, so that the piece of code belongs to
        // this block and not to the one around it.
        if block_has_defer(block) {
            self.emit("{");
            self.indent += 1;
            self.gen_body(block)?;
            self.indent -= 1;
            self.emit("}");
        } else {
            for stmt in &block.body {
                self.gen_stmt(stmt)?;
            }
        }
        self.pop_scope();
        Ok(())
    }

    /// The body of a function, with the parts that wrap it.
    ///
    /// A function that has something to do when it is left keeps a list of
    /// the pieces of code, and a function that hands a failure on catches
    /// the failure that is thrown.
    fn gen_body(&mut self, block: &Block) -> Result<(), VeyError> {
        let has_defer = block_has_defer(block);
        let has_try = block_has_try(block);
        let outer = self.current_defer.clone();

        if has_defer {
            let slot = self.temp;
            self.temp += 1;
            self.current_defer = Some(format!("__defers{}", slot));
            self.emit(&format!("const __defers{} = [];", slot));
        }
        if has_defer || has_try {
            self.emit("try {");
            self.indent += 1;
        }
        if has_try {
            self.emit("try {");
            self.indent += 1;
        }

        for stmt in &block.body {
            self.gen_stmt(stmt)?;
        }

        if has_try {
            self.indent -= 1;
            self.emit("} catch (__veyErr) {");
            self.indent += 1;
            self.emit("if (__veyErr && __veyErr.__veyFailure !== undefined) {");
            self.indent += 1;
            self.emit("return V.failure(__veyErr.__veyFailure);");
            self.indent -= 1;
            self.emit("}");
            self.emit("if (__veyErr && __veyErr.__veyNothing) {");
            self.indent += 1;
            self.emit("return null;");
            self.indent -= 1;
            self.emit("}");
            self.emit("throw __veyErr;");
            self.indent -= 1;
            self.emit("}");
        }
        if has_defer || has_try {
            self.indent -= 1;
            self.emit("} finally {");
            self.indent += 1;
            if has_defer {
                let name = self.current_defer.clone().unwrap_or_default();
                self.emit(&format!("while ({}.length > 0) {{", name));
                self.indent += 1;
                self.emit(&format!("{}.pop()();", name));
                self.indent -= 1;
                self.emit("}");
            }
            self.indent -= 1;
            self.emit("}");
        }
        self.current_defer = outer;
        Ok(())
    }

    fn gen_binding_pattern(&mut self, pattern: &BindingPattern) -> Result<String, VeyError> {
        match pattern {
            BindingPattern::Name { name, default, .. } => {
                let name = format!("_{}", name);
                match default {
                    Some(default) => Ok(format!("{} = {}", name, self.gen_expr(default)?)),
                    None => Ok(name),
                }
            }
            BindingPattern::Array { items, rest, .. } => {
                let mut parts: Vec<String> = Vec::new();
                for item in items {
                    parts.push(self.gen_binding_pattern(item)?);
                }
                if let Some(rest) = rest {
                    parts.push(format!("..._{}", rest));
                }
                Ok(format!("[{}]", parts.join(", ")))
            }
            BindingPattern::Object { fields, rest, .. } => {
                let mut parts: Vec<String> = Vec::new();
                for field in fields {
                    parts.push(format!(
                        "{}: {}",
                        js_key(&field.key),
                        self.gen_binding_pattern(&field.pattern)?
                    ));
                }
                if let Some(rest) = rest {
                    parts.push(format!("..._{}", rest));
                }
                Ok(format!("{{ {} }}", parts.join(", ")))
            }
        }
    }

    /// The values a pattern of a component parameter takes.
    ///
    /// A pattern reads the named props of the component, so `component
    /// Card({ title })` uses the prop `title`.
    fn prop_bindings(
        &mut self,
        pattern: &BindingPattern,
    ) -> Result<Vec<(String, String)>, VeyError> {
        match pattern {
            BindingPattern::Name { name, default, .. } => {
                let expression = format!("__veyProps[{}]", js_string(name));
                let expression = self.with_default(&expression, default.as_ref())?;
                Ok(vec![(name.clone(), expression)])
            }
            // A pattern that takes its value apart gets the value that is
            // given in order, and the names when there is none.
            BindingPattern::Object { .. } => self.pattern_bindings(
                pattern,
                "(__veyValues.length > 0 ? __veyValues[0] : __veyProps)",
            ),
            BindingPattern::Array { .. } => self.pattern_bindings(pattern, "__veyValues"),
        }
    }

    /// The bindings of a pattern that takes its value from an expression.
    fn pattern_bindings(
        &mut self,
        pattern: &BindingPattern,
        source: &str,
    ) -> Result<Vec<(String, String)>, VeyError> {
        match pattern {
            BindingPattern::Name { name, default, .. } => {
                let expression = self.with_default(source, default.as_ref())?;
                Ok(vec![(name.clone(), expression)])
            }
            BindingPattern::Object { fields, rest, .. } => {
                let mut out = Vec::new();
                for field in fields {
                    let expression = format!("{}[{}]", source, js_string(&field.key));
                    for (name, expression) in self.pattern_bindings(&field.pattern, &expression)? {
                        out.push((name, expression));
                    }
                }
                if let Some(rest) = rest {
                    let keys: Vec<String> =
                        fields.iter().map(|field| js_string(&field.key)).collect();
                    out.push((
                        rest.clone(),
                        format!("V.omit({}, [{}])", source, keys.join(", ")),
                    ));
                }
                Ok(out)
            }
            BindingPattern::Array { items, rest, .. } => {
                let mut out = Vec::new();
                for (index, item) in items.iter().enumerate() {
                    let expression = format!("{}[{}]", source, index);
                    for (name, expression) in self.pattern_bindings(item, &expression)? {
                        out.push((name, expression));
                    }
                }
                if let Some(rest) = rest {
                    out.push((rest.clone(), format!("{}.slice({})", source, items.len())));
                }
                Ok(out)
            }
        }
    }

    /// The value of an expression, with the value that is used when there is
    /// none.
    fn with_default(&mut self, source: &str, default: Option<&Expr>) -> Result<String, VeyError> {
        match default {
            Some(default) => Ok(format!(
                "{} !== undefined ? {} : {}",
                source,
                source,
                self.gen_expr(default)?
            )),
            None => Ok(source.to_string()),
        }
    }

    /// The name of an enum at runtime.
    ///
    /// The name has the number of the module in front of it, so that two
    /// enums of the same name in two modules do not mix. The name that is
    /// shown to the reader is the name the enum was given.
    fn enum_runtime_name(&self, name: &str) -> String {
        match name.split_once('.') {
            Some((alias, plain)) => {
                let module = self
                    .deps
                    .iter()
                    .find(|(other, _)| other == alias)
                    .map(|(_, module)| module.clone())
                    .unwrap_or_else(|| alias.to_string());
                format!("{}.{}", module, plain)
            }
            None => format!("__m{}.{}", self.source, name),
        }
    }

    fn gen_params(&mut self, params: &[Param]) -> Result<String, VeyError> {
        let mut parts = Vec::new();
        for param in params {
            // A parameter that takes the argument apart becomes a pattern.
            let pattern = self.gen_binding_pattern(&param.pattern)?;
            // The names of the parameters are known before the values of
            // them, so that a value may name the parameter in front of it.
            for (name, _) in param.bindings() {
                self.declare(name);
            }
            if param.rest {
                parts.push(format!("...{}", pattern));
                continue;
            }
            match &param.default {
                Some(default) => parts.push(format!("{} = {}", pattern, self.gen_expr(default)?)),
                None => parts.push(pattern),
            }
        }
        Ok(parts.join(", "))
    }
}

/// Whether a number is below zero.
///
/// A loop that counts down is written the other way round.
/// The value of a number, when it is one that is written.
fn number_value(expr: &Expr) -> Option<f64> {
    match expr {
        Expr::Number(value, _) => Some(*value),
        // The minus is in front of the number, so the number is above zero.
        Expr::Unary(op, inner, _) if op == "-" => match &**inner {
            Expr::Number(value, _) => Some(-*value),
            _ => None,
        },
        _ => None,
    }
}

fn is_negative(expr: &Expr) -> bool {
    match expr {
        Expr::Number(value, _) => *value < 0.0,
        // The minus is in front of the number, so the number is above zero.
        Expr::Unary(op, inner, _) if op == "-" => match &**inner {
            Expr::Number(value, _) => *value > 0.0,
            _ => false,
        },
        _ => false,
    }
}
