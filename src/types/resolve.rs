//! The resolution of the types that are written in a program.

use std::collections::HashMap;

use crate::ast::*;
use crate::suggest::did_you_mean;
use crate::types::core::{rest_element_type, union, FunctionType, ObjectField, Type};
use crate::types::rules::substitute;
use crate::VeyError;

use super::builtins::{signal_type, store_type};
use super::checker::TypeChecker;
use super::helpers::{name_alias, qualified_type, type_argument_label};

impl TypeChecker<'_> {
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
                env.types.get(name).cloned().map(|ty| qualified_type(module, ty)).ok_or_else(
                    || self.error(*pos, format!("module '{}' has no type '{}'", module, name)),
                )
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
                    "unknown" => Ok(Type::Unknown),
                    "number" => Ok(Type::Number),
                    "string" => Ok(Type::String),
                    "bool" => Ok(Type::Bool),
                    "null" => Ok(Type::Null),
                    "undefined" => Ok(Type::Undefined),
                    "void" => Ok(Type::Void),
                    "never" => Ok(Type::Never),
                    "symbol" => Ok(Type::Symbol),
                    "bigint" => Ok(Type::BigInt),
                    "object" => Ok(Type::AnyObject),
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
                        if self.interfaces.contains_key(name) {
                            // A type that holds itself stays a reference to
                            // it, so that the shape does not grow forever.
                            if stack.iter().any(|item| item == name) {
                                return Ok(Type::Interface(name.clone(), Vec::new()));
                            }
                            stack.push(name.clone());
                            let ty = self.interface_type_with(name, &HashMap::new(), stack);
                            stack.pop();
                            return ty;
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
                // A type of the language that takes type arguments, such as
                // `Promise<number>`.
                if let Some(ty) = self.builtin_generic_type(name, args, *pos, stack, vars)? {
                    return Ok(ty);
                }
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
                // An interface that takes type arguments.
                if let Some(info) = self.interfaces.get(name) {
                    if info.params.is_empty() {
                        return self.interface_type(name);
                    }
                    if info.params.len() != args.len() {
                        return Err(self.error(
                            *pos,
                            format!(
                                "type '{}' expects {} type {}, got {}",
                                name,
                                info.params.len(),
                                type_argument_label(info.params.len()),
                                args.len()
                            ),
                        ));
                    }
                    let mut resolved_args = Vec::new();
                    for arg in args {
                        resolved_args.push(self.resolve_type_with(arg, stack, vars)?);
                    }
                    let mut subst = HashMap::new();
                    for (param, ty) in info.params.iter().zip(resolved_args) {
                        subst.insert(param.name.clone(), ty);
                    }
                    // A type that holds itself stays a reference to it.
                    if stack.iter().any(|item| item == name) {
                        return Ok(Type::Interface(name.clone(), Vec::new()));
                    }
                    stack.push(name.clone());
                    let ty = self.interface_type_with(name, &subst, stack);
                    stack.pop();
                    return ty;
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
                    // The alias refers to itself. The part that is still
                    // open cannot be written out, so it stays a reference to
                    // the alias.
                    return Ok(Type::Recursion(name.clone()));
                }
                stack.push(name.clone());
                let mut alias_vars = HashMap::new();
                for (param, ty) in alias.params.iter().zip(resolved_args.iter()) {
                    alias_vars.insert(param.name.clone(), ty.clone());
                }
                // The constraint of a type parameter has to hold.
                for (param, ty) in alias.params.iter().zip(resolved_args.iter()) {
                    let Some(constraint) = &param.constraint else {
                        continue;
                    };
                    let constraint = self.resolve_type_with(constraint, stack, &alias_vars)?;
                    if !self.fits(ty, &constraint) {
                        return Err(self.error(
                            *pos,
                            format!(
                                "type '{}' expects {}, found {}",
                                name,
                                constraint.display(),
                                ty.display()
                            ),
                        ));
                    }
                }
                let resolved = self.resolve_type_with(&alias.body, stack, &alias_vars);
                stack.pop();
                resolved
            }
            TypeExpr::Array(inner, _) => Ok(Type::Array(Box::new(
                self.resolve_type_with(inner, stack, vars)?,
            ))),
            TypeExpr::Tuple(items, _) => {
                let mut out = Vec::new();
                for item in items {
                    out.push(self.resolve_type_with(item, stack, vars)?);
                }
                Ok(Type::Tuple(out))
            }
            TypeExpr::Dict(key, value, _) => {
                let key = self.resolve_type_with(key, stack, vars)?;
                let value = self.resolve_type_with(value, stack, vars)?;
                Ok(Type::Dict(Box::new(key), Box::new(value)))
            }
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
                let count = param_types.len();
                Ok(Type::Function(FunctionType {
                    params: param_types,
                    required,
                    rest: *rest,
                    ret: Box::new(ret),
                    checked: true,
                    constraints: Vec::new(),
                    type_params: Vec::new(),
                    param_names: Vec::new(),
                    defaults: vec![false; count],
                    loose: false,
                }))
            }
        }
    }

    /// The shape of an interface, with the methods as functions.
    pub(crate) fn interface_type(&self, name: &str) -> Result<Type, VeyError> {
        self.interface_type_with(name, &HashMap::new(), &mut Vec::new())
    }

    /// The shape of an interface where its type parameters have a type.
    pub(crate) fn interface_type_with(
        &self,
        name: &str,
        vars: &HashMap<String, Type>,
        stack: &mut Vec<String>,
    ) -> Result<Type, VeyError> {
        let Some(info) = self.interfaces.get(name) else {
            return Err(self.error(
                crate::lexer::Pos::new(1, 1),
                format!("unknown type '{}'", name),
            ));
        };
        let mut fields = Vec::new();
        for member in &info.members {
            match member {
                InterfaceMember::Field(field) => {
                    let mut ty = self.resolve_type_with(&field.ty, stack, vars)?;
                    if field.optional {
                        ty = union(vec![ty, Type::Null]);
                    }
                    fields.push(ObjectField {
                        name: field.name.clone(),
                        ty,
                        optional: field.optional,
                    });
                }
                InterfaceMember::Method(method) => {
                    let mut params = Vec::new();
                    let mut required = 0usize;
                    let mut rest = false;
                    let mut defaults = Vec::new();
                    for param in &method.params {
                        let ty = match &param.ty {
                            Some(ty) => self.resolve_type_with(ty, stack, vars)?,
                            None => Type::Any,
                        };
                        if param.rest {
                            rest = true;
                            params.push(rest_element_type(&ty));
                        } else {
                            params.push(ty);
                        }
                        if param.default.is_none() && !param.rest {
                            required += 1;
                        }
                        defaults.push(param.default.is_some() || param.rest);
                    }
                    let ret = self.resolve_type_with(&method.ret, stack, vars)?;
                    fields.push(ObjectField {
                        name: method.name.clone(),
                        ty: Type::Function(FunctionType {
                            params,
                            required,
                            rest,
                            ret: Box::new(ret),
                            checked: true,
                            constraints: Vec::new(),
                            type_params: Vec::new(),
                            param_names: Vec::new(),
                            defaults,
                            loose: false,
                        }),
                        optional: false,
                    });
                }
            }
        }
        Ok(Type::Interface(name.to_string(), fields))
    }

    /// The one type argument of a type that takes one.
    fn one_type_argument(
        &self,
        name: &str,
        args: &[TypeExpr],
        pos: crate::lexer::Pos,
        stack: &mut Vec<String>,
        vars: &HashMap<String, Type>,
    ) -> Result<Type, VeyError> {
        if args.len() != 1 {
            return Err(self.error(
                pos,
                format!(
                    "type '{}' expects 1 type argument, got {}",
                    name,
                    args.len()
                ),
            ));
        }
        self.resolve_type_with(&args[0], stack, vars)
    }

    /// The types of the language that take type arguments.
    fn builtin_generic_type(        &self,
        name: &str,
        args: &[TypeExpr],
        pos: crate::lexer::Pos,
        stack: &mut Vec<String>,
        vars: &HashMap<String, Type>,
    ) -> Result<Option<Type>, VeyError> {
        match name {
            "Promise" => {
                if args.len() != 1 {
                    return Err(self.error(
                        pos,
                        format!(
                            "type 'Promise' expects 1 type argument, got {}",
                            args.len()
                        ),
                    ));
                }
                let inner = self.resolve_type_with(&args[0], stack, vars)?;
                Ok(Some(Type::Promise(Box::new(inner))))
            }
            "Option" => {
                if args.len() != 1 {
                    return Err(self.error(
                        pos,
                        format!(
                            "type 'Option' expects 1 type argument, got {}",
                            args.len()
                        ),
                    ));
                }
                let inner = self.resolve_type_with(&args[0], stack, vars)?;
                // A value that may be there is the value or nothing.
                Ok(Some(union(vec![inner, Type::Null])))
            }
            // The state of a page: a value that tells when it changes, and a
            // state that is shared between the parts of a page.
            "Signal" => {
                let inner = self.one_type_argument(name, args, pos, stack, vars)?;
                Ok(Some(signal_type(inner)))
            }
            "Store" => {
                let inner = self.one_type_argument(name, args, pos, stack, vars)?;
                Ok(Some(store_type(inner)))
            }
            "Result" => {
                if args.len() != 2 {
                    return Err(self.error(
                        pos,
                        format!(
                            "type 'Result' expects 2 type arguments, got {}",
                            args.len()
                        ),
                    ));
                }
                let ok = self.resolve_type_with(&args[0], stack, vars)?;
                let err = self.resolve_type_with(&args[1], stack, vars)?;
                Ok(Some(Type::Result(Box::new(ok), Box::new(err))))
            }
            _ => Ok(None),
        }
    }
}
