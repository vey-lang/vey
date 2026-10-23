//! Parsing of markup.

use super::html::is_known_tag;
use super::Parser;
use crate::ast::*;
use crate::lexer::TokKind;
use crate::VeyError;

impl Parser {
    pub(crate) fn parse_markup_nodes(&mut self) -> Result<Vec<MarkupNode>, VeyError> {
        self.parse_markup_nodes_in(false)
    }

    /// The nodes of a markup.
    ///
    /// The children of a component may go in a slot of their own, so the
    /// name of the slot is read from them.
    pub(crate) fn parse_markup_nodes_in(
        &mut self,
        in_component: bool,
    ) -> Result<Vec<MarkupNode>, VeyError> {
        // A markup that is nested too deep takes the compiler down, so the
        // depth of it is watched.
        self.enter()?;
        let result = self.parse_markup_nodes_inner(in_component);
        self.leave();
        result
    }

    fn parse_markup_nodes_inner(
        &mut self,
        in_component: bool,
    ) -> Result<Vec<MarkupNode>, VeyError> {
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
            if self.is_kw("match") {
                nodes.push(MarkupNode::Match(self.parse_markup_match()?));
                continue;
            }
            if self.is_element_start()? {
                nodes.push(MarkupNode::Element(self.parse_element(in_component)?));
                continue;
            }
            // A name with a block behind it is meant to be a tag, so the tag
            // is named when it is not one.
            if let Some(name) = self.unknown_tag()? {
                return Err(self.error_at(self.tok.pos, format!("unknown tag '{}'", name)));
            }
            nodes.push(MarkupNode::Expr(self.parse_expression()?));
        }
        Ok(nodes)
    }

    /// The name of a tag that is not a tag of HTML and not a component.
    fn unknown_tag(&mut self) -> Result<Option<String>, VeyError> {
        if self.tok.kind != TokKind::Ident && self.tok.kind != TokKind::Keyword {
            return Ok(None);
        }
        let name = self.tok.value.clone();
        if name
            .chars()
            .next()
            .is_some_and(|first| first.is_uppercase())
        {
            return Ok(None);
        }
        if is_known_tag(&name) {
            return Ok(None);
        }
        // A custom element of the web, such as `my-element`, is a tag
        // even when it is not one of the tags the markup knows.
        if name.contains('-') {
            return Ok(None);
        }
        let next = self.peek()?;
        if next.kind == TokKind::Punct && next.value == "{" {
            return Ok(Some(name));
        }
        // A name with attributes behind it is a tag as well.
        if self.attr_ahead()? {
            return Ok(Some(name));
        }
        Ok(None)
    }

    pub(crate) fn is_element_start(&mut self) -> Result<bool, VeyError> {
        // A word of the language may be the name of a tag, such as the
        // `use` of SVG.
        if self.tok.kind != TokKind::Ident && self.tok.kind != TokKind::Keyword {
            return Ok(false);
        }
        let first = self.tok.value.chars().next();
        if first.is_some_and(|first| first.is_uppercase()) {
            return Ok(true);
        }
        // A component of another module, such as `ui.Card`.
        if self.qualified_name_ahead()? {
            return Ok(true);
        }
        // A custom element of the web, such as `my-element`. The name of
        // such an element holds a hyphen, so the token behind it is an
        // operator that is read as part of the name.
        if self.custom_element_ahead()? {
            return Ok(true);
        }
        let next = self.peek()?;
        if next.kind == TokKind::Punct
            && (next.value == "{" || next.value == "." || next.value == "#")
        {
            return Ok(is_known_tag(&self.tok.value));
        }
        if next.kind == TokKind::Op && next.value == "=" {
            // The tag has an attribute of its own, such as the name of a
            // slot: `slot="body"`.
            return Ok(is_known_tag(&self.tok.value));
        }
        if self.attr_ahead()? {
            // A tag that the markup knows is a tag, with attributes or
            // without.
            return Ok(is_known_tag(&self.tok.value));
        }
        Ok(false)
    }

    /// Whether the current token begins a custom element, such as
    /// `my-element`. A custom element's name holds a hyphen.
    fn custom_element_ahead(&mut self) -> Result<bool, VeyError> {
        let next = self.peek()?;
        if next.kind != TokKind::Op || next.value != "-" {
            return Ok(false);
        }
        // There has to be another word behind the hyphen.
        let after = self.peek_n(2)?;
        Ok(after.kind == TokKind::Ident)
    }

    /// Whether the tag is the name of a component of another module.
    ///
    /// The name of a module starts with a small letter and the name of the
    /// component with a big one, so that `div.card` stays a tag with a class.
    pub(crate) fn qualified_name_ahead(&mut self) -> Result<bool, VeyError> {
        let dot = self.peek()?;
        if dot.kind != TokKind::Punct || dot.value != "." {
            return Ok(false);
        }
        let name = self.peek_n(2)?;
        Ok(name.kind == TokKind::Ident
            && name
                .value
                .chars()
                .next()
                .is_some_and(|first| first.is_uppercase()))
    }

    /// Whether the current token is the dot of a name of a component.
    pub(crate) fn qualified_name_here(&mut self) -> Result<bool, VeyError> {
        if !self.is_punct(".") {
            return Ok(false);
        }
        let name = self.peek()?;
        Ok(name.kind == TokKind::Ident
            && name
                .value
                .chars()
                .next()
                .is_some_and(|first| first.is_uppercase()))
    }

    pub(crate) fn attr_ahead(&mut self) -> Result<bool, VeyError> {
        if self.peek()?.kind != TokKind::Ident {
            return Ok(false);
        }
        // The name of an attribute may have parts, such as `xlink:href` or
        // `data-value`.
        let mut k = 2;
        loop {
            let part = self.peek_n(k)?;
            let is_part = (part.kind == TokKind::Op && part.value == "-")
                || (part.kind == TokKind::Punct && part.value == ":");
            if is_part && self.peek_n(k + 1)?.kind == TokKind::Ident {
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
        // The name of an attribute may have parts, such as `xlink:href` or
        // `data-value`.
        let mut k = 1;
        loop {
            let token = self.peek_n(k)?;
            let is_part = (token.kind == TokKind::Op && token.value == "-")
                || (token.kind == TokKind::Punct && token.value == ":");
            if is_part {
                if self.peek_n(k + 1)?.kind == TokKind::Ident {
                    k += 2;
                    continue;
                }
                return Ok(false);
            }
            return Ok(token.kind == TokKind::Op && token.value == "=");
        }
    }

    pub(crate) fn parse_element(&mut self, in_component: bool) -> Result<Element, VeyError> {
        let tag_token = self.advance()?;
        // A custom element may hold a hyphen, such as `my-element`. The
        // hyphen is an operator in the lexer, so it is read back here and
        // folded into the name of the tag.
        let mut tag = tag_token.value.clone();
        while self.is_op("-") {
            self.advance()?;
            let part = self.expect_ident("custom element name part")?;
            tag.push('-');
            tag.push_str(&part);
        }
        let component = tag
            .chars()
            .next()
            .is_some_and(|c| c.is_uppercase());
        let mut element = Element {
            tag,
            component,
            classes: Vec::new(),
            id: None,
            attrs: Vec::new(),
            spreads: Vec::new(),
            args: Vec::new(),
            slot: None,
            children: Vec::new(),
            pos: tag_token.pos,
        };

        // A component of another module, such as `ui.Card`.
        if self.qualified_name_here()? {
            self.advance()?;
            let name = self.expect_ident("component name")?;
            element.tag = format!("{}.{}", element.tag, name);
            element.component = true;
        }

        loop {
            // The props may be spread into the element.
            if self.is_op("...") {
                self.advance()?;
                element.spreads.push(self.parse_expression()?);
                continue;
            }
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
                let value = self.parse_attr_value()?;
                // The name of the slot the element goes in. It is a name of
                // a child of a component only; on a node of its own it is
                // an attribute of HTML.
                if name == "slot" && in_component {
                    if let Expr::String(parts, _) = &value {
                        if let Some(part) = parts.first() {
                            if part.expr.is_none() {
                                element.slot = Some(part.text.clone());
                                continue;
                            }
                        }
                    }
                }
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
            element.children = self.parse_markup_nodes_in(element.component)?;
            self.expect_punct("}")?;
        }

        Ok(element)
    }

    /// The value of an attribute.
    ///
    /// The value may be written in braces, like `{name}`, so that it reads
    /// the way it does in other languages of the web. A brace that holds the
    /// fields of an object is still an object.
    fn parse_attr_value(&mut self) -> Result<Expr, VeyError> {
        if !self.is_punct("{") || self.object_ahead()? {
            return self.parse_expression();
        }
        self.advance()?;
        let value = self.parse_expression()?;
        self.expect_punct("}")?;
        Ok(value)
    }

    /// Whether the brace at the cursor holds an object.
    fn object_ahead(&mut self) -> Result<bool, VeyError> {
        let next = self.peek()?;
        if next.kind == TokKind::Punct && next.value == "}" {
            return Ok(true);
        }
        if next.kind != TokKind::Ident && next.kind != TokKind::Str {
            return Ok(false);
        }
        let after = self.peek_n(2)?;
        Ok(after.kind == TokKind::Punct && after.value == ":")
    }

    pub(crate) fn parse_attr_name(&mut self) -> Result<String, VeyError> {
        let mut name = self.expect_ident("attribute name")?;
        // The name of an attribute may have parts, such as `xlink:href` or
        // `data-value`.
        loop {
            if self.is_op("-") {
                self.advance()?;
                name.push('-');
                name.push_str(&self.expect_ident("attribute name part")?);
                continue;
            }
            if self.is_punct(":") {
                self.advance()?;
                name.push(':');
                name.push_str(&self.expect_ident("attribute name part")?);
                continue;
            }
            break;
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
        // The place of the item in the list may be wanted as well.
        let index = if self.is_punct(",") {
            self.advance()?;
            Some(self.expect_ident("loop index")?)
        } else {
            None
        };
        self.expect_kw("in")?;
        let iter = self.parse_expression()?;
        // The items of the list may be told apart, so that a list may change
        // without the page being built again. The name `key` is not a word
        // of the language, so it is a name that is written here.
        let key = if self.tok.kind == TokKind::Ident && self.tok.value == "key" {
            self.advance()?;
            self.expect_op("=")?;
            Some(self.parse_expression()?)
        } else {
            None
        };
        let body_start = self.expect_punct("{")?.pos;
        let body = self.parse_markup_nodes()?;
        let body_end = self.expect_punct("}")?.pos;
        Ok(MFor {
            name,
            index,
            iter,
            key,
            body,
            body_start,
            body_end,
        })
    }

    /// A `match` in the markup.
    pub(crate) fn parse_markup_match(&mut self) -> Result<MMatch, VeyError> {
        self.expect_kw("match")?;
        let subject = self.parse_expression()?;
        self.expect_punct("{")?;
        let mut arms = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed match"));
            }
            let mut patterns = vec![self.parse_pattern()?];
            while self.is_op("|") {
                self.advance()?;
                patterns.push(self.parse_pattern()?);
            }
            let guard = if self.eat_kw("if")? {
                Some(self.parse_expression()?)
            } else {
                None
            };
            self.expect_op("=>")?;
            let body_start = self.expect_punct("{")?.pos;
            let body = self.parse_markup_nodes()?;
            let body_end = self.expect_punct("}")?.pos;
            // The arms may be told apart with a comma, as in an expression.
            if self.is_punct(",") {
                self.advance()?;
            }
            arms.push(MMatchArm {
                patterns,
                guard,
                body,
                body_start,
                body_end,
            });
        }
        self.expect_punct("}")?;
        Ok(MMatch { subject, arms })
    }
}
