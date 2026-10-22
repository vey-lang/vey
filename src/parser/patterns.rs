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
                let first = self.advance()?.value;
                self.advance()?; // "."
                let second = self.expect_ident("enum name")?;
                // The enum of another module is written `module.Enum`, and
                // its variant `module.Enum.Variant`.
                let (enum_name, variant) = if self.is_punct(".") {
                    self.advance()?;
                    let variant = self.expect_ident("variant name")?;
                    (format!("{}.{}", first, second), variant)
                } else {
                    (first, second)
                };
                let mut bindings = Vec::new();
                if self.is_punct("(") {
                    self.advance()?;
                    while !self.is_punct(")") {
                        let bpos = self.tok.pos;
                        let name = self.expect_ident("binding name")?;
                        if self.is_punct(":") {
                            self.advance()?;
                            let second = self.expect_ident("binding name")?;
                            bindings.push(EnumBinding {
                                field: Some(name),
                                name: second,
                                pos: bpos,
                            });
                        } else {
                            bindings.push(EnumBinding {
                                field: None,
                                name,
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

            // A name that is not a pattern takes the value of the match.
            if self.tok.kind == TokKind::Ident {
                let pos = self.tok.pos;
                let name = self.advance()?.value;
                return Ok(Pattern::Binding { name, pos });
            }
        }

        let negative = self.eat_op("-")?;
        let token = self.tok.clone();

        // A list of patterns, such as `[1, name]`.
        if token.kind == TokKind::Punct && token.value == "[" {
            let pos = self.advance()?.pos;
            let mut items = Vec::new();
            while !self.is_punct("]") {
                items.push(self.parse_pattern()?);
                if self.is_punct(",") {
                    self.advance()?;
                } else {
                    break;
                }
            }
            self.expect_punct("]")?;
            return Ok(Pattern::List(items, pos));
        }

        // The fields of an object, such as `{ name, age }`.
        if token.kind == TokKind::Punct && token.value == "{" {
            let pos = self.advance()?.pos;
            let mut fields = Vec::new();
            while !self.is_punct("}") {
                let key_token = self.tok.clone();
                let key = match key_token.kind {
                    TokKind::Ident | TokKind::Keyword => {
                        self.advance()?;
                        key_token.value
                    }
                    TokKind::Str => {
                        self.advance()?;
                        key_token
                            .parts
                            .iter()
                            .map(|part| part.text.as_str())
                            .collect()
                    }
                    _ => return Err(self.error("expected a field name")),
                };
                let pattern = if self.is_punct(":") {
                    self.advance()?;
                    self.parse_pattern()?
                } else {
                    Pattern::Binding {
                        name: key.clone(),
                        pos: key_token.pos,
                    }
                };
                fields.push((key, pattern));
                if self.is_punct(",") {
                    self.advance()?;
                } else {
                    break;
                }
            }
            self.expect_punct("}")?;
            return Ok(Pattern::Object(fields, pos));
        }

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
                let text = self.parse_string_token(&token)?;
                // A text that begins with something, such as `"Vey "..`.
                if self.is_op("..") {
                    self.advance()?;
                    return Ok(Pattern::Prefix(text, token.pos));
                }
                Ok(Pattern::Literal(text))
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
