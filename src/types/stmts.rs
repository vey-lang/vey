//! The part of the checker that walks the statements of a program.

use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::core::{
    element_type, project_field, rest_element_type, union, FunctionType, Type,
};
use crate::types::rules::{assignable, has_type_var, substitute_bound, unify_types};
use crate::suggest::did_you_mean;
use crate::VeyError;

use super::checker::{ComponentParam, TypeChecker};
use super::helpers::{
    argument_label, block_always_exits, block_assigns_name, block_has_yield, extra_field,
    markup_assigns_name, name_alias, stmt_assigns_name,
};

impl TypeChecker<'_> {
    pub(crate) fn check_program(&mut self, program: &Program) -> Result<(), VeyError> {
        self.predeclare_top_level(program)?;
        for stmt in &program.body {
            match stmt {
                Stmt::Type(_) | Stmt::Enum(_) | Stmt::Interface(_) | Stmt::Use(_) | Stmt::Style(_) => {}
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
                        self.declare_alias(&decl.alias);
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
                Stmt::Impl(decl) => {
                    // The block makes a value of the type, so the name of the
                    // type is the name of a value too.
                    let ty = self
                        .alias_types
                        .get(&decl.type_name)
                        .cloned()
                        .unwrap_or(Type::Any);
                    self.declare(
                        &decl.type_name,
                        Type::Function(FunctionType {
                            params: vec![ty.clone()],
                            required: 1,
                            rest: false,
                            ret: Box::new(ty),
                            checked: true,
                            constraints: Vec::new(),
                            type_params: Vec::new(),
                            param_names: Vec::new(),
                        }),
                    );
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
            if !self.fits(actual, expected) {
                let pos = decl
                    .init
                    .as_ref()
                    .map(|expr| expr.pos())
                    .unwrap_or(decl.pos);
                // A field that the type does not have is named, because it
                // is the mistake that is easy to make.
                if let Some(field) = extra_field(actual, expected) {
                    return Err(self.error(
                        pos,
                        format!("'{}' is not a field of {}", field, expected.display()),
                    ));
                }
                return Err(self.type_error(pos, actual, expected));
            }
        }

        // The value that a name has by default is checked against the type
        // that is written.
        match &annotation {
            Some(expected) => self.check_pattern_defaults_against(&decl.pattern, expected)?,
            None => self.check_pattern_defaults(&decl.pattern)?,
        }

        if let Some(expected) = annotation {
            self.declare_pattern_typed(&decl.pattern, &expected);
        } else {
            // A value without a type stays dynamic, so that it may be
            // anything later on.
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
        // The values that this function yields belong to it, not to the
        // function around it.
        let outer = std::mem::take(&mut self.yielded);
        let checked = self.check_function(&decl.params, &decl.body, decl.ret.as_ref(), decl.pos);
        let yielded = std::mem::replace(&mut self.yielded, outer);
        let (inferred, param_types) = checked?;
        // The types of the parameters that were not written are the ones that
        // the way they are used asks for.
        if let Type::Function(function) = &mut ty {
            for (index, param) in decl.params.iter().enumerate() {
                if param.ty.is_none() {
                    if let Some(found) = param_types.get(index) {
                        function.params[index] = found.clone();
                    }
                }
            }
            // The types of the arguments are known when they were written, or
            // when the way the parameters are used says what they are.
            function.checked = decl.params.iter().enumerate().any(|(index, param)| {
                param.ty.is_some()
                    || param_types
                        .get(index)
                        .is_some_and(|ty| !matches!(ty, Type::Any))
            });
        }
        // A function that hands its values back one by one gives a list of
        // them, so that a loop may walk it.
        if block_has_yield(&decl.body) {
            if let Type::Function(function) = &mut ty {
                // The type that was written is the type of one value; a
                // generator that was not written yields the values that the
                // body yields.
                let element = if decl.ret.is_some() {
                    (*function.ret).clone()
                } else if yielded.is_empty() {
                    Type::Void
                } else {
                    union(yielded)
                };
                function.ret = Box::new(Type::Array(Box::new(element)));
            }
        }
        // A type parameter that was settled by a returned value is not
        // generic any more, so the function has that type from then on.
        let bindings = self.type_param_bindings.last().cloned().unwrap_or_default();
        if !bindings.is_empty() {
            ty = substitute_bound(&ty, &bindings);
        }
        // The type of the result is known even when it was not written.
        if decl.ret.is_none() && !matches!(inferred, Type::Void) {
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
                    rest: param.rest,
                }),
                None => {
                    let ty = self.param_type(param)?;
                    for (name, required) in prop_bindings(&param.pattern) {
                        params.push(ComponentParam {
                            name,
                            ty: ty.clone(),
                            has_default: !required,
                            rest: false,
                        });
                    }
                }
            }
        }
        Ok(params)
    }

    /// Check the functions of a type.
    pub(crate) fn check_impl(&mut self, decl: &ImplDecl) -> Result<(), VeyError> {
        let type_ty = match self.alias_types.get(&decl.type_name) {
            Some(ty) => ty.clone(),
            None => {
                return Err(self.error(
                    decl.pos,
                    format!("unknown type '{}'", decl.type_name),
                ))
            }
        };
        for method in &decl.methods {
            // The value the method is called on is named `self`.
            self.push_scope();
            self.declare("self", type_ty.clone());
            let result = self.check_function(
                &method.params,
                &method.body,
                method.ret.as_ref(),
                method.pos,
            );
            self.pop_scope();
            result?;
        }
        Ok(())
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
            Stmt::Impl(decl) => self.check_impl(decl),
            Stmt::If(decl) => {
                let test = self.infer_expr(&decl.test)?;
                self.check_condition(&test, decl.pos)?;
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
            Stmt::IfLet(decl) => {
                let value_ty = self.infer_expr(&decl.value)?;
                self.push_scope();
                self.declare_pattern_names(&decl.pattern, &value_ty);
                self.check_block(&decl.then_block)?;
                self.pop_scope();
                if let Some(branch) = &decl.else_branch {
                    self.check_stmt(branch)?;
                }
                Ok(())
            }
            Stmt::While(decl) => {
                let test = self.infer_expr(&decl.test)?;
                self.check_condition(&test, decl.pos)?;
                self.check_block(&decl.body)
            }
            Stmt::WhileLet(decl) => {
                let value_ty = self.infer_expr(&decl.value)?;
                self.push_scope();
                self.declare_pattern_names(&decl.pattern, &value_ty);
                for inner in &decl.body.body {
                    self.check_stmt(inner)?;
                }
                self.pop_scope();
                Ok(())
            }
            Stmt::LetElse(decl) => {
                let value_ty = self.infer_expr(&decl.value)?;
                // The name is bound in the scope around it, the way the name
                // of a `let` is.
                self.declare_pattern_typed(&decl.pattern, &value_ty);
                self.check_block(&decl.otherwise)
            }
            Stmt::Defer(decl) => {
                self.infer_expr(&decl.value)?;
                Ok(())
            }
            Stmt::Throw(decl) => {
                self.infer_expr(&decl.value)?;
                Ok(())
            }
            Stmt::For(decl) => {
                if let Some(step) = &decl.step {
                    let step_ty = self.infer_expr(step)?;
                    if !matches!(step_ty, Type::Any) && !assignable(&step_ty, &Type::Number) {
                        return Err(self.type_error(step.pos(), &step_ty, &Type::Number));
                    }
                }
                let iter_ty = self.infer_expr(&decl.iter)?;
                // Only a list can be walked through.
                self.check_iterable(&iter_ty, decl.pos)?;
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
                let count = self.infer_expr(&decl.count)?;
                // A loop runs a number of times.
                if !matches!(count, Type::Any) && !assignable(&count, &Type::Number) {
                    return Err(self.type_error(decl.count.pos(), &count, &Type::Number));
                }
                self.check_block(&decl.body)
            }
            Stmt::Return(decl) => {
                let expected = self.expected_returns.last().cloned().flatten();
                let actual = match &decl.value {
                    // The value is checked against the type that is written,
                    // so that a list of values may be a tuple.
                    Some(value) => self.infer_expr_as(value, expected.as_ref())?,
                    None => Type::Void,
                };
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
            Stmt::Break(_, _) | Stmt::Continue(_, _) => Ok(()),
            Stmt::Try(decl) => {
                // A `?` inside the block is caught by the catch.
                self.try_depth += 1;
                let result = self.check_block(&decl.body);
                self.try_depth -= 1;
                result?;
                if let Some(catch) = &decl.catch {
                    self.push_scope();
                    if let Some(name) = &decl.name {
                        self.declare(name, Type::Any);
                    }
                    for inner in &catch.body {
                        self.check_stmt(inner)?;
                    }
                    self.pop_scope();
                }
                if let Some(finally) = &decl.finally {
                    self.check_block(finally)?;
                }
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
            Stmt::Interface(decl) => Err(self.error(
                decl.pos,
                "interface declarations must be at the top level",
            )),
            Stmt::Expr(decl) => {
                self.infer_expr(&decl.expr)?;
                Ok(())
            }
            Stmt::Assign(decl) => self.check_assign(decl),
            Stmt::Block(block, _) => self.check_block(block),
        }
    }

    /// Only a list can be walked through.
    pub(crate) fn check_iterable(&self, ty: &Type, pos: Pos) -> Result<(), VeyError> {
        if !matches!(ty, Type::Array(_) | Type::String | Type::Any | Type::Var(_)) {
            return Err(self.error(pos, format!("cannot walk through a {}", ty.display())));
        }
        Ok(())
    }

    /// A condition has to be a yes or a no.
    ///
    /// A number or a text is always the same, so it is most likely a
    /// mistake. A value that may be nothing is asked with its truth, and the
    /// branch knows that it is there.
    pub(crate) fn check_condition(&self, ty: &Type, pos: Pos) -> Result<(), VeyError> {
        match ty {
            Type::Bool => Ok(()),
            Type::Any | Type::Unknown | Type::Var(_) => Ok(()),
            Type::Union(items) => {
                // A value that may be nothing is asked with its truth, and
                // the branch knows that it is there.
                let empty = items
                    .iter()
                    .any(|item| matches!(item, Type::Null | Type::Undefined));
                let rest: Vec<&Type> = items
                    .iter()
                    .filter(|item| !matches!(item, Type::Null | Type::Undefined))
                    .collect();
                if empty && rest.len() == 1 {
                    Ok(())
                } else if !rest.is_empty() && rest.iter().all(|item| matches!(item, Type::Bool)) {
                    Ok(())
                } else {
                    Err(self.condition_error(ty, pos))
                }
            }
            _ => Err(self.condition_error(ty, pos)),
        }
    }

    /// The error for a condition that is not a yes or a no.
    fn condition_error(&self, ty: &Type, pos: Pos) -> VeyError {
        self.error(
            pos,
            format!("the condition must be a bool, not a {}", ty.display()),
        )
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
            self.check_pattern_defaults(&param.pattern)?;
            for (name, _) in param.bindings() {
                self.declare(name, ty.clone());
            }
            if let Some(default) = &param.default {
                let actual = self.infer_expr(default)?;
                if !self.fits(&actual, &ty) {
                    return Err(self.type_error(default.pos(), &actual, &ty));
                }
            }
        }
        Ok(())
    }

    /// The names that a pattern takes apart, with the type of the value.
    /// The type of every name of an enum pattern, by the field it stands
    /// for.
    fn enum_binding_types(
        &mut self,
        enum_name: &str,
        variant: &str,
        bindings: &[crate::ast::EnumBinding],
    ) -> Vec<Type> {
        let fields = self
            .enum_info(enum_name)
            .and_then(|info| info.variant(variant))
            .map(|variant| variant.fields.clone())
            .unwrap_or_default();
        let mut positional = 0;
        let mut out = Vec::new();
        for binding in bindings {
            let field_name = match &binding.field {
                Some(name) => Some(name.clone()),
                None => {
                    let name = fields.get(positional).map(|field| field.name.clone());
                    positional += 1;
                    name
                }
            };
            let ty = field_name
                .and_then(|name| fields.iter().find(|field| field.name == name).cloned())
                .and_then(|field| field.ty)
                .and_then(|ty| self.resolve_type(&ty).ok())
                .unwrap_or(Type::Any);
            out.push(ty);
        }
        out
    }

    /// The names that a pattern of a `match` takes apart.
    ///
    /// A list or an object pattern takes the values of its parts, so that a
    /// name is given the type of the part it stands for.
    pub(crate) fn declare_pattern_names(&mut self, pattern: &Pattern, ty: &Type) {
        match pattern {
            Pattern::Binding { name, .. } => {
                self.declare(name, ty.clone());
            }
            Pattern::Enum {
                enum_name,
                variant,
                bindings,
                ..
            } => {
                // The names get the type of the field they stand for.
                let types = self.enum_binding_types(enum_name, variant, bindings);
                for (binding, field_ty) in bindings.iter().zip(types) {
                    self.declare(&binding.name, field_ty);
                }
            }
            Pattern::List(items, _) => {
                // A tuple gives every name the type of its own place, and a
                // list gives every name the type of its items.
                match ty {
                    Type::Tuple(parts) => {
                        for (index, item) in items.iter().enumerate() {
                            let part = parts.get(index).cloned().unwrap_or(Type::Any);
                            self.declare_pattern_names(item, &part);
                        }
                    }
                    other => {
                        let element = element_type(other);
                        for item in items {
                            self.declare_pattern_names(item, &element);
                        }
                    }
                }
            }
            Pattern::Object(fields, _) => {
                for (key, pattern) in fields {
                    let field_ty = project_field(ty, key);
                    self.declare_pattern_names(pattern, &field_ty);
                }
            }
            Pattern::Literal(_)
            | Pattern::Range(_, _)
            | Pattern::Wildcard { .. }
            | Pattern::Prefix(_, _) => {}
        }
    }

    /// Remember the type that a parameter has to have.    ///
    /// A parameter without a type is judged by the way it is used, so that
    /// the arguments of a call to the function are checked.
    pub(crate) fn note_param(&mut self, name: &str, wanted: Type) {
        if matches!(wanted, Type::Any) {
            return;
        }
        let Some(scope) = self.param_scopes.last_mut() else {
            return;
        };
        match scope.get_mut(name) {
            Some(current) => {
                // The type that is there is the one of the first use, and
                // the later uses have to fit it, so they do not widen it.
                if matches!(current, Type::Any) {
                    *current = wanted;
                }
            }
            None => {
                scope.insert(name.to_string(), wanted);
            }
        }
    }

    pub(crate) fn check_function(
        &mut self,
        params: &[Param],
        body: &Block,
        ret_ann: Option<&TypeExpr>,
        pos: Pos,
    ) -> Result<(Type, Vec<Type>), VeyError> {
        // The names that are narrowed inside a function stay narrowed for
        // the code inside it, and not for the code around it: the function
        // may run later, and what it does says nothing about the value that
        // is there by then.
        let outer = self.scopes.clone();
        let result = self.check_function_inner(params, body, ret_ann, pos);
        self.scopes = outer;
        result
    }

    fn check_function_inner(
        &mut self,
        params: &[Param],
        body: &Block,
        ret_ann: Option<&TypeExpr>,
        pos: Pos,
    ) -> Result<(Type, Vec<Type>), VeyError> {
        self.push_scope();
        self.declare_params(params)?;
        let expected = match ret_ann {
            Some(ty) => Some(self.resolve_type(ty)?),
            None => None,
        };
        self.expected_returns.push(expected.clone());
        self.collected_returns.push(Vec::new());
        // The parameters without a type are judged by the way they are used.
        self.param_scopes.push(HashMap::new());
        for param in params {
            if param.ty.is_some() {
                continue;
            }
            for (name, _) in param.bindings() {
                self.param_scopes
                    .last_mut()
                    .expect("the scope of the parameters")
                    .insert(name.to_string(), Type::Any);
            }
        }

        for stmt in &body.body {
            self.check_stmt(stmt)?;
        }

        let collected = self.collected_returns.pop().unwrap_or_default();
        self.expected_returns.pop();
        self.pop_scope();
        let witnesses = self.param_scopes.pop().unwrap_or_default();

        // The type of a parameter is the type it was given, or the type that
        // the way it is used asks for.
        let mut param_types = Vec::new();
        for param in params {
            let annotated = self.param_type(param)?;
            let ty = if param.ty.is_some() {
                annotated
            } else {
                // The type of a parameter is the type that the way it is
                // used asks for.
                let mut out: Option<Type> = None;
                for (name, _) in param.bindings() {
                    if let Some(found) = witnesses.get(name) {
                        out = Some(match out {
                            Some(current) => union(vec![current, found.clone()]),
                            None => found.clone(),
                        });
                    }
                }
                out.unwrap_or(annotated)
            };
            param_types.push(if param.rest {
                rest_element_type(&ty)
            } else {
                ty
            });
        }

        if let Some(expected) = &expected {
            // A function that hands its values back one by one does not
            // return, so it does not have to return on every path.
            let generator = block_has_yield(body);
            // A function that always throws never hands a value back, so it
            // keeps its promise in another way.
            if collected.is_empty()
                && !assignable(&Type::Void, expected)
                && !generator
                && !block_always_exits(body)
            {
                return Err(self.error(pos, format!("function must return {}", expected.display())));
            }
            // A function that has a return type may not fall off the end,
            // because it would hand back nothing.
            if !assignable(&Type::Void, expected) && !block_always_exits(body) && !generator {
                return Err(self.error(
                    pos,
                    format!("function must return {} on every path", expected.display()),
                ));
            }
            Ok((expected.clone(), param_types))
        } else if collected.is_empty() {
            Ok((Type::Void, param_types))
        } else {
            Ok((union(collected), param_types))
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
            return self.fits(actual, &resolved);
        }
        let mut trial = bindings.clone();
        if unify_types(actual, &resolved, &mut trial) {
            if let Some(scope) = self.type_param_bindings.last_mut() {
                *scope = trial;
            }
            return true;
        }
        self.fits(actual, &resolved)
    }

    pub(crate) fn check_arrow(
        &mut self,
        params: &[Param],
        body: &ArrowBody,
        ret_ann: Option<&TypeExpr>,
        pos: Pos,
    ) -> Result<(Type, Vec<Type>), VeyError> {
        match body {
            ArrowBody::Expr(expr) => {
                self.push_scope();
                self.declare_params(params)?;
                let ty = self.infer_expr(expr)?;
                self.pop_scope();
                // The type that is written is the type of the value.
                if let Some(ann) = ret_ann {
                    let wanted = self.resolve_type(ann)?;
                    if !self.fits(&ty, &wanted) {
                        return Err(self.type_error(expr.pos(), &ty, &wanted));
                    }
                    return Ok((wanted, Vec::new()));
                }
                Ok((ty, Vec::new()))
            }
            ArrowBody::Block(block) => self.check_function(params, block, ret_ann, pos),
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
                                    let mut error = self.error(
                                        element.pos,
                                        format!("unknown component '{}'", element.tag),
                                    );
                                    let names: Vec<&str> = self
                                        .components
                                        .keys()
                                        .map(|name| name.as_str())
                                        .collect();
                                    if let Some(hint) = did_you_mean(&element.tag, names) {
                                        error = error.with_hint(hint);
                                    }
                                    return Err(error);
                                }
                                self.components.get(&element.tag).cloned()
                            }
                        }
                    };
                    for (name, value) in &element.attrs {
                        let actual = self.infer_expr(value)?;
                        if let Some(params) = &signature {
                            if let Some(param) = params.iter().find(|param| param.name == *name) {
                                if !self.fits(&actual, &param.ty) {
                                    return Err(self.type_error(value.pos(), &actual, &param.ty));
                                }
                            }
                        }
                    }
                    if let Some(params) = &signature {
                        // Positional arguments fill the parameters in order.
                        for (index, value) in element.args.iter().enumerate() {
                            let Some(param) = params.get(index) else {
                                return Err(self.error(
                                    value.pos(),
                                    format!(
                                        "component '{}' takes {} {}, got {}",
                                        element.tag,
                                        params.len(),
                                        argument_label(params.len()),
                                        element.args.len()
                                    ),
                                ));
                            };
                            let actual = self.infer_expr(value)?;
                            if !self.fits(&actual, &param.ty) {
                                return Err(self.type_error(value.pos(), &actual, &param.ty));
                            }
                        }
                        // An argument that the component does not have is a
                        // mistake, unless it takes the rest of them.
                        if !params.iter().any(|param| param.rest) {
                            for (attr, value) in &element.attrs {
                                if params.iter().any(|param| param.name == *attr) {
                                    continue;
                                }
                                return Err(self.error(
                                    value.pos(),
                                    format!(
                                        "component '{}' has no argument '{}'",
                                        element.tag, attr
                                    ),
                                ));
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
                    self.check_iterable(&iter_ty, loop_node.iter.pos())?;
                    if let Some(key) = &loop_node.key {
                        self.infer_expr(key)?;
                    }
                    let item_ty = element_type(&iter_ty);
                    self.push_scope();
                    self.declare(&loop_node.name, item_ty);
                    if let Some(index) = &loop_node.index {
                        self.declare(index, Type::Number);
                    }
                    self.check_markup(&loop_node.body)?;
                    self.pop_scope();
                }
                MarkupNode::Match(m) => {
                    self.infer_expr(&m.subject)?;
                    for arm in &m.arms {
                        self.push_scope();
                        for pattern in &arm.patterns {
                            self.check_pattern(pattern)?;
                        }
                        if let Some(guard) = &arm.guard {
                            self.infer_expr(guard)?;
                        }
                        self.check_markup(&arm.body)?;
                        self.pop_scope();
                    }
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
        // A prop with a value that is used when there is none may be left out.
        BindingPattern::Name { name, default, .. } => out.push((name.clone(), default.is_none())),
        BindingPattern::Object { fields, rest, .. } => {
            for field in fields {
                for (name, required) in prop_bindings(&field.pattern) {
                    out.push((name.to_string(), required));
                }
            }
            if let Some(rest) = rest {
                out.push((rest.clone(), false));
            }
        }
        BindingPattern::Array { items, rest, .. } => {
            for item in items {
                for (name, required) in prop_bindings(item) {
                    out.push((name.to_string(), required));
                }
            }
            if let Some(rest) = rest {
                out.push((rest.clone(), false));
            }
        }
    }
    out
}
