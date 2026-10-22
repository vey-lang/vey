mod scopes;

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::builtins::is_builtin;
use crate::check::scopes::{BindingKind, Scopes};
use crate::lexer::Pos;
use crate::parser::html::is_html_tag;
use crate::suggest::did_you_mean;
use crate::types::helpers::block_always_exits;
use crate::warn::Warning;
use crate::VeyError;

/// Check a program and collect the warnings about it.
pub fn check(program: &Program, file: &str) -> Result<Vec<Warning>, VeyError> {
    let mut checker = Checker {
        file,
        scopes: Scopes::new(),
        loop_depth: 0,
        function_depth: 0,
        declared_at: HashMap::new(),
        statement: 0,
        exported: HashSet::new(),
        loop_labels: Vec::new(),
        warnings: Vec::new(),
    };
    checker.check_program(program)?;
    checker.pop_scope();
    Ok(checker.warnings)
}

struct Checker<'a> {
    file: &'a str,
    scopes: Scopes,
    loop_depth: usize,
    function_depth: usize,
    /// The statement a value of the top level is declared in.
    declared_at: HashMap<String, usize>,
    /// The statement that is being checked.
    statement: usize,
    /// The names the program gives to the outside world.
    exported: HashSet<String>,
    /// The names of the loops that are around.
    loop_labels: Vec<Vec<String>>,
    warnings: Vec<Warning>,
}

impl Checker<'_> {
    fn error(&self, pos: Pos, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file, pos.line, pos.col)
    }

    /// A warning about the program.
    fn warn(&mut self, warning: Warning) {
        self.warnings.push(warning);
    }

    fn declare(&mut self, name: &str, kind: BindingKind, pos: Pos) -> Result<(), VeyError> {
        // A name that hides another one is allowed, but the reader of the
        // program should know about it.
        if let Some(previous) = self.scopes.shadowed(name) {
            self.warn(Warning::new(
                format!("'{}' hides the name of line {}", name, previous.pos.line),
                self.file,
                pos.line,
                pos.col,
            ));
        }
        // A name that hides a helper of the runtime takes the helper away
        // for the whole scope, so the reader should know about it. Only the
        // names of the top level are said out loud: a parameter of a
        // function hides the helper only inside that function.
        if is_builtin(name) && self.scopes.depth() == 1 && !self.scopes.resolve(name).is_some() {
            self.warn(Warning::new(
                format!("'{}' hides a helper of the runtime", name),
                self.file,
                pos.line,
                pos.col,
            ));
        }
        let file = self.file;
        self.scopes.declare(name, kind, pos, file)
    }

    /// Leave the scope that is being checked, and say which of its names were
    /// never read.
    fn pop_scope(&mut self) {
        for (name, binding) in self.scopes.take_scope() {
            if binding.used.get() {
                continue;
            }
            // A name that begins with an underscore asks to be left alone.
            if name.starts_with('_') {
                continue;
            }
            // A function or a component may be used by the markup or by
            // another module.
            if matches!(binding.kind, BindingKind::Function | BindingKind::Component) {
                continue;
            }
            if self.exported.contains(&name) {
                continue;
            }
            let message = match binding.kind {
                BindingKind::Import => format!("the import '{}' is never used", name),
                BindingKind::Param => format!("the parameter '{}' is never used", name),
                _ => format!("'{}' is never used", name),
            };
            self.warn(Warning::new(
                message,
                self.file,
                binding.pos.line,
                binding.pos.col,
            ));
        }
    }

    fn declare_pattern(
        &mut self,
        pattern: &BindingPattern,
        kind: BindingKind,
    ) -> Result<(), VeyError> {
        match pattern {
            BindingPattern::Name { name, pos, default } => {
                // The value that is used when there is none is an expression
                // of its own.
                if let Some(default) = default {
                    self.check_expr(default)?;
                }
                self.declare(name, kind, *pos)
            }
            BindingPattern::Array { items, rest, pos } => {
                for item in items {
                    self.declare_pattern(item, kind)?;
                }
                if let Some(rest) = rest {
                    self.declare(rest, kind, *pos)?;
                }
                Ok(())
            }
            BindingPattern::Object { fields, rest, pos } => {
                for field in fields {
                    self.declare_pattern(&field.pattern, kind)?;
                }
                if let Some(rest) = rest {
                    self.declare(rest, kind, *pos)?;
                }
                Ok(())
            }
        }
    }

    /// The name, and a note that it was read.
    fn read(&self, name: &str) -> Option<std::rc::Rc<scopes::Binding>> {
        self.scopes.read(name)
    }

    /// The name, and a note that it was read only when it is read.
    fn read_or_write(&self, name: &str, reads: bool) -> Option<std::rc::Rc<scopes::Binding>> {
        if reads {
            self.scopes.read(name)
        } else {
            self.scopes.write(name)
        }
    }

    /// The names that are in scope, used to suggest a similar one.
    fn known_names(&self) -> Vec<&str> {
        self.scopes.known_names()
    }

    fn check_program(&mut self, program: &Program) -> Result<(), VeyError> {
        // The types have a name of their own, and two of them may not have
        // the same one.
        let mut type_names: HashMap<&str, Pos> = HashMap::new();
        for stmt in &program.body {
            let name = match stmt {
                Stmt::Type(decl) => Some(decl.name.as_str()),
                Stmt::Enum(decl) => Some(decl.name.as_str()),
                Stmt::Interface(decl) => Some(decl.name.as_str()),
                _ => None,
            };
            let Some(name) = name else {
                continue;
            };
            if let Some(previous) = type_names.insert(name, stmt.pos()) {
                return Err(self.error(
                    stmt.pos(),
                    format!(
                        "duplicate type '{}' (first declared on line {})",
                        name, previous.line
                    ),
                ));
            }
        }
        // The names the program gives to the outside world are used
        // elsewhere, so they are not reported as unused.
        for stmt in &program.body {
            if let Some((name, exported)) = stmt.exported_name() {
                if exported {
                    self.exported.insert(name.to_string());
                }
            }
        }
        // The constructor of a type is public when the type is.
        let exported_types: Vec<&str> = program
            .body
            .iter()
            .filter_map(|stmt| match stmt {
                Stmt::Type(decl) if decl.exported => Some(decl.name.as_str()),
                Stmt::Interface(decl) if decl.exported => Some(decl.name.as_str()),
                _ => None,
            })
            .collect();
        for stmt in &program.body {
            if let Stmt::Impl(decl) = stmt {
                if exported_types.contains(&decl.type_name.as_str()) {
                    self.exported.insert(decl.type_name.clone());
                }
            }
        }
        // The names of the top level are known before the body is checked, so
        // that a function may be used inside another one. A value is not
        // hoisted, so where it is declared matters: a name of the top level
        // may not be used before it is written.
        for (index, stmt) in program.body.iter().enumerate() {
            match stmt {
                Stmt::Use(decl) => {
                    if !decl.path.ends_with(".css") {
                        self.declare(&decl.alias, BindingKind::Import, decl.pos)?;
                    }
                }
                Stmt::Var(decl) => {
                    self.declare_pattern(&decl.pattern, BindingKind::from_var(decl.kind))?;
                    for (name, _) in decl.pattern.bindings() {
                        self.declared_at.insert(name.to_string(), index);
                    }
                }
                Stmt::Fn(decl) => {
                    // A function is hoisted, so it may be used before it is
                    // written.
                    self.declare(&decl.name, BindingKind::Function, decl.pos)?
                }
                Stmt::Component(decl) => {
                    self.declare(&decl.name, BindingKind::Component, decl.pos)?
                }
                Stmt::Enum(decl) => self.declare(&decl.name, BindingKind::Const, decl.pos)?,
                Stmt::Impl(decl) => {
                    // The block makes a value of the type, so the name of the
                    // type is the name of a value too.
                    self.declare(&decl.type_name, BindingKind::Const, decl.pos)?
                }
                _ => {}
            }
        }

        for (index, stmt) in program.body.iter().enumerate() {
            self.statement = index;
            match stmt {
                Stmt::Use(_) => {}
                Stmt::Var(decl) => {
                    if let Some(init) = &decl.init {
                        self.check_expr(init)?;
                    }
                }
                Stmt::Fn(decl) => self.check_function(&decl.params, &decl.body)?,
                Stmt::Component(decl) => self.check_component(decl)?,
                Stmt::Style(_) => {}
                other => self.check_stmt(other)?,
            }
        }
        Ok(())
    }

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
        match stmt {
            Stmt::Var(decl) => {
                if let Some(init) = &decl.init {
                    self.check_expr(init)?;
                }
                self.declare_pattern(&decl.pattern, BindingKind::from_var(decl.kind))?;
            }
            Stmt::Fn(decl) => {
                self.declare(&decl.name, BindingKind::Function, decl.pos)?;
                self.check_function(&decl.params, &decl.body)?;
            }
            Stmt::Component(decl) => {
                self.declare(&decl.name, BindingKind::Component, decl.pos)?;
                self.check_component(decl)?;
            }
            Stmt::If(decl) => {
                self.check_expr(&decl.test)?;
                self.check_block(&decl.then_block)?;
                if let Some(branch) = &decl.else_branch {
                    self.check_stmt(branch)?;
                }
            }
            Stmt::IfLet(decl) => {
                self.check_expr(&decl.value)?;
                self.scopes.push();
                self.check_pattern_names(&decl.pattern)?;
                self.check_block(&decl.then_block)?;
                self.pop_scope();
                if let Some(branch) = &decl.else_branch {
                    self.check_stmt(branch)?;
                }
            }
            Stmt::While(decl) => {
                self.check_expr(&decl.test)?;
                self.loop_depth += 1;
                self.loop_labels.push(decl.label.iter().cloned().collect());
                self.check_block(&decl.body)?;
                self.loop_labels.pop();
                self.loop_depth -= 1;
            }
            Stmt::WhileLet(decl) => {
                self.check_expr(&decl.value)?;
                self.loop_depth += 1;
                self.scopes.push();
                self.check_pattern_names(&decl.pattern)?;
                for inner in &decl.body.body {
                    self.check_stmt(inner)?;
                }
                self.pop_scope();
                self.loop_depth -= 1;
            }
            Stmt::LetElse(decl) => {
                self.check_expr(&decl.value)?;
                // The name is bound in the scope around it, the way the name
                // of a `let` is.
                self.declare_pattern(&decl.pattern, BindingKind::Let)?;
                // The block has to leave the function, or the value is used
                // even when it is not there.
                if self.function_depth > 0 && !block_always_exits(&decl.otherwise) {
                    return Err(self.error(
                        decl.pos,
                        "the block of a value that has to be there must leave the function",
                    ));
                }
                self.check_block(&decl.otherwise)?;
            }
            Stmt::Defer(decl) => {
                self.check_expr(&decl.value)?;
            }
            Stmt::Throw(decl) => {
                self.check_expr(&decl.value)?;
            }
            Stmt::For(decl) => {
                self.check_expr(&decl.iter)?;
                self.scopes.push();
                self.declare_pattern(&decl.pattern, BindingKind::Let)?;
                self.loop_depth += 1;
                self.loop_labels.push(decl.label.iter().cloned().collect());
                for inner in &decl.body.body {
                    self.check_stmt(inner)?;
                }
                self.loop_labels.pop();
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Stmt::Repeat(decl) => {
                self.check_expr(&decl.count)?;
                self.loop_depth += 1;
                self.loop_labels.push(decl.label.iter().cloned().collect());
                self.check_block(&decl.body)?;
                self.loop_labels.pop();
                self.loop_depth -= 1;
            }
            Stmt::Return(decl) => {
                if self.function_depth == 0 {
                    return Err(self.error(decl.pos, "return outside a function"));
                }
                if let Some(value) = &decl.value {
                    self.check_expr(value)?;
                }
            }
            Stmt::Break(label, pos) => {
                if self.loop_depth == 0 {
                    return Err(self.error(*pos, "break outside a loop"));
                }
                if let Some(label) = label {
                    self.check_label(label, *pos, "break")?;
                }
            }
            Stmt::Continue(label, pos) => {
                if self.loop_depth == 0 {
                    return Err(self.error(*pos, "continue outside a loop"));
                }
                if let Some(label) = label {
                    self.check_label(label, *pos, "continue")?;
                }
            }
            Stmt::Try(decl) => {
                self.check_block(&decl.body)?;
                if let Some(catch) = &decl.catch {
                    self.scopes.push();
                    if let Some(name) = &decl.name {
                        self.declare(name, BindingKind::Let, decl.pos)?;
                    }
                    for inner in &catch.body {
                        self.check_stmt(inner)?;
                    }
                    self.pop_scope();
                }
                if let Some(finally) = &decl.finally {
                    self.check_block(finally)?;
                }
            }
            Stmt::Use(decl) => {
                // A module is used at the top level, because the code of a
                // module is written before the program runs.
                if self.scopes.depth() > 1 {
                    return Err(self.error(
                        decl.pos,
                        "a module may only be used at the top level",
                    ));
                }
                if !decl.path.ends_with(".css") {
                    self.declare(&decl.alias, BindingKind::Import, decl.pos)?;
                }
            }
            Stmt::Style(decl) => {
                return Err(self.error(decl.pos, "style blocks must be at the top level"));
            }
            Stmt::Type(_) => {}
            Stmt::Interface(_) => {}
            Stmt::Impl(decl) => self.check_impl(decl)?,
            Stmt::Enum(_) => {}
            Stmt::Expr(decl) => self.check_expr(&decl.expr)?,
            Stmt::Assign(decl) => {
                // A name that is only written is not used, but a name that
                // is added to is read as well.
                self.check_target(&decl.target, decl.op != "=")?;
                self.check_expr(&decl.value)?;
            }
            Stmt::Block(block, _) => self.check_block(block)?,
        }
        Ok(())
    }

    /// The names of the loops that are around, so that a `break` may name
    /// one of them.
    fn check_label(&self, label: &str, pos: Pos, what: &str) -> Result<(), VeyError> {
        if self.loop_labels.iter().any(|labels| labels.contains(&label.to_string())) {
            return Ok(());
        }
        Err(self.error(
            pos,
            format!("no loop named '{}' is around this '{}'", label, what),
        ))
    }

    fn check_block(&mut self, block: &Block) -> Result<(), VeyError> {
        self.scopes.push();
        self.check_statements(&block.body)?;
        self.pop_scope();
        Ok(())
    }

    /// The statements of a block, with a warning for the code that is never
    /// run.
    fn check_statements(&mut self, body: &[Stmt]) -> Result<(), VeyError> {
        // The code after a `return`, a `break` or a `continue` is never run.
        let mut ended = false;
        for stmt in body {
            if ended {
                self.warn(Warning::new(
                    "the code after this is never run",
                    self.file,
                    stmt.pos().line,
                    stmt.pos().col,
                ));
                ended = false;
            }
            self.check_stmt(stmt)?;
            if matches!(
                stmt,
                Stmt::Return(_) | Stmt::Break(_, _) | Stmt::Continue(_, _)
            ) {
                ended = true;
            }
        }
        Ok(())
    }

    fn check_function(&mut self, params: &[Param], body: &Block) -> Result<(), VeyError> {
        let loop_depth = self.loop_depth;
        self.loop_depth = 0;
        self.function_depth += 1;
        self.scopes.push();

        for param in params {
            if let Some(default) = &param.default {
                self.check_expr(default)?;
            }
            for (name, _) in param.bindings() {
                self.declare(name, BindingKind::Param, param.pos)?;
            }
        }
        // The body of a function is in the scope of its parameters, so that
        // a name may not be written twice.
        self.check_statements(&body.body)?;

        self.pop_scope();
        self.function_depth -= 1;
        self.loop_depth = loop_depth;
        Ok(())
    }
    /// Check the functions of a type.
    fn check_impl(&mut self, decl: &ImplDecl) -> Result<(), VeyError> {
        for method in &decl.methods {
            self.scopes.push();
            // The value the method is called on is named `self`.
            self.declare("self", BindingKind::Let, method.pos)?;
            for param in &method.params {
                for (name, _) in param.bindings() {
                    self.declare(name, BindingKind::Param, param.pos)?;
                }
            }
            self.function_depth += 1;
            for stmt in &method.body.body {
                self.check_stmt(stmt)?;
            }
            self.function_depth -= 1;
            self.pop_scope();
        }
        Ok(())
    }

    fn check_component(&mut self, decl: &ComponentDecl) -> Result<(), VeyError> {        self.scopes.push();
        for param in &decl.params {
            if let Some(default) = &param.default {
                self.check_expr(default)?;
            }
            for (name, _) in param.bindings() {
                self.declare(name, BindingKind::Param, param.pos)?;
            }
        }
        self.check_markup_nodes(&decl.children)?;
        self.pop_scope();
        Ok(())
    }

    fn check_markup_nodes(&mut self, nodes: &[MarkupNode]) -> Result<(), VeyError> {
        for node in nodes {
            match node {
                MarkupNode::Element(element) => {
                    for (_, value) in &element.attrs {
                        self.check_expr(value)?;
                    }
                    self.check_markup_nodes(&element.children)?;
                }
                MarkupNode::Expr(expr) => self.check_expr(expr)?,
                MarkupNode::If(branch) => {
                    self.check_expr(&branch.test)?;
                    self.check_markup_nodes(&branch.then_nodes)?;
                    self.check_markup_nodes(&branch.else_nodes)?;
                }
                MarkupNode::For(loop_node) => {
                    self.check_expr(&loop_node.iter)?;
                    self.scopes.push();
                    self.declare(&loop_node.name, BindingKind::Let, loop_node.iter.pos())?;
                    if let Some(index) = &loop_node.index {
                        self.declare(index, BindingKind::Let, loop_node.iter.pos())?;
                    }
                    // The name of an item is written in the scope of the
                    // loop, so that it may be the item itself.
                    if let Some(key) = &loop_node.key {
                        self.check_expr(key)?;
                    }
                    self.check_markup_nodes(&loop_node.body)?;
                    self.pop_scope();
                }
                MarkupNode::Match(m) => {
                    self.check_expr(&m.subject)?;
                    for arm in &m.arms {
                        self.scopes.push();
                        for pattern in &arm.patterns {
                            self.check_pattern(pattern)?;
                            for (name, pos) in pattern.bindings() {
                                self.declare(&name, BindingKind::Let, pos)?;
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            self.check_expr(guard)?;
                        }
                        self.check_markup_nodes(&arm.body)?;
                        self.pop_scope();
                    }
                }
            }
        }
        Ok(())
    }

    fn check_target(&mut self, target: &Expr, reads: bool) -> Result<(), VeyError> {
        match target {
            Expr::Ident(name, pos) => match self.read_or_write(name, reads) {
                Some(binding) if binding.kind.immutable() => Err(self.error(
                    *pos,
                    format!("cannot assign to {} '{}'", binding.kind.label(), name),
                )),
                Some(_) => Ok(()),
                None => Err(self.error(
                    *pos,
                    format!("cannot assign to unknown variable '{}'", name),
                )),
            },
            Expr::Member(object, prop, optional, pos) => {
                if *optional {
                    return Err(self.error(*pos, "cannot assign through optional chaining"));
                }
                self.check_expr(object)?;
                if let MemberProp::Computed(index) = prop {
                    self.check_expr(index)?;
                }
                Ok(())
            }
            other => Err(self.error(other.pos(), "invalid assignment target")),
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> Result<(), VeyError> {
        match expr {
            Expr::Number(_, _) | Expr::Bool(_, _) | Expr::Null(_) => {}
            Expr::Ident(name, pos) => {
                if self.read(name).is_none() && !is_builtin(name) {
                    let mut error = self.error(*pos, format!("unknown variable '{}'", name));
                    if is_html_tag(name) {
                        // A name of a tag of HTML is often a tag that was
                        // meant to be written with its content.
                        error = error.with_hint(format!(
                            "'{}' is a tag of HTML; write it as '{} {{ }}' if you mean the tag",
                            name, name
                        ));
                    } else if let Some(hint) = did_you_mean(name, self.known_names()) {
                        error = error.with_hint(hint);
                    }
                    return Err(error);
                }
                // A value of the top level is not hoisted, so it may not be
                // used before the line that declares it. A function or a
                // component may use it, because it runs later.
                if self.scopes.depth() == 1 {
                    if let Some(declared) = self.declared_at.get(name) {
                        // The name may not be used before it is written, and
                        // it may not be used in the statement that writes
                        // it either.
                        if *declared >= self.statement {
                            return Err(self
                                .error(*pos, format!("'{}' is used before it is declared", name)));
                        }
                    }
                }
            }
            Expr::String(parts, _) => {
                for part in parts {
                    if let Some(expr) = &part.expr {
                        // A name that is only known at runtime is most often a
                        // string that should have been literal.
                        self.check_expr(expr)
                            .map_err(|err| err.with_hint("single quoted strings are literal"))?;
                    }
                }
            }
            Expr::Array(items, _) => {
                for item in items {
                    self.check_expr(item)?;
                }
            }
            Expr::Object(props, _) => {
                for prop in props {
                    match prop {
                        ObjectProp::Field(_, value) | ObjectProp::Spread(value) => {
                            self.check_expr(value)?;
                        }
                    }
                }
            }
            Expr::Unary(_, arg, _) => self.check_expr(arg)?,
            Expr::Binary(_, left, right, _)
            | Expr::Logical(_, left, right, _)
            | Expr::Nullish(left, right, _)
            | Expr::Range(left, right, _) => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            Expr::Ternary(test, then_value, else_value, _) => {
                self.check_expr(test)?;
                self.check_expr(then_value)?;
                self.check_expr(else_value)?;
            }
            Expr::Call(callee, args, _, _) => {
                self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
            }
            Expr::CallGeneric(callee, _, args, _, _) => {
                self.check_expr(callee)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
            }
            Expr::Member(object, prop, _, _) => {
                self.check_expr(object)?;
                if let MemberProp::Computed(index) = prop {
                    self.check_expr(index)?;
                }
            }
            Expr::Fn(params, body, _, _) => self.check_function(params, body)?,
            Expr::Arrow(_, params, body, _, _) => self.check_arrow(params, body)?,
            Expr::Match(subject, arms, _) => {
                self.check_expr(subject)?;
                for arm in arms {
                    self.scopes.push();
                    for pattern in &arm.patterns {
                        self.check_pattern(pattern)?;
                        match pattern {
                            Pattern::Enum { bindings, .. } => {
                                let mut seen = std::collections::HashSet::new();
                                for binding in bindings {
                                    if seen.insert(binding.name.clone()) {
                                        self.declare(&binding.name, BindingKind::Let, binding.pos)?;
                                    }
                                }
                            }
                            Pattern::Binding { name, pos } => {
                                self.declare(name, BindingKind::Let, *pos)?;
                            }
                            _ => {
                                for (name, pos) in pattern.bindings() {
                                    self.declare(&name, BindingKind::Let, pos)?;
                                }
                            }
                        }
                    }
                    // A name takes the whole value, so it cannot stand next
                    // to another pattern.
                    if arm.patterns.len() > 1
                        && arm
                            .patterns
                            .iter()
                            .any(|pattern| matches!(pattern, Pattern::Binding { .. }))
                    {
                        return Err(self.error(
                            arm.patterns[1].pos(),
                            "a name cannot be used with other patterns",
                        ));
                    }
                    if let Some(guard) = &arm.guard {
                        self.check_expr(guard)?;
                    }
                    self.check_expr(&arm.body)?;
                    self.pop_scope();
                }
            }
            Expr::Await(arg, _) => self.check_expr(arg)?,
            Expr::Try(arg, _) => self.check_expr(arg)?,
            Expr::As(arg, _, _) | Expr::Is(arg, _, _) => self.check_expr(arg)?,
            Expr::Yield(arg, _) => self.check_expr(arg)?,
            Expr::Spread(arg, _) => self.check_expr(arg)?,
        }
        Ok(())
    }

    fn check_pattern(&mut self, pattern: &Pattern) -> Result<(), VeyError> {
        match pattern {
            Pattern::Literal(expr) => self.check_expr(expr)?,
            Pattern::Range(start, end) => {
                self.check_expr(start)?;
                self.check_expr(end)?;
            }
            Pattern::Wildcard { .. } => {}
            Pattern::Binding { .. } => {}
            Pattern::Enum { .. } => {}
            Pattern::List(items, _) => {
                for item in items {
                    self.check_pattern(item)?;
                }
            }
            Pattern::Object(fields, _) => {
                for (_, item) in fields {
                    self.check_pattern(item)?;
                }
            }
            Pattern::Prefix(expr, _) => self.check_expr(expr)?,
        }
        Ok(())
    }

    /// The names that a pattern of a `match` takes apart.
    fn check_pattern_names(&mut self, pattern: &Pattern) -> Result<(), VeyError> {
        match pattern {
            Pattern::Enum { bindings, .. } => {
                let mut seen = std::collections::HashSet::new();
                for binding in bindings {
                    if seen.insert(binding.name.clone()) {
                        self.declare(&binding.name, BindingKind::Let, binding.pos)?;
                    }
                }
            }
            // A name of a list or an object pattern is a name of its own.
            _ => {
                for (name, pos) in pattern.bindings() {
                    self.declare(&name, BindingKind::Let, pos)?;
                }
            }
        }
        Ok(())
    }

    fn check_arrow(&mut self, params: &[Param], body: &ArrowBody) -> Result<(), VeyError> {
        match body {
            ArrowBody::Expr(expr) => {
                let loop_depth = self.loop_depth;
                self.loop_depth = 0;
                self.scopes.push();
                for param in params {
                    if let Some(default) = &param.default {
                        self.check_expr(default)?;
                    }
                    for (name, _) in param.bindings() {
                        self.declare(name, BindingKind::Param, param.pos)?;
                    }
                }
                self.check_expr(expr)?;
                self.pop_scope();
                self.loop_depth = loop_depth;
            }
            ArrowBody::Block(block) => self.check_function(params, block)?,
        }
        Ok(())
    }
}
