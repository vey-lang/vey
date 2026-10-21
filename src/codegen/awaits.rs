//! Finding out whether a piece of code uses `await`.

use crate::ast::*;

pub(crate) fn block_has_await(block: &Block) -> bool {
    block.body.iter().any(stmt_has_await)
}

pub(crate) fn stmt_has_await(stmt: &Stmt) -> bool {
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
        Stmt::Try(decl) => {
            block_has_await(&decl.body)
                || decl.catch.as_ref().is_some_and(block_has_await)
                || decl.finally.as_ref().is_some_and(block_has_await)
        }
        Stmt::Expr(decl) => expr_has_await(&decl.expr),
        Stmt::Assign(decl) => expr_has_await(&decl.target) || expr_has_await(&decl.value),
        Stmt::Block(block, _) => block_has_await(block),
    }
}

pub(crate) fn expr_has_await(expr: &Expr) -> bool {
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
