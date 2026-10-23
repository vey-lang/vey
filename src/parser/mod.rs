//! The recursive descent parser.
//!
//! The grammar is split over a few modules: `types` reads type expressions,
//! `patterns` reads binding patterns and `markup` reads the tag syntax.

pub(crate) mod exprs;
pub(crate) mod html;
mod markup;
mod patterns;
mod strings;
mod types;

use crate::ast::*;
use crate::lexer::{Lexer, Pos, TokKind, Token};
use crate::VeyError;

const ASSIGN_OPS: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "**=", "??=", "&=", "|=", "^=", "<<=", ">>=",
];

#[derive(Clone)]
pub struct Parser {
    lexer: Lexer,
    file: String,
    tok: Token,
    /// How deep the parser is in nested code.
    depth: usize,
    /// The names that the program keeps to itself.
    private_names: Vec<String>,
    /// Whether the expression that is read is the whole of a statement.
    statement_expr: bool,
}

/// How deeply code may be nested before the parser gives up.
///
/// The parser reads the code with recursion, and every level of an
/// expression uses a few calls, so a very deep expression would otherwise
/// use up the stack.
const MAX_DEPTH: usize = 64;

impl Parser {
    pub fn new(src: &str, file: &str) -> Result<Self, VeyError> {
        Self::with_pos(src, file, 1, 1)
    }

    pub fn with_pos(src: &str, file: &str, line: usize, col: usize) -> Result<Self, VeyError> {
        let mut lexer = Lexer::with_pos(src, file, line, col);
        let tok = lexer.next_token()?;
        Ok(Self {
            lexer,
            file: file.to_string(),
            tok,
            depth: 0,
            private_names: Vec::new(),
            statement_expr: false,
        })
    }

    fn advance(&mut self) -> Result<Token, VeyError> {
        let current = self.tok.clone();
        self.tok = self.lexer.next_token()?;
        Ok(current)
    }

    fn peek(&mut self) -> Result<Token, VeyError> {
        let mut lexer = self.lexer.clone();
        lexer.next_token()
    }

    fn peek_n(&mut self, n: usize) -> Result<Token, VeyError> {
        let mut lexer = self.lexer.clone();
        let mut token = lexer.next_token()?;
        for _ in 1..n {
            token = lexer.next_token()?;
        }
        Ok(token)
    }

    fn error(&self, message: impl Into<String>) -> VeyError {
        VeyError::new(
            message,
            self.file.clone(),
            self.tok.pos.line,
            self.tok.pos.col,
        )
    }

    fn error_at(&self, pos: Pos, message: impl Into<String>) -> VeyError {
        VeyError::new(message, self.file.clone(), pos.line, pos.col)
    }

    fn is_kw(&self, word: &str) -> bool {
        self.tok.kind == TokKind::Keyword && self.tok.value == word
    }

    fn is_op(&self, op: &str) -> bool {
        self.tok.kind == TokKind::Op && self.tok.value == op
    }

    fn is_punct(&self, p: &str) -> bool {
        self.tok.kind == TokKind::Punct && self.tok.value == p
    }

    fn eat_kw(&mut self, word: &str) -> Result<bool, VeyError> {
        if self.is_kw(word) {
            self.advance()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn eat_op(&mut self, op: &str) -> Result<bool, VeyError> {
        if self.is_op(op) {
            self.advance()?;
            return Ok(true);
        }
        Ok(false)
    }

    fn expect_kw(&mut self, word: &str) -> Result<Token, VeyError> {
        if !self.is_kw(word) {
            return Err(self.error(format!("expected '{}'", word)));
        }
        self.advance()
    }

    fn expect_op(&mut self, op: &str) -> Result<Token, VeyError> {
        if !self.is_op(op) {
            return Err(self.error(format!("expected '{}'", op)));
        }
        self.advance()
    }

    /// Take one `>` of the marks that close a type.
    ///
    /// The lexer reads `>>` as one mark, because it is the shift of the
    /// language. Two types that close one after the other, such as
    /// `Box<Maybe<number>>`, need both of them, so one is kept for the next
    /// time.
    pub(crate) fn eat_angle(&mut self) -> Result<bool, VeyError> {
        if self.is_op(">") {
            self.advance()?;
            return Ok(true);
        }
        if self.is_op(">>") {
            let mut token = self.tok.clone();
            token.value = ">".to_string();
            token.start += 1;
            self.tok = token;
            return Ok(true);
        }
        Ok(false)
    }

    /// Take the mark that closes a type.
    pub(crate) fn expect_angle(&mut self) -> Result<(), VeyError> {
        if self.eat_angle()? {
            return Ok(());
        }
        Err(self.error("expected '>'"))
    }

    fn expect_punct(&mut self, p: &str) -> Result<Token, VeyError> {
        if !self.is_punct(p) {
            return Err(self.error(format!("expected '{}'", p)));
        }
        self.advance()
    }

    fn expect_ident(&mut self, what: &str) -> Result<String, VeyError> {
        if self.tok.kind != TokKind::Ident {
            return Err(self.reserved_word_error(what));
        }
        Ok(self.advance()?.value)
    }

    /// The name of a member of a value.
    ///
    /// A word of the language may be the name of a member, because the dot
    /// says that it is a name and not a word: `app.use(...)`.
    fn expect_member_name(&mut self, what: &str) -> Result<String, VeyError> {
        if self.tok.kind != TokKind::Ident && self.tok.kind != TokKind::Keyword {
            return Err(self.reserved_word_error(what));
        }
        Ok(self.advance()?.value)
    }

    /// `async` is reserved so that it cannot be used as a plain name, but it
    /// is not part of the language. Report that instead of a bare
    /// "expected identifier".
    fn reserved_word_error(&self, what: &str) -> VeyError {
        if self.tok.kind == TokKind::Keyword && self.tok.value == "async" {
            return self
                .error_at(self.tok.pos, "'async' is not supported")
                .with_hint("a function that uses `await` is already async");
        }
        self.error(format!("expected {}", what))
    }

    /// Go one level deeper into the code.
    pub(crate) fn enter(&mut self) -> Result<(), VeyError> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.error("the code is nested too deeply"));
        }
        Ok(())
    }

    /// Leave the current level of the code.
    pub(crate) fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    pub fn parse_program(&mut self) -> Result<Program, VeyError> {
        let mut body = Vec::new();
        let mut private = Vec::new();
        while self.tok.kind != TokKind::Eof {
            let stmt = self.parse_statement()?;
            if let Some((name, _)) = stmt.exported_name() {
                if self.private_names.contains(&name.to_string()) {
                    private.push(name.to_string());
                }
            }
            body.push(stmt);
        }
        // The doc comments and the version of the file are read by the
        // lexer, which has seen the whole file by now.
        let version = self.lexer.version().map(|version| version.to_string());
        let docs = self
            .lexer
            .take_docs()
            .into_iter()
            .map(|(text, pos)| DocComment { text, pos })
            .collect();
        Ok(Program {
            body,
            docs,
            version,
            private,
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt, VeyError> {
        self.enter()?;
        // A semicolon is a separator between statements, and it may be left
        // out. The ones that stand alone are read here, so that a line of
        // semicolons is not a mistake.
        while self.is_punct(";") {
            self.advance()?;
        }
        let result = self.parse_statement_inner();
        self.leave();
        result
    }

    fn parse_statement_inner(&mut self) -> Result<Stmt, VeyError> {
        // A name may be public or private.
        if self.is_kw("pub") || self.is_kw("priv") {
            let private = self.tok.value == "priv";
            let word = self.tok.value.clone();
            self.advance()?;
            let mut stmt = self.parse_declaration()?;
            if let Stmt::Var(decl) = &stmt {
                if !matches!(decl.pattern, BindingPattern::Name { .. }) {
                    return Err(self.error_at(
                        decl.pos,
                        format!("{} requires a single name", word),
                    ));
                }
            }
            if let Some((name, _)) = stmt.exported_name() {
                if private {
                    self.private_names.push(name.to_string());
                } else {
                    mark_exported(&mut stmt);
                }
            }
            return Ok(stmt);
        }

        if self.is_kw("export") {
            self.advance()?;
            let mut stmt = self.parse_declaration()?;
            if let Stmt::Var(decl) = &stmt {
                if !matches!(decl.pattern, BindingPattern::Name { .. }) {
                    return Err(self.error_at(decl.pos, "export requires a single name"));
                }
            }
            mark_exported(&mut stmt);
            return Ok(stmt);
        }

        if self.type_decl_here()? {
            return self.parse_type_decl(false);
        }

        if self.interface_decl_here()? {
            return self.parse_interface(false);
        }

        if self.impl_decl_here()? {
            return self.parse_impl(false);
        }

        if self.style_block_here()? {
            return self.parse_style_block();
        }

        // A loop may have a name in front of it.
        if self.tok.kind == TokKind::Ident {
            if let Some(label) = self.parse_loop_label()? {
                return match self.tok.value.as_str() {
                    "while" => self.parse_while(Some(label)),
                    "for" => self.parse_for(Some(label)),
                    "repeat" => self.parse_repeat(Some(label)),
                    _ => Err(self.error("a name in front of a loop names a loop")),
                };
            }
        }

        if self.tok.kind == TokKind::Keyword {
            match self.tok.value.as_str() {
                "let" | "const" => return self.parse_var(false),
                "fn" => {
                    let next = self.peek()?;
                    if next.kind == TokKind::Ident {
                        return self.parse_fn(false);
                    }
                }
                "component" => return self.parse_component(false),
                "enum" => return self.parse_enum(false),
                "if" => return self.parse_if(),
                "while" => return self.parse_while(None),
                "for" => return self.parse_for(None),
                "repeat" => return self.parse_repeat(None),
                "defer" => {
                    let pos = self.advance()?.pos;
                    let value = self.parse_expression()?;
                    return Ok(Stmt::Defer(DeferStmt { value, pos }));
                }
                "return" => return self.parse_return(),
                "break" => {
                    let pos = self.advance()?.pos;
                    let label = self.parse_label()?;
                    return Ok(Stmt::Break(label, pos));
                }
                "continue" => {
                    let pos = self.advance()?.pos;
                    let label = self.parse_label()?;
                    return Ok(Stmt::Continue(label, pos));
                }
                "throw" => {
                    let pos = self.advance()?.pos;
                    let value = self.parse_expression()?;
                    return Ok(Stmt::Throw(ThrowStmt { value, pos }));
                }
                "try" => return self.parse_try(),
                "use" => return self.parse_use(),
                _ => {}
            }
        }

        self.parse_expr_or_assign()
    }

    /// Whether the code that is here is a call of its own.
    ///
    /// A component may have work of its own before its markup, such as
    /// `provide("theme", theme)`. A name with a big letter is a component
    /// of the markup, so it is not a call of its own.
    fn call_statement_ahead(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident {
            return Ok(false);
        }
        if self
            .tok
            .value
            .chars()
            .next()
            .is_some_and(|first| first.is_uppercase())
        {
            return Ok(false);
        }
        let next = self.peek()?;
        Ok(next.kind == TokKind::Punct && next.value == "(")
    }

    fn parse_declaration(&mut self) -> Result<Stmt, VeyError> {
        if self.is_kw("let") || self.is_kw("const") {
            return self.parse_var(false);
        }
        if self.is_kw("fn") {
            return self.parse_fn(false);
        }
        if self.is_kw("component") {
            return self.parse_component(false);
        }
        if self.is_kw("enum") {
            return self.parse_enum(false);
        }
        if self.type_decl_here()? {
            return self.parse_type_decl(false);
        }
        if self.interface_decl_here()? {
            return self.parse_interface(false);
        }
        if self.impl_decl_here()? {
            return self.parse_impl(false);
        }
        Err(self.error("export must be followed by let, const, fn, component, enum, type or interface"))
    }

    fn parse_var(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.advance()?;
        let kind = if start.value == "let" {
            VarKind::Let
        } else {
            VarKind::Const
        };
        let pattern = self.parse_binding_pattern()?;
        let ty = if self.is_punct(":") {
            self.advance()?;
            Some(self.parse_type()?)
        } else {
            None
        };
        let init = if self.eat_op("=")? {
            Some(self.parse_statement_expression()?)
        } else {
            None
        };
        if kind == VarKind::Const && init.is_none() {
            return Err(self.error_at(start.pos, "const requires a value"));
        }
        if !matches!(pattern, BindingPattern::Name { .. }) && init.is_none() {
            return Err(self.error_at(start.pos, "destructuring requires a value"));
        }
        // A value that has to be there: `let value = find() else { return }`.
        if self.is_kw("else") {
            let Some(value) = init else {
                return Err(self.error_at(start.pos, "a value is needed before 'else'"));
            };
            self.advance()?;
            let otherwise = self.parse_block()?;
            return Ok(Stmt::LetElse(LetElseStmt {
                pattern,
                value,
                otherwise,
                pos: start.pos,
            }));
        }
        Ok(Stmt::Var(VarDecl {
            kind,
            pattern,
            ty,
            init,
            exported,
            pos: start.pos,
        }))
    }

    fn parse_fn(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("fn")?;
        let name = self.expect_ident("function name")?;
        let type_params = self.parse_type_params()?;
        let params = self.parse_params()?;
        // The return type is written after an arrow, not after a colon.
        if self.is_punct(":") {
            return Err(self.error_at(self.tok.pos, "the return type is written after '->'"));
        }
        let ret = if self.is_op("->") {
            self.advance()?;
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Stmt::Fn(FnDecl {
            name,
            type_params,
            params,
            ret,
            body,
            exported,
            pos: start.pos,
        }))
    }

    fn parse_component(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("component")?;
        let name = self.expect_ident("component name")?;
        if !name.chars().next().is_some_and(|c| c.is_uppercase()) {
            return Err(self.error_at(
                start.pos,
                "component names must start with an uppercase letter",
            ));
        }
        let params = self.parse_params()?;
        // A component has no return type.
        if self.is_punct(":") {
            return Err(self.error_at(self.tok.pos, "a component has no return type"));
        }
        let open = self.expect_punct("{")?;
        // A component may have values of its own, which are kept when the
        // page is written again.
        let mut body = Vec::new();
        while self.is_kw("let") || self.is_kw("const") || self.call_statement_ahead()? {
            body.push(self.parse_statement()?);
        }
        let children = self.parse_markup_nodes()?;
        let close = self.expect_punct("}")?;
        Ok(Stmt::Component(ComponentDecl {
            name,
            params,
            body,
            children,
            exported,
            pos: start.pos,
            body_start: open.pos,
            body_end: close.pos,
        }))
    }

    fn parse_enum(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("enum")?;
        let name = self.expect_ident("enum name")?;
        self.expect_punct("{")?;
        let mut variants = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed enum body"));
            }
            let pos = self.tok.pos;
            let variant = self.expect_ident("variant name")?;
            let mut fields = Vec::new();
            if self.is_punct("(") {
                self.advance()?;
                while !self.is_punct(")") {
                    let fpos = self.tok.pos;
                    let fname = self.expect_ident("field name")?;
                    let ty = if self.is_punct(":") {
                        self.advance()?;
                        Some(self.parse_type()?)
                    } else {
                        None
                    };
                    fields.push(EnumField {
                        name: fname,
                        ty,
                        pos: fpos,
                    });
                    if self.is_punct(",") {
                        self.advance()?;
                    } else {
                        break;
                    }
                }
                self.expect_punct(")")?;
            }
            variants.push(EnumVariant {
                name: variant,
                fields,
                pos,
            });
            if self.is_punct(",") {
                self.advance()?;
            } else if !self.is_punct("}") {
                // allow newline-separated variants
                if self.tok.pos.line == pos.line {
                    return Err(self.error_at(pos, "expected ',' between enum variants"));
                }
            }
        }
        self.expect_punct("}")?;
        if variants.is_empty() {
            return Err(self.error_at(start.pos, "enum requires at least one variant"));
        }
        // check duplicate variants
        let mut seen = std::collections::HashSet::new();
        for v in &variants {
            if !seen.insert(v.name.clone()) {
                return Err(self.error_at(v.pos, format!("duplicate variant '{}'", v.name)));
            }
        }
        Ok(Stmt::Enum(EnumDecl {
            name,
            variants,
            exported,
            pos: start.pos,
        }))
    }

    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>, VeyError> {
        if !self.is_op("<") {
            return Ok(Vec::new());
        }
        self.advance()?;
        let mut params: Vec<TypeParam> = Vec::new();
        while !self.is_op(">") {
            let pos = self.tok.pos;
            let name = self.expect_ident("type parameter")?;
            if params.iter().any(|param: &TypeParam| param.name == name) {
                return Err(self.error_at(pos, format!("duplicate type parameter '{}'", name)));
            }
            let constraint = if self.is_punct(":") {
                self.advance()?;
                Some(self.parse_type()?)
            } else {
                None
            };
            params.push(TypeParam {
                name,
                constraint,
                pos,
            });
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        if params.is_empty() {
            return Err(self.error("expected a type parameter"));
        }
        self.expect_angle()?;
        Ok(params)
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, VeyError> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        while !self.is_punct(")") {
            let pos = self.tok.pos;
            let rest = self.eat_op("...")?;
            // A parameter may be a name or a pattern that takes the argument
            // apart, such as `{ name, age }`.
            let pattern = if self.is_punct("{") || self.is_punct("[") {
                self.parse_binding_pattern()?
            } else {
                let name = self.expect_ident("parameter name")?;
                BindingPattern::Name {
                    name,
                    default: None,
                    pos,
                }
            };
            let ty = if self.is_punct(":") {
                self.advance()?;
                Some(self.parse_type()?)
            } else {
                None
            };
            let default = if self.eat_op("=")? {
                Some(self.parse_expression()?)
            } else {
                None
            };
            params.push(Param {
                pattern,
                ty,
                default,
                rest,
                pos,
            });
            if rest {
                break;
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(params)
    }

    fn parse_block(&mut self) -> Result<Block, VeyError> {
        let open = self.expect_punct("{")?;
        let mut body = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed block"));
            }
            body.push(self.parse_statement()?);
        }
        let close = self.expect_punct("}")?;
        Ok(Block {
            body,
            start: open.pos,
            end: close.pos,
        })
    }

    /// The name of a loop, when a `break` or a `continue` names it.
    fn parse_label(&mut self) -> Result<Option<String>, VeyError> {
        // The name of a loop is written on the same line as the `break` or
        // the `continue`. A word on the next line is the start of another
        // statement.
        if self.tok.kind == TokKind::Ident && !self.tok.newline_before {
            return Ok(Some(self.advance()?.value));
        }
        Ok(None)
    }

    /// The name of a loop that is written with a name in front of it.
    fn parse_loop_label(&mut self) -> Result<Option<String>, VeyError> {
        if self.tok.kind != TokKind::Ident {
            return Ok(None);
        }
        let next = self.peek()?;
        if next.kind != TokKind::Punct || next.value != ":" {
            return Ok(None);
        }
        let after = self.peek_n(2)?;
        let is_loop = matches!(after.value.as_str(), "for" | "while" | "repeat")
            && after.kind == TokKind::Keyword;
        if !is_loop {
            return Ok(None);
        }
        let name = self.advance()?.value;
        self.advance()?;
        Ok(Some(name))
    }

    fn parse_if(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("if")?;
        // A value that is taken apart, as in `if let Some(x) = value { }`.
        if self.is_kw("let") {
            self.advance()?;
            let pattern = self.parse_pattern()?;
            self.expect_op("=")?;
            let value = self.parse_expression()?;
            let then_block = self.parse_block()?;
            let else_branch = self.parse_else_branch()?;
            return Ok(Stmt::IfLet(IfLetStmt {
                pattern,
                value,
                then_block,
                else_branch,
                pos: start.pos,
            }));
        }
        let test = self.parse_expression()?;
        let then_block = self.parse_block()?;
        let else_branch = self.parse_else_branch()?;
        Ok(Stmt::If(IfStmt {
            test,
            then_block,
            else_branch,
            pos: start.pos,
        }))
    }

    /// The `else` of an `if`, when there is one.
    fn parse_else_branch(&mut self) -> Result<Option<Box<Stmt>>, VeyError> {
        if !self.eat_kw("else")? {
            return Ok(None);
        }
        if self.is_kw("if") {
            return Ok(Some(Box::new(self.parse_if()?)));
        }
        let pos = self.tok.pos;
        Ok(Some(Box::new(Stmt::Block(self.parse_block()?, pos))))
    }

    fn parse_while(&mut self, label: Option<String>) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("while")?;
        // A value that is taken apart, as in `while let Some(x) = value { }`.
        if self.is_kw("let") {
            self.advance()?;
            let pattern = self.parse_pattern()?;
            self.expect_op("=")?;
            let value = self.parse_expression()?;
            let body = self.parse_block()?;
            return Ok(Stmt::WhileLet(WhileLetStmt {
                pattern,
                value,
                body,
                pos: start.pos,
            }));
        }
        let test = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::While(WhileStmt {
            test,
            body,
            label,
            pos: start.pos,
        }))
    }

    fn parse_for(&mut self, label: Option<String>) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("for")?;
        let pattern = self.parse_binding_pattern()?;
        let ty = if self.is_punct(":") {
            self.advance()?;
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect_kw("in")?;
        let iter = self.parse_expression()?;
        // A loop over a range may have a step, as in `for i in 0..10 by 2`.
        let step = if self.tok.kind == TokKind::Ident && self.tok.value == "by" {
            self.advance()?;
            Some(self.parse_expression()?)
        } else {
            None
        };
        let body = self.parse_block()?;
        Ok(Stmt::For(ForStmt {
            pattern,
            ty,
            iter,
            body,
            label,
            step,
            pos: start.pos,
        }))
    }
    fn parse_repeat(&mut self, label: Option<String>) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("repeat")?;
        let count = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::Repeat(RepeatStmt {
            count,
            body,
            label,
            pos: start.pos,
        }))
    }

    fn parse_return(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("return")?;
        // The value of a `return` is written on the same line. The end of a
        // line ends a statement.
        let value = if self.is_punct("}")
            || self.is_punct(";")
            || self.tok.kind == TokKind::Eof
            || self.tok.newline_before
        {
            None
        } else {
            Some(self.parse_statement_expression()?)
        };
        Ok(Stmt::Return(ReturnStmt {
            value,
            pos: start.pos,
        }))
    }

    fn parse_try(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("try")?;
        let body = self.parse_block()?;

        // The catch and the finally blocks are both optional, but at least one
        // of them has to be there.
        let mut name = None;
        let mut catch = None;
        if self.eat_kw("catch")? {
            if self.tok.kind == TokKind::Ident {
                name = Some(self.advance()?.value);
            }
            catch = Some(self.parse_block()?);
        }
        let finally = if self.eat_kw("finally")? {
            Some(self.parse_block()?)
        } else {
            None
        };
        if catch.is_none() && finally.is_none() {
            return Err(self.error_at(start.pos, "expected a catch or a finally block"));
        }

        Ok(Stmt::Try(TryStmt {
            body,
            name,
            catch,
            finally,
            pos: start.pos,
        }))
    }

    fn parse_use(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("use")?;
        if self.tok.kind != TokKind::Str {
            return Err(self.error("expected a module path string"));
        }
        let token = self.advance()?;
        if token.parts.iter().any(|p| p.expr.is_some()) {
            return Err(self.error_at(start.pos, "module paths cannot interpolate"));
        }
        let path: String = token.parts.iter().map(|p| p.text.as_str()).collect();
        let (alias, alias_pos) = if self.eat_kw("as")? {
            let alias_pos = self.tok.pos;
            (self.expect_ident("module alias")?, Some(alias_pos))
        } else {
            let file_name = path
                .rsplit('/')
                .next()
                .filter(|name| !name.is_empty())
                .unwrap_or("module");
            let stem = file_name
                .rsplit_once('.')
                .map(|(stem, _)| stem)
                .unwrap_or(file_name);
            let mut alias: String = stem
                .chars()
                .map(|c| {
                    if c.is_alphanumeric() || c == '_' || c == '$' {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if !alias
                .chars()
                .next()
                .is_some_and(|c| c.is_alphabetic() || c == '_' || c == '$')
            {
                alias.insert(0, '_');
            }
            (alias, None)
        };
        if !is_ident(&alias) {
            return Err(self.error_at(start.pos, format!("invalid module alias '{}'", alias)));
        }
        Ok(Stmt::Use(UseStmt {
            path,
            alias,
            alias_pos,
            pos: start.pos,
        }))
    }

    fn style_block_here(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident || self.tok.value != "style" {
            return Ok(false);
        }
        let next = self.peek()?;
        Ok(next.kind == TokKind::Punct && next.value == "{")
    }

    fn parse_style_block(&mut self) -> Result<Stmt, VeyError> {
        let start = self.tok.clone();
        let (css, _) = self.lexer.raw_brace_block()?;
        self.tok = self.lexer.next_token()?;
        Ok(Stmt::Style(StyleDecl {
            css,
            pos: start.pos,
        }))
    }

    fn type_decl_here(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident || self.tok.value != "type" {
            return Ok(false);
        }
        let name = self.peek_n(1)?;
        let next = self.peek_n(2)?;
        Ok(name.kind == TokKind::Ident
            && next.kind == TokKind::Op
            && (next.value == "=" || next.value == "<"))
    }

    /// Whether an interface is written here.
    fn interface_decl_here(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident || self.tok.value != "interface" {
            return Ok(false);
        }
        let name = self.peek_n(1)?;
        Ok(name.kind == TokKind::Ident)
    }

    /// Whether an `impl` block is written here.
    fn impl_decl_here(&mut self) -> Result<bool, VeyError> {
        if self.tok.kind != TokKind::Ident || self.tok.value != "impl" {
            return Ok(false);
        }
        let name = self.peek_n(1)?;
        Ok(name.kind == TokKind::Ident)
    }

    fn parse_impl(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.advance()?;
        let type_name = self.expect_ident("type name")?;
        self.expect_punct("{")?;
        let mut methods = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed impl block"));
            }
            let pos = self.tok.pos;
            self.expect_kw("fn")?;
            let name = self.expect_ident("method name")?;
            let params = self.parse_params()?;
            let ret = if self.is_op("->") {
                self.advance()?;
                Some(self.parse_type()?)
            } else {
                None
            };
            let body = self.parse_block()?;
            methods.push(ImplMethod {
                name,
                params,
                ret,
                body,
                pos,
            });
        }
        self.expect_punct("}")?;
        Ok(Stmt::Impl(ImplDecl {
            type_name,
            methods,
            exported,
            pos: start.pos,
        }))
    }

    fn parse_interface(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.advance()?;
        let name = self.expect_ident("interface name")?;
        let type_params = self.parse_type_params()?;
        self.expect_punct("{")?;
        let mut members = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed interface body"));
            }
            let pos = self.tok.pos;
            let member_name = self.expect_ident("member name")?;
            // A member with parentheses is a method.
            if self.is_punct("(") {
                let params = self.parse_params()?;
                let ret = if self.is_op("->") {
                    self.advance()?;
                    self.parse_type()?
                } else {
                    return Err(self.error_at(pos, "a method needs a return type"));
                };                members.push(InterfaceMember::Method(InterfaceMethod {
                    name: member_name,
                    params,
                    ret,
                    pos,
                }));
            } else {
                let optional = self.eat_op("?")?;
                self.expect_punct(":")?;
                let ty = self.parse_type()?;
                members.push(InterfaceMember::Field(InterfaceField {
                    name: member_name,
                    ty,
                    optional,
                    pos,
                }));
            }
            if self.is_punct(",") {
                self.advance()?;
            }
        }
        self.expect_punct("}")?;
        Ok(Stmt::Interface(InterfaceDecl {
            name,
            type_params,
            members,
            exported,
            pos: start.pos,
        }))
    }

    fn parse_type_decl(&mut self, exported: bool) -> Result<Stmt, VeyError> {
        let start = self.advance()?;
        let name = self.expect_ident("type name")?;
        let type_params = self.parse_type_params()?;
        self.expect_op("=")?;
        let ty = self.parse_type()?;
        Ok(Stmt::Type(TypeDecl {
            name,
            type_params,
            ty,
            exported,
            pos: start.pos,
        }))
    }
    fn parse_expr_or_assign(&mut self) -> Result<Stmt, VeyError> {
        let expr = self.parse_statement_expression()?;
        let pos = expr.pos();
        if self.tok.kind == TokKind::Op && ASSIGN_OPS.contains(&self.tok.value.as_str()) {
            if !matches!(expr, Expr::Ident(..) | Expr::Member(..)) {
                return Err(self.error_at(pos, "invalid assignment target"));
            }
            let op = self.advance()?.value;
            let value = self.parse_expression()?;
            return Ok(Stmt::Assign(AssignStmt {
                target: expr,
                op,
                value,
                pos,
            }));
        }
        Ok(Stmt::Expr(ExprStmt { expr, pos }))
    }

    pub fn parse_expression(&mut self) -> Result<Expr, VeyError> {
        // An expression that is not the whole of a statement may go on for
        // several lines.
        let outer = self.statement_expr;
        self.statement_expr = false;
        let result = self.parse_expression_inner();
        self.statement_expr = outer;
        result
    }

    /// The expression of a statement.
    ///
    /// The end of a line ends a statement, so the expression of a statement
    /// ends with the line it is written on.
    pub(crate) fn parse_statement_expression(&mut self) -> Result<Expr, VeyError> {
        self.statement_expr = true;
        let result = self.parse_expression_inner();
        self.statement_expr = false;
        result
    }

    fn parse_expression_inner(&mut self) -> Result<Expr, VeyError> {
        self.enter()?;
        let result = self.parse_pipe();
        self.leave();
        result
    }

    /// Whether the end of the line ends the statement.
    ///
    /// The end of a line ends a statement, so that a line that begins with
    /// a mark is a new statement and not the rest of the one before it.
    /// A line that begins with a dot goes on, because that is a chain of
    /// members and there is no other way to write it.
    pub(crate) fn line_ends_statement(&self) -> bool {
        if !self.statement_expr {
            return false;
        }
        if !self.tok.newline_before {
            return false;
        }
        !matches!(self.tok.value.as_str(), "." | "?." | "|>")
    }
}

fn mark_exported(stmt: &mut Stmt) {
    match stmt {
        Stmt::Var(v) => v.exported = true,
        Stmt::Fn(f) => f.exported = true,
        Stmt::Component(c) => c.exported = true,
        Stmt::Type(t) => t.exported = true,
        Stmt::Enum(e) => e.exported = true,
        Stmt::Interface(i) => i.exported = true,
        Stmt::Impl(i) => i.exported = true,
        _ => {}
    }
}

pub fn is_ident(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_alphabetic() || c == '_' || c == '$' => {}
        _ => return false,
    }
    chars.all(|c| c.is_alphanumeric() || c == '_' || c == '$')
}
