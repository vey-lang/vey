use std::collections::HashMap;

use crate::ast::*;
use crate::builtins::is_builtin;
use crate::lexer::Pos;
use crate::VeyError;

pub fn check(program: &Program, file: &str) -> Result<(), VeyError> {
    let mut checker = Checker {
        file,
        scopes: vec![HashMap::new()],
        loop_depth: 0,
        function_depth: 0,
    };
    checker.check_program(program)
}

#[derive(Clone, Copy)]
enum BindingKind {
    Let,
    Const,
    Param,
    Function,
    Component,
    Import,
}

impl BindingKind {
    fn immutable(self) -> bool {
        matches!(
            self,
            Self::Const | Self::Function | Self::Component | Self::Import
        )
    }

    fn label(self) -> &'static str {
        match self {
            Self::Const => "const",
            Self::Function => "function",
            Self::Component => "component",
            Self::Import => "import",
            Self::Let | Self::Param => "variable",
        }
    }

    fn from_var(kind: VarKind) -> Self {
        match kind {
            VarKind::Let => Self::Let,
            VarKind::Const => Self::Const,
        }
    }
}

#[derive(Clone, Copy)]
struct Binding {
    kind: BindingKind,
    pos: Pos,
}

struct Checker<'a> {
    file: &'a str,
    scopes: Vec<HashMap<String, Binding>>,
    loop_depth: usize,
    function_depth: usize,
}

impl Checker<'_> {
    fn error(&self, pos: Pos, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file, pos.line, pos.col)
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, kind: BindingKind, pos: Pos) -> Result<(), VeyError> {
        let previous = self
            .scopes
            .last()
            .and_then(|scope| scope.get(name))
            .copied();

        if let Some(previous) = previous {
            return Err(self.error(
                pos,
                format!(
                    "duplicate declaration '{}' (first declared on line {})",
                    name, previous.pos.line
                ),
            ));
        }

        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Binding { kind, pos });
        }
        Ok(())
    }

    fn declare_pattern(
        &mut self,
        pattern: &BindingPattern,
        kind: BindingKind,
    ) -> Result<(), VeyError> {
        match pattern {
            BindingPattern::Name { name, pos } => self.declare(name, kind, *pos),
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

    fn resolve(&self, name: &str) -> Option<Binding> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn check_program(&mut self, program: &Program) -> Result<(), VeyError> {
        for stmt in &program.body {
            match stmt {
                Stmt::Use(decl) => {
                    if !decl.path.ends_with(".css") {
                        self.declare(&decl.alias, BindingKind::Import, decl.pos)?;
                    }
                }
                Stmt::Var(decl) => {
                    self.declare_pattern(&decl.pattern, BindingKind::from_var(decl.kind))?
                }
                Stmt::Fn(decl) => self.declare(&decl.name, BindingKind::Function, decl.pos)?,
                Stmt::Component(decl) => {
                    self.declare(&decl.name, BindingKind::Component, decl.pos)?
                }
                Stmt::Enum(decl) => self.declare(&decl.name, BindingKind::Const, decl.pos)?,
                _ => {}
            }
        }

        for stmt in &program.body {
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
            Stmt::While(decl) => {
                self.check_expr(&decl.test)?;
                self.loop_depth += 1;
                self.check_block(&decl.body)?;
                self.loop_depth -= 1;
            }
            Stmt::For(decl) => {
                self.check_expr(&decl.iter)?;
                self.push_scope();
                self.declare_pattern(&decl.pattern, BindingKind::Let)?;
                self.loop_depth += 1;
                for inner in &decl.body.body {
                    self.check_stmt(inner)?;
                }
                self.loop_depth -= 1;
                self.pop_scope();
            }
            Stmt::Repeat(decl) => {
                self.check_expr(&decl.count)?;
                self.loop_depth += 1;
                self.check_block(&decl.body)?;
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
            Stmt::Break(pos) => {
                if self.loop_depth == 0 {
                    return Err(self.error(*pos, "break outside a loop"));
                }
            }
            Stmt::Continue(pos) => {
                if self.loop_depth == 0 {
                    return Err(self.error(*pos, "continue outside a loop"));
                }
            }
            Stmt::Try(decl) => {
                self.check_block(&decl.body)?;
                self.push_scope();
                if let Some(name) = &decl.name {
                    self.declare(name, BindingKind::Let, decl.pos)?;
                }
                for inner in &decl.catch.body {
                    self.check_stmt(inner)?;
                }
                self.pop_scope();
            }
            Stmt::Use(decl) => {
                if !decl.path.ends_with(".css") {
                    self.declare(&decl.alias, BindingKind::Import, decl.pos)?;
                }
            }
            Stmt::Style(decl) => {
                return Err(self.error(decl.pos, "style blocks must be at the top level"));
            }
            Stmt::Type(_) => {}
            Stmt::Enum(_) => {}
            Stmt::Expr(decl) => self.check_expr(&decl.expr)?,
            Stmt::Assign(decl) => {
                self.check_target(&decl.target)?;
                self.check_expr(&decl.value)?;
            }
            Stmt::Block(block, _) => self.check_block(block)?,
        }
        Ok(())
    }

    fn check_block(&mut self, block: &Block) -> Result<(), VeyError> {
        self.push_scope();
        for stmt in &block.body {
            self.check_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn check_function(&mut self, params: &[Param], body: &Block) -> Result<(), VeyError> {
        let loop_depth = self.loop_depth;
        self.loop_depth = 0;
        self.function_depth += 1;
        self.push_scope();

        for param in params {
            if let Some(default) = &param.default {
                self.check_expr(default)?;
            }
            self.declare(&param.name, BindingKind::Param, param.pos)?;
        }
        for stmt in &body.body {
            self.check_stmt(stmt)?;
        }

        self.pop_scope();
        self.function_depth -= 1;
        self.loop_depth = loop_depth;
        Ok(())
    }

    fn check_component(&mut self, decl: &ComponentDecl) -> Result<(), VeyError> {
        self.push_scope();
        for param in &decl.params {
            if let Some(default) = &param.default {
                self.check_expr(default)?;
            }
            self.declare(&param.name, BindingKind::Param, param.pos)?;
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
                    self.push_scope();
                    self.declare(&loop_node.name, BindingKind::Let, loop_node.iter.pos())?;
                    self.check_markup_nodes(&loop_node.body)?;
                    self.pop_scope();
                }
            }
        }
        Ok(())
    }

    fn check_target(&mut self, target: &Expr) -> Result<(), VeyError> {
        match target {
            Expr::Ident(name, pos) => match self.resolve(name) {
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
                if self.resolve(name).is_none() && !is_builtin(name) {
                    return Err(self.error(*pos, format!("unknown variable '{}'", name)));
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
            Expr::Arrow(params, body, _) => self.check_arrow(params, body)?,
            Expr::Match(subject, arms, _) => {
                self.check_expr(subject)?;
                for arm in arms {
                    self.push_scope();
                    for pattern in &arm.patterns {
                        self.check_pattern(pattern)?;
                        if let Pattern::Enum { bindings, .. } = pattern {
                            let mut seen = std::collections::HashSet::new();
                            for binding in bindings {
                                if seen.insert(binding.name.clone()) {
                                    self.declare(&binding.name, BindingKind::Let, binding.pos)?;
                                }
                            }
                        }
                    }
                    self.check_expr(&arm.body)?;
                    self.pop_scope();
                }
            }
            Expr::Await(arg, _) => self.check_expr(arg)?,
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
            Pattern::Wildcard => {}
            Pattern::Enum { .. } => {}
        }
        Ok(())
    }

    fn check_arrow(&mut self, params: &[Param], body: &ArrowBody) -> Result<(), VeyError> {
        match body {
            ArrowBody::Expr(expr) => {
                let loop_depth = self.loop_depth;
                self.loop_depth = 0;
                self.push_scope();
                for param in params {
                    if let Some(default) = &param.default {
                        self.check_expr(default)?;
                    }
                    self.declare(&param.name, BindingKind::Param, param.pos)?;
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
