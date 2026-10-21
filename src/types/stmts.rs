//! The part of the checker that walks the statements of a program.

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::core::{
    assignable, element_type, has_type_var, substitute_bound, unify_types, union, Type,
};
use crate::VeyError;

use super::checker::{ComponentParam, TypeChecker};
use super::helpers::{
    block_always_exits, block_always_returns, block_assigns_name, inferred_const_type,
    markup_assigns_name, name_alias, stmt_assigns_name,
};

impl TypeChecker<'_> {
    pub(crate) fn check_program(&mut self, program: &Program) -> Result<(), VeyError> {
        self.predeclare_top_level(program)?;
        for stmt in &program.body {
            match stmt {
                Stmt::Type(_) | Stmt::Enum(_) | Stmt::Use(_) | Stmt::Style(_) => {}
                Stmt::Var(decl) => self.check_var_stmt(decl)?,
                Stmt::Fn(decl) => self.check_fn_decl(decl)?,
                Stmt::Component(decl) => self.check_component(decl)?,
                other => self.check_stmt(other)?,
            }
        }
        Ok(())
    }

    pub(crate) fn predeclare_top_level(&mut self, program: &Program) -> Result<(), VeyError> {
        // The body of a type alias is resolved on its own, so that an
        // unknown name is reported and the body cannot pick up the type
        // parameters of the function that uses the alias.
        for stmt in &program.body {
            if let Stmt::Type(decl) = stmt {
                let vars = decl
                    .type_params
                    .iter()
                    .map(|param| (param.name.clone(), Type::Var(param.name.clone())))
                    .collect();
                // The name of the alias is on the stack, so that a reference
                // to it inside its own body stays a reference.
                let mut stack = vec![decl.name.clone()];
                let ty = self.resolve_type_with(&decl.ty, &mut stack, &vars)?;
                self.alias_types
                    .insert(decl.name.clone(), name_alias(&decl.name, ty));
            }
        }

        for stmt in &program.body {
            match stmt {
                Stmt::Use(decl) => {
                    if !decl.path.ends_with(".css") {
                        self.declare(&decl.alias, Type::Any);
                    }
                }
                Stmt::Var(decl) => {
                    let ty = match &decl.ty {
                        Some(annotation) => self.resolve_type(annotation)?,
                        None => Type::Any,
                    };
                    self.declare_pattern_typed(&decl.pattern, &ty);
                }
                Stmt::Fn(decl) => {
                    self.push_type_params(&decl.type_params)?;
                    let ty = self.function_type_for_decl(decl);
                    self.pop_type_params();
                    self.declare(&decl.name, ty?);
                }
                Stmt::Component(decl) => {
                    let params = self.component_params(decl)?;
                    self.components.insert(decl.name.clone(), params);
                    self.declare(&decl.name, Type::Any);
                }
                Stmt::Enum(decl) => {
                    self.declare(&decl.name, Type::EnumNamespace(decl.name.clone()));
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn check_var_stmt(&mut self, decl: &VarDecl) -> Result<(), VeyError> {
        let annotation = match &decl.ty {
            Some(ty) => Some(self.resolve_type(ty)?),
            None => None,
        };
        let init_ty = match &decl.init {
            Some(expr) => Some(self.infer_expr_as(expr, annotation.as_ref())?),
            None => None,
        };

        if let (Some(expected), Some(actual)) = (&annotation, &init_ty) {
            if !assignable(actual, expected) {
                let pos = decl
                    .init
                    .as_ref()
                    .map(|expr| expr.pos())
                    .unwrap_or(decl.pos);
                return Err(self.type_error(pos, actual, expected));
            }
        }

        if let Some(expected) = annotation {
            self.declare_pattern_typed(&decl.pattern, &expected);
        } else if decl.kind == VarKind::Const {
            let ty = inferred_const_type(init_ty.unwrap_or(Type::Any));
            self.declare_pattern_typed(&decl.pattern, &ty);
        } else {
            self.declare_pattern_typed(&decl.pattern, &Type::Any);
        }

        Ok(())
    }

    pub(crate) fn check_fn_decl(&mut self, decl: &FnDecl) -> Result<(), VeyError> {
        self.push_type_params(&decl.type_params)?;
        let result = self.check_fn_decl_inner(decl);
        self.pop_type_params();
        result
    }

    pub(crate) fn check_fn_decl_inner(&mut self, decl: &FnDecl) -> Result<(), VeyError> {
        let mut ty = self.function_type_for_decl(decl)?;
        self.set_binding(&decl.name, ty.clone());
        let inferred =
            self.check_function(&decl.params, &decl.body, decl.ret.as_ref(), decl.pos)?;
        // A type parameter that was settled by a returned value is not
        // generic any more, so the function has that type from then on.
        let bindings = self.type_param_bindings.last().cloned().unwrap_or_default();
        if !bindings.is_empty() {
            ty = substitute_bound(&ty, &bindings);
        }
        let checked = match &ty {
            Type::Function(function) => function.checked,
            _ => false,
        };
        if checked && decl.ret.is_none() {
            if let Type::Function(function) = &mut ty {
                *function.ret = inferred;
            }
        }
        self.set_binding(&decl.name, ty);
        Ok(())
    }

    pub(crate) fn component_params(
        &self,
        decl: &ComponentDecl,
    ) -> Result<Vec<ComponentParam>, VeyError> {
        let mut params = Vec::new();
        for param in &decl.params {
            // A pattern of a component parameter names more than one prop.
            match param.name() {
                Some(name) => params.push(ComponentParam {
                    name: name.to_string(),
                    ty: self.param_type(param)?,
                    has_default: param.default.is_some(),
                }),
                None => {
                    let ty = self.param_type(param)?;
                    for (name, required) in prop_bindings(&param.pattern) {
                        params.push(ComponentParam {
                            name,
                            ty: ty.clone(),
                            has_default: !required,
                        });
                    }
                }
            }
        }
        Ok(params)
    }

    pub(crate) fn check_component(&mut self, decl: &ComponentDecl) -> Result<(), VeyError> {
        let params = self.component_params(decl)?;
        self.components.insert(decl.name.clone(), params);
        self.set_binding(&decl.name, Type::Any);
        self.push_scope();
        self.declare_params(&decl.params)?;
        self.check_markup(&decl.children)?;
        self.pop_scope();
        Ok(())
    }

    pub(crate) fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
        match stmt {
            Stmt::Var(decl) => self.check_var_stmt(decl),
            Stmt::Fn(decl) => self.check_fn_decl(decl),
            Stmt::Component(decl) => self.check_component(decl),
            Stmt::If(decl) => {
                self.infer_expr(&decl.test)?;
                let narrowing = self.narrow_condition(&decl.test).filter(|(name, _, _)| {
                    !block_assigns_name(&decl.then_block, name)
                        && !decl
                            .else_branch
                            .as_ref()
                            .is_some_and(|branch| stmt_assigns_name(branch, name))
                });
                if let Some((name, then_ty, else_ty)) = narrowing {
                    self.check_block_narrowed(&decl.then_block, &name, then_ty)?;
                    if let Some(branch) = &decl.else_branch {
                        self.push_scope();
                        self.declare(&name, else_ty);
                        self.check_stmt(branch)?;
                        self.pop_scope();
                    } else if block_always_exits(&decl.then_block) {
                        // Only the other path reaches the code after the
                        // `if`, so the narrowed away type no longer applies.
                        self.narrow_binding(&name, else_ty);
                    }
                } else {
                    self.check_block(&decl.then_block)?;
                    if let Some(branch) = &decl.else_branch {
                        self.check_stmt(branch)?;
                    }
                }
                Ok(())
            }
            Stmt::While(decl) => {
                self.infer_expr(&decl.test)?;
                self.check_block(&decl.body)
            }
            Stmt::For(decl) => {
                let iter_ty = self.infer_expr(&decl.iter)?;
                // Only a list can be walked through.
                if !matches!(
                    iter_ty,
                    Type::Array(_) | Type::String | Type::Any | Type::Var(_)
                ) {
                    return Err(self.error(
                        decl.pos,
                        format!("cannot walk through a {}", iter_ty.display()),
                    ));
                }
                let item_ty = match &decl.ty {
                    Some(annotation) => {
                        let expected = self.resolve_type(annotation)?;
                        let actual = element_type(&iter_ty);
                        if !assignable(&actual, &expected) {
                            return Err(self.type_error(decl.pos, &actual, &expected));
                        }
                        expected
                    }
                    None => element_type(&iter_ty),
                };
                self.push_scope();
                self.declare_pattern_typed(&decl.pattern, &item_ty);
                for inner in &decl.body.body {
                    self.check_stmt(inner)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::Repeat(decl) => {
                self.infer_expr(&decl.count)?;
                self.check_block(&decl.body)
            }
            Stmt::Return(decl) => {
                let actual = match &decl.value {
                    Some(value) => self.infer_expr(value)?,
                    None => Type::Void,
                };
                let expected = self.expected_returns.last().cloned().flatten();
                if let Some(expected) = &expected {
                    if !self.check_return_value(&actual, expected) {
                        return Err(self.type_error(decl.pos, &actual, expected));
                    }
                }
                if let Some(collected) = self.collected_returns.last_mut() {
                    collected.push(actual);
                }
                Ok(())
            }
            Stmt::Break(_) | Stmt::Continue(_) => Ok(()),
            Stmt::Try(decl) => {
                self.check_block(&decl.body)?;
                self.push_scope();
                if let Some(name) = &decl.name {
                    self.declare(name, Type::Any);
                }
                for inner in &decl.catch.body {
                    self.check_stmt(inner)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::Use(decl) => {
                if !decl.path.ends_with(".css") {
                    self.declare(&decl.alias, Type::Any);
                }
                Ok(())
            }
            Stmt::Style(decl) => Err(self.error(decl.pos, "style blocks must be at the top level")),
            Stmt::Type(decl) => {
                Err(self.error(decl.pos, "type declarations must be at the top level"))
            }
            Stmt::Enum(decl) => {
                Err(self.error(decl.pos, "enum declarations must be at the top level"))
            }
            Stmt::Expr(decl) => {
                self.infer_expr(&decl.expr)?;
                Ok(())
            }
            Stmt::Assign(decl) => self.check_assign(decl),
            Stmt::Block(block, _) => self.check_block(block),
        }
    }

    pub(crate) fn check_block(&mut self, block: &Block) -> Result<(), VeyError> {
        self.push_scope();
        for stmt in &block.body {
            self.check_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    pub(crate) fn check_block_narrowed(
        &mut self,
        block: &Block,
        name: &str,
        ty: Type,
    ) -> Result<(), VeyError> {
        self.push_scope();
        self.declare(name, ty);
        for stmt in &block.body {
            self.check_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    pub(crate) fn declare_params(&mut self, params: &[Param]) -> Result<(), VeyError> {
        for param in params {
            let ty = self.param_type(param)?;
            for (name, _) in param.bindings() {
                self.declare(name, ty.clone());
            }
            if let Some(default) = &param.default {
                let actual = self.infer_expr(default)?;
                if !assignable(&actual, &ty) {
                    return Err(self.type_error(default.pos(), &actual, &ty));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn check_function(
        &mut self,
        params: &[Param],
        body: &Block,
        ret_ann: Option<&TypeExpr>,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        self.push_scope();
        self.declare_params(params)?;
        let expected = match ret_ann {
            Some(ty) => Some(self.resolve_type(ty)?),
            None => None,
        };
        self.expected_returns.push(expected.clone());
        self.collected_returns.push(Vec::new());

        for stmt in &body.body {
            self.check_stmt(stmt)?;
        }

        let collected = self.collected_returns.pop().unwrap_or_default();
        self.expected_returns.pop();
        self.pop_scope();

        if let Some(expected) = &expected {
            if collected.is_empty() && !assignable(&Type::Void, expected) {
                return Err(self.error(pos, format!("function must return {}", expected.display())));
            }
            // A function that has a return type may not fall off the end,
            // because it would hand back nothing.
            if !assignable(&Type::Void, expected) && !block_always_returns(body) {
                return Err(self.error(
                    pos,
                    format!("function must return {} on every path", expected.display()),
                ));
            }
            Ok(expected.clone())
        } else if collected.is_empty() {
            Ok(Type::Void)
        } else {
            Ok(union(collected))
        }
    }

    /// Check a returned value against the declared return type.
    ///
    /// A type parameter that is not bound yet takes the type of the value,
    /// so that a generic function may return a value of the type it was
    /// asked for.
    pub(crate) fn check_return_value(&mut self, actual: &Type, expected: &Type) -> bool {
        let bindings = self.type_param_bindings.last().cloned().unwrap_or_default();
        let resolved = substitute_bound(expected, &bindings);
        if !has_type_var(&resolved) {
            return assignable(actual, &resolved);
        }
        let mut trial = bindings.clone();
        if unify_types(actual, &resolved, &mut trial) {
            if let Some(scope) = self.type_param_bindings.last_mut() {
                *scope = trial;
            }
            return true;
        }
        assignable(actual, &resolved)
    }

    pub(crate) fn check_arrow(
        &mut self,
        params: &[Param],
        body: &ArrowBody,
        pos: Pos,
    ) -> Result<Type, VeyError> {
        match body {
            ArrowBody::Expr(expr) => {
                self.push_scope();
                self.declare_params(params)?;
                let ty = self.infer_expr(expr)?;
                self.pop_scope();
                Ok(ty)
            }
            ArrowBody::Block(block) => self.check_function(params, block, None, pos),
        }
    }

    pub(crate) fn check_markup(&mut self, nodes: &[MarkupNode]) -> Result<(), VeyError> {
        for node in nodes {
            match node {
                MarkupNode::Element(element) => {
                    // A tag that starts with a capital is a component, and it
                    // has to be one of the components of the program. A
                    // component of another module is written `module.Name`.
                    let signature = if !element.component {
                        None
                    } else {
                        match element.tag.rsplit_once('.') {
                            Some((module, name)) => {
                                let env = self.imports.get(module).ok_or_else(|| {
                                    self.error(element.pos, format!("unknown module '{}'", module))
                                })?;
                                let params = env.components.get(name).ok_or_else(|| {
                                    self.error(
                                        element.pos,
                                        format!("unknown component '{}.{}'", module, name),
                                    )
                                })?;
                                Some(params.clone())
                            }
                            None => {
                                if !self.components.contains_key(&element.tag) {
                                    return Err(self.error(
                                        element.pos,
                                        format!("unknown component '{}'", element.tag),
                                    ));
                                }
                                self.components.get(&element.tag).cloned()
                            }
                        }
                    };
                    for (name, value) in &element.attrs {
                        let actual = self.infer_expr(value)?;
                        if let Some(params) = &signature {
                            if let Some(param) = params.iter().find(|param| param.name == *name) {
                                if !assignable(&actual, &param.ty) {
                                    return Err(self.type_error(value.pos(), &actual, &param.ty));
                                }
                            }
                        }
                    }
                    if let Some(params) = &signature {
                        // Positional arguments fill the parameters in order.
                        for (index, value) in element.args.iter().enumerate() {
                            let Some(param) = params.get(index) else {
                                continue;
                            };
                            let actual = self.infer_expr(value)?;
                            if !assignable(&actual, &param.ty) {
                                return Err(self.type_error(value.pos(), &actual, &param.ty));
                            }
                        }
                        // Every parameter without a default needs a value.
                        // `children` is filled by the markup itself.
                        for (index, param) in params.iter().enumerate() {
                            if param.has_default || param.name == "children" {
                                continue;
                            }
                            let given = element.args.len() > index
                                || element.attrs.iter().any(|(attr, _)| *attr == param.name);
                            if !given {
                                return Err(self.error(
                                    element.pos,
                                    format!(
                                        "component '{}' is missing the argument '{}'",
                                        element.tag, param.name
                                    ),
                                ));
                            }
                        }
                    }
                    self.check_markup(&element.children)?;
                }
                MarkupNode::Expr(expr) => {
                    self.infer_expr(expr)?;
                }
                MarkupNode::If(branch) => {
                    self.infer_expr(&branch.test)?;
                    let narrowing = self.narrow_condition(&branch.test).filter(|(name, _, _)| {
                        !markup_assigns_name(&branch.then_nodes, name)
                            && !markup_assigns_name(&branch.else_nodes, name)
                    });
                    if let Some((name, then_ty, else_ty)) = narrowing {
                        self.check_markup_narrowed(&branch.then_nodes, &name, then_ty)?;
                        self.check_markup_narrowed(&branch.else_nodes, &name, else_ty)?;
                    } else {
                        self.check_markup(&branch.then_nodes)?;
                        self.check_markup(&branch.else_nodes)?;
                    }
                }
                MarkupNode::For(loop_node) => {
                    let iter_ty = self.infer_expr(&loop_node.iter)?;
                    let item_ty = element_type(&iter_ty);
                    self.push_scope();
                    self.declare(&loop_node.name, item_ty);
                    self.check_markup(&loop_node.body)?;
                    self.pop_scope();
                }
            }
        }
        Ok(())
    }

    pub(crate) fn check_markup_narrowed(
        &mut self,
        nodes: &[MarkupNode],
        name: &str,
        ty: Type,
    ) -> Result<(), VeyError> {
        self.push_scope();
        self.declare(name, ty);
        self.check_markup(nodes)?;
        self.pop_scope();
        Ok(())
    }

    pub(crate) fn check_assign(&mut self, decl: &AssignStmt) -> Result<(), VeyError> {
        let target_ty = self.target_type(&decl.target)?;
        let value_ty = self.infer_expr_as(&decl.value, target_ty.as_ref())?;

        if let Some(target_ty) = target_ty {
            let result = if decl.op == "=" {
                value_ty
            } else {
                let op = decl.op.trim_end_matches('=');
                self.binary_result(op, &target_ty, &value_ty, decl.pos)?
            };
            if !assignable(&result, &target_ty) {
                return Err(self.type_error(decl.pos, &result, &target_ty));
            }
        }

        Ok(())
    }

    pub(crate) fn target_type(&mut self, target: &Expr) -> Result<Option<Type>, VeyError> {
        match target {
            Expr::Ident(name, _) => Ok(self.lookup(name)),
            Expr::Member(object, prop, optional, pos) => {
                if *optional {
                    return Ok(Some(Type::Any));
                }
                let ty = self.member_type(object, prop, false, *pos)?;
                Ok(Some(ty))
            }
            _ => Ok(None),
        }
    }
}

/// The props a pattern of a component parameter takes.
///
/// The second value says whether the prop has to be given. A rest binding
/// takes what is left, so it is never required.
fn prop_bindings(pattern: &BindingPattern) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    match pattern {
        BindingPattern::Name { name, .. } => out.push((name.clone(), true)),
        BindingPattern::Object { fields, rest, .. } => {
            for field in fields {
                for (name, _) in field.pattern.bindings() {
                    out.push((name.to_string(), true));
                }
            }
            if let Some(rest) = rest {
                out.push((rest.clone(), false));
            }
        }
        BindingPattern::Array { items, rest, .. } => {
            for item in items {
                for (name, _) in item.bindings() {
                    out.push((name.to_string(), true));
                }
            }
            if let Some(rest) = rest {
                out.push((rest.clone(), false));
            }
        }
    }
    out
}
