//! Parsing of type expressions.

use super::Parser;
use crate::ast::*;
use crate::lexer::TokKind;
use crate::VeyError;

impl Parser {
    pub(crate) fn parse_type(&mut self) -> Result<TypeExpr, VeyError> {
        self.enter()?;
        let result = self.parse_type_inner();
        self.leave();
        result
    }

    fn parse_type_inner(&mut self) -> Result<TypeExpr, VeyError> {
        let first = self.parse_optional_type()?;
        let mut parts = vec![first];
        while self.is_op("|") {
            self.advance()?;
            parts.push(self.parse_optional_type()?);
        }
        if parts.len() == 1 {
            Ok(parts.remove(0))
        } else {
            let pos = parts[0].pos();
            Ok(TypeExpr::Union(parts, pos))
        }
    }

    pub(crate) fn parse_optional_type(&mut self) -> Result<TypeExpr, VeyError> {
        let mut ty = self.parse_primary_type()?;
        while self.is_op("?") {
            let pos = self.advance()?.pos;
            ty = TypeExpr::Optional(Box::new(ty), pos);
        }
        Ok(ty)
    }

    pub(crate) fn parse_primary_type(&mut self) -> Result<TypeExpr, VeyError> {
        if self.is_punct("[") {
            let pos = self.advance()?.pos;
            let inner = self.parse_type()?;
            self.expect_punct("]")?;
            return Ok(TypeExpr::Array(Box::new(inner), pos));
        }

        if self.is_punct("{") {
            return self.parse_object_type();
        }

        if self.is_punct("(") {
            let pos = self.advance()?.pos;
            let mut params = Vec::new();
            let mut rest = false;
            while !self.is_punct(")") {
                if self.eat_op("...")? {
                    rest = true;
                }
                params.push(self.parse_type()?);
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
            self.expect_op("->")?;
            let ret = self.parse_type()?;
            return Ok(TypeExpr::Function(params, Box::new(ret), rest, pos));
        }

        let token = self.tok.clone();
        if token.kind == TokKind::Ident || (token.kind == TokKind::Keyword && token.value == "null")
        {
            self.advance()?;
            if self.is_punct(".") {
                self.advance()?;
                let second = self.expect_ident("type name")?;
                if self.is_op("<") {
                    self.advance()?;
                    let mut args = Vec::new();
                    while !self.is_op(">") {
                        args.push(self.parse_type()?);
                        if self.is_punct(",") {
                            self.advance()?;
                        } else {
                            break;
                        }
                    }
                    if args.is_empty() {
                        return Err(self.error("expected a type argument"));
                    }
                    self.expect_op(">")?;
                    return Ok(TypeExpr::Generic(
                        format!("{}.{}", token.value, second),
                        args,
                        token.pos,
                    ));
                }
                return Ok(TypeExpr::Qualified(token.value, second, token.pos));
            }
            if self.is_op("<") {
                self.advance()?;
                let mut args = Vec::new();
                while !self.is_op(">") {
                    args.push(self.parse_type()?);
                    if self.is_punct(",") {
                        self.advance()?;
                    } else {
                        break;
                    }
                }
                if args.is_empty() {
                    return Err(self.error("expected a type argument"));
                }
                self.expect_op(">")?;
                return Ok(TypeExpr::Generic(token.value, args, token.pos));
            }
            return Ok(TypeExpr::Name(token.value, token.pos));
        }

        Err(self.error("expected a type"))
    }

    pub(crate) fn parse_object_type(&mut self) -> Result<TypeExpr, VeyError> {
        let pos = self.expect_punct("{")?.pos;
        let mut fields = Vec::new();

        while !self.is_punct("}") {
            let token = self.tok.clone();
            let name = match token.kind {
                TokKind::Ident | TokKind::Keyword => {
                    self.advance()?;
                    token.value
                }
                _ => return Err(self.error("expected a type field name")),
            };
            let optional = self.eat_op("?")?;
            self.expect_punct(":")?;
            let ty = self.parse_type()?;
            let line = token.pos.line;
            fields.push(TypeField {
                name,
                ty,
                optional,
                pos: token.pos,
            });

            if self.is_punct(",") {
                self.advance()?;
            } else if self.tok.kind == TokKind::Eof || self.tok.pos.line <= line {
                // The fields may also be written one under the other.
                break;
            }
        }

        self.expect_punct("}")?;
        Ok(TypeExpr::Object(fields, pos))
    }
}
