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
            Stmt::Fn(decl) => self.body(&decl.body.body),
            Stmt::Impl(decl) => {
                for method in &decl.methods {
                    self.body(&method.body.body);
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
            _ => {}
        }
    }

    fn expr(&mut self, expr: &Expr) {
        let pos = expr.pos();
        if pos.line <= self.point.line
            && (pos.line < self.point.line || pos.col <= self.point.col)
        {
            let closer = match &self.best {
                Some(best) => best.pos().after(&pos),
                None => true,
            };
            if closer {
                self.best = Some(expr.clone());
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
            Expr::Await(inner, _) | Expr::Try(inner, _) | Expr::Yield(inner, _) => {
                self.expr(inner)
            }
            Expr::Spread(inner, _) => self.expr(inner),
            _ => {}
        }
    }
}
