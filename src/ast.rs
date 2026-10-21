use crate::lexer::Pos;

#[derive(Debug, Clone)]
pub struct Program {
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Block {
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Param {
    /// The name of the parameter, or a pattern that takes it apart.
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub default: Option<Expr>,
    pub rest: bool,
    pub pos: Pos,
}

impl Param {
    /// The name of the parameter, when it is a simple one.
    pub fn name(&self) -> Option<&str> {
        match &self.pattern {
            BindingPattern::Name { name, .. } => Some(name),
            _ => None,
        }
    }

    /// The names the parameter binds.
    pub fn bindings(&self) -> Vec<(&str, Pos)> {
        self.pattern.bindings()
    }
}

#[derive(Debug, Clone)]
pub struct VarDecl {
    pub kind: VarKind,
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub init: Option<Expr>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum BindingPattern {
    Name {
        name: String,
        pos: Pos,
    },
    Array {
        items: Vec<BindingPattern>,
        rest: Option<String>,
        pos: Pos,
    },
    Object {
        fields: Vec<BindingField>,
        rest: Option<String>,
        pos: Pos,
    },
}

#[derive(Debug, Clone)]
pub struct BindingField {
    pub key: String,
    pub pattern: BindingPattern,
    pub pos: Pos,
}

impl BindingPattern {
    pub fn bindings(&self) -> Vec<(&str, Pos)> {
        let mut names = Vec::new();
        self.collect_bindings(&mut names);
        names
    }

    fn collect_bindings<'a>(&'a self, names: &mut Vec<(&'a str, Pos)>) {
        match self {
            Self::Name { name, pos } => names.push((name.as_str(), *pos)),
            Self::Array { items, rest, pos } => {
                for item in items {
                    item.collect_bindings(names);
                }
                if let Some(rest) = rest {
                    names.push((rest.as_str(), *pos));
                }
            }
            Self::Object { fields, rest, pos } => {
                for field in fields {
                    field.pattern.collect_bindings(names);
                }
                if let Some(rest) = rest {
                    names.push((rest.as_str(), *pos));
                }
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    Let,
    Const,
}

/// A type parameter of a function or a type alias.
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    /// The type the parameter has to satisfy, for example `T: number`.
    pub constraint: Option<TypeExpr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct FnDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret: Option<TypeExpr>,
    pub body: Block,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ComponentDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub children: Vec<MarkupNode>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct IfStmt {
    pub test: Expr,
    pub then_block: Block,
    pub else_branch: Option<Box<Stmt>>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct WhileStmt {
    pub test: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ForStmt {
    pub pattern: BindingPattern,
    pub ty: Option<TypeExpr>,
    pub iter: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct RepeatStmt {
    pub count: Expr,
    pub body: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct ReturnStmt {
    pub value: Option<Expr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct TryStmt {
    pub body: Block,
    pub name: Option<String>,
    pub catch: Block,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct UseStmt {
    pub path: String,
    pub alias: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct StyleDecl {
    pub css: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct TypeDecl {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub ty: TypeExpr,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub exported: bool,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumVariant {
    pub name: String,
    pub fields: Vec<EnumField>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumField {
    pub name: String,
    pub ty: Option<TypeExpr>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct EnumBinding {
    pub field: Option<String>,
    pub name: String,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Name(String, Pos),
    Qualified(String, String, Pos),
    Generic(String, Vec<TypeExpr>, Pos),
    Array(Box<TypeExpr>, Pos),
    Object(Vec<TypeField>, Pos),
    Optional(Box<TypeExpr>, Pos),
    Union(Vec<TypeExpr>, Pos),
    Function(Vec<TypeExpr>, Box<TypeExpr>, bool, Pos),
}

#[derive(Debug, Clone)]
pub struct TypeField {
    pub name: String,
    pub ty: TypeExpr,
    pub optional: bool,
    pub pos: Pos,
}

impl TypeExpr {
    pub fn pos(&self) -> Pos {
        match self {
            Self::Name(_, pos)
            | Self::Qualified(_, _, pos)
            | Self::Generic(_, _, pos)
            | Self::Array(_, pos)
            | Self::Object(_, pos)
            | Self::Optional(_, pos)
            | Self::Union(_, pos)
            | Self::Function(_, _, _, pos) => *pos,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExprStmt {
    pub expr: Expr,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct AssignStmt {
    pub target: Expr,
    pub op: String,
    pub value: Expr,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Var(VarDecl),
    Fn(FnDecl),
    Component(ComponentDecl),
    Enum(EnumDecl),
    If(IfStmt),
    While(WhileStmt),
    For(ForStmt),
    Repeat(RepeatStmt),
    Return(ReturnStmt),
    Break(Pos),
    Continue(Pos),
    Try(TryStmt),
    Use(UseStmt),
    Style(StyleDecl),
    Type(TypeDecl),
    Expr(ExprStmt),
    Assign(AssignStmt),
    Block(Block, Pos),
}

impl Stmt {
    pub fn pos(&self) -> Pos {
        match self {
            Stmt::Var(s) => s.pos,
            Stmt::Fn(s) => s.pos,
            Stmt::Component(s) => s.pos,
            Stmt::Enum(s) => s.pos,
            Stmt::If(s) => s.pos,
            Stmt::While(s) => s.pos,
            Stmt::For(s) => s.pos,
            Stmt::Repeat(s) => s.pos,
            Stmt::Return(s) => s.pos,
            Stmt::Break(p) => *p,
            Stmt::Continue(p) => *p,
            Stmt::Try(s) => s.pos,
            Stmt::Use(s) => s.pos,
            Stmt::Style(s) => s.pos,
            Stmt::Type(s) => s.pos,
            Stmt::Expr(s) => s.pos,
            Stmt::Assign(s) => s.pos,
            Stmt::Block(_, p) => *p,
        }
    }

    pub fn exported_name(&self) -> Option<(&str, bool)> {
        match self {
            Stmt::Var(s) => match &s.pattern {
                BindingPattern::Name { name, .. } => Some((name.as_str(), s.exported)),
                _ => None,
            },
            Stmt::Fn(s) => Some((s.name.as_str(), s.exported)),
            Stmt::Component(s) => Some((s.name.as_str(), s.exported)),
            Stmt::Enum(s) => Some((s.name.as_str(), s.exported)),
            // types are compile-time only; not exported as runtime values
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StrPart {
    pub text: String,
    pub expr: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub patterns: Vec<Pattern>,
    pub body: Expr,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Literal(Expr),
    Range(Expr, Expr),
    Wildcard,
    Enum {
        enum_name: String,
        variant: String,
        bindings: Vec<EnumBinding>,
        pos: Pos,
    },
}

#[derive(Debug, Clone)]
pub enum MemberProp {
    Name(String),
    Computed(Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum ArrowBody {
    Expr(Box<Expr>),
    Block(Block),
}

#[derive(Debug, Clone)]
pub enum ObjectProp {
    Field(String, Expr),
    Spread(Expr),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Number(f64, Pos),
    String(Vec<StrPart>, Pos),
    Bool(bool, Pos),
    Null(Pos),
    Ident(String, Pos),
    Array(Vec<Expr>, Pos),
    Object(Vec<ObjectProp>, Pos),
    Unary(String, Box<Expr>, Pos),
    Binary(String, Box<Expr>, Box<Expr>, Pos),
    Logical(String, Box<Expr>, Box<Expr>, Pos),
    Nullish(Box<Expr>, Box<Expr>, Pos),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>, Pos),
    Call(Box<Expr>, Vec<Expr>, bool, Pos),
    /// A call with explicit type arguments, such as `parse<number>(text)`.
    CallGeneric(Box<Expr>, Vec<TypeExpr>, Vec<Expr>, bool, Pos),
    Member(Box<Expr>, MemberProp, bool, Pos),
    Fn(Vec<Param>, Block, Option<TypeExpr>, Pos),
    Arrow(Vec<Param>, ArrowBody, Pos),
    Range(Box<Expr>, Box<Expr>, Pos),
    Match(Box<Expr>, Vec<MatchArm>, Pos),
    Await(Box<Expr>, Pos),
    Spread(Box<Expr>, Pos),
}

impl Expr {
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Number(_, p)
            | Expr::String(_, p)
            | Expr::Bool(_, p)
            | Expr::Null(p)
            | Expr::Ident(_, p)
            | Expr::Array(_, p)
            | Expr::Object(_, p)
            | Expr::Unary(_, _, p)
            | Expr::Binary(_, _, _, p)
            | Expr::Logical(_, _, _, p)
            | Expr::Nullish(_, _, p)
            | Expr::Ternary(_, _, _, p)
            | Expr::Call(_, _, _, p)
            | Expr::CallGeneric(_, _, _, _, p)
            | Expr::Member(_, _, _, p)
            | Expr::Fn(_, _, _, p)
            | Expr::Arrow(_, _, p)
            | Expr::Range(_, _, p)
            | Expr::Match(_, _, p)
            | Expr::Await(_, p)
            | Expr::Spread(_, p) => *p,
        }
    }
}

#[derive(Debug, Clone)]
pub enum MarkupNode {
    Element(Element),
    Expr(Expr),
    If(MIf),
    For(MFor),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub tag: String,
    pub component: bool,
    pub classes: Vec<String>,
    pub id: Option<String>,
    pub attrs: Vec<(String, Expr)>,
    /// Positional arguments of a component call, e.g. `Card("title")`.
    pub args: Vec<Expr>,
    pub children: Vec<MarkupNode>,
    pub pos: Pos,
}

#[derive(Debug, Clone)]
pub struct MIf {
    pub test: Expr,
    pub then_nodes: Vec<MarkupNode>,
    pub else_nodes: Vec<MarkupNode>,
}

#[derive(Debug, Clone)]
pub struct MFor {
    pub name: String,
    pub iter: Expr,
    pub body: Vec<MarkupNode>,
}
