//! The generation of expressions.

use crate::ast::*;
use crate::VeyError;

use super::awaits::{block_has_await, expr_has_await};
use super::js::{format_number, js_key, js_string};
use super::Codegen;

impl Codegen {
    pub(crate) fn gen_expr(&mut self, expr: &Expr) -> Result<String, VeyError> {
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
            Expr::Call(callee, args, optional, _) => {
                let callee = self.gen_expr(callee)?;
                let mut parts = Vec::new();
                for arg in args {
                    parts.push(self.gen_expr(arg)?);
                }
                if *optional {
                    Ok(format!("{}?.({})", callee, parts.join(", ")))
                } else {
                    Ok(format!("{}({})", callee, parts.join(", ")))
                }
            }
            // The type arguments are only used by the checker and are erased.
            Expr::CallGeneric(callee, _, args, optional, _) => self.gen_expr(&Expr::Call(
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
            Expr::Arrow(params, body, _) => self.gen_arrow(params, body),
            Expr::Range(start, end, _) => Ok(format!(
                "V.range({}, {})",
                self.gen_expr(start)?,
                self.gen_expr(end)?
            )),
            Expr::Match(subject, arms, _) => self.gen_match(subject, arms),
            Expr::Await(arg, _) => Ok(format!("(await {})", self.gen_expr(arg)?)),
            Expr::Spread(arg, _) => Ok(format!("...{}", self.gen_expr(arg)?)),
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
        let captured = self.capture(|codegen| {
            codegen.push_scope();
            for param in params {
                codegen.declare(&param.name);
            }
            for stmt in &body.body {
                codegen.gen_stmt(stmt)?;
            }
            codegen.pop_scope();
            Ok(())
        })?;
        let pad = "  ".repeat(self.indent);
        Ok(format!(
            "({}function({}) {{\n{}\n{}}})",
            async_kw,
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
                let captured = self.capture(|codegen| {
                    codegen.push_scope();
                    for param in params {
                        codegen.declare(&param.name);
                    }
                    for stmt in &block.body {
                        codegen.gen_stmt(stmt)?;
                    }
                    codegen.pop_scope();
                    Ok(())
                })?;
                let pad = "  ".repeat(self.indent);
                Ok(format!(
                    "({}({}) => {{\n{}\n{}}})",
                    async_kw,
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
        let captured =
            self.capture(|codegen| {
                let mut has_default = false;
                for arm in arms {
                    if arm.patterns.iter().any(|p| matches!(p, Pattern::Wildcard)) {
                        let body = codegen.gen_expr(&arm.body)?;
                        codegen.emit(&format!("return {};", body));
                        has_default = true;
                        break;
                    }
                    // determine binding names for this arm
                    let mut binding_names: Vec<String> = Vec::new();
                    for pattern in &arm.patterns {
                        if let Pattern::Enum { bindings, .. } = pattern {
                            for b in bindings {
                                if !binding_names.contains(&b.name) {
                                    binding_names.push(b.name.clone());
                                }
                            }
                        }
                    }
                    // generate body in a scope with bindings declared
                    codegen.push_scope();
                    for name in &binding_names {
                        codegen.declare(name);
                    }
                    let body = codegen.gen_expr(&arm.body)?;
                    codegen.pop_scope();

                    // check if any pattern has bindings -> per-pattern blocks
                    let has_bindings = arm.patterns.iter().any(
                        |p| matches!(p, Pattern::Enum { bindings, .. } if !bindings.is_empty()),
                    );

                    if has_bindings {
                        for pattern in &arm.patterns {
                            match pattern {
                                Pattern::Enum {
                                    enum_name,
                                    variant,
                                    bindings,
                                    ..
                                } => {
                                    let cond = format!(
                                    "{} != null && {}.__veyEnum === {} && {}.__veyVariant === {}",
                                    temp, temp, js_string(enum_name), temp, js_string(variant)
                                );
                                    codegen.emit(&format!("if ({}) {{", cond));
                                    codegen.indent += 1;
                                    // resolve variant field names from registry
                                    let variant_fields = codegen
                                        .enums
                                        .get(enum_name)
                                        .and_then(|v| v.get(variant))
                                        .cloned()
                                        .unwrap_or_default();
                                    let mut positional_idx = 0usize;
                                    for b in bindings {
                                        let field = if let Some(f) = &b.field {
                                            f.clone()
                                        } else {
                                            let idx = positional_idx;
                                            positional_idx += 1;
                                            variant_fields
                                                .get(idx)
                                                .cloned()
                                                .unwrap_or_else(|| b.name.clone())
                                        };
                                        codegen.emit(&format!(
                                            "const _{} = {}.{};",
                                            b.name, temp, field
                                        ));
                                    }
                                    codegen.emit(&format!("return {};", body));
                                    codegen.indent -= 1;
                                    codegen.emit("}");
                                }
                                Pattern::Literal(expr) => {
                                    let cond = format!("{} === {}", temp, codegen.gen_expr(expr)?);
                                    codegen.emit(&format!("if ({}) return {};", cond, body));
                                }
                                Pattern::Range(start, end) => {
                                    let cond = format!(
                                        "({} >= {} && {} < {})",
                                        temp,
                                        codegen.gen_expr(start)?,
                                        temp,
                                        codegen.gen_expr(end)?
                                    );
                                    codegen.emit(&format!("if ({}) return {};", cond, body));
                                }
                                _ => {}
                            }
                        }
                    } else {
                        let mut conditions = Vec::new();
                        for pattern in &arm.patterns {
                            match pattern {
                                Pattern::Literal(expr) => conditions.push(format!(
                                    "{} === {}",
                                    temp,
                                    codegen.gen_expr(expr)?
                                )),
                                Pattern::Range(start, end) => conditions.push(format!(
                                    "({} >= {} && {} < {})",
                                    temp,
                                    codegen.gen_expr(start)?,
                                    temp,
                                    codegen.gen_expr(end)?
                                )),
                                Pattern::Enum {
                                    enum_name, variant, ..
                                } => {
                                    conditions.push(format!(
                                    "{} != null && {}.__veyEnum === {} && {}.__veyVariant === {}",
                                    temp, temp, js_string(enum_name), temp, js_string(variant)
                                ));
                                }
                                _ => {}
                            }
                        }
                        codegen.emit(&format!(
                            "if ({}) return {};",
                            conditions.join(" || "),
                            body
                        ));
                    }
                }
                if !has_default {
                    codegen.emit("return undefined;");
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
