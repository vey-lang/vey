//! The generation of expressions.

use crate::ast::*;
use crate::VeyError;

use super::awaits::{block_has_await, block_has_yield, expr_has_await};
use super::js::{format_number, js_key, js_string};
use super::Codegen;

/// The name of an enum, without the name of the module it comes from.
///
/// The value of an enum carries the name it was given where it was written,
/// so that two enums of the same name do not mix.
fn local_enum_name(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

impl Codegen {
    /// The code of an expression, with a mark that says where it came from.
    ///
    /// The mark of the expression that is inside another one comes first in
    /// the line, so that a debugger that looks for the last mark before a
    /// column finds the inner one.
    pub(crate) fn gen_expr(&mut self, expr: &Expr) -> Result<String, VeyError> {
        let text = self.gen_expr_inner(expr)?;
        Ok(format!("{}{}", self.mark(expr.pos()), text))
    }

    pub(crate) fn gen_expr_inner(&mut self, expr: &Expr) -> Result<String, VeyError> {
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
            Expr::Call(callee, args, optional, pos) => {
                let callee = self.gen_expr(callee)?;
                let mut parts = Vec::new();
                // The arguments that are given by name are put in the order
                // of the parameters.
                let order = self.resolved.get(&(pos.line, pos.col)).cloned();
                match order {
                    Some(order) => {
                        for arg in &order {
                            parts.push(self.gen_expr(arg)?);
                        }
                    }
                    None => {
                        for arg in args {
                            parts.push(self.gen_expr(arg)?);
                        }
                    }
                }
                if *optional {
                    Ok(format!("{}?.({})", callee, parts.join(", ")))
                } else {
                    Ok(format!("{}({})", callee, parts.join(", ")))
                }
            }
            // The type arguments are only used by the checker and are erased.
            Expr::CallGeneric(callee, _, args, optional, _) => self.gen_expr_inner(&Expr::Call(
                callee.clone(),
                args.clone(),
                *optional,
                expr.pos(),
            )),
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
            Expr::Arrow(params, body, _, _) => self.gen_arrow(params, body),
            Expr::Range(start, end, _) => Ok(format!(
                "V.range({}, {})",
                self.gen_expr(start)?,
                self.gen_expr(end)?
            )),
            Expr::Match(subject, arms, _) => self.gen_match(subject, arms),
            Expr::Await(arg, _) => Ok(format!("(await {})", self.gen_expr(arg)?)),
            Expr::Try(arg, _) => Ok(format!("V.propagate({})", self.gen_expr(arg)?)),
            // A cast is erased: the value is the same.
            Expr::As(arg, _, _) => self.gen_expr(arg),
            Expr::Is(arg, ty, _) => {
                let value = self.gen_expr(arg)?;
                Ok(format!("V.is({}, {})", value, js_string(&kind_of(ty))))
            }
            Expr::Yield(arg, _) => Ok(format!("(yield {})", self.gen_expr(arg)?)),
            Expr::Spread(arg, _) => Ok(format!("...{}", self.gen_expr(arg)?)),
        }
    }

    /// The condition of a pattern, and the names it binds.
    ///
    /// The condition says whether a value fits the pattern, and the names
    /// are the values that the pattern takes apart.
    pub(crate) fn gen_pattern(
        &mut self,
        pattern: &Pattern,
        subject: &str,
    ) -> Result<(String, Vec<(String, String)>), VeyError> {
        match pattern {
            Pattern::Binding { name, .. } => Ok((
                "true".to_string(),
                vec![(name.clone(), subject.to_string())],
            )),
            Pattern::Wildcard => Ok(("true".to_string(), Vec::new())),
            Pattern::Literal(expr) => {
                let value = self.gen_expr(expr)?;
                Ok((format!("{} === {}", subject, value), Vec::new()))
            }
            Pattern::Range(start, end) => {
                let start = self.gen_expr(start)?;
                let end = self.gen_expr(end)?;
                Ok((
                    format!("({} >= {} && {} < {})", subject, start, subject, end),
                    Vec::new(),
                ))
            }
            Pattern::Enum {
                enum_name,
                variant,
                bindings,
                ..
            } => {
                let condition = format!(
                    "{} != null && {}.__veyEnum === {} && {}.__veyVariant === {}",
                    subject,
                    subject,
                    js_string(local_enum_name(enum_name)),
                    subject,
                    js_string(variant)
                );
                let fields = self
                    .enums
                    .get(enum_name)
                    .map(|info| info.field_names(variant))
                    .unwrap_or_default();
                let mut taken = Vec::new();
                let mut positional = 0usize;
                for binding in bindings {
                    let field = if let Some(field) = &binding.field {
                        field.clone()
                    } else {
                        let index = positional;
                        positional += 1;
                        fields.get(index).cloned().unwrap_or_else(|| binding.name.clone())
                    };
                    taken.push((binding.name.clone(), format!("{}.{}", subject, field)));
                }
                Ok((condition, taken))
            }
            Pattern::List(items, _) => {
                // A list of patterns, such as `[1, name]`.
                let mut conditions = vec![format!("Array.isArray({})", subject)];
                conditions.push(format!("{}.length === {}", subject, items.len()));
                let mut taken = Vec::new();
                for (index, item) in items.iter().enumerate() {
                    match item {
                        Pattern::Binding { name, .. } => {
                            taken.push((name.clone(), format!("{}[{}]", subject, index)));
                        }
                        Pattern::Wildcard => {}
                        Pattern::Literal(expr) => {
                            let value = self.gen_expr(expr)?;
                            conditions.push(format!("{}[{}] === {}", subject, index, value));
                        }
                        other => {
                            let (condition, bindings) =
                                self.gen_pattern(other, &format!("{}[{}]", subject, index))?;
                            conditions.push(format!("({})", condition));
                            taken.extend(bindings);
                        }
                    }
                }
                Ok((conditions.join(" && "), taken))
            }
            Pattern::Object(fields, _) => {
                // The fields of an object, such as `{ name, age }`.
                let mut conditions = vec![format!("{} !== null", subject)];
                let mut taken = Vec::new();
                for (key, pattern) in fields {
                    let value = format!("{}[{}]", subject, js_string(key));
                    match pattern {
                        Pattern::Binding { name, .. } => taken.push((name.clone(), value)),
                        Pattern::Wildcard => {}
                        Pattern::Literal(expr) => {
                            let text = self.gen_expr(expr)?;
                            conditions.push(format!("{} === {}", value, text));
                        }
                        other => {
                            let (condition, bindings) = self.gen_pattern(other, &value)?;
                            conditions.push(format!("({})", condition));
                            taken.extend(bindings);
                        }
                    }
                }
                Ok((conditions.join(" && "), taken))
            }
            Pattern::Prefix(expr, _) => {
                // A text that begins with something, such as `"Vey "..`.
                let text = self.gen_expr(expr)?;
                Ok((
                    format!(
                        "typeof {} === \"string\" && {}.startsWith({})",
                        subject, subject, text
                    ),
                    Vec::new(),
                ))
            }
        }
    }

    pub(crate) fn gen_string(&mut self, parts: &[StrPart]) -> Result<String, VeyError> {
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

    pub(crate) fn gen_fn_expr(
        &mut self,
        params: &[Param],
        body: &Block,
    ) -> Result<String, VeyError> {
        let params_text = self.gen_params(params)?;
        let async_kw = if block_has_await(body) { "async " } else { "" };
        let generator = if block_has_yield(body) { "*" } else { "" };
        let captured = self.capture(|codegen| {
            codegen.push_scope();
            for param in params {
                for (name, _) in param.bindings() {
                    codegen.declare(name);
                }
            }
            codegen.gen_body(body)?;
            codegen.pop_scope();
            Ok(())
        })?;
        let pad = "  ".repeat(self.indent);
        Ok(format!(
            "({}function{}({}) {{\n{}\n{}}})",
            async_kw,
            generator,
            params_text,
            captured.trim_end_matches('\n'),
            pad
        ))
    }

    pub(crate) fn gen_arrow(
        &mut self,
        params: &[Param],
        body: &ArrowBody,
    ) -> Result<String, VeyError> {
        let param_text = self.gen_params(params)?;
        match body {
            ArrowBody::Expr(expr) => {
                let value = self.gen_expr(expr)?;
                let async_kw = if expr_has_await(expr) { "async " } else { "" };
                Ok(format!("({}({}) => {})", async_kw, param_text, value))
            }
            ArrowBody::Block(block) => {
                let async_kw = if block_has_await(block) { "async " } else { "" };
                let generator = if block_has_yield(block) { "*" } else { "" };
                let captured = self.capture(|codegen| {
                    codegen.push_scope();
                    for param in params {
                        for (name, _) in param.bindings() {
                            codegen.declare(name);
                        }
                    }
                    codegen.gen_body(block)?;
                    codegen.pop_scope();
                    Ok(())
                })?;
                let pad = "  ".repeat(self.indent);
                Ok(format!(
                    "({}{}({}) => {{\n{}\n{}}})",
                    async_kw,
                    generator,
                    param_text,
                    captured.trim_end_matches('\n'),
                    pad
                ))
            }
        }
    }

    pub(crate) fn gen_match(
        &mut self,
        subject: &Expr,
        arms: &[MatchArm],
    ) -> Result<String, VeyError> {
        let subject = self.gen_expr(subject)?;
        let temp = format!("__match{}", self.temp);
        self.temp += 1;
        let captured = self.capture(|codegen| {
            let mut has_default = false;
            for arm in arms {
                // The names that the patterns of the arm bind.
                let mut bound: Vec<String> = Vec::new();
                for pattern in &arm.patterns {
                    match pattern {
                        Pattern::Enum { bindings, .. } => {
                            for binding in bindings {
                                if !bound.contains(&binding.name) {
                                    bound.push(binding.name.clone());
                                }
                            }
                        }
                        Pattern::Binding { name, .. } if !bound.contains(name) => {
                            bound.push(name.clone());
                        }
                        _ => {}
                    }
                }

                // The body is written in a scope with the names in it.
                codegen.push_scope();
                for name in &bound {
                    codegen.declare(name);
                }
                let body = codegen.gen_expr(&arm.body)?;
                codegen.pop_scope();

                // The condition of the arm is written with the names in
                // it as well.
                let guard = match &arm.guard {
                    Some(guard) => {
                        codegen.push_scope();
                        for name in &bound {
                            codegen.declare(name);
                        }
                        let text = codegen.gen_expr(guard)?;
                        codegen.pop_scope();
                        Some(text)
                    }
                    None => None,
                };

                // A wildcard without a condition takes every value.
                if arm.guard.is_none()
                    && arm
                        .patterns
                        .iter()
                        .any(|pattern| matches!(pattern, Pattern::Wildcard))
                {
                    codegen.emit(&format!("return {};", body));
                    has_default = true;
                    break;
                }

                // The condition of every pattern of the arm.
                let mut conditions: Vec<String> = Vec::new();
                // The patterns that take a value apart. They need a block
                // of their own, because the values are read after the
                // pattern is known to hold.
                let mut blocks: Vec<(String, Vec<(String, String)>)> = Vec::new();
                // The names that take the value of the match.
                let mut value_names: Vec<String> = Vec::new();
                for pattern in &arm.patterns {
                    match pattern {
                        // A name always holds, it only takes the value.
                        Pattern::Wildcard => {}
                        Pattern::Binding { name, .. } => value_names.push(name.clone()),
                        other => {
                            let (condition, taken) = codegen.gen_pattern(other, &temp)?;
                            if taken.is_empty() {
                                conditions.push(condition);
                            } else {
                                blocks.push((condition, taken));
                            }
                        }
                    }
                }

                // The arm that holds the value of the match.
                if !value_names.is_empty() {
                    codegen.emit("{");
                    codegen.indent += 1;
                    for name in &value_names {
                        codegen.emit(&format!("const _{} = {};", name, temp));
                    }
                    match &guard {
                        Some(guard) => codegen.emit(&format!("if ({}) return {};", guard, body)),
                        None => codegen.emit(&format!("return {};", body)),
                    }
                    codegen.indent -= 1;
                    codegen.emit("}");
                    continue;
                }

                // The arm that takes the fields of an enum.
                if !blocks.is_empty() {
                    for (condition, taken) in &blocks {
                        codegen.emit(&format!("if ({}) {{", condition));
                        codegen.indent += 1;
                        for (name, expression) in taken {
                            codegen.emit(&format!("const _{} = {};", name, expression));
                        }
                        match &guard {
                            Some(guard) => {
                                codegen.emit(&format!("if ({}) return {};", guard, body))
                            }
                            None => codegen.emit(&format!("return {};", body)),
                        }
                        codegen.indent -= 1;
                        codegen.emit("}");
                    }
                    continue;
                }

                // The arm that is one condition.
                let mut all = conditions;
                if let Some(guard) = guard {
                    all.push(guard);
                }
                if all.is_empty() {
                    codegen.emit(&format!("return {};", body));
                } else {
                    codegen.emit(&format!("if ({}) return {};", all.join(" || "), body));
                }
            }
            if !has_default {
                // The match does not take every value, so the program says
                // so instead of handing back nothing.
                codegen.emit("V.error(\"the match is not exhaustive\");");
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
}

/// The kind of a value at runtime, which is what a check of a type looks at.
fn kind_of(ty: &TypeExpr) -> String {
    match ty {
        TypeExpr::Name(name, _) => match name.as_str() {
            "number" | "int" => "number".to_string(),
            "string" => "string".to_string(),
            "bool" | "boolean" => "boolean".to_string(),
            "null" => "null".to_string(),
            "undefined" => "undefined".to_string(),
            other => other.to_string(),
        },
        TypeExpr::Array(_, _) => "list".to_string(),
        TypeExpr::Object(_, _) => "object".to_string(),
        TypeExpr::Tuple(_, _) => "list".to_string(),
        TypeExpr::Function(_, _, _, _) => "function".to_string(),
        _ => "object".to_string(),
    }
}
