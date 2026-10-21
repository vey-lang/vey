//! Parsing of binding patterns.

use super::Parser;
use crate::ast::*;
use crate::lexer::TokKind;
use crate::VeyError;

impl Parser {
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
            pos: token.pos,
        })
    }

    pub(crate) fn parse_object_pattern(&mut self) -> Result<BindingPattern, VeyError> {
        let pos = self.expect_punct("{")?.pos;
        let mut fields = Vec::new();
        let mut rest = None;

        while !self.is_punct("}") {
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
                    pos: key_token.pos,
                }
            };

            fields.push(BindingField {
                key,
                pattern,
                pos: key_token.pos,
            });

            if self.is_punct(",") {
                self.advance()?;
            } else {
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

            items.push(self.parse_binding_pattern()?);
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }

        self.expect_punct("]")?;
        Ok(BindingPattern::Array { items, rest, pos })
    }
}
