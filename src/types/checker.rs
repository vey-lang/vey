//! The checker that walks a program and reports type mismatches.

use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::lexer::Pos;
use crate::types::builtins::is_builtin_type;
use crate::types::core::{
    rest_element_type, Alias, EnumFieldInfo, EnumInfo, EnumVariantInfo, FunctionType, ImplInfo,
    InterfaceInfo, ResolvedCalls, Type,
};
use crate::types::helpers::name_alias;
use crate::types::rules::{assignable_with, Opening};
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
    /// The functions that the types of the module have of their own, by the
    /// name of the type and the name of the function.
    pub impls: HashMap<String, HashMap<String, Type>>,
    /// The names the module keeps to itself.
    pub private: HashSet<String>,
    /// Whether the module is one that the compiler cannot read.
    ///
    /// A package of npm is not checked, so its members are not known and
    /// are not reported.
    pub unchecked: bool,
    /// The calls whose arguments are given by name, and the order of the
    /// arguments.
    pub resolved: ResolvedCalls,
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
        member_narrowings: vec![HashMap::new()],
        type_param_scopes: Vec::new(),
        type_param_bindings: Vec::new(),
        type_param_constraints: Vec::new(),
        components: HashMap::new(),
        param_scopes: Vec::new(),
        resolved: HashMap::new(),
        try_depth: 0,
        shadowed: HashSet::new(),
        expected_returns: Vec::new(),
        collected_returns: Vec::new(),
        yielded: Vec::new(),
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
    // The calls whose arguments are given by name.
    env.resolved = checker.resolved.clone();
    // The calls whose arguments are given by name.
    env.resolved = checker.resolved.clone();
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
            Stmt::Impl(decl) => {
                // The functions of the type are known to the modules that
                // use it.
                let mut methods = HashMap::new();
                for method in &decl.methods {
                    if let Ok(ty) = checker.method_type(method) {
                        methods.insert(method.name.clone(), ty);
                    }
                }
                env.impls.insert(decl.type_name.clone(), methods);
                // The constructor of a type is public when the type is, so
                // that the users of the module may make a value of it.
                if !env.types.contains_key(&decl.type_name) {
                    continue;
                }
                let base = env.types.get(&decl.type_name).cloned().unwrap_or(Type::Any);
                // The constructor takes the shape of the type, and gives
                // the value the functions of the type.
                let shape = match &base {
                    Type::Named(_, fields) => Type::Object(fields.clone()),
                    other => other.clone(),
                };
                env.values.insert(
                    decl.type_name.clone(),
                    Type::Function(FunctionType {
                        params: vec![shape],
                        required: 1,
                        rest: false,
                        ret: Box::new(base),
                        checked: true,
                        constraints: Vec::new(),
                        type_params: Vec::new(),
                        param_names: vec![decl.type_name.clone()],
                        defaults: vec![false],
                    }),
                );
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
                params: decl.type_params.clone(),
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

/// Whether the body of an alias is a type of another module.
///
/// A local alias that is written over a type of another module names that
/// type, and the values of it are made in the module that owns it.
fn is_foreign_type(aliases: &HashMap<String, Alias>, body: &TypeExpr, depth: usize) -> bool {
    if depth > 8 {
        return false;
    }
    match body {
        TypeExpr::Qualified(..) => true,
        TypeExpr::Name(name, _) => match aliases.get(name) {
            Some(alias) => is_foreign_type(aliases, &alias.body, depth + 1),
            None => false,
        },
        TypeExpr::Union(items, _) => {
            !items.is_empty()
                && items
                    .iter()
                    .all(|item| is_foreign_type(aliases, item, depth + 1))
        }
        _ => false,
    }
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
            let Some(alias) = aliases.get(&decl.type_name) else {
                return Err(VeyError::new(
                    format!("unknown type '{}'", decl.type_name),
                    file,
                    decl.pos.line,
                    decl.pos.col,
                ));
            };
            // The methods of a type are written where the type is made. A
            // type of another module is made there, so a method that is
            // written here would never reach a value of it.
            if is_foreign_type(aliases, &alias.body, 0) {
                return Err(VeyError::new(
                    format!(
                        "cannot add methods to '{}', because the type belongs to another module",
                        decl.type_name
                    ),
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
    /// The members that a check has narrowed, such as `user.name`, kept
    /// apart from the names of the program.
    pub(crate) member_narrowings: Vec<HashMap<String, Type>>,
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
    /// The calls whose arguments are given by name, and the order of the
    /// arguments.
    pub(crate) resolved: ResolvedCalls,
    /// How many `try` blocks are around, so that a `?` may be caught.
    pub(crate) try_depth: usize,
    /// The names of the modules that a name of the program hides.
    pub(crate) shadowed: HashSet<String>,
    pub(crate) expected_returns: Vec<Option<Type>>,
    pub(crate) collected_returns: Vec<Vec<Type>>,
    /// The types of the values that a generator hands back one by one, so
    /// that the type of a generator that was not written is known.
    pub(crate) yielded: Vec<Type>,
}

impl TypeChecker<'_> {
    /// A checker that knows the types of a program but does not check it.
    ///
    /// It is used to say the type of an expression, where the mistakes of
    /// the program do not matter.
    /// Check the body of a function and leave its names in the scope, so
    /// that the type of an expression inside it may be asked about.
    pub(crate) fn open_function(
        &mut self,
        params: &[Param],
        body: &Block,
        ret: &Option<TypeExpr>,
    ) -> Result<(), VeyError> {
        self.push_scope();
        for param in params {
            let ty = match &param.ty {
                Some(ty) => self.resolve_type(ty)?,
                None => Type::Any,
            };
            for (name, _) in param.pattern.bindings() {
                self.declare(&name, ty.clone());
            }
        }
        self.expected_returns.push(match ret {
            Some(ty) => Some(self.resolve_type(ty)?),
            None => None,
        });
        for stmt in &body.body {
            self.check_stmt(stmt)?;
        }
        self.expected_returns.pop();
        Ok(())
    }

    pub(crate) fn for_explain<'a>(
        program: &Program,
        file: &'a str,
    ) -> Result<TypeChecker<'a>, VeyError> {
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
            imports: HashMap::new(),
            scopes: vec![HashMap::new()],
            member_narrowings: vec![HashMap::new()],
            type_param_scopes: Vec::new(),
            type_param_bindings: Vec::new(),
            type_param_constraints: Vec::new(),
            components: HashMap::new(),
            param_scopes: Vec::new(),
            resolved: HashMap::new(),
            try_depth: 0,
            yielded: Vec::new(),
            shadowed: HashSet::new(),
            expected_returns: Vec::new(),
            collected_returns: Vec::new(),
        };
        checker.predeclare_top_level(program)?;
        Ok(checker)
    }

    /// The type of an expression.
    pub(crate) fn infer_public(&mut self, expr: &Expr) -> Result<Type, VeyError> {
        self.infer_expr(expr)
    }

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
        self.member_narrowings.push(HashMap::new());
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
        self.member_narrowings.pop();
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
        if self.imports.contains_key(name) {
            // A name of the program hides the alias of a module.
            self.shadowed.insert(name.to_string());
        }
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), ty);
        }
    }

    /// Narrow a member such as `user.name`, in the scope that is open.
    pub(crate) fn declare_member(&mut self, path: &str, ty: Type) {
        if let Some(scope) = self.member_narrowings.last_mut() {
            scope.insert(path.to_string(), ty);
        }
    }

    /// The narrowed type of a member, when a check has given one.
    pub(crate) fn lookup_member_path(&self, path: &str) -> Option<Type> {
        self.member_narrowings
            .iter()
            .rev()
            .find_map(|scope| scope.get(path).cloned())
    }

    /// A narrowing is no longer true when the value it is about is written.
    ///
    /// A write to `user` says nothing about `user.name`, but a write to
    /// `user.name` or to a member of it does.
    pub(crate) fn forget_member(&mut self, path: &str) {
        for scope in self.member_narrowings.iter_mut() {
            scope.retain(|narrowed, _| {
                narrowed != path
                    && !narrowed.starts_with(&format!("{}.", path))
                    && !path.starts_with(&format!("{}.", narrowed))
            });
        }
    }

    /// Declare a narrowing that a check gave, of a name or of a member.
    pub(crate) fn declare_narrowed(&mut self, name: &str, ty: Type) {
        if name.contains('.') {
            self.declare_member(name, ty);
        } else {
            self.declare(name, ty);
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

    /// The alias of a module is a name of its own.
    pub(crate) fn declare_alias(&mut self, name: &str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), Type::Any);
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
        if name.contains('.') {
            // A member is not a binding, so its narrowing lives in its own
            // scope.
            self.declare_member(name, ty);
            return;
        }
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

    /// Whether a value of one type may be used where another is expected.
    ///
    /// The types that refer to themselves are opened up first, so that the
    /// end of a list of links is checked like the rest of it.
    pub(crate) fn fits(&self, source: &Type, target: &Type) -> bool {
        // A type that has functions of its own is only made by its
        // constructor, which gives the value those functions. A plain
        // object is not such a value.
        if matches!(source, Type::Object(_)) && self.has_methods(target) {
            return false;
        }
        // A type that refers to itself is opened where the check reaches
        // it, and not into a copy of itself: a shape of six fields would
        // otherwise become six copies of six copies, six times over.
        let mut opening = Opening::new(&self.alias_types);
        assignable_with(source, target, &mut opening)
    }

    /// Whether a type has functions of its own.
    ///
    /// The functions of a type of another module are known to the module
    /// that declares it.
    pub(crate) fn has_methods(&self, ty: &Type) -> bool {
        let name = match ty {
            Type::Named(name, _) | Type::Interface(name, _) => name,
            _ => return false,
        };
        match name.split_once('.') {
            Some((alias, plain)) => self
                .imports
                .get(alias)
                .and_then(|env| env.impls.get(plain))
                .is_some(),
            None => self.impls.contains_key(name),
        }
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
        // A parameter that comes after one with a value is still needed:
        // the caller has to give the ones in front of it as well.
        let required = params
            .iter()
            .enumerate()
            .filter(|(_, param)| param.default.is_none() && !param.rest)
            .map(|(index, _)| index + 1)
            .max()
            .unwrap_or(0);
        // The names of the parameters, so that the arguments that are given
        // by name may be matched.
        let param_names: Vec<String> = params
            .iter()
            .map(|param| param.name().unwrap_or("").to_string())
            .collect();
        let rest = params.last().map(|param| param.rest).unwrap_or(false);
        // A function whose parameters were guessed is checked like one that
        // writes its types, so that the same code is checked in both forms.
        let checked = ret_annotated
            || params.iter().any(|param| param.ty.is_some())
            || inferred.iter().any(|ty| !matches!(ty, Type::Any));
        let mut constraints = Vec::new();
        for param in type_params {
            if let Some(expr) = &param.constraint {
                constraints.push((param.name.clone(), self.resolve_type(expr)?));
            }
        }
        let defaults = params
            .iter()
            .map(|param| param.default.is_some() || param.rest)
            .collect();
        Ok(Type::Function(FunctionType {
            params: param_types,
            required,
            rest,
            ret: Box::new(ret),
            checked,
            constraints,
            type_params: type_params.iter().map(|param| param.name.clone()).collect(),
            param_names,
            defaults,
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
