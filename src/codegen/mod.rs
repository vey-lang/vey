use std::collections::HashSet;

mod awaits;
mod exprs;
mod js;
mod markup;

use crate::ast::*;
use crate::builtins::BUILTINS;
use crate::VeyError;

pub(crate) use awaits::block_has_await;
pub(crate) use js::{build_enum_map, js_key, js_string};

pub struct Options {
    pub file: String,
    pub module: bool,
    pub deps: Vec<(String, String)>,
    pub exports: Vec<String>,
}

impl Options {
    pub fn new(file: &str) -> Self {
        Self {
            file: file.to_string(),
            module: false,
            deps: Vec::new(),
            exports: Vec::new(),
        }
    }
}

pub struct Generated {
    pub code: String,
    pub lines: Vec<Option<usize>>,
}

pub fn generate(program: &Program, opts: &Options) -> Result<Generated, VeyError> {
    let mut codegen = Codegen {
        file: opts.file.clone(),
        out: String::new(),
        lines: Vec::new(),
        current_line: 1,
        indent: 0,
        scopes: vec![HashSet::new()],
        temp: 0,
        deps: opts.deps.clone(),
        module: opts.module,
        exports: opts.exports.clone(),
        enums: build_enum_map(program),
    };
    codegen.run(program)?;
    Ok(Generated {
        code: codegen.out,
        lines: codegen.lines,
    })
}

struct Codegen {
    file: String,
    out: String,
    lines: Vec<Option<usize>>,
    current_line: usize,
    indent: usize,
    scopes: Vec<HashSet<String>>,
    temp: usize,
    deps: Vec<(String, String)>,
    module: bool,
    exports: Vec<String>,
    enums: std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>>,
}

impl Codegen {
    fn err(&self, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file.clone(), 1, 1)
    }

    fn emit(&mut self, text: &str) {
        let pad = "  ".repeat(self.indent);
        for (i, line) in text.split('\n').enumerate() {
            if i > 0 {
                self.out.push('\n');
            } else {
                self.out.push_str(&pad);
            }
            self.out.push_str(line);
            self.lines.push(Some(self.current_line));
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
        for stmt in &program.body {
            if matches!(stmt, Stmt::Use(_)) {
                continue;
            }
            self.gen_stmt(stmt)?;
        }
        if self.module {
            for name in self.exports.clone() {
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

    fn capture<F>(&mut self, body: F) -> Result<String, VeyError>
    where
        F: FnOnce(&mut Self) -> Result<(), VeyError>,
    {
        let outer = std::mem::take(&mut self.out);
        let outer_lines = std::mem::take(&mut self.lines);
        let outer_indent = self.indent;
        self.indent += 1;
        let result = body(self);
        let captured = std::mem::replace(&mut self.out, outer);
        self.lines = outer_lines;
        self.indent = outer_indent;
        result?;
        Ok(captured)
    }

    fn gen_stmt(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
        self.current_line = stmt.pos().line;
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
                let pattern = self.gen_binding_pattern(&decl.pattern);
                self.emit(&format!("{} {}{};", keyword, pattern, init));
                for (name, _) in decl.pattern.bindings() {
                    self.declare(name);
                }
            }
            Stmt::Fn(decl) => {
                self.declare(&decl.name);
                let params = self.gen_params(&decl.params)?;
                let async_kw = if block_has_await(&decl.body) {
                    "async "
                } else {
                    ""
                };
                self.emit(&format!(
                    "{}function _{} ({}) {{",
                    async_kw, decl.name, params
                ));
                self.indent += 1;
                self.push_scope();
                for param in &decl.params {
                    for (name, _) in param.bindings() {
                        self.declare(name);
                    }
                }
                for inner in &decl.body.body {
                    self.gen_stmt(inner)?;
                }
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
                            "{}: V.enumValue({}, {})",
                            js_key(&variant.name),
                            js_string(&decl.name),
                            js_string(&variant.name)
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
                            js_string(&decl.name),
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
                self.emit(&format!(
                    "function _{} (__veyFirst, ...__veyRest) {{",
                    decl.name
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
                            for (name, expression) in self.prop_bindings(pattern) {
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
            Stmt::While(decl) => {
                let test = self.gen_expr(&decl.test)?;
                self.emit(&format!("while ({}) {{", test));
                self.indent += 1;
                self.gen_block(&decl.body)?;
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::For(decl) => {
                let iter = self.gen_expr(&decl.iter)?;
                let pattern = self.gen_binding_pattern(&decl.pattern);
                self.emit(&format!("for (let {} of {}) {{", pattern, iter));
                self.indent += 1;
                self.push_scope();
                for (name, _) in decl.pattern.bindings() {
                    self.declare(name);
                }
                for inner in &decl.body.body {
                    self.gen_stmt(inner)?;
                }
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
                self.emit(&format!("const {} = ({});", limit, count));
                self.emit(&format!(
                    "for (let {} = 0; {} < {}; {}++) {{",
                    index, index, limit, index
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
            Stmt::Break(_) => self.emit("break;"),
            Stmt::Continue(_) => self.emit("continue;"),
            Stmt::Try(decl) => {
                self.emit("try {");
                self.indent += 1;
                self.gen_block(&decl.body)?;
                self.indent -= 1;
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
                for inner in &decl.catch.body {
                    self.gen_stmt(inner)?;
                }
                self.pop_scope();
                self.indent -= 1;
                self.emit("}");
            }
            Stmt::Type(_) => {}
            Stmt::Style(_) => {}
            Stmt::Use(decl) => {
                if decl.path.ends_with(".css") {
                    return Ok(());
                }
                let target = self
                    .deps
                    .iter()
                    .find(|(alias, _)| alias == &decl.alias)
                    .map(|(_, module)| module.clone())
                    .ok_or_else(|| self.err(format!("module '{}' was not resolved", decl.path)))?;
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
        for stmt in &block.body {
            self.gen_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn gen_binding_pattern(&self, pattern: &BindingPattern) -> String {
        match pattern {
            BindingPattern::Name { name, .. } => format!("_{}", name),
            BindingPattern::Array { items, rest, .. } => {
                let mut parts: Vec<String> = items
                    .iter()
                    .map(|item| self.gen_binding_pattern(item))
                    .collect();
                if let Some(rest) = rest {
                    parts.push(format!("..._{}", rest));
                }
                format!("[{}]", parts.join(", "))
            }
            BindingPattern::Object { fields, rest, .. } => {
                let mut parts: Vec<String> = fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{}: {}",
                            js_key(&field.key),
                            self.gen_binding_pattern(&field.pattern)
                        )
                    })
                    .collect();
                if let Some(rest) = rest {
                    parts.push(format!("..._{}", rest));
                }
                format!("{{ {} }}", parts.join(", "))
            }
        }
    }

    /// The values a pattern of a component parameter takes.
    ///
    /// A pattern reads the named props of the component, so `component
    /// Card({ title })` uses the prop `title`.
    fn prop_bindings(&self, pattern: &BindingPattern) -> Vec<(String, String)> {
        let mut out = Vec::new();
        match pattern {
            BindingPattern::Name { name, .. } => {
                out.push((name.clone(), format!("__veyProps[{}]", js_string(name))));
            }
            BindingPattern::Object { fields, rest, .. } => {
                for field in fields {
                    let expression = format!("__veyProps[{}]", js_string(&field.key));
                    for (name, _) in field.pattern.bindings() {
                        out.push((name.to_string(), expression.clone()));
                    }
                }
                if let Some(rest) = rest {
                    let keys: Vec<String> =
                        fields.iter().map(|field| js_string(&field.key)).collect();
                    out.push((
                        rest.clone(),
                        format!("V.omit(__veyProps, [{}])", keys.join(", ")),
                    ));
                }
            }
            BindingPattern::Array { items, rest, .. } => {
                for (index, item) in items.iter().enumerate() {
                    let expression = format!("__veyValues[{}]", index);
                    for (name, _) in item.bindings() {
                        out.push((name.to_string(), expression.clone()));
                    }
                }
                if let Some(rest) = rest {
                    out.push((rest.clone(), format!("__veyValues.slice({})", items.len())));
                }
            }
        }
        out
    }

    fn gen_params(&mut self, params: &[Param]) -> Result<String, VeyError> {
        let mut parts = Vec::new();
        for param in params {
            // A parameter that takes the argument apart becomes a pattern.
            let pattern = self.gen_binding_pattern(&param.pattern);
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
