//! The generation of markup.

use crate::ast::*;
use crate::VeyError;

use super::awaits::{expr_has_await, markup_has_await};
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
                // The items of the list may be told apart, so that a list may
                // change without the page being built again.
                let key = match &loop_node.key {
                    Some(key) => {
                        let key = self.gen_expr(key)?;
                        format!("V.withKey({}, {})", body, key)
                    }
                    None => body.clone(),
                };
                // The place of the item in the list may be wanted as well.
                let index = match &loop_node.index {
                    Some(index) => format!(", _{}", index),
                    None => String::new(),
                };
                Ok(format!(
                    "V.flatMap({}, function(_{}{}) {{ return {}; }})",
                    iter, loop_node.name, index, key
                ))
            }
            MarkupNode::Match(m) => self.gen_markup_match(m),
        }
    }

    /// A `match` in the markup.
    ///
    /// The arms of the match give the markup that belongs to them, so the
    /// whole is a function that is called at once.
    fn gen_markup_match(&mut self, m: &MMatch) -> Result<String, VeyError> {
        let subject = self.gen_expr(&m.subject)?;
        let slot = self.temp;
        self.temp += 1;
        let temp = format!("__match{}", slot);
        let captured = self.capture(|codegen| {
            for arm in &m.arms {
                let mut conditions = Vec::new();
                let mut value_names = Vec::new();
                let mut taken: Vec<(String, String)> = Vec::new();
                for pattern in &arm.patterns {
                    match pattern {
                        Pattern::Wildcard { .. } => {}
                        Pattern::Binding { name, .. } => value_names.push(name.clone()),
                        other => {
                            let (condition, bindings) = codegen.gen_pattern(other, &temp)?;
                            conditions.push(condition);
                            taken.extend(bindings);
                        }
                    }
                }
                // A pattern that only names the value matches everything.
                let test = if conditions.is_empty() {
                    "true".to_string()
                } else {
                    conditions.join(" || ")
                };
                codegen.emit(&format!("if ({}) {{", test));
                codegen.indent += 1;
                // The names are there before the guard reads them.
                for (name, expression) in &taken {
                    codegen.emit(&format!("const _{} = {};", name, expression));
                }
                for name in &value_names {
                    codegen.emit(&format!("const _{} = {};", name, temp));
                }
                // A guard that says no lets the next arm try.
                let inner = match &arm.guard {
                    Some(guard) => format!("if ({}) {{", codegen.gen_expr(guard)?),
                    None => String::new(),
                };
                let nested = arm.guard.is_some();
                if nested {
                    codegen.emit(&inner);
                    codegen.indent += 1;
                }
                let body = codegen.gen_markup_nodes(&arm.body)?;
                codegen.emit(&format!("return {};", body));
                if nested {
                    codegen.indent -= 1;
                    codegen.emit("}");
                }
                codegen.indent -= 1;
                codegen.emit("}");
            }
            codegen.emit("return [];");
            Ok(())
        })?;
        let pad = "  ".repeat(self.indent);
        // An `await` inside an arm needs an async function, otherwise the
        // generated JavaScript is a syntax error. The condition of an arm
        // may wait as well.
        let async_kw = if m.arms.iter().any(|arm| {
            markup_has_await(&arm.body)
                || arm
                    .guard
                    .as_ref()
                    .is_some_and(|guard| expr_has_await(guard))
        }) {
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

    pub(crate) fn gen_element(&mut self, element: &Element) -> Result<String, VeyError> {
        let tag = if element.component {
            // A component of another module, such as `ui.Card`, is a member
            // of the module.
            match element.tag.rsplit_once('.') {
                Some((module, name)) => format!("{}.{}", self.resolve(module), name),
                None => self.resolve(&element.tag),
            }
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

        // The children that go in a slot of their own are given to the
        // component as arguments of their own.
        let mut plain: Vec<MarkupNode> = Vec::new();
        for child in &element.children {
            let slot = match child {
                MarkupNode::Element(element) => element.slot.clone(),
                _ => None,
            };
            match slot {
                Some(slot) => {
                    let children = self.gen_markup_nodes(std::slice::from_ref(child))?;
                    entries.push(format!("{}: {}", js_key(&slot), children));
                }
                None => plain.push(child.clone()),
            }
        }

        let props = if entries.is_empty() {
            "null".to_string()
        } else {
            format!("{{ {} }}", entries.join(", "))
        };

        let children = self.gen_markup_nodes(&plain)?;
        if element.component && !element.args.is_empty() {
            let mut args = Vec::new();
            for arg in &element.args {
                args.push(self.gen_expr(arg)?);
            }
            return Ok(format!(
                "V.__h({}, {}, {}, [{}])",
                tag,
                props,
                children,
                args.join(", ")
            ));
        }
        Ok(format!("V.__h({}, {}, {})", tag, props, children))
    }
}
