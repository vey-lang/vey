//! Parsing of binding patterns.

use super::Parser;
use crate::ast::*;
use crate::lexer::TokKind;
use crate::VeyError;

impl Parser {
    pub(crate) fn parse_pattern(&mut self) -> Result<Pattern, VeyError> {
        if self.tok.kind == TokKind::Ident && self.tok.value == "_" {
            self.advance()?;
            return Ok(Pattern::Wildcard);
        }

        if self.tok.kind == TokKind::Ident {
            let next = self.peek()?;
            if next.kind == TokKind::Punct && next.value == "." {
                let pos = self.tok.pos;
                let enum_name = self.advance()?.value;
                self.advance()?; // "."
                let variant = self.expect_ident("variant name")?;
                let mut bindings = Vec::new();
                if self.is_punct("(") {
                    self.advance()?;
                    while !self.is_punct(")") {
                        let bpos = self.tok.pos;
                        let first = self.expect_ident("binding name")?;
                        if self.is_punct(":") {
                            self.advance()?;
                            let second = self.expect_ident("binding name")?;
                            bindings.push(EnumBinding {
                                field: Some(first),
                                name: second,
                                pos: bpos,
                            });
                        } else {
                            bindings.push(EnumBinding {
                                field: None,
                                name: first,
                                pos: bpos,
                            });
                        }
                        if self.is_punct(",") {
                            self.advance()?;
                        } else {
                            break;
                        }
                    }
                    self.expect_punct(")")?;
                }
                return Ok(Pattern::Enum {
                    enum_name,
                    variant,
                    bindings,
                    pos,
                });
            }
        }

        let negative = self.eat_op("-")?;
        let token = self.tok.clone();

        if token.kind == TokKind::Number {
            self.advance()?;
            let start = Expr::Number(
                if negative {
                    -token.number
                } else {
                    token.number
                },
                token.pos,
            );
            if self.is_op("..") {
                self.advance()?;
                let end_neg = self.eat_op("-")?;
                let end_tok = self.tok.clone();
                if end_tok.kind != TokKind::Number {
                    return Err(self.error("expected a number after '..'"));
                }
                self.advance()?;
                let end = Expr::Number(
                    if end_neg {
                        -end_tok.number
                    } else {
                        end_tok.number
                    },
                    end_tok.pos,
                );
                return Ok(Pattern::Range(start, end));
            }
            return Ok(Pattern::Literal(start));
        }

        if negative {
            return Err(self.error("expected a number after '-'"));
        }

        match token.kind {
            TokKind::Str => {
                self.advance()?;
                Ok(Pattern::Literal(self.parse_string_token(&token)?))
            }
            TokKind::Keyword if token.value == "true" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Bool(true, token.pos)))
            }
            TokKind::Keyword if token.value == "false" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Bool(false, token.pos)))
            }
            TokKind::Keyword if token.value == "null" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Null(token.pos)))
            }
            _ => Err(self.error("expected a literal, range or _ in match pattern")),
        }
    }

    pub(crate) fn parse_binding_pattern(&mut self) -> Result<BindingPattern, VeyError> {
        if self.is_punct("{") {
            return self.parse_object_pattern();
        }
        if self.is_punct("[") {
            return self.parse_array_pattern();
        }

        let token = self.tok.clone();
        let name = self.expect_ident("binding name")?;
        Ok(BindingPattern::Name {
            name,
            default: None,
            pos: token.pos,
        })
    }

    pub(crate) fn parse_object_pattern(&mut self) -> Result<BindingPattern, VeyError> {
        let pos = self.expect_punct("{")?.pos;
        let mut fields = Vec::new();
        let mut rest = None;

        while !self.is_punct("}") {
            let line = self.tok.pos.line;
            if self.eat_op("...")? {
                rest = Some(self.expect_ident("rest name")?);
                if self.is_punct(",") {
                    self.advance()?;
                }
                break;
            }

            let key_token = self.tok.clone();
            let key = match key_token.kind {
                TokKind::Ident | TokKind::Keyword => {
                    self.advance()?;
                    key_token.value
                }
                TokKind::Str => {
                    self.advance()?;
                    if key_token.parts.iter().any(|part| part.expr.is_some()) {
                        return Err(self.error_at(key_token.pos, "binding keys cannot interpolate"));
                    }
                    key_token
                        .parts
                        .iter()
                        .map(|part| part.text.as_str())
                        .collect()
                }
                _ => return Err(self.error("expected a binding key")),
            };

            let pattern = if self.is_punct(":") {
                self.advance()?;
                self.parse_binding_pattern()?
            } else {
                if key_token.kind != TokKind::Ident {
                    return Err(self.error_at(key_token.pos, "expected ':' after binding key"));
                }
                BindingPattern::Name {
                    name: key.clone(),
                    default: None,
                    pos: key_token.pos,
                }
            };

            // A name may have a value that is used when there is none.
            let pattern = self.attach_default(pattern)?;

            fields.push(BindingField {
                key,
                pattern,
                pos: key_token.pos,
            });

            if self.is_punct(",") {
                self.advance()?;
            } else if self.tok.kind == TokKind::Eof || self.tok.pos.line <= line {
                // The fields may also be written one under the other.
                break;
            }
        }

        self.expect_punct("}")?;
        Ok(BindingPattern::Object { fields, rest, pos })
    }

    pub(crate) fn parse_array_pattern(&mut self) -> Result<BindingPattern, VeyError> {
        let pos = self.expect_punct("[")?.pos;
        let mut items = Vec::new();
        let mut rest = None;

        while !self.is_punct("]") {
            if self.eat_op("...")? {
                rest = Some(self.expect_ident("rest name")?);
                if self.is_punct(",") {
                    self.advance()?;
                }
                break;
            }

            let item = self.parse_binding_pattern()?;
            items.push(self.attach_default(item)?);
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }

        self.expect_punct("]")?;
        Ok(BindingPattern::Array { items, rest, pos })
    }

    /// Give a pattern the value that is used when it has none.
    fn attach_default(&mut self, pattern: BindingPattern) -> Result<BindingPattern, VeyError> {
        if !self.is_op("=") {
            return Ok(pattern);
        }
        let pos = self.tok.pos;
        self.advance()?;
        let default = self.parse_expression()?;
        match pattern {
            BindingPattern::Name {
                name,
                pos: name_pos,
                ..
            } => Ok(BindingPattern::Name {
                name,
                default: Some(default),
                pos: name_pos,
            }),
            _ => Err(self.error_at(pos, "a default value needs a name, such as { name = 1 }")),
        }
    }
}
