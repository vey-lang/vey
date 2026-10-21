//! The checker that walks a program and reports type mismatches.

use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::builtins::is_builtin_type;
use crate::types::core::{
    assignable, element_type, has_type_var, remove_type, rest_element_type, string_literal,
    substitute, substitute_bound, type_from_kind, unify_types, union, Alias, EnumFieldInfo,
    EnumInfo, EnumVariantInfo, FunctionType, ObjectField, Type,
};
use crate::VeyError;

/// Whether every path through the block leaves the function or the loop.
///
/// A block that ends in `return`, `break` or `continue` never falls through
/// to the code that follows it.
pub(crate) fn block_always_exits(block: &Block) -> bool {
    match block.body.last() {
        Some(Stmt::Return(_)) | Some(Stmt::Break(_)) | Some(Stmt::Continue(_)) => true,
        Some(Stmt::If(decl)) => {
            let then_exits = block_always_exits(&decl.then_block);
            let else_exits = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_exits(branch));
            then_exits && else_exits
        }
        _ => false,
    }
}

/// Whether the statement always leaves the function or the loop.
pub(crate) fn stmt_always_exits(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) | Stmt::Break(_) | Stmt::Continue(_) => true,
        Stmt::If(decl) => {
            let then_exits = block_always_exits(&decl.then_block);
            let else_exits = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_exits(branch));
            then_exits && else_exits
        }
        Stmt::Block(block, _) => block_always_exits(block),
        _ => false,
    }
}

/// Whether every path through the block returns a value.
pub(crate) fn block_always_returns(block: &Block) -> bool {
    match block.body.last() {
        Some(Stmt::Return(_)) => true,
        Some(Stmt::If(decl)) => {
            let then_returns = block_always_returns(&decl.then_block);
            let else_returns = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_returns(branch));
            then_returns && else_returns
        }
        Some(Stmt::Block(inner, _)) => block_always_returns(inner),
        _ => false,
    }
}

/// Whether every path through the statement returns a value.
pub(crate) fn stmt_always_returns(stmt: &Stmt) -> bool {
    match stmt {
        Stmt::Return(_) => true,
        Stmt::If(decl) => {
            let then_returns = block_always_returns(&decl.then_block);
            let else_returns = decl
                .else_branch
                .as_ref()
                .is_some_and(|branch| stmt_always_returns(branch));
            then_returns && else_returns
        }
        Stmt::Block(block, _) => block_always_returns(block),
        _ => false,
    }
}

pub(crate) fn set_object_field(fields: &mut Vec<ObjectField>, field: ObjectField) {
    if let Some(existing) = fields.iter_mut().find(|item| item.name == field.name) {
        *existing = field;
    } else {
        fields.push(field);
    }
}

fn block_assigns_name(block: &Block, name: &str) -> bool {
    block.body.iter().any(|stmt| stmt_assigns_name(stmt, name))
}

fn stmt_assigns_name(stmt: &Stmt, name: &str) -> bool {
    match stmt {
        Stmt::Var(decl) => decl
            .init
            .as_ref()
            .is_some_and(|init| expr_assigns_name(init, name)),
        Stmt::Fn(decl) => block_assigns_name(&decl.body, name),
        Stmt::If(decl) => {
            block_assigns_name(&decl.then_block, name)
                || decl
                    .else_branch
                    .as_ref()
                    .is_some_and(|branch| stmt_assigns_name(branch, name))
        }
        Stmt::While(decl) => block_assigns_name(&decl.body, name),
        Stmt::For(decl) => block_assigns_name(&decl.body, name),
        Stmt::Repeat(decl) => block_assigns_name(&decl.body, name),
        Stmt::Return(decl) => decl
            .value
            .as_ref()
            .is_some_and(|value| expr_assigns_name(value, name)),
        Stmt::Try(decl) => {
            block_assigns_name(&decl.body, name) || block_assigns_name(&decl.catch, name)
        }
        Stmt::Expr(decl) => expr_assigns_name(&decl.expr, name),
        Stmt::Assign(decl) => {
            matches!(&decl.target, Expr::Ident(target, _) if target == name)
                || expr_assigns_name(&decl.value, name)
        }
        Stmt::Block(block, _) => block_assigns_name(block, name),
        _ => false,
    }
}

fn markup_assigns_name(nodes: &[MarkupNode], name: &str) -> bool {
    nodes.iter().any(|node| match node {
        MarkupNode::Element(element) => {
            element
                .attrs
                .iter()
                .any(|(_, value)| expr_assigns_name(value, name))
                || markup_assigns_name(&element.children, name)
        }
        MarkupNode::Expr(expr) => expr_assigns_name(expr, name),
        MarkupNode::If(branch) => {
            expr_assigns_name(&branch.test, name)
                || markup_assigns_name(&branch.then_nodes, name)
                || markup_assigns_name(&branch.else_nodes, name)
        }
        MarkupNode::For(loop_node) => {
            expr_assigns_name(&loop_node.iter, name) || markup_assigns_name(&loop_node.body, name)
        }
    })
}

fn expr_assigns_name(expr: &Expr, name: &str) -> bool {
    match expr {
        Expr::Fn(_, body, _, _) => block_assigns_name(body, name),
        Expr::Arrow(_, body, _) => match body {
            ArrowBody::Block(block) => block_assigns_name(block, name),
            ArrowBody::Expr(expr) => expr_assigns_name(expr, name),
        },
        Expr::Array(items, _) => items.iter().any(|item| expr_assigns_name(item, name)),
        Expr::Object(props, _) => props.iter().any(|prop| match prop {
            ObjectProp::Field(_, value) | ObjectProp::Spread(value) => {
                expr_assigns_name(value, name)
            }
        }),
        Expr::Unary(_, value, _) | Expr::Await(value, _) | Expr::Spread(value, _) => {
            expr_assigns_name(value, name)
        }
        Expr::Binary(_, left, right, _)
        | Expr::Logical(_, left, right, _)
        | Expr::Nullish(left, right, _)
        | Expr::Range(left, right, _) => {
            expr_assigns_name(left, name) || expr_assigns_name(right, name)
        }
        Expr::Ternary(test, then_value, else_value, _) => {
            expr_assigns_name(test, name)
                || expr_assigns_name(then_value, name)
                || expr_assigns_name(else_value, name)
        }
        Expr::Call(callee, args, _, _) => {
            expr_assigns_name(callee, name) || args.iter().any(|arg| expr_assigns_name(arg, name))
        }
        Expr::Member(object, _, _, _) => expr_assigns_name(object, name),
        Expr::Match(value, arms, _) => {
            expr_assigns_name(value, name)
                || arms.iter().any(|arm| expr_assigns_name(&arm.body, name))
        }
        _ => false,
    }
}

fn inferred_const_type(ty: Type) -> Type {
    match ty {
        Type::Number
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Function(_)
        | Type::EnumUnion(_)
        | Type::Enum(_, _) => ty,
        _ => Type::Any,
    }
}

pub(crate) fn argument_label(count: usize) -> &'static str {
    if count == 1 {
        "argument"
    } else {
        "arguments"
    }
}

pub(crate) fn type_argument_label(count: usize) -> &'static str {
    if count == 1 {
        "argument"
    } else {
        "arguments"
    }
}

/// A generic type alias of a module.
///
/// The body is stored with the type parameters still free, so that a use in
/// another module can fill them in.
#[derive(Clone)]
pub struct GenericAlias {
    pub params: Vec<String>,
    pub body: Type,
}

#[derive(Clone, Default)]
pub struct ModuleTypeEnv {
    pub types: HashMap<String, Type>,
    pub generic_aliases: HashMap<String, GenericAlias>,
    /// The types of the exported values, so that a call to a function of
    /// another module is checked as well.
    pub values: HashMap<String, Type>,
}

pub fn check(
    program: &Program,
    file: &str,
    imports: &HashMap<String, ModuleTypeEnv>,
) -> Result<ModuleTypeEnv, VeyError> {
    let enums = collect_enums(program, file)?;
    let aliases = collect_aliases(program, file, &enums)?;
    let mut checker = TypeChecker {
        file,
        aliases,
        alias_types: HashMap::new(),
        enums,
        imports: imports.clone(),
        scopes: vec![HashMap::new()],
        type_param_scopes: Vec::new(),
        type_param_bindings: Vec::new(),
        type_param_constraints: Vec::new(),
        components: HashMap::new(),
        expected_returns: Vec::new(),
        collected_returns: Vec::new(),
    };
    checker.check_program(program)?;
    // build the module's exported type environment
    let mut env = ModuleTypeEnv::default();
    for stmt in &program.body {
        match stmt {
            Stmt::Type(decl) if decl.exported => {
                let Some(alias) = checker.aliases.get(&decl.name) else {
                    continue;
                };
                if decl.type_params.is_empty() {
                    let mut stack = Vec::new();
                    let ty = checker.resolve_type_with(&alias.body, &mut stack, &HashMap::new())?;
                    env.types.insert(decl.name.clone(), ty);
                } else {
                    let body = checker
                        .alias_types
                        .get(&decl.name)
                        .cloned()
                        .unwrap_or(Type::Any);
                    env.generic_aliases.insert(
                        decl.name.clone(),
                        GenericAlias {
                            params: decl
                                .type_params
                                .iter()
                                .map(|param| param.name.clone())
                                .collect(),
                            body,
                        },
                    );
                }
            }
            Stmt::Enum(decl) if decl.exported => {
                env.types
                    .insert(decl.name.clone(), Type::EnumUnion(decl.name.clone()));
            }
            _ => {}
        }
    }
    // The exported values keep their signature, so that a call to a function
    // of another module is checked in the same way as a local one.
    for stmt in &program.body {
        match stmt {
            Stmt::Fn(decl) if decl.exported => {
                if let Some(ty) = checker.lookup(&decl.name) {
                    env.values.insert(decl.name.clone(), ty);
                }
            }
            Stmt::Var(decl) if decl.exported => {
                if let BindingPattern::Name { name, .. } = &decl.pattern {
                    if let Some(ty) = checker.lookup(name) {
                        env.values.insert(name.clone(), ty);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(env)
}

fn collect_enums(program: &Program, file: &str) -> Result<HashMap<String, EnumInfo>, VeyError> {
    let mut enums = HashMap::new();
    for stmt in &program.body {
        if let Stmt::Enum(decl) = stmt {
            if is_builtin_type(&decl.name) {
                return Err(VeyError::new(
                    format!("cannot redefine built-in type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            let mut variant_names = std::collections::HashSet::new();
            let mut variants = Vec::new();
            for v in &decl.variants {
                if !variant_names.insert(v.name.clone()) {
                    return Err(VeyError::new(
                        format!("duplicate variant '{}' in enum '{}'", v.name, decl.name),
                        file,
                        v.pos.line,
                        v.pos.col,
                    ));
                }
                let mut field_names = std::collections::HashSet::new();
                let mut fields = Vec::new();
                for f in &v.fields {
                    if !field_names.insert(f.name.clone()) {
                        return Err(VeyError::new(
                            format!("duplicate field '{}' in variant '{}'", f.name, v.name),
                            file,
                            f.pos.line,
                            f.pos.col,
                        ));
                    }
                    fields.push(EnumFieldInfo {
                        name: f.name.clone(),
                        ty: f.ty.clone(),
                    });
                }
                variants.push(EnumVariantInfo {
                    name: v.name.clone(),
                    fields,
                });
            }
            if variants.is_empty() {
                return Err(VeyError::new(
                    format!("enum '{}' must have at least one variant", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            let info = EnumInfo { variants };
            if enums.insert(decl.name.clone(), info).is_some() {
                return Err(VeyError::new(
                    format!("duplicate enum '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
        }
    }
    Ok(enums)
}

fn collect_aliases(
    program: &Program,
    file: &str,
    enums: &HashMap<String, EnumInfo>,
) -> Result<HashMap<String, Alias>, VeyError> {
    let mut aliases = HashMap::new();
    for stmt in &program.body {
        if let Stmt::Type(decl) = stmt {
            if is_builtin_type(&decl.name) {
                return Err(VeyError::new(
                    format!("cannot redefine built-in type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            if enums.contains_key(&decl.name) {
                return Err(VeyError::new(
                    format!("duplicate type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            let alias = Alias {
                params: decl.type_params.clone(),
                body: decl.ty.clone(),
            };
            if aliases.insert(decl.name.clone(), alias).is_some() {
                return Err(VeyError::new(
                    format!("duplicate type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
        }
    }
    Ok(aliases)
}

/// A parameter of a component.
#[derive(Clone)]
pub(crate) struct ComponentParam {
    pub name: String,
    pub ty: Type,
    pub has_default: bool,
}

pub(crate) struct TypeChecker<'a> {
    pub(crate) file: &'a str,
    pub(crate) aliases: HashMap<String, Alias>,
    /// The resolved body of every type alias.
    pub(crate) alias_types: HashMap<String, Type>,
    pub(crate) enums: HashMap<String, EnumInfo>,
    pub(crate) imports: HashMap<String, ModuleTypeEnv>,
    pub(crate) scopes: Vec<HashMap<String, Type>>,
    pub(crate) type_param_scopes: Vec<HashMap<String, Type>>,
    /// The types the type parameters of the current scope were bound to.
    pub(crate) type_param_bindings: Vec<HashMap<String, Type>>,
    /// The constraints of the type parameters of the current scope.
    pub(crate) type_param_constraints: Vec<Vec<(String, Option<Type>)>>,
    pub(crate) components: HashMap<String, Vec<ComponentParam>>,
    pub(crate) expected_returns: Vec<Option<Type>>,
    pub(crate) collected_returns: Vec<Vec<Type>>,
}

impl TypeChecker<'_> {
    pub(crate) fn error(&self, pos: Pos, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file, pos.line, pos.col)
    }

    pub(crate) fn type_error(&self, pos: Pos, actual: &Type, expected: &Type) -> VeyError {
        self.error(
            pos,
            format!(
                "expected {}, found {}",
                expected.display(),
                actual.display()
            ),
        )
    }

    pub(crate) fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn push_type_params(&mut self, params: &[TypeParam]) -> Result<(), VeyError> {
        let mut scope = HashMap::new();
        for param in params {
            scope.insert(param.name.clone(), Type::Var(param.name.clone()));
        }
        self.type_param_scopes.push(scope);
        self.type_param_bindings.push(HashMap::new());

        // The constraints are resolved with the parameters in scope, so that
        // a constraint may mention another parameter.
        let mut constraints = Vec::new();
        for param in params {
            let constraint = match &param.constraint {
                Some(expr) => Some(self.resolve_type(expr)?),
                None => None,
            };
            constraints.push((param.name.clone(), constraint));
        }
        self.type_param_constraints.push(constraints);
        Ok(())
    }

    fn pop_type_params(&mut self) {
        self.type_param_scopes.pop();
        self.type_param_bindings.pop();
        self.type_param_constraints.pop();
    }

    pub(crate) fn lookup_type_param(&self, name: &str) -> Option<Type> {
        self.type_param_scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(crate) fn declare(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    /// Update the type of a binding in the current scope.
    ///
    /// Only the innermost scope is looked at, so that a name that shadows an
    /// outer one does not change the type of the outer binding.
    pub(crate) fn set_binding(&mut self, name: &str, ty: Type) {
        if let Some(scope) = self.scopes.last_mut() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = ty;
                return;
            }
        }
        self.declare(name, ty);
    }

    /// Narrow the type of an existing binding, wherever it was declared.
    ///
    /// This is used after a check such as `if value == null { return }`, where
    /// the rest of the block only sees the narrowed type.
    pub(crate) fn narrow_binding(&mut self, name: &str, ty: Type) {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(slot) = scope.get_mut(name) {
                *slot = ty;
                return;
            }
        }
    }

    pub(crate) fn lookup(&self, name: &str) -> Option<Type> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).cloned())
    }

    pub(crate) fn resolve_type(&self, expr: &TypeExpr) -> Result<Type, VeyError> {
        self.resolve_type_with(expr, &mut Vec::new(), &HashMap::new())
    }

    fn resolve_type_with(
        &self,
        expr: &TypeExpr,
        stack: &mut Vec<String>,
        vars: &HashMap<String, Type>,
    ) -> Result<Type, VeyError> {
        match expr {
            TypeExpr::Qualified(module, name, pos) => {
                let env = self
                    .imports
                    .get(module)
                    .ok_or_else(|| self.error(*pos, format!("unknown module '{}'", module)))?;
                if let Some(alias) = env.generic_aliases.get(name) {
                    return Err(self.error(
                        *pos,
                        format!(
                            "type '{}.{}' expects {} type {}, got 0",
                            module,
                            name,
                            alias.params.len(),
                            type_argument_label(alias.params.len())
                        ),
                    ));
                }
                env.types.get(name).cloned().ok_or_else(|| {
                    self.error(*pos, format!("module '{}' has no type '{}'", module, name))
                })
            }
            TypeExpr::Name(name, pos) => {
                if let Some(ty) = vars.get(name) {
                    return Ok(ty.clone());
                }
                if let Some(ty) = self.lookup_type_param(name) {
                    return Ok(ty);
                }
                match name.as_str() {
                    "any" => Ok(Type::Any),
                    "number" => Ok(Type::Number),
                    "string" => Ok(Type::String),
                    "bool" => Ok(Type::Bool),
                    "null" => Ok(Type::Null),
                    "void" => Ok(Type::Void),
                    "never" => Ok(Type::Never),
                    _ => {
                        if let Some(alias) = self.aliases.get(name) {
                            if !alias.params.is_empty() {
                                return Err(self.error(
                                    *pos,
                                    format!(
                                        "type '{}' expects {} type {}, got 0",
                                        name,
                                        alias.params.len(),
                                        type_argument_label(alias.params.len())
                                    ),
                                ));
                            }
                            if stack.iter().any(|item| item == name) {
                                return Err(
                                    self.error(*pos, format!("recursive type alias '{}'", name))
                                );
                            }
                            // The body of an alias was resolved when it was
                            // declared, so it cannot pick up the type
                            // parameters of the code that uses it.
                            if let Some(ty) = self.alias_types.get(name) {
                                return Ok(ty.clone());
                            }
                            stack.push(name.clone());
                            let resolved =
                                self.resolve_type_with(&alias.body, stack, &HashMap::new());
                            stack.pop();
                            return resolved;
                        }
                        if self.enums.contains_key(name) {
                            return Ok(Type::EnumUnion(name.clone()));
                        }
                        Err(self.error(*pos, format!("unknown type '{}'", name)))
                    }
                }
            }
            TypeExpr::Generic(name, args, pos) => {
                // A generic alias of another module, written `module.Name<...>`.
                if let Some((module, alias_name)) = name.split_once('.') {
                    let env = self
                        .imports
                        .get(module)
                        .ok_or_else(|| self.error(*pos, format!("unknown module '{}'", module)))?;
                    let alias = env.generic_aliases.get(alias_name).ok_or_else(|| {
                        self.error(
                            *pos,
                            format!("module '{}' has no type '{}'", module, alias_name),
                        )
                    })?;
                    if alias.params.len() != args.len() {
                        return Err(self.error(
                            *pos,
                            format!(
                                "type '{}.{}' expects {} type {}, got {}",
                                module,
                                alias_name,
                                alias.params.len(),
                                type_argument_label(alias.params.len()),
                                args.len()
                            ),
                        ));
                    }
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_type_with(arg, stack, vars)?);
                    }
                    let mut subst = HashMap::new();
                    for (param, ty) in alias.params.iter().zip(resolved_args) {
                        subst.insert(param.clone(), ty);
                    }
                    return Ok(substitute(&alias.body, &subst));
                }
                let alias = self
                    .aliases
                    .get(name)
                    .ok_or_else(|| self.error(*pos, format!("unknown type '{}'", name)))?;
                if alias.params.len() != args.len() {
                    return Err(self.error(
                        *pos,
                        format!(
                            "type '{}' expects {} type {}, got {}",
                            name,
                            alias.params.len(),
                            type_argument_label(alias.params.len()),
                            args.len()
                        ),
                    ));
                }
                let mut resolved_args = Vec::new();
                for arg in args {
                    resolved_args.push(self.resolve_type_with(arg, stack, vars)?);
                }
                if stack.iter().any(|item| item == name) {
                    return Err(self.error(*pos, format!("recursive type alias '{}'", name)));
                }
                stack.push(name.clone());
                let mut alias_vars = HashMap::new();
                for (param, ty) in alias.params.iter().zip(resolved_args) {
                    alias_vars.insert(param.name.clone(), ty);
                }
                let resolved = self.resolve_type_with(&alias.body, stack, &alias_vars);
                stack.pop();
                resolved
            }
            TypeExpr::Array(inner, _) => Ok(Type::Array(Box::new(
                self.resolve_type_with(inner, stack, vars)?,
            ))),
            TypeExpr::Object(fields, _) => {
                let mut out = Vec::new();
                for field in fields {
                    let mut ty = self.resolve_type_with(&field.ty, stack, vars)?;
                    if field.optional {
                        ty = union(vec![ty, Type::Null]);
                    }
                    out.push(ObjectField {
                        name: field.name.clone(),
                        ty,
                        optional: field.optional,
                    });
                }
                Ok(Type::Object(out))
            }
            TypeExpr::Optional(inner, _) => Ok(union(vec![
                self.resolve_type_with(inner, stack, vars)?,
                Type::Null,
            ])),
            TypeExpr::Union(parts, _) => {
                let mut out = Vec::new();
                for part in parts {
                    out.push(self.resolve_type_with(part, stack, vars)?);
                }
                Ok(union(out))
            }
            TypeExpr::Function(params, ret, rest, _) => {
                let mut param_types = Vec::new();
                for param in params {
                    param_types.push(self.resolve_type_with(param, stack, vars)?);
                }
                let required = if *rest {
                    param_types.len().saturating_sub(1)
                } else {
                    param_types.len()
                };
                let ret = self.resolve_type_with(ret, stack, vars)?;
                Ok(Type::Function(FunctionType {
                    params: param_types,
                    required,
                    rest: *rest,
                    ret: Box::new(ret),
                    checked: true,
                    constraints: Vec::new(),
                    type_params: Vec::new(),
                }))
            }
        }
    }

    pub(crate) fn param_type(&self, param: &Param) -> Result<Type, VeyError> {
        match &param.ty {
            Some(ty) => self.resolve_type(ty),
            None => Ok(Type::Any),
        }
    }

    pub(crate) fn function_type_from_parts(
        &self,
        params: &[Param],
        ret: Type,
        ret_annotated: bool,
        type_params: &[TypeParam],
    ) -> Result<Type, VeyError> {
        let mut param_types = Vec::new();
        for param in params {
            let ty = self.param_type(param)?;
            if param.rest {
                // A rest parameter is written as a list, but it stands for
                // the type of a single extra argument.
                param_types.push(rest_element_type(&ty));
            } else {
                param_types.push(ty);
            }
        }
        let required = params
            .iter()
            .position(|param| param.default.is_some() || param.rest)
            .unwrap_or(params.len());
        let rest = params.last().map(|param| param.rest).unwrap_or(false);
        let checked = ret_annotated || params.iter().any(|param| param.ty.is_some());
        let mut constraints = Vec::new();
        for param in type_params {
            if let Some(expr) = &param.constraint {
                constraints.push((param.name.clone(), self.resolve_type(expr)?));
            }
        }
        Ok(Type::Function(FunctionType {
            params: param_types,
            required,
            rest,
            ret: Box::new(ret),
            checked,
            constraints,
            type_params: type_params.iter().map(|param| param.name.clone()).collect(),
        }))
    }

    fn function_type_for_decl(&self, decl: &FnDecl) -> Result<Type, VeyError> {
        let ret = match &decl.ret {
            Some(ty) => self.resolve_type(ty)?,
            None => Type::Any,
        };
        self.function_type_from_parts(&decl.params, ret, decl.ret.is_some(), &decl.type_params)
    }

    fn check_program(&mut self, program: &Program) -> Result<(), VeyError> {
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

    fn predeclare_top_level(&mut self, program: &Program) -> Result<(), VeyError> {
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
                let mut stack = Vec::new();
                let ty = self.resolve_type_with(&decl.ty, &mut stack, &vars)?;
                self.alias_types.insert(decl.name.clone(), ty);
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

    fn check_fn_decl(&mut self, decl: &FnDecl) -> Result<(), VeyError> {
        self.push_type_params(&decl.type_params)?;
        let result = self.check_fn_decl_inner(decl);
        self.pop_type_params();
        result
    }

    fn check_fn_decl_inner(&mut self, decl: &FnDecl) -> Result<(), VeyError> {
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
            params.push(ComponentParam {
                name: param.name.clone(),
                ty: self.param_type(param)?,
                has_default: param.default.is_some(),
            });
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

    fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), VeyError> {
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

    fn check_block(&mut self, block: &Block) -> Result<(), VeyError> {
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

    pub(crate) fn narrow_condition(&self, expr: &Expr) -> Option<(String, Type, Type)> {
        let Expr::Binary(op, left, right, _) = expr else {
            return None;
        };
        if !matches!(op.as_str(), "==" | "===" | "!=" | "!==") {
            return None;
        }
        let (name, then_ty, else_ty) = self.narrow_comparison(left, right)?;
        if op == "!=" || op == "!==" {
            Some((name, else_ty, then_ty))
        } else {
            Some((name, then_ty, else_ty))
        }
    }

    pub(crate) fn narrow_comparison(
        &self,
        left: &Expr,
        right: &Expr,
    ) -> Option<(String, Type, Type)> {
        if let Some((name, original)) = self.type_call(left) {
            if let Some(kind) = string_literal(right) {
                let target = type_from_kind(kind, &original)?;
                let else_ty = remove_type(&original, &target);
                return Some((name, target, else_ty));
            }
        }

        if let Some((name, original)) = self.type_call(right) {
            if let Some(kind) = string_literal(left) {
                let target = type_from_kind(kind, &original)?;
                let else_ty = remove_type(&original, &target);
                return Some((name, target, else_ty));
            }
        }

        if let Expr::Ident(name, _) = left {
            if matches!(right, Expr::Null(_)) {
                let original = self.lookup(name)?;
                let else_ty = original.without_null();
                return Some((name.clone(), Type::Null, else_ty));
            }
        }

        if let Expr::Ident(name, _) = right {
            if matches!(left, Expr::Null(_)) {
                let original = self.lookup(name)?;
                let else_ty = original.without_null();
                return Some((name.clone(), Type::Null, else_ty));
            }
        }

        None
    }

    pub(crate) fn type_call(&self, expr: &Expr) -> Option<(String, Type)> {
        let Expr::Call(callee, args, _, _) = expr else {
            return None;
        };
        let Expr::Ident(name, _) = &**callee else {
            return None;
        };
        if name != "type" || args.len() != 1 {
            return None;
        }
        let Expr::Ident(variable, _) = &args[0] else {
            return None;
        };
        self.lookup(variable).map(|ty| (variable.clone(), ty))
    }

    pub(crate) fn declare_params(&mut self, params: &[Param]) -> Result<(), VeyError> {
        for param in params {
            let ty = self.param_type(param)?;
            self.declare(&param.name, ty.clone());
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
                    let signature = if element.component {
                        self.components.get(&element.tag).cloned()
                    } else {
                        None
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
