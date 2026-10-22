//! The type of the expression at a place of a program.
//!
//! The command `vey explain` asks this question: what is the type of the
//! expression that is written at that line and column?

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::checker::TypeChecker;
use crate::VeyError;

/// The type of the expression at a place of a program.
pub fn explain(program: &Program, file: &str, line: usize, col: usize) -> Result<String, VeyError> {
    let point = Pos::new(line, col);
    let mut walker = Walker {
        point,
        best: None,
        function: None,
        best_function: None,
    };
    walker.program(program);
    let Some(expr) = walker.best else {
        return Err(VeyError::new(
            "there is no expression at that place",
            file,
            line,
            col,
        ));
    };

    let mut checker = TypeChecker::for_explain(program, file)?;
    // A name inside a function is known to that function, so the function
    // that holds it is checked first.
    if let Some((params, body, ret)) = &walker.best_function {
        checker.open_function(params, body, ret)?;
    }
    let ty = checker.infer_public(&expr)?;
    Ok(ty.display())
}

/// The expression that is at a place.
///
/// The expression that is found is the one that begins last before the
/// place, so that the innermost one of a nested expression is the one that
/// is asked about.
struct Walker {
    point: Pos,
    best: Option<Expr>,
    /// The parts of the function that holds the place, if the place is
    /// inside one: its parameters, its body and the type it returns.
    function: Option<(Vec<Param>, Block, Option<TypeExpr>)>,
    /// The function that holds the expression that was found.
    best_function: Option<(Vec<Param>, Block, Option<TypeExpr>)>,
}

impl Walker {
    fn program(&mut self, program: &Program) {
        self.body(&program.body);
    }

    fn body(&mut self, body: &[Stmt]) {
        for stmt in body {
            self.stmt(stmt);
        }
    }

    fn stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Var(decl) => {
                if let Some(init) = &decl.init {
                    self.expr(init);
                }
            }
            Stmt::Fn(decl) => {
                let outer = self.function.clone();
                self.function = Some((decl.params.clone(), decl.body.clone(), decl.ret.clone()));
                self.body(&decl.body.body);
                self.function = outer;
            }
            Stmt::Impl(decl) => {
                for method in &decl.methods {
                    let outer = self.function.clone();
                    self.function = Some((
                        method.params.clone(),
                        method.body.clone(),
                        method.ret.clone(),
                    ));
                    self.body(&method.body.body);
                    self.function = outer;
                }
            }
            Stmt::If(decl) => {
                self.expr(&decl.test);
                self.body(&decl.then_block.body);
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::IfLet(decl) => {
                self.expr(&decl.value);
                self.body(&decl.then_block.body);
                if let Some(branch) = &decl.else_branch {
                    self.stmt(branch);
                }
            }
            Stmt::While(decl) => {
                self.expr(&decl.test);
                self.body(&decl.body.body);
            }
            Stmt::WhileLet(decl) => {
                self.expr(&decl.value);
                self.body(&decl.body.body);
            }
            Stmt::LetElse(decl) => {
                self.expr(&decl.value);
                self.body(&decl.otherwise.body);
            }
            Stmt::Defer(decl) => self.expr(&decl.value),
            Stmt::Throw(decl) => self.expr(&decl.value),
            Stmt::For(decl) => {
                self.expr(&decl.iter);
                self.body(&decl.body.body);
            }
            Stmt::Repeat(decl) => {
                self.expr(&decl.count);
                self.body(&decl.body.body);
            }
            Stmt::Return(decl) => {
                if let Some(value) = &decl.value {
                    self.expr(value);
                }
            }
            Stmt::Try(decl) => {
                self.body(&decl.body.body);
                if let Some(catch) = &decl.catch {
                    self.body(&catch.body);
                }
                if let Some(finally) = &decl.finally {
                    self.body(&finally.body);
                }
            }
            Stmt::Expr(decl) => self.expr(&decl.expr),
            Stmt::Assign(decl) => {
                self.expr(&decl.target);
                self.expr(&decl.value);
            }
            Stmt::Block(block, _) => self.body(&block.body),
            // The markup of a component holds expressions as well.
            Stmt::Component(decl) => {
                for child in &decl.children {
                    self.markup(child);
                }
            }
            _ => {}
        }
    }

    /// The expressions of a piece of markup.
    fn markup(&mut self, node: &MarkupNode) {
        match node {
            MarkupNode::Element(element) => {
                for (_, value) in &element.attrs {
                    self.expr(value);
                }
                for arg in &element.args {
                    self.expr(arg);
                }
                for child in &element.children {
                    self.markup(child);
                }
            }
            MarkupNode::Expr(expr) => self.expr(expr),
            MarkupNode::If(decl) => {
                self.expr(&decl.test);
                for node in &decl.then_nodes {
                    self.markup(node);
                }
                for node in &decl.else_nodes {
                    self.markup(node);
                }
            }
            MarkupNode::For(decl) => {
                self.expr(&decl.iter);
                if let Some(key) = &decl.key {
                    self.expr(key);
                }
                for node in &decl.body {
                    self.markup(node);
                }
            }
            MarkupNode::Match(decl) => {
                self.expr(&decl.subject);
                for arm in &decl.arms {
                    if let Some(guard) = &arm.guard {
                        self.expr(guard);
                    }
                    for node in &arm.body {
                        self.markup(node);
                    }
                }
            }
        }
    }

    fn expr(&mut self, expr: &Expr) {
        let pos = expr.pos();
        if pos.line <= self.point.line
            && (pos.line < self.point.line || pos.col <= self.point.col)
        {
            // The expression that comes last before the place is the one
            // that is asked about.
            let closer = match &self.best {
                Some(best) => pos.after(&best.pos()),
                None => true,
            };
            if closer {
                self.best = Some(expr.clone());
                self.best_function = self.function.clone();
            }
        }
        match expr {
            Expr::String(parts, _) => {
                for part in parts {
                    if let Some(inner) = &part.expr {
                        self.expr(inner);
                    }
                }
            }
            Expr::Array(items, _) => {
                for item in items {
                    self.expr(item);
                }
            }
            Expr::Object(props, _) => {
                for prop in props {
                    match prop {
                        ObjectProp::Field(_, value) => self.expr(value),
                        ObjectProp::Spread(value) => self.expr(value),
                    }
                }
            }
            Expr::Unary(_, arg, _) => self.expr(arg),
            Expr::Binary(_, left, right, _)
            | Expr::Logical(_, left, right, _)
            | Expr::Nullish(left, right, _)
            | Expr::Range(left, right, _) => {
                self.expr(left);
                self.expr(right);
            }
            Expr::Ternary(test, cons, alt, _) => {
                self.expr(test);
                self.expr(cons);
                self.expr(alt);
            }
            Expr::Call(callee, args, _, _) | Expr::CallGeneric(callee, _, args, _, _) => {
                self.expr(callee);
                for arg in args {
                    self.expr(arg);
                }
            }
            Expr::Member(object, prop, _, _) => {
                self.expr(object);
                if let MemberProp::Computed(index) = prop {
                    self.expr(index);
                }
            }
            Expr::Match(subject, arms, _) => {
                self.expr(subject);
                for arm in arms {
                    self.expr(&arm.body);
                }
            }
            Expr::Await(inner, _)
            | Expr::Try(inner, _)
            | Expr::Yield(inner, _)
            | Expr::As(inner, _, _)
            | Expr::Is(inner, _, _)
            | Expr::Spread(inner, _) => self.expr(inner),
            // The body of a function is walked as well, so that the type of
            // a name inside it is found.
            Expr::Fn(_, body, _, _) => self.body(&body.body),
            Expr::Arrow(_, _, body, _, _) => match body {
                ArrowBody::Expr(expr) => self.expr(expr),
                ArrowBody::Block(block) => self.body(&block.body),
            },
            _ => {}
        }
    }
}
