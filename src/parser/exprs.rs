//! The parsing of expressions.

use crate::ast::*;
use crate::lexer::{Pos, TokKind};
use crate::VeyError;

use super::Parser;

impl Parser {
    pub(crate) fn parse_pipe(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_ternary()?;
        while self.is_op("|>") {
            self.advance()?;
            let right = self.parse_ternary()?;
            let pos = right.pos();
            left = match right {
                Expr::Call(callee, mut args, optional, p) => {
                    args.insert(0, left);
                    Expr::Call(callee, args, optional, p)
                }
                other => Expr::Call(Box::new(other), vec![left], false, pos),
            };
        }
        Ok(left)
    }

    pub(crate) fn parse_ternary(&mut self) -> Result<Expr, VeyError> {
        let test = self.parse_nullish()?;
        if self.is_op("?") {
            let pos = self.tok.pos;
            self.advance()?;
            let cons = self.parse_expression()?;
            self.expect_punct(":")?;
            let alt = self.parse_expression()?;
            return Ok(Expr::Ternary(
                Box::new(test),
                Box::new(cons),
                Box::new(alt),
                pos,
            ));
        }
        Ok(test)
    }

    pub(crate) fn parse_nullish(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_or()?;
        while self.is_op("??") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_or()?;
            left = Expr::Nullish(Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_or(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_and()?;
        while self.is_kw("or") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_and()?;
            left = Expr::Logical("||".into(), Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_and(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_equality()?;
        while self.is_kw("and") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_equality()?;
            left = Expr::Logical("&&".into(), Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_equality(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_comparison()?;
        while self.is_op("==") || self.is_op("!=") {
            let op_tok = self.advance()?;
            let op = if op_tok.value == "==" { "===" } else { "!==" };
            let right = self.parse_comparison()?;
            left = Expr::Binary(op.into(), Box::new(left), Box::new(right), op_tok.pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_comparison(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_range()?;
        while self.is_op("<") || self.is_op("<=") || self.is_op(">") || self.is_op(">=") {
            let op_tok = self.advance()?;
            let right = self.parse_range()?;
            left = Expr::Binary(op_tok.value, Box::new(left), Box::new(right), op_tok.pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_range(&mut self) -> Result<Expr, VeyError> {
        let start = self.parse_additive()?;
        if self.is_op("..") {
            let pos = self.advance()?.pos;
            let end = self.parse_additive()?;
            return Ok(Expr::Range(Box::new(start), Box::new(end), pos));
        }
        Ok(start)
    }

    pub(crate) fn parse_additive(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_multiplicative()?;
        while self.is_op("+") || self.is_op("-") {
            let op = self.advance()?;
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(op.value, Box::new(left), Box::new(right), op.pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_multiplicative(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_power()?;
        while self.is_op("*") || self.is_op("/") || self.is_op("%") {
            let op = self.advance()?;
            let right = self.parse_power()?;
            left = Expr::Binary(op.value, Box::new(left), Box::new(right), op.pos);
        }
        Ok(left)
    }

    pub(crate) fn parse_power(&mut self) -> Result<Expr, VeyError> {
        let left = self.parse_unary()?;
        if self.is_op("**") {
            let pos = self.advance()?.pos;
            let right = self.parse_power()?;
            return Ok(Expr::Binary(
                "**".into(),
                Box::new(left),
                Box::new(right),
                pos,
            ));
        }
        Ok(left)
    }

    pub(crate) fn parse_unary(&mut self) -> Result<Expr, VeyError> {
        if self.is_op("-") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Unary("-".into(), Box::new(self.parse_unary()?), pos));
        }
        if self.is_kw("not") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Unary("!".into(), Box::new(self.parse_unary()?), pos));
        }
        if self.is_kw("await") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Await(Box::new(self.parse_unary()?), pos));
        }
        self.parse_postfix()
    }

    pub(crate) fn parse_postfix(&mut self) -> Result<Expr, VeyError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.is_punct(".") {
                let pos = self.advance()?.pos;
                let name = self.expect_ident("property name")?;
                expr = Expr::Member(Box::new(expr), MemberProp::Name(name), false, pos);
            } else if self.is_op("?.") {
                let pos = self.advance()?.pos;
                if self.is_punct("(") {
                    let args = self.parse_args()?;
                    expr = Expr::Call(Box::new(expr), args, true, pos);
                } else if self.is_punct("[") {
                    self.advance()?;
                    let prop = self.parse_expression()?;
                    self.expect_punct("]")?;
                    expr = Expr::Member(
                        Box::new(expr),
                        MemberProp::Computed(Box::new(prop)),
                        true,
                        pos,
                    );
                } else {
                    let name = self.expect_ident("property name")?;
                    expr = Expr::Member(Box::new(expr), MemberProp::Name(name), true, pos);
                }
            } else if self.is_punct("[") {
                let pos = self.advance()?.pos;
                let prop = self.parse_expression()?;
                self.expect_punct("]")?;
                expr = Expr::Member(
                    Box::new(expr),
                    MemberProp::Computed(Box::new(prop)),
                    false,
                    pos,
                );
            } else if self.is_punct("(") {
                let pos = self.tok.pos;
                let args = self.parse_args()?;
                expr = Expr::Call(Box::new(expr), args, false, pos);
            } else if self.is_op("<") {
                // The call may name its type arguments, as in
                // `parse<number>(text)`. When the text after the `<` is not a
                // list of types it is a comparison instead.
                let saved = self.clone();
                match self.try_parse_type_arguments() {
                    Ok(Some(type_args)) if self.is_punct("(") => {
                        let pos = self.tok.pos;
                        let args = self.parse_args()?;
                        expr = Expr::CallGeneric(Box::new(expr), type_args, args, false, pos);
                    }
                    _ => {
                        *self = saved;
                        break;
                    }
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// Read the type arguments of a call, if the text is a list of types.
    pub(crate) fn try_parse_type_arguments(&mut self) -> Result<Option<Vec<TypeExpr>>, VeyError> {
        if !self.is_op("<") {
            return Ok(None);
        }
        self.advance()?;
        let mut args = Vec::new();
        while !self.is_op(">") {
            if self.tok.kind == TokKind::Eof {
                return Ok(None);
            }
            match self.parse_type() {
                Ok(ty) => args.push(ty),
                Err(_) => return Ok(None),
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        if !self.is_op(">") {
            return Ok(None);
        }
        self.advance()?;
        Ok(Some(args))
    }

    pub(crate) fn parse_args(&mut self) -> Result<Vec<Expr>, VeyError> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        while !self.is_punct(")") {
            if self.is_op("...") {
                let pos = self.advance()?.pos;
                let value = self.parse_expression()?;
                args.push(Expr::Spread(Box::new(value), pos));
            } else {
                args.push(self.parse_expression()?);
            }
            if self.is_op("=") {
                return Err(self
                    .error("named arguments are only supported in markup")
                    .with_hint(
                        "use `Card title=\"value\"` inside markup, or pass the value in order",
                    ));
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    pub(crate) fn parse_primary(&mut self) -> Result<Expr, VeyError> {
        let token = self.tok.clone();

        match token.kind {
            TokKind::Number => {
                self.advance()?;
                return Ok(Expr::Number(token.number, token.pos));
            }
            TokKind::Str => {
                self.advance()?;
                return self.parse_string_token(&token);
            }
            TokKind::Ident => {
                let next = self.peek()?;
                if next.kind == TokKind::Op && next.value == "=>" {
                    self.advance()?;
                    self.advance()?;
                    let body = self.parse_arrow_body()?;
                    return Ok(Expr::Arrow(
                        vec![Param {
                            pattern: BindingPattern::Name {
                                name: token.value,
                                pos: token.pos,
                            },
                            ty: None,
                            default: None,
                            rest: false,
                            pos: token.pos,
                        }],
                        body,
                        token.pos,
                    ));
                }
                self.advance()?;
                return Ok(Expr::Ident(token.value, token.pos));
            }
            TokKind::Punct if token.value == "(" => return self.parse_group_or_arrow(),
            TokKind::Punct if token.value == "[" => return self.parse_array(),
            TokKind::Punct if token.value == "{" => return self.parse_object(),
            TokKind::Keyword if token.value == "fn" => {
                self.advance()?;
                let params = self.parse_params()?;
                let ret = if self.is_op("->") {
                    self.advance()?;
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = self.parse_block()?;
                return Ok(Expr::Fn(params, body, ret, token.pos));
            }
            TokKind::Keyword if token.value == "match" => return self.parse_match(),
            TokKind::Keyword if token.value == "true" => {
                self.advance()?;
                return Ok(Expr::Bool(true, token.pos));
            }
            TokKind::Keyword if token.value == "false" => {
                self.advance()?;
                return Ok(Expr::Bool(false, token.pos));
            }
            TokKind::Keyword if token.value == "null" => {
                self.advance()?;
                return Ok(Expr::Null(token.pos));
            }
            TokKind::Keyword if token.value == "async" => {
                return Err(self
                    .reserved_word_error("an expression")
                    .with_hint("a function that uses `await` is already async"));
            }
            _ => {}
        }

        Err(self.error(format!("unexpected token '{}'", token.value)))
    }

    pub(crate) fn try_parse_typed_arrow(&mut self) -> Result<Option<Expr>, VeyError> {
        let pos = self.tok.pos;
        let saved = self.clone();
        let mut committed = false;
        let result: Result<Option<Expr>, VeyError> = (|| {
            self.expect_punct("(")?;
            let mut params = Vec::new();
            while !self.is_punct(")") {
                let param_pos = self.tok.pos;
                let rest = self.eat_op("...")?;
                let name = self.expect_ident("parameter name")?;
                let ty = if self.is_punct(":") {
                    self.advance()?;
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let default = if self.eat_op("=")? {
                    Some(self.parse_expression()?)
                } else {
                    None
                };
                params.push(Param {
                    pattern: BindingPattern::Name {
                        name,
                        pos: param_pos,
                    },
                    ty,
                    default,
                    rest,
                    pos: param_pos,
                });
                if rest {
                    break;
                }
                if self.is_punct(",") {
                    self.advance()?;
                } else {
                    break;
                }
            }
            self.expect_punct(")")?;
            if !self.is_op("=>") {
                return Ok(None);
            }
            self.advance()?;
            committed = true;
            let body = self.parse_arrow_body()?;
            Ok(Some(Expr::Arrow(params, body, pos)))
        })();

        match result {
            Ok(Some(expr)) => Ok(Some(expr)),
            Err(err) if committed => Err(err),
            _ => {
                *self = saved;
                Ok(None)
            }
        }
    }

    pub(crate) fn parse_group_or_arrow(&mut self) -> Result<Expr, VeyError> {
        if let Some(expr) = self.try_parse_typed_arrow()? {
            return Ok(expr);
        }

        let pos = self.advance()?.pos;
        if self.is_punct(")") {
            self.advance()?;
            self.expect_op("=>")?;
            let body = self.parse_arrow_body()?;
            return Ok(Expr::Arrow(Vec::new(), body, pos));
        }

        let mut exprs = vec![self.parse_expression()?];
        while self.is_punct(",") {
            self.advance()?;
            exprs.push(self.parse_expression()?);
        }
        self.expect_punct(")")?;

        if self.is_op("=>") {
            self.advance()?;
            let mut params = Vec::new();
            for expr in exprs {
                let Some(pattern) = pattern_from_expr(&expr) else {
                    return Err(self.error_at(pos, "arrow parameters must be names or patterns"));
                };
                let pos = pattern_pos(&pattern);
                params.push(Param {
                    pattern,
                    ty: None,
                    default: None,
                    rest: false,
                    pos,
                });
            }
            let body = self.parse_arrow_body()?;
            return Ok(Expr::Arrow(params, body, pos));
        }

        if exprs.len() > 1 {
            return Err(self.error_at(pos, "comma expressions are not supported"));
        }
        Ok(exprs.remove(0))
    }

    pub(crate) fn parse_arrow_body(&mut self) -> Result<ArrowBody, VeyError> {
        if self.is_punct("{") {
            return Ok(ArrowBody::Block(self.parse_block()?));
        }
        Ok(ArrowBody::Expr(Box::new(self.parse_expression()?)))
    }

    pub(crate) fn parse_array(&mut self) -> Result<Expr, VeyError> {
        let pos = self.advance()?.pos;
        let mut elements = Vec::new();
        while !self.is_punct("]") {
            if self.is_op("...") {
                let spread_pos = self.advance()?.pos;
                let value = self.parse_expression()?;
                elements.push(Expr::Spread(Box::new(value), spread_pos));
            } else {
                elements.push(self.parse_expression()?);
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct("]")?;
        Ok(Expr::Array(elements, pos))
    }

    pub(crate) fn parse_object(&mut self) -> Result<Expr, VeyError> {
        let pos = self.advance()?.pos;
        let mut props = Vec::new();
        while !self.is_punct("}") {
            if self.is_op("...") {
                self.advance()?;
                props.push(ObjectProp::Spread(self.parse_expression()?));
            } else {
                let key_tok = self.tok.clone();
                let (key, shorthand) = match key_tok.kind {
                    TokKind::Ident | TokKind::Keyword => {
                        self.advance()?;
                        (key_tok.value, true)
                    }
                    TokKind::Str => {
                        self.advance()?;
                        if key_tok.parts.iter().any(|p| p.expr.is_some()) {
                            return Err(
                                self.error_at(key_tok.pos, "object keys cannot interpolate")
                            );
                        }
                        let text: String = key_tok.parts.iter().map(|p| p.text.as_str()).collect();
                        (text, false)
                    }
                    TokKind::Number => {
                        self.advance()?;
                        (key_tok.value, false)
                    }
                    _ => return Err(self.error("expected an object key")),
                };

                let value = if self.is_punct(":") {
                    self.advance()?;
                    self.parse_expression()?
                } else if shorthand {
                    Expr::Ident(key.clone(), key_tok.pos)
                } else {
                    return Err(self.error("expected ':' after object key"));
                };
                props.push(ObjectProp::Field(key, value));
            }

            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct("}")?;
        Ok(Expr::Object(props, pos))
    }

    pub(crate) fn parse_match(&mut self) -> Result<Expr, VeyError> {
        let pos = self.expect_kw("match")?.pos;
        let subject = self.parse_expression()?;
        self.expect_punct("{")?;
        let mut arms = Vec::new();
        while !self.is_punct("}") {
            let mut patterns = vec![self.parse_pattern()?];
            while self.is_op("|") {
                self.advance()?;
                patterns.push(self.parse_pattern()?);
            }
            self.expect_op("=>")?;
            let body = self.parse_expression()?;
            arms.push(MatchArm { patterns, body });
            if self.is_punct(",") {
                self.advance()?;
            }
        }
        self.expect_punct("}")?;
        Ok(Expr::Match(Box::new(subject), arms, pos))
    }
}

/// The pattern of a parameter, when the expression is one.
///
/// An arrow may take its parameters apart, as in `({ name }) => name`.
fn pattern_from_expr(expr: &Expr) -> Option<BindingPattern> {
    match expr {
        Expr::Ident(name, pos) => Some(BindingPattern::Name {
            name: name.clone(),
            pos: *pos,
        }),
        Expr::Object(props, pos) => {
            let mut fields = Vec::new();
            for prop in props {
                let ObjectProp::Field(key, value) = prop else {
                    return None;
                };
                let pattern = pattern_from_expr(value)?;
                fields.push(BindingField {
                    key: key.clone(),
                    pattern,
                    pos: *pos,
                });
            }
            Some(BindingPattern::Object {
                fields,
                rest: None,
                pos: *pos,
            })
        }
        Expr::Array(items, pos) => {
            let mut patterns = Vec::new();
            for item in items {
                patterns.push(pattern_from_expr(item)?);
            }
            Some(BindingPattern::Array {
                items: patterns,
                rest: None,
                pos: *pos,
            })
        }
        _ => None,
    }
}

/// The place of a pattern.
fn pattern_pos(pattern: &BindingPattern) -> Pos {
    match pattern {
        BindingPattern::Name { pos, .. }
        | BindingPattern::Array { pos, .. }
        | BindingPattern::Object { pos, .. } => *pos,
    }
}
