//! The helpers of the checker.

use crate::ast::*;
use crate::types::core::{ObjectField, Type};

pub(crate) fn block_always_exits(block: &Block) -> bool {
    match block.body.last() {
        // A `throw` leaves the function as well, so the code after it is
        // never run.
        Some(Stmt::Return(_))
        | Some(Stmt::Break(_, _))
        | Some(Stmt::Continue(_, _))
        | Some(Stmt::Throw(_)) => true,
        Some(Stmt::If(decl)) => {
            let then_exits = block_always_exits(&decl.then_block);
            let else_exits = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_exits(branch));
            then_exits && else_exits
        }
        Some(Stmt::Try(decl)) => decl.finally.as_ref().is_some_and(block_always_exits),
        _ => false,
    }
}

/// Whether the statement always leaves the function or the loop.
pub(crate) fn stmt_always_exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Break(_, _) | Stmt::Continue(_, _) | Stmt::Throw(_) => true,
        Stmt::If(decl) => {
            let then_exits = block_always_exits(&decl.then_block);
            let else_exits = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_exits(branch));
            then_exits && else_exits
        }
        Stmt::Try(decl) => decl.finally.as_ref().is_some_and(block_always_exits),
        Stmt::Block(block, _) => block_always_exits(block),
        _ => false,
    }
}

pub(crate) fn set_object_field(fields: &mut Vec<ObjectField>, field: ObjectField) {
    if let Some(existing) = fields.iter_mut().find(|item| item.name == field.name) {
        *existing = field;
    } else {
        fields.push(field);
    }
}

pub(crate) fn block_assigns_name(block: &Block, name: &str) -> bool {
    block.body.iter().any(|stmt| stmt_assigns_name(stmt, name))
}

pub(crate) fn stmt_assigns_name(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Var(decl) => decl
            .init
            .as_ref()
            .is_some_and(|init| expr_assigns_name(init, name)),
        Stmt::Fn(decl) => block_assigns_name(&decl.body, name),
        Stmt::If(decl) => {
            block_assigns_name(&decl.then_block, name)
                || decl
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| stmt_assigns_name(branch, name))
        }
        Stmt::While(decl) => block_assigns_name(&decl.body, name),
        Stmt::For(decl) => block_assigns_name(&decl.body, name),
        Stmt::Repeat(decl) => block_assigns_name(&decl.body, name),
        Stmt::Return(decl) => decl
            .value
            .as_ref()
            .is_some_and(|value| expr_assigns_name(value, name)),
        Stmt::Try(decl) => {
            block_assigns_name(&decl.body, name)
                || decl
                    .catch
                    .as_ref()
                    .is_some_and(|catch| block_assigns_name(catch, name))
                || decl
                    .finally
                    .as_ref()
                    .is_some_and(|finally| block_assigns_name(finally, name))
        }
        Stmt::Expr(decl) => expr_assigns_name(&decl.expr, name),
        Stmt::Assign(decl) => {
            matches!(&decl.target, Expr::Ident(target, _) if target == name)
                || expr_assigns_name(&decl.value, name)
        }
        Stmt::Block(block, _) => block_assigns_name(block, name),
        _ => false,
    }
}

pub(crate) fn markup_assigns_name(nodes: &[MarkupNode], name: &str) -> bool {
    nodes.iter().any(|node| match node {
        MarkupNode::Element(element) => {
            element
                .attrs
                .iter()
                .any(|(_, value)| expr_assigns_name(value, name))
                || markup_assigns_name(&element.children, name)
        }
        MarkupNode::Expr(expr) => expr_assigns_name(expr, name),
        MarkupNode::If(branch) => {
            expr_assigns_name(&branch.test, name)
                || markup_assigns_name(&branch.then_nodes, name)
                || markup_assigns_name(&branch.else_nodes, name)
        }
        MarkupNode::For(loop_node) => {
            expr_assigns_name(&loop_node.iter, name) || markup_assigns_name(&loop_node.body, name)
        }
        MarkupNode::Match(m) => {
            expr_assigns_name(&m.subject, name)
                || m.arms.iter().any(|arm| markup_assigns_name(&arm.body, name))
        }
    })
}

/// The path of a value that a check may narrow, such as `user.name`.
pub(crate) fn place_path(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Ident(name, _) => Some(name.clone()),
        Expr::Member(object, MemberProp::Name(name), _, _) => {
            Some(format!("{}.{}", place_path(object)?, name))
        }
        _ => None,
    }
}

pub(crate) fn expr_assigns_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Fn(_, body, _, _) => block_assigns_name(body, name),
        Expr::Arrow(_, _, body, _, _) => match body {
            ArrowBody::Block(block) => block_assigns_name(block, name),
            ArrowBody::Expr(expr, _, _) => expr_assigns_name(expr, name),
        },
        Expr::Array(items, _) => items.iter().any(|item| expr_assigns_name(item, name)),
        Expr::Object(props, _) => props.iter().any(|prop| match prop {
            ObjectProp::Field(_, value) | ObjectProp::Spread(value) => {
                expr_assigns_name(value, name)
            }
        }),
        Expr::Unary(_, value, _) | Expr::Await(value, _) | Expr::Spread(value, _) => {
            expr_assigns_name(value, name)
        }
        Expr::Binary(_, left, right, _)
        | Expr::Logical(_, left, right, _)
        | Expr::Nullish(left, right, _)
        | Expr::Range(left, right, _) => {
            expr_assigns_name(left, name) || expr_assigns_name(right, name)
        }
        Expr::Ternary(test, then_value, else_value, _) => {
            expr_assigns_name(test, name)
                || expr_assigns_name(then_value, name)
                || expr_assigns_name(else_value, name)
        }
        Expr::Call(callee, args, _, _) => {
            expr_assigns_name(callee, name) || args.iter().any(|arg| expr_assigns_name(arg, name))
        }
        Expr::Member(object, _, _, _) => expr_assigns_name(object, name),
        Expr::Match(value, arms, _) => {
            expr_assigns_name(value, name)
                || arms.iter().any(|arm| expr_assigns_name(&arm.body, name))
        }
        _ => false,
    }
}

/// Give an object type the name of the alias it came from.
///
/// The name makes the type easier to read in an error message.
pub(crate) fn name_alias(name: &str, ty: Type) -> Type {
    match ty {
        Type::Object(fields) => Type::Named(name.to_string(), fields),
        other => other,
    }
}

pub(crate) fn argument_label(count: usize) -> &'static str {
    if count == 1 {
        "argument"
    } else {
        "arguments"
    }
}

/// The type that a value has to have, judged by the operator it is used with.///
/// A number cannot be compared with a text, and a text cannot be added to a
/// number, so the operator says what the type of the value is.
pub(crate) fn operand_type(op: &str, other: &Type) -> Option<Type> {
    match op {
        "-" | "*" | "/" | "%" | "**" => Some(Type::Number),
        "<" | "<=" | ">" | ">=" => Some(Type::Number),
        "+" => match other {
            Type::String => Some(Type::String),
            Type::Number => Some(Type::Number),
            _ => None,
        },
        "==" | "!=" | "===" | "!==" => match other {
            // The absence of a value does not say what the value is.
            Type::Any | Type::Var(_) | Type::Null | Type::Undefined => None,
            other => Some(other.clone()),
        },
        _ => None,
    }
}

pub(crate) fn type_argument_label(count: usize) -> &'static str {
    if count == 1 {
        "argument"
    } else {
        "arguments"
    }
}

/// Whether a piece of code hands its values back one by one.
///
/// A function that yields is a generator, and it does not return.
pub(crate) fn block_has_yield(block: &Block) -> bool {
    block.body.iter().any(stmt_has_yield)
}

fn stmt_has_yield(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Var(decl) => decl.init.as_ref().is_some_and(expr_has_yield),
        Stmt::If(decl) => {
            expr_has_yield(&decl.test)
                || block_has_yield(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| stmt_has_yield(branch))
        }
        Stmt::IfLet(decl) => {
            expr_has_yield(&decl.value)
                || block_has_yield(&decl.then_block)
                || decl
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| stmt_has_yield(branch))
        }
        Stmt::While(decl) => expr_has_yield(&decl.test) || block_has_yield(&decl.body),
        Stmt::WhileLet(decl) => expr_has_yield(&decl.value) || block_has_yield(&decl.body),
        Stmt::LetElse(decl) => expr_has_yield(&decl.value) || block_has_yield(&decl.otherwise),
        Stmt::Defer(decl) => expr_has_yield(&decl.value),
        Stmt::Throw(decl) => expr_has_yield(&decl.value),
        Stmt::For(decl) => expr_has_yield(&decl.iter) || block_has_yield(&decl.body),
        Stmt::Repeat(decl) => expr_has_yield(&decl.count) || block_has_yield(&decl.body),
        Stmt::Return(decl) => decl.value.as_ref().is_some_and(expr_has_yield),
        Stmt::Try(decl) => {
            block_has_yield(&decl.body)
                || decl.catch.as_ref().is_some_and(block_has_yield)
                || decl.finally.as_ref().is_some_and(block_has_yield)
        }
        Stmt::Expr(decl) => expr_has_yield(&decl.expr),
        Stmt::Assign(decl) => expr_has_yield(&decl.target) || expr_has_yield(&decl.value),
        Stmt::Block(block, _) => block_has_yield(block),
        _ => false,
    }
}

pub(crate) fn expr_has_yield(expr: &Expr) -> bool {
    match expr {
        Expr::Yield(_, _) => true,
        Expr::Fn(..) | Expr::Arrow(..) => false,
        Expr::String(parts, _) => parts
            .iter()
            .any(|part| part.expr.as_ref().is_some_and(expr_has_yield)),
        Expr::Array(items, _) => items.iter().any(expr_has_yield),
        Expr::Object(props, _) => props.iter().any(|prop| match prop {
            ObjectProp::Field(_, value) | ObjectProp::Spread(value) => expr_has_yield(value),
        }),
        Expr::Unary(_, arg, _) => expr_has_yield(arg),
        Expr::Binary(_, left, right, _)
        | Expr::Logical(_, left, right, _)
        | Expr::Nullish(left, right, _)
        | Expr::Range(left, right, _) => expr_has_yield(left) || expr_has_yield(right),
        Expr::Ternary(test, cons, alt, _) => {
            expr_has_yield(test) || expr_has_yield(cons) || expr_has_yield(alt)
        }
        Expr::Call(callee, args, _, _) | Expr::CallGeneric(callee, _, args, _, _) => {
            expr_has_yield(callee) || args.iter().any(expr_has_yield)
        }
        Expr::Member(object, prop, _, _) => {
            expr_has_yield(object)
                || match prop {
                    MemberProp::Computed(index) => expr_has_yield(index),
                    MemberProp::Name(_) => false,
                }
        }
        Expr::Match(subject, arms, _) => {
            expr_has_yield(subject)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(expr_has_yield) || expr_has_yield(&arm.body)
                })
        }
        Expr::Await(inner, _)
        | Expr::Try(inner, _)
        | Expr::As(inner, _, _)
        | Expr::Is(inner, _, _)
        | Expr::Spread(inner, _) => expr_has_yield(inner),
        _ => false,
    }
}

// The helpers that say whether a piece of code waits for something. They
// live here, and not in the code generator, because the checker has to know
// that a function waits, so that its type is a promise.

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

/// Whether an expression uses `await`.
pub(crate) fn expr_has_await(expr: &Expr) -> bool {
    walk(expr, &|expr| matches!(expr, Expr::Await(_, _)))
}

/// Walk an expression and its parts.
///
/// A function of its own is not walked, because it has a scope of its own.
pub(crate) fn walk(expr: &Expr, test: &dyn Fn(&Expr) -> bool) -> bool {
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

/// The name of a type of another module, with the name of the module in
/// front of it.
///
/// Two modules may have types of the same name, so the name of the module is
/// part of the name of the type.
pub(crate) fn qualified_type(module: &str, ty: Type) -> Type {
    fn qualify_recursions(module: &str, ty: &mut Type) {
        match ty {
            Type::Recursion(name, args) => {
                if !name.contains('.') {
                    *name = format!("{}.{}", module, name);
                }
                for arg in args {
                    qualify_recursions(module, arg);
                }
            }
            Type::Array(inner) | Type::Promise(inner) => qualify_recursions(module, inner),
            Type::Tuple(items) | Type::Union(items) => {
                for item in items {
                    qualify_recursions(module, item);
                }
            }
            Type::Dict(key, value) | Type::Result(key, value) => {
                qualify_recursions(module, key);
                qualify_recursions(module, value);
            }
            Type::Object(fields) | Type::Named(_, fields) | Type::Interface(_, fields) => {
                for field in fields {
                    qualify_recursions(module, &mut field.ty);
                }
            }
            Type::Function(function) => {
                for param in &mut function.params {
                    qualify_recursions(module, param);
                }
                qualify_recursions(module, &mut function.ret);
                for (_, constraint) in &mut function.constraints {
                    qualify_recursions(module, constraint);
                }
            }
            _ => {}
        }
    }

    let mut ty = ty;
    qualify_recursions(module, &mut ty);
    match ty {
        Type::EnumUnion(name) if !name.contains('.') => {
            Type::EnumUnion(format!("{}.{}", module, name))
        }
        Type::Named(name, fields) if !name.contains('.') => {
            Type::Named(format!("{}.{}", module, name), fields)
        }
        Type::Interface(name, fields) => {
            let name = if name.contains('.') { name } else { format!("{}.{}", module, name) };
            Type::Interface(name, fields)
        }
        other => other,
    }
}

/// The number of an expression, when it is a plain number.
pub(crate) fn literal_number(expr: &Expr) -> Option<usize> {
    match expr {
        Expr::Number(value, _) if *value >= 0.0 => Some(*value as usize),
        _ => None,
    }
}

/// The field of a value that the type it is given does not have.
pub(crate) fn extra_field(source: &Type, target: &Type) -> Option<String> {
    let (source_fields, target_fields) = match (source, target) {
        (Type::Object(source), Type::Object(target)) => (source, target),
        (Type::Object(source), Type::Named(_, target)) => (source, target),
        (Type::Object(source), Type::Interface(_, target)) => (source, target),
        _ => return None,
    };
    source_fields
        .iter()
        .find(|field| {
            !target_fields
                .iter()
                .any(|target_field| target_field.name == field.name)
        })
        .map(|field| field.name.clone())
}
