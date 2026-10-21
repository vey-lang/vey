use std::collections::HashSet;

use crate::ast::*;
use crate::builtins::BUILTINS;
use crate::parser::is_ident;
use crate::VeyError;

const JS_RESERVED: &[&str] = &[
    "let",
    "const",
    "var",
    "function",
    "class",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "new",
    "delete",
    "typeof",
    "instanceof",
    "in",
    "of",
    "this",
    "super",
    "yield",
    "await",
    "async",
    "static",
    "import",
    "export",
    "extends",
    "try",
    "catch",
    "finally",
    "throw",
    "void",
    "with",
    "debugger",
];

const EVENT_ATTRS: &[(&str, &str)] = &[
    ("click", "onclick"),
    ("dblclick", "ondblclick"),
    ("change", "onchange"),
    ("input", "oninput"),
    ("submit", "onsubmit"),
    ("load", "onload"),
    ("focus", "onfocus"),
    ("blur", "onblur"),
    ("keydown", "onkeydown"),
    ("keyup", "onkeyup"),
    ("mouseover", "onmouseover"),
    ("mouseout", "onmouseout"),
];

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
                    self.declare(&param.name);
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
                    self.declare(&param.name);
                    let key = js_string(&param.name);
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
                    self.emit(&format!("const _{} = {};", param.name, value));
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

    fn gen_params(&mut self, params: &[Param]) -> Result<String, VeyError> {
        let mut parts = Vec::new();
        for param in params {
            if param.rest {
                parts.push(format!("..._{}", param.name));
                continue;
            }
            match &param.default {
                Some(default) => {
                    parts.push(format!("_{} = {}", param.name, self.gen_expr(default)?))
                }
                None => parts.push(format!("_{}", param.name)),
            }
        }
        Ok(parts.join(", "))
    }

    fn gen_expr(&mut self, expr: &Expr) -> Result<String, VeyError> {
        match expr {
            Expr::Number(value, _) => Ok(format_number(*value)),
            Expr::String(parts, _) => Ok(self.gen_string(parts)?),
            Expr::Bool(value, _) => Ok(if *value { "true" } else { "false" }.to_string()),
            Expr::Null(_) => Ok("null".to_string()),
            Expr::Ident(name, _) => Ok(self.resolve(name)),
            Expr::Array(items, _) => {
                let mut parts = Vec::new();
                for item in items {
                    parts.push(self.gen_expr(item)?);
                }
                Ok(format!("[{}]", parts.join(", ")))
            }
            Expr::Object(props, _) => {
                let mut parts = Vec::new();
                for prop in props {
                    match prop {
                        ObjectProp::Field(key, value) => {
                            parts.push(format!("{}: {}", js_key(key), self.gen_expr(value)?));
                        }
                        ObjectProp::Spread(value) => {
                            parts.push(format!("...{}", self.gen_expr(value)?));
                        }
                    }
                }
                Ok(format!("{{ {} }}", parts.join(", ")))
            }
            Expr::Unary(op, arg, _) => Ok(format!("({}{})", op, self.gen_expr(arg)?)),
            Expr::Binary(op, left, right, _) => Ok(format!(
                "({} {} {})",
                self.gen_expr(left)?,
                op,
                self.gen_expr(right)?
            )),
            Expr::Logical(op, left, right, _) => Ok(format!(
                "({} {} {})",
                self.gen_expr(left)?,
                op,
                self.gen_expr(right)?
            )),
            Expr::Nullish(left, right, _) => Ok(format!(
                "({} ?? {})",
                self.gen_expr(left)?,
                self.gen_expr(right)?
            )),
            Expr::Ternary(test, cons, alt, _) => Ok(format!(
                "({} ? {} : {})",
                self.gen_expr(test)?,
                self.gen_expr(cons)?,
                self.gen_expr(alt)?
            )),
            Expr::Call(callee, args, optional, _) => {
                let callee = self.gen_expr(callee)?;
                let mut parts = Vec::new();
                for arg in args {
                    parts.push(self.gen_expr(arg)?);
                }
                if *optional {
                    Ok(format!("{}?.({})", callee, parts.join(", ")))
                } else {
                    Ok(format!("{}({})", callee, parts.join(", ")))
                }
            }
            // The type arguments are only used by the checker and are erased.
            Expr::CallGeneric(callee, _, args, optional, _) => {
                self.gen_expr(&Expr::Call(callee.clone(), args.clone(), *optional, expr.pos()))
            }
            Expr::Member(object, prop, optional, _) => {
                let object = self.gen_expr(object)?;
                match prop {
                    MemberProp::Name(name) => {
                        if *optional {
                            Ok(format!("({})?.{}", object, name))
                        } else {
                            Ok(format!("({}).{}", object, name))
                        }
                    }
                    MemberProp::Computed(index) => {
                        let index = self.gen_expr(index)?;
                        if *optional {
                            Ok(format!("({})?.[{}]", object, index))
                        } else {
                            Ok(format!("({})[{}]", object, index))
                        }
                    }
                }
            }
            Expr::Fn(params, body, _, _) => self.gen_fn_expr(params, body),
            Expr::Arrow(params, body, _) => self.gen_arrow(params, body),
            Expr::Range(start, end, _) => Ok(format!(
                "V.range({}, {})",
                self.gen_expr(start)?,
                self.gen_expr(end)?
            )),
            Expr::Match(subject, arms, _) => self.gen_match(subject, arms),
            Expr::Await(arg, _) => Ok(format!("(await {})", self.gen_expr(arg)?)),
            Expr::Spread(arg, _) => Ok(format!("...{}", self.gen_expr(arg)?)),
        }
    }

    fn gen_string(&mut self, parts: &[StrPart]) -> Result<String, VeyError> {
        if parts.len() == 1 && parts[0].expr.is_none() {
            return Ok(js_string(&parts[0].text));
        }
        let mut chunks = Vec::new();
        for part in parts {
            match &part.expr {
                Some(expr) => chunks.push(format!("({})", self.gen_expr(expr)?)),
                None => chunks.push(js_string(&part.text)),
            }
        }
        Ok(format!("({})", chunks.join(" + ")))
    }

    fn gen_fn_expr(&mut self, params: &[Param], body: &Block) -> Result<String, VeyError> {
        let params_text = self.gen_params(params)?;
        let async_kw = if block_has_await(body) { "async " } else { "" };
        let captured = self.capture(|codegen| {
            codegen.push_scope();
            for param in params {
                codegen.declare(&param.name);
            }
            for stmt in &body.body {
                codegen.gen_stmt(stmt)?;
            }
            codegen.pop_scope();
            Ok(())
        })?;
        let pad = "  ".repeat(self.indent);
        Ok(format!(
            "({}function({}) {{\n{}\n{}}})",
            async_kw,
            params_text,
            captured.trim_end_matches('\n'),
            pad
        ))
    }

    fn gen_arrow(&mut self, params: &[Param], body: &ArrowBody) -> Result<String, VeyError> {
        let param_text = self.gen_params(params)?;
        match body {
            ArrowBody::Expr(expr) => {
                let value = self.gen_expr(expr)?;
                let async_kw = if expr_has_await(expr) { "async " } else { "" };
                Ok(format!("({}({}) => {})", async_kw, param_text, value))
            }
            ArrowBody::Block(block) => {
                let async_kw = if block_has_await(block) { "async " } else { "" };
                let captured = self.capture(|codegen| {
                    codegen.push_scope();
                    for param in params {
                        codegen.declare(&param.name);
                    }
                    for stmt in &block.body {
                        codegen.gen_stmt(stmt)?;
                    }
                    codegen.pop_scope();
                    Ok(())
                })?;
                let pad = "  ".repeat(self.indent);
                Ok(format!(
                    "({}({}) => {{\n{}\n{}}})",
                    async_kw,
                    param_text,
                    captured.trim_end_matches('\n'),
                    pad
                ))
            }
        }
    }

    fn gen_match(&mut self, subject: &Expr, arms: &[MatchArm]) -> Result<String, VeyError> {
        let subject = self.gen_expr(subject)?;
        let temp = format!("__match{}", self.temp);
        self.temp += 1;
        let captured =
            self.capture(|codegen| {
                let mut has_default = false;
                for arm in arms {
                    if arm.patterns.iter().any(|p| matches!(p, Pattern::Wildcard)) {
                        let body = codegen.gen_expr(&arm.body)?;
                        codegen.emit(&format!("return {};", body));
                        has_default = true;
                        break;
                    }
                    // determine binding names for this arm
                    let mut binding_names: Vec<String> = Vec::new();
                    for pattern in &arm.patterns {
                        if let Pattern::Enum { bindings, .. } = pattern {
                            for b in bindings {
                                if !binding_names.contains(&b.name) {
                                    binding_names.push(b.name.clone());
                                }
                            }
                        }
                    }
                    // generate body in a scope with bindings declared
                    codegen.push_scope();
                    for name in &binding_names {
                        codegen.declare(name);
                    }
                    let body = codegen.gen_expr(&arm.body)?;
                    codegen.pop_scope();

                    // check if any pattern has bindings -> per-pattern blocks
                    let has_bindings = arm.patterns.iter().any(
                        |p| matches!(p, Pattern::Enum { bindings, .. } if !bindings.is_empty()),
                    );

                    if has_bindings {
                        for pattern in &arm.patterns {
                            match pattern {
                                Pattern::Enum {
                                    enum_name,
                                    variant,
                                    bindings,
                                    ..
                                } => {
                                    let cond = format!(
                                    "{} != null && {}.__veyEnum === {} && {}.__veyVariant === {}",
                                    temp, temp, js_string(enum_name), temp, js_string(variant)
                                );
                                    codegen.emit(&format!("if ({}) {{", cond));
                                    codegen.indent += 1;
                                    // resolve variant field names from registry
                                    let variant_fields = codegen
                                        .enums
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant))
                                        .cloned()
                                        .unwrap_or_default();
                                    let mut positional_idx = 0usize;
                                    for b in bindings {
                                        let field = if let Some(f) = &b.field {
                                            f.clone()
                                        } else {
                                            let idx = positional_idx;
                                            positional_idx += 1;
                                            variant_fields
                                                .get(idx)
                                                .cloned()
                                                .unwrap_or_else(|| b.name.clone())
                                        };
                                        codegen.emit(&format!(
                                            "const _{} = {}.{};",
                                            b.name, temp, field
                                        ));
                                    }
                                    codegen.emit(&format!("return {};", body));
                                    codegen.indent -= 1;
                                    codegen.emit("}");
                                }
                                Pattern::Literal(expr) => {
                                    let cond = format!("{} === {}", temp, codegen.gen_expr(expr)?);
                                    codegen.emit(&format!("if ({}) return {};", cond, body));
                                }
                                Pattern::Range(start, end) => {
                                    let cond = format!(
                                        "({} >= {} && {} < {})",
                                        temp,
                                        codegen.gen_expr(start)?,
                                        temp,
                                        codegen.gen_expr(end)?
                                    );
                                    codegen.emit(&format!("if ({}) return {};", cond, body));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        let mut conditions = Vec::new();
                        for pattern in &arm.patterns {
                            match pattern {
                                Pattern::Literal(expr) => conditions.push(format!(
                                    "{} === {}",
                                    temp,
                                    codegen.gen_expr(expr)?
                                )),
                                Pattern::Range(start, end) => conditions.push(format!(
                                    "({} >= {} && {} < {})",
                                    temp,
                                    codegen.gen_expr(start)?,
                                    temp,
                                    codegen.gen_expr(end)?
                                )),
                                Pattern::Enum {
                                    enum_name, variant, ..
                                } => {
                                    conditions.push(format!(
                                    "{} != null && {}.__veyEnum === {} && {}.__veyVariant === {}",
                                    temp, temp, js_string(enum_name), temp, js_string(variant)
                                ));
                                }
                                _ => {}
                            }
                        }
                        codegen.emit(&format!(
                            "if ({}) return {};",
                            conditions.join(" || "),
                            body
                        ));
                    }
                }
                if !has_default {
                    codegen.emit("return undefined;");
                }
                Ok(())
            })?;
        let pad = "  ".repeat(self.indent);
        // An `await` inside an arm needs an async arrow, otherwise the
        // generated JavaScript is a syntax error.
        let async_kw = if arms.iter().any(|arm| expr_has_await(&arm.body)) {
            "async "
        } else {
            ""
        };
        Ok(format!(
            "({}({}) => {{\n{}\n{}}})({})",
            async_kw,
            temp,
            captured.trim_end_matches('\n'),
            pad,
            subject
        ))
    }

    fn gen_markup_nodes(&mut self, nodes: &[MarkupNode]) -> Result<String, VeyError> {
        let mut parts = Vec::new();
        for node in nodes {
            parts.push(self.gen_markup_node(node)?);
        }
        Ok(format!("[{}]", parts.join(", ")))
    }

    fn gen_markup_node(&mut self, node: &MarkupNode) -> Result<String, VeyError> {
        match node {
            MarkupNode::Expr(expr) => self.gen_expr(expr),
            MarkupNode::Element(element) => self.gen_element(element),
            MarkupNode::If(branch) => {
                let test = self.gen_expr(&branch.test)?;
                let then_nodes = self.gen_markup_nodes(&branch.then_nodes)?;
                let else_nodes = if branch.else_nodes.is_empty() {
                    "null".to_string()
                } else {
                    self.gen_markup_nodes(&branch.else_nodes)?
                };
                Ok(format!("({} ? {} : {})", test, then_nodes, else_nodes))
            }
            MarkupNode::For(loop_node) => {
                let iter = self.gen_expr(&loop_node.iter)?;
                let body = self.gen_markup_nodes(&loop_node.body)?;
                Ok(format!(
                    "V.flatMap({}, function(_{}) {{ return {}; }})",
                    iter, loop_node.name, body
                ))
            }
        }
    }

    fn gen_element(&mut self, element: &Element) -> Result<String, VeyError> {
        let tag = if element.component {
            self.resolve(&element.tag)
        } else {
            js_string(&element.tag)
        };

        let mut entries: Vec<String> = Vec::new();
        let mut class_index = None;
        let mut has_id = false;

        for (name, value) in &element.attrs {
            let mapped = event_attr(name);
            if mapped == "class" {
                class_index = Some(entries.len());
            }
            if mapped == "id" {
                has_id = true;
            }
            entries.push(format!("{}: {}", js_key(mapped), self.gen_expr(value)?));
        }

        if !element.classes.is_empty() {
            let classes = js_string(&element.classes.join(" "));
            if let Some(index) = class_index {
                let value = entries[index]
                    .split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default();
                entries[index] = format!("class: V.cls({}, {})", classes, value);
            } else {
                entries.push(format!("class: {}", classes));
            }
        }

        if let Some(id) = &element.id {
            if !has_id {
                entries.push(format!("id: {}", js_string(id)));
            }
        }

        let props = if entries.is_empty() {
            "null".to_string()
        } else {
            format!("{{ {} }}", entries.join(", "))
        };

        let children = self.gen_markup_nodes(&element.children)?;
        if element.component && !element.args.is_empty() {
            let mut args = Vec::new();
            for arg in &element.args {
                args.push(self.gen_expr(arg)?);
            }
            return Ok(format!(
                "V.h({}, {}, {}, [{}])",
                tag,
                props,
                children,
                args.join(", ")
            ));
        }
        Ok(format!("V.h({}, {}, {})", tag, props, children))
    }
}

fn event_attr(name: &str) -> &str {
    for (from, to) in EVENT_ATTRS {
        if *from == name {
            return to;
        }
    }
    name
}

fn format_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 && value.abs() <= 9_007_199_254_740_991.0 {
        format!("{}", value as i64)
    } else {
        format!("{}", value)
    }
}

fn js_string(value: &str) -> String {
    let mut out = String::from("\"");
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if (other as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", other as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn js_key(key: &str) -> String {
    if is_ident(key) && !JS_RESERVED.contains(&key) {
        key.to_string()
    } else {
        js_string(key)
    }
}

fn build_enum_map(
    program: &Program,
) -> std::collections::HashMap<String, std::collections::HashMap<String, Vec<String>>> {
    let mut out = std::collections::HashMap::new();
    for stmt in &program.body {
        if let Stmt::Enum(decl) = stmt {
            let mut variants = std::collections::HashMap::new();
            for v in &decl.variants {
                variants.insert(
                    v.name.clone(),
                    v.fields.iter().map(|f| f.name.clone()).collect(),
                );
            }
            out.insert(decl.name.clone(), variants);
        }
    }
    out
}

fn block_has_await(block: &Block) -> bool {
    block.body.iter().any(stmt_has_await)
}

fn stmt_has_await(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Var(decl) => decl.init.as_ref().map(expr_has_await).unwrap_or(false),
        Stmt::Fn(_) | Stmt::Component(_) => false,
        Stmt::If(decl) => {
            expr_has_await(&decl.test)
                || block_has_await(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_await(s))
                    .unwrap_or(false)
        }
        Stmt::While(decl) => expr_has_await(&decl.test) || block_has_await(&decl.body),
        Stmt::For(decl) => expr_has_await(&decl.iter) || block_has_await(&decl.body),
        Stmt::Repeat(decl) => expr_has_await(&decl.count) || block_has_await(&decl.body),
        Stmt::Return(decl) => decl.value.as_ref().map(expr_has_await).unwrap_or(false),
        Stmt::Break(_)
        | Stmt::Continue(_)
        | Stmt::Use(_)
        | Stmt::Style(_)
        | Stmt::Type(_)
        | Stmt::Enum(_) => false,
        Stmt::Try(decl) => block_has_await(&decl.body) || block_has_await(&decl.catch),
        Stmt::Expr(decl) => expr_has_await(&decl.expr),
        Stmt::Assign(decl) => expr_has_await(&decl.target) || expr_has_await(&decl.value),
        Stmt::Block(block, _) => block_has_await(block),
    }
}

fn expr_has_await(expr: &Expr) -> bool {
    match expr {
        Expr::Fn(..) | Expr::Arrow(_, _, _) => false,
        Expr::Number(_, _) | Expr::Bool(_, _) | Expr::Null(_) | Expr::Ident(_, _) => false,
        Expr::String(parts, _) => parts
            .iter()
            .any(|part| part.expr.as_ref().map(expr_has_await).unwrap_or(false)),
        Expr::Array(items, _) => items.iter().any(expr_has_await),
        Expr::Object(props, _) => props.iter().any(|prop| match prop {
            ObjectProp::Field(_, value) => expr_has_await(value),
            ObjectProp::Spread(value) => expr_has_await(value),
        }),
        Expr::Unary(_, arg, _) => expr_has_await(arg),
        Expr::Binary(_, left, right, _)
        | Expr::Logical(_, left, right, _)
        | Expr::Nullish(left, right, _)
        | Expr::Range(left, right, _) => expr_has_await(left) || expr_has_await(right),
        Expr::Ternary(test, cons, alt, _) => {
            expr_has_await(test) || expr_has_await(cons) || expr_has_await(alt)
        }
        Expr::Call(callee, args, _, _) => expr_has_await(callee) || args.iter().any(expr_has_await),
        Expr::CallGeneric(callee, _, args, _, _) => {
            expr_has_await(callee) || args.iter().any(expr_has_await)
        }
        Expr::Member(object, prop, _, _) => {
            expr_has_await(object)
                || match prop {
                    MemberProp::Computed(index) => expr_has_await(index),
                    MemberProp::Name(_) => false,
                }
        }
        Expr::Match(subject, arms, _) => {
            expr_has_await(subject) || arms.iter().any(|arm| expr_has_await(&arm.body))
        }
        Expr::Await(_, _) => true,
        Expr::Spread(arg, _) => expr_has_await(arg),
    }
}
