//! The parsing of string literals.

use crate::ast::*;
use crate::lexer::{Pos, TokKind, Token};
use crate::VeyError;

use super::Parser;

impl Parser {
    pub(crate) fn parse_string_token(&self, token: &Token) -> Result<Expr, VeyError> {
        let mut parts = Vec::new();
        for part in &token.parts {
            if let Some((source, pos)) = &part.expr {
                match parse_interpolation(source, &self.file, *pos)? {
                    Some(expr) => parts.push(StrPart {
                        text: String::new(),
                        expr: Some(expr),
                    }),
                    // The braces hold text rather than an expression, for
                    // example a JSON object, so they stay literal.
                    None => parts.push(StrPart {
                        text: format!("{{{}}}", crate::lexer::process_escapes(source)),
                        expr: None,
                    }),
                }
            } else {
                parts.push(StrPart {
                    text: part.text.clone(),
                    expr: None,
                });
            }
        }
        Ok(Expr::String(parts, token.pos))
    }
}

/// Parse the source of a string interpolation.
///
/// `Ok(Some(expr))` is a real interpolation, `Ok(None)` means the braces hold
/// text instead of an expression and should stay literal, and an error means
/// the expression is broken and has to be reported.
fn parse_interpolation(source: &str, file: &str, pos: Pos) -> Result<Option<Expr>, VeyError> {
    if source.trim().is_empty() {
        // "{}" is plain text, not an interpolation.
        return Ok(None);
    }
    let mut sub = match Parser::with_pos(source, file, pos.line, pos.col) {
        Ok(parser) => parser,
        // The text is not Vey source at all, for example a JSON object.
        Err(_) => return Ok(None),
    };
    let expr = sub.parse_expression()?;
    if sub.tok.kind != TokKind::Eof {
        return Ok(None);
    }
    Ok(Some(expr))
}
