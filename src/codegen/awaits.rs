//! Finding out whether a piece of code uses `await`, `yield`, `?` or `defer`.

use crate::ast::*;

// The question "does this piece of code yield?" is answered in one place,
// so that the checker and the writer of the code cannot disagree.
pub(crate) use crate::types::helpers::{block_has_yield, expr_has_yield};

pub(crate) fn block_has_await(block: &Block) -> bool {
    block.body.iter().any(stmt_has_await)
}

/// Whether a piece of code has something to do when the block is left.
pub(crate) fn block_has_defer(block: &Block) -> bool {
    block.body.iter().any(stmt_has_defer)
}

/// Whether a piece of code hands a failure on.
pub(crate) fn block_has_try(block: &Block) -> bool {
    block.body.iter().any(stmt_has_try)
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
        Stmt::IfLet(decl) => {
            expr_has_await(&decl.value)
                || block_has_await(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .map(|s| stmt_has_await(s))
                    .unwrap_or(false)
        }
        Stmt::While(decl) => expr_has_await(&decl.test) || block_has_await(&decl.body),
        Stmt::WhileLet(decl) => expr_has_await(&decl.value) || block_has_await(&decl.body),
        Stmt::LetElse(decl) => expr_has_await(&decl.value) || block_has_await(&decl.otherwise),
        Stmt::Defer(decl) => expr_has_await(&decl.value),
        Stmt::Throw(decl) => expr_has_await(&decl.value),
        Stmt::For(decl) => expr_has_await(&decl.iter) || block_has_await(&decl.body),
        Stmt::Repeat(decl) => expr_has_await(&decl.count) || block_has_await(&decl.body),
        Stmt::Return(decl) => decl.value.as_ref().map(expr_has_await).unwrap_or(false),
        Stmt::Break(_, _)
        | Stmt::Continue(_, _)
        | Stmt::Use(_)
        | Stmt::Style(_)
        | Stmt::Type(_)
        | Stmt::Interface(_)
        | Stmt::Impl(_)
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

/// Whether an expression uses `await`.
pub(crate) fn expr_has_await(expr: &Expr) -> bool {
    walk(expr, &|expr| matches!(expr, Expr::Await(_, _)))
}

/// Whether an expression hands a failure on.
pub(crate) fn expr_has_try(expr: &Expr) -> bool {
    walk(expr, &|expr| matches!(expr, Expr::Try(_, _)))
}

/// Walk an expression and its parts.
///
/// A function of its own is not walked, because it has a scope of its own.
fn walk(expr: &Expr, test: &dyn Fn(&Expr) -> bool) -> bool {
    if test(expr) {
        return true;
    }
    match expr {
        Expr::Fn(..) | Expr::Arrow(_, _, _, _, _) => false,
        Expr::Number(_, _) | Expr::Bool(_, _) | Expr::Null(_) | Expr::Ident(_, _) => false,
        Expr::String(parts, _) => parts
            .iter()
            .any(|part| part.expr.as_ref().map(|expr| walk(expr, test)).unwrap_or(false)),
        Expr::Array(items, _) => items.iter().any(|item| walk(item, test)),
        Expr::Object(props, _) => props.iter().any(|prop| match prop {
            ObjectProp::Field(_, value) => walk(value, test),
            ObjectProp::Spread(value) => walk(value, test),
        }),
        Expr::Unary(_, arg, _) => walk(arg, test),
        Expr::Binary(_, left, right, _)
        | Expr::Logical(_, left, right, _)
        | Expr::Nullish(left, right, _)
        | Expr::Range(left, right, _) => walk(left, test) || walk(right, test),
        Expr::Ternary(test_value, cons, alt, _) => {
            walk(test_value, test) || walk(cons, test) || walk(alt, test)
        }
        Expr::Call(callee, args, _, _) => {
            walk(callee, test) || args.iter().any(|arg| walk(arg, test))
        }
        Expr::CallGeneric(callee, _, args, _, _) => {
            walk(callee, test) || args.iter().any(|arg| walk(arg, test))
        }
        Expr::Member(object, prop, _, _) => {
            walk(object, test)
                || match prop {
                    MemberProp::Computed(index) => walk(index, test),
                    MemberProp::Name(_) => false,
                }
        }
        Expr::Match(subject, arms, _) => {
            walk(subject, test)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().map(|guard| walk(guard, test)).unwrap_or(false)
                        || walk(&arm.body, test)
                })
        }
        Expr::Await(inner, _) => walk(inner, test),
        Expr::Try(inner, _) => walk(inner, test),
        Expr::Yield(inner, _) => walk(inner, test),
        Expr::As(inner, _, _) | Expr::Is(inner, _, _) => walk(inner, test),
        Expr::Spread(arg, _) => walk(arg, test),
    }
}


/// Whether the markup of a component waits for something.
pub(crate) fn markup_has_await(nodes: &[MarkupNode]) -> bool {
    nodes.iter().any(|node| match node {
        MarkupNode::Expr(expr) => expr_has_await(expr),
        MarkupNode::Element(element) => {
            element.attrs.iter().any(|(_, value)| expr_has_await(value))
                || element.args.iter().any(expr_has_await)
                || markup_has_await(&element.children)
        }
        MarkupNode::If(branch) => {
            expr_has_await(&branch.test)
                || markup_has_await(&branch.then_nodes)
                || markup_has_await(&branch.else_nodes)
        }
        MarkupNode::For(loop_node) => {
            expr_has_await(&loop_node.iter)
                || loop_node.key.as_ref().is_some_and(expr_has_await)
                || markup_has_await(&loop_node.body)
        }
        MarkupNode::Match(m) => {
            expr_has_await(&m.subject)
                || m.arms.iter().any(|arm| {
                    arm.guard.as_ref().map(|guard| expr_has_await(guard)).unwrap_or(false)
                        || markup_has_await(&arm.body)
                })
        }
    })
}
