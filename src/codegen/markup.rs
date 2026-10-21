//! The generation of markup.

use crate::ast::*;
use crate::VeyError;

use super::js::{event_attr, js_key, js_string};
use super::Codegen;

impl Codegen {
    pub(crate) fn gen_markup_nodes(&mut self, nodes: &[MarkupNode]) -> Result<String, VeyError> {
        let mut parts = Vec::new();
        for node in nodes {
            parts.push(self.gen_markup_node(node)?);
        }
        Ok(format!("[{}]", parts.join(", ")))
    }

    pub(crate) fn gen_markup_node(&mut self, node: &MarkupNode) -> Result<String, VeyError> {
        match node {
            MarkupNode::Expr(expr) => self.gen_expr(expr),
            MarkupNode::Element(element) => self.gen_element(element),
            MarkupNode::If(branch) => {
                let test = self.gen_expr(&branch.test)?;
                let then_nodes = self.gen_markup_nodes(&branch.then_nodes)?;
                let else_nodes = if branch.else_nodes.is_empty() {
                    "null".to_string()
                } else {
                    self.gen_markup_nodes(&branch.else_nodes)?
                };
                Ok(format!("({} ? {} : {})", test, then_nodes, else_nodes))
            }
            MarkupNode::For(loop_node) => {
                let iter = self.gen_expr(&loop_node.iter)?;
                let body = self.gen_markup_nodes(&loop_node.body)?;
                Ok(format!(
                    "V.flatMap({}, function(_{}) {{ return {}; }})",
                    iter, loop_node.name, body
                ))
            }
        }
    }

    pub(crate) fn gen_element(&mut self, element: &Element) -> Result<String, VeyError> {
        let tag = if element.component {
            self.resolve(&element.tag)
        } else {
            js_string(&element.tag)
        };

        let mut entries: Vec<String> = Vec::new();
        let mut class_index = None;
        let mut has_id = false;

        for (name, value) in &element.attrs {
            let mapped = event_attr(name);
            if mapped == "class" {
                class_index = Some(entries.len());
            }
            if mapped == "id" {
                has_id = true;
            }
            entries.push(format!("{}: {}", js_key(mapped), self.gen_expr(value)?));
        }

        if !element.classes.is_empty() {
            let classes = js_string(&element.classes.join(" "));
            if let Some(index) = class_index {
                let value = entries[index]
                    .split_once(':')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default();
                entries[index] = format!("class: V.cls({}, {})", classes, value);
            } else {
                entries.push(format!("class: {}", classes));
            }
        }

        if let Some(id) = &element.id {
            if !has_id {
                entries.push(format!("id: {}", js_string(id)));
            }
        }

        let props = if entries.is_empty() {
            "null".to_string()
        } else {
            format!("{{ {} }}", entries.join(", "))
        };

        let children = self.gen_markup_nodes(&element.children)?;
        if element.component && !element.args.is_empty() {
            let mut args = Vec::new();
            for arg in &element.args {
                args.push(self.gen_expr(arg)?);
            }
            return Ok(format!(
                "V.h({}, {}, {}, [{}])",
                tag,
                props,
                children,
                args.join(", ")
            ));
        }
        Ok(format!("V.h({}, {}, {})", tag, props, children))
    }
}
