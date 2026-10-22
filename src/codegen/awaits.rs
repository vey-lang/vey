//! Finding out whether a piece of code uses `await`, `yield`, `?` or `defer`.

use crate::ast::*;

// The question "does this piece of code yield?" is answered in one place,
// so that the checker and the writer of the code cannot disagree.
pub(crate) use crate::types::helpers::{
    block_has_await, block_has_yield, expr_has_await, expr_has_yield, markup_has_await, walk,
};

/// Whether a piece of code has something to do when the block is left.
pub(crate) fn block_has_defer(block: &Block) -> bool {
    block.body.iter().any(stmt_has_defer)
}

/// Whether a piece of code hands a failure on.
pub(crate) fn block_has_try(block: &Block) -> bool {
    block.body.iter().any(stmt_has_try)
}

pub(crate) fn stmt_has_defer(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Defer(_) => true,
        Stmt::Throw(_) => false,
        Stmt::Var(decl) => decl.init.as_ref().map(|_| false).unwrap_or(false),
        Stmt::Fn(_) | Stmt::Component(_) => false,
        Stmt::If(decl) => {
            false
                || block_has_defer(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_defer(s))
                    .unwrap_or(false)
        }
        Stmt::IfLet(decl) => {
            false
                || block_has_defer(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_defer(s))
                    .unwrap_or(false)
        }
        Stmt::While(decl) => false || block_has_defer(&decl.body),
        Stmt::WhileLet(decl) => false || block_has_defer(&decl.body),
        Stmt::LetElse(decl) => false || block_has_defer(&decl.otherwise),
        Stmt::For(decl) => false || block_has_defer(&decl.body),
        Stmt::Repeat(decl) => false || block_has_defer(&decl.body),
        Stmt::Return(decl) => decl.value.as_ref().map(|_| false).unwrap_or(false),
        Stmt::Break(_, _)
        | Stmt::Continue(_, _)
        | Stmt::Use(_)
        | Stmt::Style(_)
        | Stmt::Type(_)
        | Stmt::Interface(_)
        | Stmt::Impl(_)
        | Stmt::Enum(_) => false,
        Stmt::Try(decl) => {
            block_has_defer(&decl.body)
                || decl.catch.as_ref().is_some_and(block_has_defer)
                || decl.finally.as_ref().is_some_and(block_has_defer)
        }
        Stmt::Expr(_) => false,
        Stmt::Assign(_) => false,
        Stmt::Block(block, _) => block_has_defer(block),
    }
}

pub(crate) fn stmt_has_try(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Var(decl) => decl.init.as_ref().map(expr_has_try).unwrap_or(false),
        Stmt::Fn(_) | Stmt::Component(_) => false,
        Stmt::If(decl) => {
            expr_has_try(&decl.test)
                || block_has_try(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_try(s))
                    .unwrap_or(false)
        }
        Stmt::IfLet(decl) => {
            expr_has_try(&decl.value)
                || block_has_try(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_try(s))
                    .unwrap_or(false)
        }
        Stmt::While(decl) => expr_has_try(&decl.test) || block_has_try(&decl.body),
        Stmt::WhileLet(decl) => expr_has_try(&decl.value) || block_has_try(&decl.body),
        Stmt::LetElse(decl) => expr_has_try(&decl.value) || block_has_try(&decl.otherwise),
        Stmt::Defer(decl) => expr_has_try(&decl.value),
        Stmt::Throw(decl) => expr_has_try(&decl.value),
        Stmt::For(decl) => expr_has_try(&decl.iter) || block_has_try(&decl.body),
        Stmt::Repeat(decl) => expr_has_try(&decl.count) || block_has_try(&decl.body),
        Stmt::Return(decl) => decl.value.as_ref().map(expr_has_try).unwrap_or(false),
        Stmt::Break(_, _)
        | Stmt::Continue(_, _)
        | Stmt::Use(_)
        | Stmt::Style(_)
        | Stmt::Type(_)
        | Stmt::Interface(_)
        | Stmt::Impl(_)
        | Stmt::Enum(_) => false,
        Stmt::Try(decl) => {
            block_has_try(&decl.body)
                || decl.catch.as_ref().is_some_and(block_has_try)
                || decl.finally.as_ref().is_some_and(block_has_try)
        }
        Stmt::Expr(decl) => expr_has_try(&decl.expr),
        Stmt::Assign(decl) => expr_has_try(&decl.target) || expr_has_try(&decl.value),
        Stmt::Block(block, _) => block_has_try(block),
    }
}

/// Whether an expression hands a failure on.
pub(crate) fn expr_has_try(expr: &Expr) -> bool {
    walk(expr, &|expr| matches!(expr, Expr::Try(_, _)))
}

