//! The checker that walks a program and reports type mismatches.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::builtins::is_builtin_type;
use crate::types::core::{
    rest_element_type, union, Alias, EnumFieldInfo, EnumInfo, EnumVariantInfo, FunctionType,
    ImplInfo, InterfaceInfo, ObjectField, Type,
};
use crate::types::helpers::name_alias;
use crate::types::rules::assignable;
use crate::VeyError;

/// Whether every path through the block leaves the function or the loop.
///
/// A block that ends in `return`, `break` or `continue` never falls through
/// to the code that follows it.
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
    /// The parameters of the exported components, so that a component of
    /// another module is checked as well.
    pub components: HashMap<String, Vec<ComponentParam>>,
    /// The enums of the module, so that the code generator does not have to
    /// describe them a second time.
    pub enums: HashMap<String, EnumInfo>,
    /// The interfaces of the module.
    pub interfaces: HashSet<String>,
    /// The names the module keeps to itself.
    pub private: HashSet<String>,
    /// Whether the module is one that the compiler cannot read.
    ///
    /// A package of npm is not checked, so its members are not known and
    /// are not reported.
    pub unchecked: bool,
}

pub fn check(
    program: &Program,
    file: &str,
    imports: &HashMap<String, ModuleTypeEnv>,
) -> Result<ModuleTypeEnv, VeyError> {
    let enums = collect_enums(program, file)?;
    let aliases = collect_aliases(program, file, &enums)?;
    let interfaces = collect_interfaces(program, file, &enums, &aliases)?;
    let impls = collect_impls(program, file, &aliases)?;
    let mut checker = TypeChecker {
        file,
        aliases,
        alias_types: HashMap::new(),
        enums,
        interfaces,
        impls,
        imports: imports.clone(),
        scopes: vec![HashMap::new()],
        type_param_scopes: Vec::new(),
        type_param_bindings: Vec::new(),
        type_param_constraints: Vec::new(),
        components: HashMap::new(),
        param_scopes: Vec::new(),
        expected_returns: Vec::new(),
        collected_returns: Vec::new(),
    };
    checker.check_program(program)?;
    // build the module's exported type environment
    let mut env = ModuleTypeEnv::default();
    // The enums of the module are described once, here. The code generator
    // uses the same description.
    env.enums = checker.enums.clone();
    // The interfaces of the module are known to the modules that use them.
    env.interfaces = checker.interfaces.keys().cloned().collect();
    // The names the module keeps to itself.
    env.private = program.private.iter().cloned().collect();
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
            Stmt::Interface(decl) if decl.exported => {
                if let Ok(ty) = checker.interface_type(&decl.name) {
                    env.types.insert(decl.name.clone(), ty);
                }
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
            Stmt::Component(decl) if decl.exported => {
                if let Some(params) = checker.components.get(&decl.name) {
                    env.components.insert(decl.name.clone(), params.clone());
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

/// The interfaces of a program.
fn collect_interfaces(
    program: &Program,
    file: &str,
    enums: &HashMap<String, EnumInfo>,
    aliases: &HashMap<String, Alias>,
) -> Result<HashMap<String, InterfaceInfo>, VeyError> {
    let mut interfaces = HashMap::new();
    for stmt in &program.body {
        if let Stmt::Interface(decl) = stmt {
            if is_builtin_type(&decl.name) {
                return Err(VeyError::new(
                    format!("cannot redefine built-in type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            if enums.contains_key(&decl.name) || aliases.contains_key(&decl.name) {
                return Err(VeyError::new(
                    format!("duplicate type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            let mut names = std::collections::HashSet::new();
            for member in &decl.members {
                let name = match member {
                    InterfaceMember::Field(field) => field.name.clone(),
                    InterfaceMember::Method(method) => method.name.clone(),
                };
                if !names.insert(name.clone()) {
                    return Err(VeyError::new(
                        format!("duplicate member '{}' in interface '{}'", name, decl.name),
                        file,
                        decl.pos.line,
                        decl.pos.col,
                    ));
                }
            }
            let info = InterfaceInfo {
                members: decl.members.clone(),
            };
            if interfaces.insert(decl.name.clone(), info).is_some() {
                return Err(VeyError::new(
                    format!("duplicate type '{}'", decl.name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
        }
    }
    Ok(interfaces)
}

/// The functions of the types of a program.
fn collect_impls(
    program: &Program,
    file: &str,
    aliases: &HashMap<String, Alias>,
) -> Result<HashMap<String, ImplInfo>, VeyError> {
    let mut impls = HashMap::new();
    for stmt in &program.body {
        if let Stmt::Impl(decl) = stmt {
            // The type has to be one of the program, or one of the modules
            // it uses.
            if !aliases.contains_key(&decl.type_name) {
                return Err(VeyError::new(
                    format!("unknown type '{}'", decl.type_name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
            let mut names = std::collections::HashSet::new();
            for method in &decl.methods {
                if !names.insert(method.name.clone()) {
                    return Err(VeyError::new(
                        format!(
                            "duplicate method '{}' in impl '{}'",
                            method.name, decl.type_name
                        ),
                        file,
                        method.pos.line,
                        method.pos.col,
                    ));
                }
            }
            let info = ImplInfo {
                methods: decl.methods.clone(),
            };
            if impls.insert(decl.type_name.clone(), info).is_some() {
                return Err(VeyError::new(
                    format!("duplicate impl for type '{}'", decl.type_name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            }
        }
    }
    Ok(impls)
}

/// A parameter of a component.
#[derive(Clone)]
pub struct ComponentParam {
    pub name: String,
    pub ty: Type,
    pub has_default: bool,
    /// Whether the component takes the rest of the arguments.
    pub rest: bool,
}

pub(crate) struct TypeChecker<'a> {
    pub(crate) file: &'a str,
    pub(crate) aliases: HashMap<String, Alias>,
    /// The resolved body of every type alias.
    pub(crate) alias_types: HashMap<String, Type>,
    pub(crate) enums: HashMap<String, EnumInfo>,
    /// The interfaces of the program.
    pub(crate) interfaces: HashMap<String, InterfaceInfo>,
    /// The functions that the types of the program have of their own.
    pub(crate) impls: HashMap<String, ImplInfo>,
    pub(crate) imports: HashMap<String, ModuleTypeEnv>,
    pub(crate) scopes: Vec<HashMap<String, Type>>,
    pub(crate) type_param_scopes: Vec<HashMap<String, Type>>,
    /// The types the type parameters of the current scope were bound to.
    pub(crate) type_param_bindings: Vec<HashMap<String, Type>>,
    /// The constraints of the type parameters of the current scope.
    pub(crate) type_param_constraints: Vec<Vec<(String, Option<Type>)>>,
    pub(crate) components: HashMap<String, Vec<ComponentParam>>,
    /// The types that the parameters of the functions that are being checked
    /// are known to have.
    ///
    /// A parameter without a type is judged by the way it is used, and the
    /// type it is given here is the type of the arguments of a call.
    pub(crate) param_scopes: Vec<HashMap<String, Type>>,
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
        names.extend(self.interfaces.keys().map(|name| name.as_str()));
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

    /// Open a type that refers to itself.
    ///
    /// A type that cannot be written out, such as the end of a list of
    /// links, is opened up to the shape of its alias. The opening has a
    /// depth, so that a type that refers to itself does not open for ever.
    pub(crate) fn expand(&self, ty: &Type, depth: usize) -> Type {
        if depth == 0 {
            return ty.clone();
        }
        match ty {
            Type::Recursion(name) => match self.alias_types.get(name) {
                Some(shape) => self.expand(shape, depth - 1),
                None => ty.clone(),
            },
            Type::Array(inner) => Type::Array(Box::new(self.expand(inner, depth))),
            Type::Tuple(items) => {
                Type::Tuple(items.iter().map(|item| self.expand(item, depth)).collect())
            }
            Type::Dict(key, value) => Type::Dict(
                Box::new(self.expand(key, depth)),
                Box::new(self.expand(value, depth)),
            ),
            Type::Promise(inner) => Type::Promise(Box::new(self.expand(inner, depth))),
            Type::Result(ok, err) => Type::Result(
                Box::new(self.expand(ok, depth)),
                Box::new(self.expand(err, depth)),
            ),
            Type::Object(fields) => Type::Object(
                fields
                    .iter()
                    .map(|field| ObjectField {
                        name: field.name.clone(),
                        ty: self.expand(&field.ty, depth),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Named(name, fields) => Type::Named(
                name.clone(),
                fields
                    .iter()
                    .map(|field| ObjectField {
                        name: field.name.clone(),
                        ty: self.expand(&field.ty, depth),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Interface(name, fields) => Type::Interface(
                name.clone(),
                fields
                    .iter()
                    .map(|field| ObjectField {
                        name: field.name.clone(),
                        ty: self.expand(&field.ty, depth),
                        optional: field.optional,
                    })
                    .collect(),
            ),
            Type::Union(items) => union(
                items
                    .iter()
                    .map(|item| self.expand(item, depth))
                    .collect(),
            ),
            other => other.clone(),
        }
    }

    /// Whether a value of one type may be used where another is expected.
    ///
    /// The types that refer to themselves are opened up first, so that the
    /// end of a list of links is checked like the rest of it.
    pub(crate) fn fits(&self, source: &Type, target: &Type) -> bool {
        assignable(&self.expand(source, 6), &self.expand(target, 6))
    }
    pub(crate) fn param_type(&self, param: &Param) -> Result<Type, VeyError> {
        let ty = match &param.ty {
            Some(ty) => self.resolve_type(ty)?,
            None => Type::Any,
        };
        if param.rest {
            // Inside the function the rest parameter is a list of the values,
            // while a call gives every one of them on its own.
            return Ok(Type::Array(Box::new(rest_element_type(&ty))));
        }
        Ok(ty)
    }

    pub(crate) fn function_type_from_parts(
        &self,
        params: &[Param],
        ret: Type,
        ret_annotated: bool,
        type_params: &[TypeParam],
        inferred: &[Type],
    ) -> Result<Type, VeyError> {
        let mut param_types = Vec::new();
        for (index, param) in params.iter().enumerate() {
            let ty = self.param_type(param)?;
            if param.rest {
                // A rest parameter is written as a list, but it stands for
                // the type of a single extra argument.
                param_types.push(rest_element_type(&ty));
            } else if param.ty.is_none() {
                // The type of a parameter that was not written is the one
                // that the way it is used asks for.
                let found = inferred.get(index).cloned().unwrap_or(Type::Any);
                param_types.push(found);
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
        self.function_type_from_parts(&decl.params, ret, decl.ret.is_some(), &decl.type_params, &[])
    }
}
