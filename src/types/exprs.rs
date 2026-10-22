//! The part of the checker that works on expressions.

use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::builtins::builtin_type;
use crate::types::core::{union, without_empty, EnumInfo, EnumVariantInfo, FunctionType, ObjectField, Type};
use crate::types::rules::{
    assignable, collect_type_vars, comparable, erase_type_vars, has_type_var, orderable, substitute,
    substitute_bound, unify_types,
};
use crate::VeyError;

use super::checker::TypeChecker;
use super::helpers::{
    argument_label, literal_number, operand_type, qualified_type, set_object_field,
    type_argument_label,
};
use crate::parser::exprs::NAMED_MARKER;
use crate::suggest::did_you_mean;

impl TypeChecker<'_> {
    pub(crate) fn infer_expr(&mut self, expr: &Expr) -> Result<Type, VeyError> {
        match expr {
            Expr::Number(_, _) => Ok(Type::Number),
            Expr::String(parts, _) => {
                for part in parts {
                    if let Some(expr) = &part.expr {
                        self.infer_expr(expr)?;
                    }
                }
                Ok(Type::String)
            }
            Expr::Bool(_, _) => Ok(Type::Bool),
            Expr::Null(_) => Ok(Type::Null),
            Expr::Ident(name, _) => Ok(self
                .lookup(name)
                .or_else(|| builtin_type(name))
                .unwrap_or(Type::Any)),
            Expr::Array(items, _) => {
                let mut types = Vec::new();
                for item in items {
                    match item {
                        Expr::Spread(inner, spread_pos) => {
                            let ty = self.infer_expr(inner)?;
                            match ty {
                                Type::Array(element) => types.push(*element),
                                Type::Any | Type::Unknown => types.push(Type::Any),
                                _ => {
                                    return Err(self.error(
                                        *spread_pos,
                                        format!("cannot spread a {}", ty.display()),
                                    ))
                                }
                            }
                        }
                        other => types.push(self.infer_expr(other)?),
                    }
                }
                if types.is_empty() {
                    Ok(Type::Array(Box::new(Type::Any)))
                } else {
                    Ok(Type::Array(Box::new(union(types))))
                }
            }
            Expr::Object(props, _) => {
                let mut out = Vec::new();
                let mut unknown = false;
                for prop in props {
                    match prop {
                        ObjectProp::Field(name, value) => {
                            let ty = self.infer_expr(value)?;
                            set_object_field(
                                &mut out,
                                ObjectField {
                                    name: name.clone(),
                                    ty,
                                    optional: false,
                                },
                            );
                        }
                        ObjectProp::Spread(value) => {
                            let ty = self.infer_expr(value)?;
                            match ty {
                                Type::Object(fields)
                                | Type::Named(_, fields)
                                | Type::Interface(_, fields) => {
                                    for field in fields {
                                        set_object_field(&mut out, field);
                                    }
                                }
                                Type::Any | Type::Unknown => unknown = true,
                                _ => {
                                    return Err(self.error(
                                        value.pos(),
                                        format!("cannot spread a {}", ty.display()),
                                    ))
                                }
                            }
                        }
                    }
                }
                if unknown {
                    Ok(Type::Any)
                } else {
                    Ok(Type::Object(out))
                }
            }
            Expr::Unary(op, inner, pos) => {
                let ty = self.infer_expr(inner)?;
                match op.as_str() {
                    "!" | "not" => Ok(Type::Bool),
                    "-" | "+" => {
                        if matches!(ty, Type::Any) || assignable(&ty, &Type::Number) {
                            Ok(Type::Number)
                        } else {
                            Err(self.error(
                                *pos,
                                format!(
                                    "operator '{}' requires a number, found {}",
                                    op,
                                    ty.display()
                                ),
                            ))
                        }
                    }
                    _ => Ok(Type::Any),
                }
            }
            Expr::Binary(op, left, right, pos) => {
                let left_ty = self.infer_expr(left)?;
                let right_ty = self.infer_expr(right)?;
                // The way a value is used says what its type has to be.
                self.note_operand(op, left, &left_ty, &right_ty);
                self.note_operand(op, right, &right_ty, &left_ty);
                self.binary_result(op, &left_ty, &right_ty, *pos)
            }
            Expr::Logical(_, left, right, pos) => {
                let left = self.infer_expr(left)?;
                let right = self.infer_expr(right)?;
                // A condition is a yes or a no.
                for ty in [&left, &right] {
                    if matches!(ty, Type::Any | Type::Unknown | Type::Var(_)) {
                        continue;
                    }
                    if !assignable(ty, &Type::Bool) {
                        return Err(self.type_error(*pos, ty, &Type::Bool));
                    }
                }
                Ok(Type::Bool)
            }
            Expr::Nullish(left, right, _) => {
                let left = self.infer_expr(left)?;
                let right = self.infer_expr(right)?;
                Ok(union(vec![without_empty(&left), right]))
            }
            Expr::Ternary(test, then_value, else_value, pos) => {
                let test_ty = self.infer_expr(test)?;
                self.check_condition(&test_ty, *pos)?;
                let then_ty = self.infer_expr(then_value)?;
                let else_ty = self.infer_expr(else_value)?;
                Ok(union(vec![then_ty, else_ty]))
            }
            Expr::Call(callee, args, optional, pos) => {
                self.infer_call(callee, args, *optional, *pos, None, None)
            }
            Expr::CallGeneric(callee, type_args, args, optional, pos) => {
                let mut explicit = Vec::new();
                for arg in type_args {
                    explicit.push(self.resolve_type(arg)?);
                }
                self.infer_call(callee, args, *optional, *pos, None, Some(&explicit))
            }
            Expr::Member(object, prop, optional, pos) => {
                self.member_type(object, prop, *optional, *pos)
            }
            Expr::Fn(params, body, ret_ann, pos) => {
                let (ret, param_types) =
                    self.check_function(params, body, ret_ann.as_ref(), *pos)?;
                // The type of the result is known even when the parameters
                // are not, so that a closure fits the type it is given.
                self.function_type_from_parts(params, ret, ret_ann.is_some(), &[], &param_types)
            }
            Expr::Arrow(type_params, params, body, ret_ann, pos) => {
                // The type parameters of the arrow are in scope while its
                // body is checked.
                self.push_type_params(type_params)?;
                let result = self
                    .check_arrow(params, body, ret_ann.as_ref(), *pos)
                    .and_then(|(ret, param_types)| {
                        // The type of the arrow is made while its type
                        // parameters are still in scope.
                        self.function_type_from_parts(
                            params,
                            ret,
                            ret_ann.is_some(),
                            type_params,
                            &param_types,
                        )
                    });
                self.pop_type_params();
                result
            }
            Expr::Range(start, end, pos) => {
                let start_ty = self.infer_expr(start)?;
                let end_ty = self.infer_expr(end)?;
                if !matches!(start_ty, Type::Any) && !assignable(&start_ty, &Type::Number) {
                    return Err(self.type_error(*pos, &start_ty, &Type::Number));
                }
                if !matches!(end_ty, Type::Any) && !assignable(&end_ty, &Type::Number) {
                    return Err(self.type_error(*pos, &end_ty, &Type::Number));
                }
                Ok(Type::Array(Box::new(Type::Number)))
            }
            Expr::Match(subject, arms, _) => {
                let subject_ty = self.infer_expr(subject)?;
                let mut types = Vec::new();
                let mut covered = std::collections::HashSet::new();
                // The two values of a bool that the arms take.
                let mut covered_bools = std::collections::HashSet::new();
                let mut has_wildcard = false;
                for arm in arms {
                    self.push_scope();
                    for pattern in &arm.patterns {
                        match pattern {
                            Pattern::Enum {
                                enum_name,
                                variant,
                                bindings,
                                pos,
                            } => {
                                // resolve enum
                                let Some(info) = self.enum_info(enum_name) else {
                                    return Err(
                                        self.error(*pos, format!("unknown enum '{}'", enum_name))
                                    );
                                };
                                let v_fields: Vec<(String, Option<TypeExpr>)> = {
                                    let v = info.variant(variant).ok_or_else(|| {
                                        self.error(
                                            *pos,
                                            format!(
                                                "enum '{}' has no variant '{}'",
                                                enum_name, variant
                                            ),
                                        )
                                    })?;
                                    v.fields
                                        .iter()
                                        .map(|f| (f.name.clone(), f.ty.clone()))
                                        .collect()
                                };
                                // subject compatibility
                                match &subject_ty {
                                    Type::EnumUnion(e) if e != enum_name => {
                                        return Err(self.error(
                                            *pos,
                                            format!(
                                                "pattern '{}.{}' does not match '{}'",
                                                enum_name, variant, e
                                            ),
                                        ));
                                    }
                                    Type::Enum(e, _) if e != enum_name => {
                                        return Err(self.error(
                                            *pos,
                                            format!(
                                                "pattern '{}.{}' does not match '{}'",
                                                enum_name, variant, e
                                            ),
                                        ));
                                    }
                                    _ => {}
                                }
                                // binding count check
                                if bindings.len() != v_fields.len() {
                                    return Err(self.error(
                                        *pos,
                                        format!(
                                            "variant '{}.{}' expects {} binding(s), found {}",
                                            enum_name,
                                            variant,
                                            v_fields.len(),
                                            bindings.len()
                                        ),
                                    ));
                                }
                                // resolve field types and declare bindings
                                // positional vs named
                                let mut positional_idx = 0;
                                for b in bindings {
                                    let field_name = if let Some(f) = &b.field {
                                        f.clone()
                                    } else {
                                        if positional_idx >= v_fields.len() {
                                            return Err(self.error(b.pos, format!("variant '{}.{}' expects {} binding(s), found {}", enum_name, variant, v_fields.len(), bindings.len())));
                                        }
                                        let name = v_fields[positional_idx].0.clone();
                                        positional_idx += 1;
                                        name
                                    };
                                    let field_ty = v_fields
                                        .iter()
                                        .find(|(n, _)| n == &field_name)
                                        .map(|(_, t)| t.clone())
                                        .ok_or_else(|| {
                                            self.error(
                                                b.pos,
                                                format!(
                                                    "variant '{}.{}' has no field '{}'",
                                                    enum_name, variant, field_name
                                                ),
                                            )
                                        })?;
                                    let ty = match &field_ty {
                                        Some(ty) => self.resolve_type(ty)?,
                                        None => Type::Any,
                                    };
                                    self.declare(&b.name, ty);
                                }
                                // An arm with a condition does not cover its
                                // variant on its own: the value may fall
                                // through to the next arm.
                                if arm.guard.is_none() {
                                    covered.insert(format!("{}.{}", enum_name, variant));
                                }
                            }
                            Pattern::Wildcard { .. } => {
                                if arm.guard.is_none() {
                                    has_wildcard = true;
                                }
                            }
                            Pattern::Binding { name, .. } => {
                                // The name takes the value of the match.
                                self.declare(name, subject_ty.clone());
                                if arm.guard.is_none() {
                                    has_wildcard = true;
                                }
                            }
                            Pattern::Literal(expr) => {
                                // The pattern has to be of the type of the
                                // subject, or it never holds.
                                let literal = self.infer_expr(expr)?;
                                if !matches!(subject_ty, Type::Any)
                                    && !comparable(&subject_ty, &literal)
                                {
                                    return Err(self.error(
                                        expr.pos(),
                                        format!(
                                            "this pattern is a {}, and the subject is a {}",
                                            literal.display(),
                                            subject_ty.display()
                                        ),
                                    ));
                                }
                                if let Expr::Bool(value, _) = expr {
                                    covered_bools.insert(*value);
                                }
                            }
                            _ => {
                                self.check_pattern(pattern)?;
                            }
                        }
                    }
                    // The condition of the arm is checked with the names of
                    // the patterns in it.
                    if let Some(guard) = &arm.guard {
                        let actual = self.infer_expr(guard)?;
                        if !assignable(&actual, &Type::Bool) {
                            return Err(self.type_error(guard.pos(), &actual, &Type::Bool));
                        }
                    }
                    types.push(self.infer_expr(&arm.body)?);
                    self.pop_scope();
                }
                // exhaustiveness
                if !has_wildcard {
                    match &subject_ty {
                        Type::EnumUnion(name) => {
                            if let Some(info) = self.enum_info(name) {
                                let missing: Vec<String> = info
                                    .variants
                                    .iter()
                                    .filter(|v| {
                                        !covered
                                            .iter()
                                            .any(|c| c == &format!("{}.{}", name, v.name))
                                    })
                                    .map(|v| v.name.clone())
                                    .collect();
                                if !missing.is_empty() {
                                    return Err(self.error(
                                        subject.pos(),
                                        format!(
                                            "match on '{}' is not exhaustive, missing: {}",
                                            name,
                                            missing.join(", ")
                                        ),
                                    ));
                                }
                            }
                        }
                        Type::Enum(name, v) if !covered.contains(&format!("{}.{}", name, v)) => {
                            return Err(self.error(
                                subject.pos(),
                                format!(
                                    "match on '{}.{}' is not exhaustive, missing: {}",
                                    name, v, v
                                ),
                            ));
                        }
                        // A match on a bool has to take both of its values.
                        Type::Bool => {
                            let missing: Vec<&str> = [true, false]
                                .iter()
                                .filter(|value| !covered_bools.contains(value))
                                .map(|value| if *value { "true" } else { "false" })
                                .collect();
                            if !missing.is_empty() {
                                return Err(self.error(
                                    subject.pos(),
                                    format!(
                                        "match on 'bool' is not exhaustive, missing: {}",
                                        missing.join(", ")
                                    ),
                                ));
                            }
                        }
                        // A value of a union may be one of many, so the match
                        // has to take them all.
                        Type::Union(items) => {
                            let parts: Vec<String> = items
                                .iter()
                                .map(|item| item.display())
                                .collect();
                            return Err(self.error(
                                subject.pos(),
                                format!(
                                    "match on '{}' is not exhaustive: a name or a _ is needed",
                                    parts.join(" | ")
                                ),
                            ));
                        }
                        _ => {}
                    }
                }
                if types.is_empty() {
                    Ok(Type::Void)
                } else {
                    Ok(union(types))
                }
            }
            Expr::Await(inner, _) => {
                let ty = self.infer_expr(inner)?;
                // The type of an `await` is the type of the value that will
                // be there.
                Ok(match ty {
                    Type::Promise(inner) => *inner,
                    Type::Union(items) => union(
                        items
                            .iter()
                            .map(|item| match item {
                                Type::Promise(inner) => (**inner).clone(),
                                other => other.clone(),
                            })
                            .collect(),
                    ),
                    _ => Type::Any,
                })
            }
            Expr::Try(inner, pos) => {
                // The failure is handed on to the function that is around,
                // so a `?` outside a function has nowhere to go.
                if self.expected_returns.is_empty() && self.try_depth == 0 {
                    return Err(self.error(
                        *pos,
                        "the ? is only allowed inside a function",
                    ));
                }
                // The type of a value that hands its failure on is the type
                // of the value inside it.
                let ty = self.infer_expr(inner)?;
                Ok(match ty {
                    Type::Result(ok, _) => *ok,
                    Type::Union(items) => union(
                        items
                            .iter()
                            .map(|item| match item {
                                Type::Result(ok, _) => (**ok).clone(),
                                other => other.clone(),
                            })
                            .collect(),
                    ),
                    other => other,
                })
            }
            Expr::Yield(inner, _) => self.infer_expr(inner),
            Expr::As(inner, ty, pos) => {
                // A cast is only possible between types that are near one
                // another.
                let actual = self.infer_expr(inner)?;
                let wanted = self.resolve_type(ty)?;
                if !self.fits(&actual, &wanted) && !self.fits(&wanted, &actual) {
                    return Err(self.type_error(*pos, &actual, &wanted));
                }
                Ok(wanted)
            }
            Expr::Is(inner, ty, pos) => {
                let actual = self.infer_expr(inner)?;
                let wanted = self.resolve_type(ty)?;
                if !self.fits(&actual, &wanted) && !self.fits(&wanted, &actual) {
                    return Err(self.type_error(*pos, &actual, &wanted));
                }
                Ok(Type::Bool)
            }
            Expr::Spread(inner, _) => self.infer_expr(inner),
        }
    }

    /// Remember the type that a parameter has to have.
    ///
    /// The operator a value is used with says what its type is, so a
    /// parameter without a type is known when it is used.
    pub(crate) fn note_operand(&mut self, op: &str, expr: &Expr, own: &Type, other: &Type) {
        let Expr::Ident(name, _) = expr else {
            return;
        };
        let Some(wanted) = operand_type(op, other) else {
            return;
        };
        // The value has to fit the type that is already known.
        if !matches!(own, Type::Any) && !assignable(own, &wanted) {
            return;
        }
        self.note_param(name, wanted);
    }

    /// The type of a call.
    ///
    /// The arguments that are given by name are matched with the parameters,
    /// and the order of the arguments is written down for the code generator.
    pub(crate) fn infer_call(
        &mut self,
        callee: &Expr,
        args: &[Expr],
        optional: bool,
        pos: Pos,
        expected: Option<&Type>,
        explicit: Option<&[Type]>,
    ) -> Result<Type, VeyError> {
        let callee_ty = self.infer_expr(callee)?;
        // The callee may be a value that may be nothing.
        let callee_ty = if optional {
            without_empty(&callee_ty)
        } else {
            callee_ty
        };
        let (positional, named) = self.split_named(args)?;
        let order = if named.is_empty() {
            positional.clone()
        } else {
            self.match_named(&callee_ty, &positional, &named, pos)?
        };
        // The arguments are checked against the parameters, so that a list
        // of values may be a tuple.
        let (arg_types, has_spread) = self.argument_types_against(&callee_ty, &order)?;
        if !named.is_empty() {
            self.resolved.insert((pos.line, pos.col), order);
        }
        self.check_call(
            &callee_ty,
            &arg_types,
            has_spread,
            pos,
            explicit,
            expected,
        )
    }

    /// The types of the arguments of a call, checked against the
    /// parameters.
    ///
    /// A list of values may be a tuple, so the type that is expected is
    /// needed before the type of the argument is known.
    fn argument_types_against(
        &mut self,
        callee: &Type,
        args: &[Expr],
    ) -> Result<(Vec<Type>, bool), VeyError> {
        let expected: Vec<Type> = match callee {
            Type::Function(function) => function.params.clone(),
            _ => Vec::new(),
        };
        let mut types = Vec::new();
        let mut has_spread = false;
        for (index, arg) in args.iter().enumerate() {
            match arg {
                Expr::Spread(inner, _) => {
                    has_spread = true;
                    types.push(self.infer_expr(inner)?);
                }
                other => {
                    let want = expected.get(index).cloned();
                    types.push(self.infer_expr_as(other, want.as_ref())?);
                }
            }
        }
        Ok((types, has_spread))
    }

    /// The types of a list of arguments.
    /// The arguments of a call, in the order they are given, and the ones
    /// that are given by name.
    fn split_named(&mut self, args: &[Expr]) -> Result<(Vec<Expr>, Vec<(String, Expr)>), VeyError> {
        let count = args.len();
        let mut positional = Vec::new();
        let mut named = Vec::new();
        for (index, arg) in args.iter().enumerate() {
            // The last argument may hold the arguments that are given by
            // name.
            if index + 1 == count {
                if let Some(found) = self.named_arguments(arg) {
                    named = found;
                    continue;
                }
            }
            positional.push(arg.clone());
        }
        Ok((positional, named))
    }

    /// The arguments of a call that are given by name.
    fn named_arguments(&self, expr: &Expr) -> Option<Vec<(String, Expr)>> {
        let Expr::Object(props, _) = expr else {
            return None;
        };
        let mut named = Vec::new();
        let mut marked = false;
        for prop in props {
            let ObjectProp::Field(name, value) = prop else {
                continue;
            };
            if name == NAMED_MARKER {
                marked = true;
                continue;
            }
            named.push((name.clone(), value.clone()));
        }
        if marked {
            Some(named)
        } else {
            None
        }
    }

    /// Match the arguments that are given by name with the parameters.
    ///
    /// The arguments that are given by name fill their own parameter, and
    /// the ones that are given in order fill the parameters that are left.
    /// The arguments in the order of the parameters are returned.
    fn match_named(
        &mut self,
        callee: &Type,
        positional: &[Expr],
        named: &[(String, Expr)],
        pos: Pos,
    ) -> Result<Vec<Expr>, VeyError> {
        let Type::Function(function) = callee else {
            return Err(self.error(pos, "this value does not take its arguments by name"));
        };
        if function.param_names.is_empty() {
            return Err(self.error(
                pos,
                "this function does not take its arguments by name",
            ));
        }
        // The parameters that are given by name.
        let mut taken: Vec<usize> = Vec::new();
        for (name, value) in named {
            let Some(index) = function.param_names.iter().position(|n| n == name) else {
                return Err(self.error(
                    pos,
                    format!("this function has no parameter named '{}'", name),
                ));
            };
            let actual = self.infer_expr(value)?;
            let expected = &function.params[index];
            if !assignable(&actual, expected) {
                return Err(self.type_error(value.pos(), &actual, expected));
            }
            taken.push(index);
        }
        // The arguments in the order of the parameters.
        let mut order: Vec<Expr> = Vec::new();
        let mut at = 0usize;
        for index in 0..function.params.len() {
            if taken.contains(&index) {
                let value = named
                    .iter()
                    .find(|(name, _)| {
                        function.param_names.iter().position(|n| n == name) == Some(index)
                    })
                    .map(|(_, value)| value.clone());
                if let Some(value) = value {
                    order.push(value);
                }
                continue;
            }
            if at < positional.len() {
                order.push(positional[at].clone());
            }
            at += 1;
        }
        Ok(order)
    }

    /// Infer the type of an expression that is used as a certain type.
    ///
    /// The expected type settles the type parameters of a generic call that
    /// the arguments do not determine.
    pub(crate) fn infer_expr_as(
        &mut self,
        expr: &Expr,
        expected: Option<&Type>,
    ) -> Result<Type, VeyError> {
        match expr {
            Expr::Call(callee, args, optional, pos) => {
                self.infer_call(callee, args, *optional, *pos, expected, None)
            }
            // A list of values may be a tuple, when that is what is expected.
            Expr::Array(items, pos) => {                if let Some(Type::Tuple(wanted)) = expected {
                    if items.len() != wanted.len() {
                        return Err(self.error(
                            *pos,
                            format!(
                                "expected {} values, found {}",
                                wanted.len(),
                                items.len()
                            ),
                        ));
                    }
                    let mut out = Vec::new();
                    for (index, item) in items.iter().enumerate() {
                        let actual = self.infer_expr_as(item, Some(&wanted[index]))?;
                        if !assignable(&actual, &wanted[index]) {
                            return Err(self.type_error(item.pos(), &actual, &wanted[index]));
                        }
                        out.push(actual);
                    }
                    return Ok(Type::Tuple(out));
                }
                self.infer_expr(expr)
            }
            // An object may be a dictionary, when that is what is expected.
            Expr::Object(props, pos) => {
                if let Some(Type::Dict(key_ty, value_ty)) = expected {
                    for prop in props {
                        let ObjectProp::Field(_, value) = prop else {
                            return Err(self.error(*pos, "a dictionary has no spread"));
                        };
                        if !assignable(&Type::String, key_ty) {
                            return Err(self.type_error(*pos, &Type::String, key_ty));
                        }
                        let actual = self.infer_expr_as(value, Some(value_ty))?;
                        if !assignable(&actual, value_ty) {
                            return Err(self.type_error(value.pos(), &actual, value_ty));
                        }
                    }
                    return Ok(Type::Dict(key_ty.clone(), value_ty.clone()));
                }
                self.infer_expr(expr)
            }
            other => self.infer_expr(other),
        }
    }

    pub(crate) fn check_call(
        &self,
        callee: &Type,
        args: &[Type],
        has_spread: bool,
        pos: Pos,
        type_args: Option<&[Type]>,
        expected: Option<&Type>,
    ) -> Result<Type, VeyError> {
        match callee {
            Type::Any => Ok(Type::Any),
            Type::Function(function) => {
                // The number of arguments is always known, so it is always
                // checked, even when the types are not written.
                if !has_spread {
                    if function.rest {
                        if args.len() < function.required {
                            return Err(self.error(
                                pos,
                                format!(
                                    "expected at least {} {}, got {}",
                                    function.required,
                                    argument_label(function.required),
                                    args.len()
                                ),
                            ));
                        }
                    } else {
                        if args.len() < function.required {
                            return Err(self.error(
                                pos,
                                format!(
                                    "expected at least {} {}, got {}",
                                    function.required,
                                    argument_label(function.required),
                                    args.len()
                                ),
                            ));
                        }
                        if args.len() > function.params.len() {
                            return Err(self.error(
                                pos,
                                format!(
                                    "expected at most {} {}, got {}",
                                    function.params.len(),
                                    argument_label(function.params.len()),
                                    args.len()
                                ),
                            ));
                        }
                    }
                }

                // The types of the arguments are only checked when they are
                // known.
                if has_spread || !function.checked {
                    return Ok(substitute(&function.ret, &HashMap::new()));
                }

                let mut subst = HashMap::new();
                // The type arguments of the call fill the type parameters of
                // the function, so they do not have to be inferred.
                if let Some(explicit) = type_args {
                    let mut free = Vec::new();
                    for param in &function.params {
                        collect_type_vars(param, &mut free);
                    }
                    collect_type_vars(&function.ret, &mut free);

                    let names: Vec<String> = if function.type_params.is_empty() {
                        free.clone()
                    } else {
                        function.type_params.clone()
                    };
                    if explicit.len() != names.len() {
                        return Err(self.error(
                            pos,
                            format!(
                                "expected {} type {}, got {}",
                                names.len(),
                                type_argument_label(names.len()),
                                explicit.len()
                            ),
                        ));
                    }
                    for (name, ty) in names.iter().zip(explicit) {
                        // A parameter that was settled already keeps its type.
                        if free.iter().any(|item| item == name) {
                            subst.insert(name.clone(), ty.clone());
                        }
                    }
                }
                for (index, arg) in args.iter().enumerate() {
                    // A rest parameter stands for the type of a single extra
                    // argument, so the last entry is used for all of them.
                    let param = if index < function.params.len() {
                        &function.params[index]
                    } else {
                        function.params.last().unwrap_or(&Type::Any)
                    };
                    // A type parameter that is already bound stands for the
                    // type it was bound to.
                    let expected = match param {
                        Type::Var(name) => {
                            subst.get(name).cloned().unwrap_or_else(|| param.clone())
                        }
                        other => other.clone(),
                    };
                    let mut trial = subst.clone();
                    if unify_types(arg, &expected, &mut trial) {
                        subst = trial;
                    } else if !self.fits(arg, &expected) {
                        return Err(self.error(
                            pos,
                            format!(
                                "argument {} expects {}, found {}",
                                index + 1,
                                expected.display(),
                                arg.display()
                            ),
                        ));
                    }
                }

                // The type the call is used as may settle a type parameter
                // that the arguments do not determine.
                if let Some(expected) = expected {
                    let mut trial = subst.clone();
                    if unify_types(expected, &function.ret, &mut trial) {
                        subst = trial;
                    }
                }

                // A type parameter has to satisfy the constraint of the
                // function it was inferred for.
                for (name, constraint) in &function.constraints {
                    let Some(actual) = subst.get(name) else {
                        continue;
                    };
                    let constraint = substitute_bound(constraint, &subst);
                    if !self.fits(actual, &constraint) {
                        return Err(self.error(
                            pos,
                            format!(
                                "type argument '{}' expects {}, found {}",
                                name,
                                constraint.display(),
                                actual.display()
                            ),
                        ));
                    }
                }

                // A type parameter that could not be inferred is a mistake:
                // the answer of the call would be of an unknown type.
                let result = substitute(&function.ret, &subst);
                if has_type_var(&result) {
                    let mut names = Vec::new();
                    collect_type_vars(&result, &mut names);
                    return Err(self.error(
                        pos,
                        format!("cannot infer the type of {}", names.join(", ")),
                    ));
                }
                Ok(erase_type_vars(&result))
            }
            Type::Union(items) => {
                // Every member of the union has to take the arguments.
                let mut results = Vec::new();
                for item in items {
                    if matches!(item, Type::Null | Type::Undefined | Type::Void) {
                        continue;
                    }
                    let result =
                        self.check_call(item, args, has_spread, pos, type_args, expected)?;
                    results.push(result);
                }
                if results.is_empty() {
                    Ok(Type::Any)
                } else {
                    Ok(union(results))
                }
            }
            other => Err(self.error(
                pos,
                format!("value of type {} is not callable", other.display()),
            )),
        }
    }

    pub(crate) fn binary_result(
        &self,
        op: &str,
        left: &Type,
        right: &Type,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        match op {
            "+" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    return Ok(Type::Any);
                }
                if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else if assignable(left, &Type::String) && assignable(right, &Type::String) {
                    Ok(Type::String)
                } else if assignable(left, &Type::String) || assignable(right, &Type::String) {
                    // A text and a number are put together as a text.
                    Ok(Type::String)
                } else {
                    Err(self.error(
                        pos,
                        format!(
                            "operator '+' needs numbers or texts, found {} and {}",
                            left.display(),
                            right.display()
                        ),
                    ))
                }
            }
            "-" | "*" | "/" | "%" | "**" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    return Ok(Type::Any);
                }
                if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else {
                    Err(self.error(
                        pos,
                        format!(
                            "operator '{}' requires numbers, found {} and {}",
                            op,
                            left.display(),
                            right.display()
                        ),
                    ))
                }
            }
            "==" | "!=" | "===" | "!==" => {
                // Two values of different types are never the same, so the
                // comparison is most likely a mistake.
                if !comparable(left, right) {
                    return Err(self.error(
                        pos,
                        format!(
                            "{} and {} are never the same",
                            left.display(),
                            right.display()
                        ),
                    ));
                }
                Ok(Type::Bool)
            }
            "<" | "<=" | ">" | ">=" => {
                // Only numbers have an order. A value that may be nothing is
                // fine, because it is never greater than another value.
                if !orderable(left) || !orderable(right) {
                    return Err(self.error(
                        pos,
                        format!(
                            "operator '{}' requires numbers, found {} and {}",
                            op,
                            left.display(),
                            right.display()
                        ),
                    ));
                }
                Ok(Type::Bool)
            }
            "&" | "|" | "^" | "<<" | ">>" => {
                if matches!(left, Type::Any) || matches!(right, Type::Any) {
                    Ok(Type::Any)
                } else if assignable(left, &Type::Number) && assignable(right, &Type::Number) {
                    Ok(Type::Number)
                } else {
                    Err(self.error(
                        pos,
                        format!(
                            "operator '{}' requires numbers, found {} and {}",
                            op,
                            left.display(),
                            right.display()
                        ),
                    ))
                }
            }
            _ => Ok(Type::Any),
        }
    }

    pub(crate) fn member_type(
        &mut self,
        object: &Expr,
        prop: &MemberProp,
        optional: bool,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        // A member of another module is looked up in the environment of that
        // module, so that its signature is checked.
        if let (MemberProp::Name(name), Expr::Ident(alias, _)) = (prop, object) {
            // A local name of its own wins over the alias of a module.
            if self.shadowed.contains(alias) {
                let object_ty = self.infer_expr(object)?;
                let ty = self.lookup_member(&object_ty, name, pos)?;
                return Ok(if optional {
                    union(vec![ty, Type::Null])
                } else {
                    ty
                });
            }
            if let Some(env) = self.imports.get(alias) {
                if env.unchecked {
                    return Ok(Type::Any);
                }
                let ty = match env.values.get(name) {
                    Some(ty) => ty.clone(),
                    None => match env.types.get(name) {
                        Some(ty) => {
                            // The type may have functions of its own.
                            let ty = qualified_type(alias, ty.clone());
                            match &ty {
                                Type::Named(type_name, _) | Type::Interface(type_name, _) => {
                                    if let Some(methods) = env.impls.get(type_name) {
                                        if let Some(method) = methods.get(name) {
                                            return Ok(if optional {
                                                union(vec![method.clone(), Type::Null])
                                            } else {
                                                method.clone()
                                            });
                                        }
                                    }
                                }
                                _ => {}
                            }
                            ty
                        }
                        None => {
                            if env.components.contains_key(name) {
                                Type::Any
                            } else if env.enums.contains_key(name) {
                                Type::EnumNamespace(format!("{}.{}", alias, name))
                            } else if env.private.contains(name) {
                                return Err(self.error(
                                    pos,
                                    format!("'{}' is private to the module '{}'", name, alias),
                                ));
                            } else {
                                let mut error = self.error(
                                    pos,
                                    format!("module '{}' has no '{}'", alias, name),
                                );
                                let names: Vec<&str> = env
                                    .values
                                    .keys()
                                    .chain(env.types.keys())
                                    .chain(env.components.keys())
                                    .chain(env.enums.keys())
                                    .map(|name| name.as_str())
                                    .collect();
                                if let Some(hint) = did_you_mean(name, names) {
                                    error = error.with_hint(hint);
                                }
                                return Err(error);
                            }
                        }
                    },
                };
                return Ok(if optional {
                    union(vec![ty, Type::Null])
                } else {
                    ty
                });
            }
        }

        // A variant of an enum of another module, such as `t.Status.Nope`.
        if let (MemberProp::Name(variant), Expr::Member(base, MemberProp::Name(enum_name), _, _)) =
            (prop, object)
        {
            if let Expr::Ident(alias, _) = &**base {
                if let Some(env) = self.imports.get(alias) {
                    if let Some(info) = env.enums.get(enum_name) {
                        let Some(found) = info.variant(variant) else {
                            return Err(self.error(
                                pos,
                                format!("enum '{}' has no variant '{}'", enum_name, variant),
                            ));
                        };
                        let ty = self.variant_type(&format!("{}.{}", alias, enum_name), found)?;
                        return Ok(if optional {
                            union(vec![ty, Type::Null])
                        } else {
                            ty
                        });
                    }
                }
            }
        }

        let object_ty = self.infer_expr(object)?;
        let ty = match prop {
            MemberProp::Name(name) => self.lookup_member(&object_ty, name, pos)?,
            MemberProp::Computed(index) => {
                let index_ty = self.infer_expr(index)?;
                match &object_ty {
                    Type::Array(inner) if assignable(&index_ty, &Type::Number) => (**inner).clone(),
                    Type::String if assignable(&index_ty, &Type::Number) => Type::String,
                    Type::Tuple(items) if assignable(&index_ty, &Type::Number) => {
                        match literal_number(index) {
                            Some(index) => items.get(index).cloned().unwrap_or(Type::Any),
                            None => Type::Any,
                        }
                    }
                    Type::Array(_) | Type::String => {
                        return Err(self.type_error(pos, &index_ty, &Type::Number));
                    }
                    // Every key of a dictionary has the same type of value.
                    Type::Dict(_, value) => (**value).clone(),
                    _ => Type::Any,
                }
            }
        };
        if optional {
            Ok(union(vec![ty, Type::Null]))
        } else {
            Ok(ty)
        }
    }

    /// The type of a variant of an enum.
    ///
    /// A variant with fields is a function that takes them and gives the
    /// variant back.
    pub(crate) fn variant_type(
        &self,
        enum_name: &str,
        variant: &EnumVariantInfo,
    ) -> Result<Type, VeyError> {
        if variant.fields.is_empty() {
            return Ok(Type::Enum(enum_name.to_string(), variant.name.clone()));
        }
        let mut params = Vec::new();
        for field in &variant.fields {
            params.push(match &field.ty {
                Some(ty) => self.resolve_type(ty)?,
                None => Type::Any,
            });
        }
        Ok(Type::Function(FunctionType {
            params,
            required: variant.fields.len(),
            rest: false,
            ret: Box::new(Type::Enum(enum_name.to_string(), variant.name.clone())),
            checked: true,
            constraints: Vec::new(),
            type_params: Vec::new(),
            param_names: Vec::new(),
        }))
    }

    /// The error for a variant that an enum does not have.
    fn variant_error(
        &self,
        enum_name: &str,
        name: &str,
        info: &EnumInfo,
        pos: Pos,
    ) -> VeyError {
        let mut error = self.error(
            pos,
            format!("enum '{}' has no variant '{}'", enum_name, name),
        );
        let names: Vec<&str> = info
            .variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect();
        if let Some(hint) = did_you_mean(name, names) {
            error = error.with_hint(hint);
        }
        error
    }

    /// The type of a function of a type.
    pub(crate) fn method_type(&self, method: &ImplMethod) -> Result<Type, VeyError> {
        let mut params = Vec::new();
        for param in &method.params {
            params.push(match &param.ty {
                Some(ty) => self.resolve_type(ty)?,
                None => Type::Any,
            });
        }
        let ret = match &method.ret {
            Some(ty) => self.resolve_type(ty)?,
            None => Type::Any,
        };
        let required = params.len();
        Ok(Type::Function(FunctionType {
            params,
            required,
            rest: false,
            ret: Box::new(ret),
            checked: true,
            constraints: Vec::new(),
            type_params: Vec::new(),
            param_names: Vec::new(),
        }))
    }

    /// The variants of an enum, whether it is of this module or of another.
    pub(crate) fn enum_info(&self, name: &str) -> Option<&EnumInfo> {
        if let Some((module, name)) = name.split_once('.') {
            return self
                .imports
                .get(module)
                .and_then(|env| env.enums.get(name));
        }
        if let Some(info) = self.enums.get(name) {
            return Some(info);
        }
        // The modules are looked at in the order of their names, so that the
        // answer is the same every time.
        let mut aliases: Vec<&String> = self.imports.keys().collect();
        aliases.sort();
        for alias in aliases {
            if let Some(info) = self.imports[alias].enums.get(name) {
                return Some(info);
            }
        }
        None
    }

    pub(crate) fn lookup_member(&self, ty: &Type, name: &str, pos: Pos) -> Result<Type, VeyError> {
        match ty {
            Type::Any => Ok(Type::Any),
            Type::EnumNamespace(enum_name) => {
                let info = self
                    .enum_info(enum_name)
                    .ok_or_else(|| self.error(pos, format!("unknown enum '{}'", enum_name)))?;
                let variant = info.variants.iter().find(|v| v.name == name).ok_or_else(|| {
                    self.variant_error(enum_name, name, &info, pos)
                })?;
                self.variant_type(enum_name, variant)
            }
            Type::EnumUnion(enum_name) => {
                // The variants of an enum are not members of it, but the
                // name of the enum is.
                let info = self
                    .enum_info(enum_name)
                    .ok_or_else(|| self.error(pos, format!("unknown enum '{}'", enum_name)))?;
                let variant = info.variants.iter().find(|v| v.name == name).ok_or_else(|| {
                    self.variant_error(enum_name, name, &info, pos)
                })?;
                self.variant_type(enum_name, variant)
            }
            Type::Enum(enum_name, variant) => {
                let info = self
                    .enums
                    .get(enum_name)
                    .ok_or_else(|| self.error(pos, format!("unknown enum '{}'", enum_name)))?;
                let v = info
                    .variants
                    .iter()
                    .find(|v| v.name == *variant)
                    .ok_or_else(|| {
                        self.error(
                            pos,
                            format!("enum '{}' has no variant '{}'", enum_name, variant),
                        )
                    })?;
                v.fields
                    .iter()
                    .find(|f| f.name == name)
                    .map(|f| match &f.ty {
                        Some(ty) => self.resolve_type(ty),
                        None => Ok(Type::Any),
                    })
                    .unwrap_or(Ok(Type::Any))
            }
            Type::Object(fields) | Type::Named(_, fields) | Type::Interface(_, fields) => {
                if let Some(field) = fields.iter().find(|field| field.name == name) {
                    return Ok(field.ty.clone());
                }
                // The type may have functions of its own.
                let type_name = match ty {
                    Type::Named(type_name, _) | Type::Interface(type_name, _) => Some(type_name),
                    _ => None,
                };
                if let Some(type_name) = type_name {
                    if let Some(info) = self.impls.get(type_name) {
                        if let Some(method) = info.methods.iter().find(|m| m.name == name) {
                            return self.method_type(method);
                        }
                    }
                }
                Err(self.no_field_error(ty, name, pos))
            }
            Type::Array(_) if name == "length" => Ok(Type::Number),
            Type::String if name == "length" => Ok(Type::Number),
            Type::Union(items) => {
                let mut out = Vec::new();
                for item in items {
                    if matches!(item, Type::Null | Type::Void) {
                        continue;
                    }
                    match self.lookup_member(item, name, pos) {
                        Ok(ty) => out.push(ty),
                        Err(_) => return Ok(Type::Any),
                    }
                }
                if out.is_empty() {
                    Ok(Type::Null)
                } else {
                    Ok(union(out))
                }
            }
            // The values of the language have no fields of their own.
            Type::Number
            | Type::Bool
            | Type::Null
            | Type::Void
            | Type::Never
            | Type::Array(_)
            | Type::String => Err(self.no_field_error(ty, name, pos)),
            _ => Ok(Type::Any),
        }
    }

    /// The error for a field that a type does not have.
    ///
    /// The helpers of the runtime are free functions, so `upper(text)` is
    /// written and not `text.upper()`.
    fn no_field_error(&self, ty: &Type, name: &str, pos: Pos) -> VeyError {
        let error = self.error(
            pos,
            format!("type {} has no field '{}'", ty.display(), name),
        );
        if builtin_type(name).is_some() {
            return error.with_hint(format!("use {}(...) instead", name));
        }
        error
    }
}
