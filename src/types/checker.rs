//! The checker that walks a program and reports type mismatches.

use std::collections::HashMap;

use crate::ast::*;
use crate::lexer::Pos;
use crate::suggest::did_you_mean;
use crate::types::builtins::is_builtin_type;
use crate::types::core::{
    rest_element_type, substitute, union, Alias, EnumFieldInfo, EnumInfo, EnumVariantInfo,
    FunctionType, ObjectField, Type,
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

pub(crate) fn block_assigns_name(block: &Block, name: &str) -> bool {
    block.body.iter().any(|stmt| stmt_assigns_name(stmt, name))
}

pub(crate) fn stmt_assigns_name(stmt: &Stmt, name: &str) -> bool {
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

pub(crate) fn markup_assigns_name(nodes: &[MarkupNode], name: &str) -> bool {
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

pub(crate) fn expr_assigns_name(expr: &Expr, name: &str) -> bool {
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

pub(crate) fn inferred_const_type(ty: Type) -> Type {
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

/// Give an object type the name of the alias it came from.
///
/// The name makes the type easier to read in an error message.
pub(crate) fn name_alias(name: &str, ty: Type) -> Type {
    match ty {
        Type::Object(fields) => Type::Named(name.to_string(), fields),
        other => other,
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
                    env.types
                        .insert(decl.name.clone(), name_alias(&decl.name, ty));
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
pub struct ComponentParam {
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

    pub(crate) fn push_type_params(&mut self, params: &[TypeParam]) -> Result<(), VeyError> {
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

    pub(crate) fn pop_type_params(&mut self) {
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

    /// The names of the known types, used to suggest a similar one.
    pub(crate) fn known_types(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.aliases.keys().map(|name| name.as_str()).collect();
        names.extend(self.enums.keys().map(|name| name.as_str()));
        for scope in &self.type_param_scopes {
            names.extend(scope.keys().map(|name| name.as_str()));
        }
        names
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

    pub(crate) fn resolve_type_with(
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
                                // The alias refers to itself. The part that
                                // is still open cannot be written out, so it
                                // stays a reference to the alias.
                                return Ok(Type::Recursion(name.clone()));
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
                            return resolved.map(|ty| name_alias(name, ty));
                        }
                        if self.enums.contains_key(name) {
                            return Ok(Type::EnumUnion(name.clone()));
                        }
                        let mut error = self.error(*pos, format!("unknown type '{}'", name));
                        if let Some(hint) = did_you_mean(name, self.known_types()) {
                            error = error.with_hint(hint);
                        }
                        Err(error)
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

    pub(crate) fn function_type_for_decl(&self, decl: &FnDecl) -> Result<Type, VeyError> {
        let ret = match &decl.ret {
            Some(ty) => self.resolve_type(ty)?,
            None => Type::Any,
        };
        self.function_type_from_parts(&decl.params, ret, decl.ret.is_some(), &decl.type_params)
    }
}
