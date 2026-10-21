//! The resolution of the types that are written in a program.

use std::collections::HashMap;

use crate::ast::*;
use crate::suggest::did_you_mean;
use crate::types::core::{substitute, union, FunctionType, ObjectField, Type};
use crate::VeyError;

use super::checker::TypeChecker;
use super::helpers::{name_alias, type_argument_label};

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
}
