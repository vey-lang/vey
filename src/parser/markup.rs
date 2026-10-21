//! Parsing of markup.

use super::html::{is_html_tag, is_void_tag};
use super::Parser;
use crate::ast::*;
use crate::lexer::TokKind;
use crate::VeyError;

impl Parser {
    pub(crate) fn parse_markup_nodes(&mut self) -> Result<Vec<MarkupNode>, VeyError> {
        let mut nodes = Vec::new();
        while !self.is_punct("}") && self.tok.kind != TokKind::Eof {
            if self.is_kw("if") {
                nodes.push(MarkupNode::If(self.parse_markup_if()?));
                continue;
            }
            if self.is_kw("for") {
                nodes.push(MarkupNode::For(self.parse_markup_for()?));
                continue;
            }
            if self.is_element_start()? {
                nodes.push(MarkupNode::Element(self.parse_element()?));
                continue;
            }
            nodes.push(MarkupNode::Expr(self.parse_expression()?));
        }
        Ok(nodes)
    }

    pub(crate) fn is_element_start(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident {
            return Ok(false);
        }
        let first = self.tok.value.chars().next().unwrap();
        if first.is_uppercase() {
            return Ok(true);
        }
        let next = self.peek()?;
        if next.kind == TokKind::Punct
            && (next.value == "{" || next.value == "." || next.value == "#")
        {
            return Ok(is_html_tag(&self.tok.value));
        }
        if self.attr_ahead()? {
            return Ok(true);
        }
        if is_void_tag(&self.tok.value) {
            return Ok(true);
        }
        Ok(false)
    }

    pub(crate) fn attr_ahead(&mut self) -> Result<bool, VeyError> {
        if self.peek()?.kind != TokKind::Ident {
            return Ok(false);
        }
        let mut k = 2;
        loop {
            let dash = self.peek_n(k)?;
            if dash.kind == TokKind::Op
                && dash.value == "-"
                && self.peek_n(k + 1)?.kind == TokKind::Ident
            {
                k += 2;
                continue;
            }
            break;
        }
        let eq = self.peek_n(k)?;
        Ok(eq.kind == TokKind::Op && eq.value == "=")
    }

    pub(crate) fn attr_here(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident {
            return Ok(false);
        }
        let mut k = 1;
        loop {
            let token = self.peek_n(k)?;
            if token.kind == TokKind::Op && token.value == "-" {
                if self.peek_n(k + 1)?.kind == TokKind::Ident {
                    k += 2;
                    continue;
                }
                return Ok(false);
            }
            return Ok(token.kind == TokKind::Op && token.value == "=");
        }
    }

    pub(crate) fn parse_element(&mut self) -> Result<Element, VeyError> {
        let tag_token = self.advance()?;
        let component = tag_token
            .value
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase());
        let mut element = Element {
            tag: tag_token.value,
            component,
            classes: Vec::new(),
            id: None,
            attrs: Vec::new(),
            args: Vec::new(),
            children: Vec::new(),
            pos: tag_token.pos,
        };

        loop {
            if self.is_punct(".") || self.is_punct("#") {
                let selector = self.advance()?.value;
                let name = self.parse_attr_name()?;
                if selector == "." {
                    element.classes.push(name);
                } else {
                    element.id = Some(name);
                }
                continue;
            }
            if self.attr_here()? {
                let name = self.parse_attr_name()?;
                self.expect_op("=")?;
                let value = self.parse_expression()?;
                element.attrs.push((name, value));
                continue;
            }
            break;
        }

        // Components may be written as `Card(...) { ... }`; the arguments are
        // passed to the component parameters in order.
        if element.component && self.is_punct("(") {
            self.advance()?;
            while !self.is_punct(")") {
                element.args.push(self.parse_expression()?);
                if self.is_punct(",") {
                    self.advance()?;
                } else {
                    break;
                }
            }
            self.expect_punct(")")?;
        }

        if self.is_punct("{") {
            self.advance()?;
            element.children = self.parse_markup_nodes()?;
            self.expect_punct("}")?;
        }

        Ok(element)
    }

    pub(crate) fn parse_attr_name(&mut self) -> Result<String, VeyError> {
        let mut name = self.expect_ident("attribute name")?;
        while self.is_op("-") {
            self.advance()?;
            name.push('-');
            name.push_str(&self.expect_ident("attribute name part")?);
        }
        Ok(name)
    }

    pub(crate) fn parse_markup_if(&mut self) -> Result<MIf, VeyError> {
        self.expect_kw("if")?;
        let test = self.parse_expression()?;
        self.expect_punct("{")?;
        let then_nodes = self.parse_markup_nodes()?;
        self.expect_punct("}")?;
        let mut else_nodes = Vec::new();
        if self.eat_kw("else")? {
            if self.is_kw("if") {
                else_nodes.push(MarkupNode::If(self.parse_markup_if()?));
            } else {
                self.expect_punct("{")?;
                else_nodes = self.parse_markup_nodes()?;
                self.expect_punct("}")?;
            }
        }
        Ok(MIf {
            test,
            then_nodes,
            else_nodes,
        })
    }

    pub(crate) fn parse_markup_for(&mut self) -> Result<MFor, VeyError> {
        self.expect_kw("for")?;
        let name = self.expect_ident("loop variable")?;
        self.expect_kw("in")?;
        let iter = self.parse_expression()?;
        self.expect_punct("{")?;
        let body = self.parse_markup_nodes()?;
        self.expect_punct("}")?;
        Ok(MFor { name, iter, body })
    }
}
