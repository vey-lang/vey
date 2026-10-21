//! The recursive descent parser.
//!
//! The grammar is split over a few modules: `types` reads type expressions,
//! `patterns` reads binding patterns and `markup` reads the tag syntax.

mod html;
mod markup;
mod patterns;
mod types;

use crate::ast::*;
use crate::lexer::{Lexer, Pos, TokKind, Token};
use crate::VeyError;

const ASSIGN_OPS: &[&str] = &["=", "+=", "-=", "*=", "/=", "%="];

#[derive(Clone)]
pub struct Parser {
    lexer: Lexer,
    file: String,
    tok: Token,
}

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

    pub fn parse_program(&mut self) -> Result<Program, VeyError> {
        let mut body = Vec::new();
        while self.tok.kind != TokKind::Eof {
            body.push(self.parse_statement()?);
        }
        Ok(Program { body })
    }

    fn parse_statement(&mut self) -> Result<Stmt, VeyError> {
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

        if self.style_block_here()? {
            return self.parse_style_block();
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
                "while" => return self.parse_while(),
                "for" => return self.parse_for(),
                "repeat" => return self.parse_repeat(),
                "return" => return self.parse_return(),
                "break" => {
                    let pos = self.advance()?.pos;
                    return Ok(Stmt::Break(pos));
                }
                "continue" => {
                    let pos = self.advance()?.pos;
                    return Ok(Stmt::Continue(pos));
                }
                "try" => return self.parse_try(),
                "use" => return self.parse_use(),
                _ => {}
            }
        }

        self.parse_expr_or_assign()
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
        Err(self.error("export must be followed by let, const, fn, component, enum or type"))
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
            Some(self.parse_expression()?)
        } else {
            None
        };
        if kind == VarKind::Const && init.is_none() {
            return Err(self.error_at(start.pos, "const requires a value"));
        }
        if !matches!(pattern, BindingPattern::Name { .. }) && init.is_none() {
            return Err(self.error_at(start.pos, "destructuring requires a value"));
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
        self.expect_punct("{")?;
        let children = self.parse_markup_nodes()?;
        self.expect_punct("}")?;
        Ok(Stmt::Component(ComponentDecl {
            name,
            params,
            children,
            exported,
            pos: start.pos,
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
        self.expect_op(">")?;
        Ok(params)
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, VeyError> {
        self.expect_punct("(")?;
        let mut params = Vec::new();
        while !self.is_punct(")") {
            let pos = self.tok.pos;
            let rest = self.eat_op("...")?;
            let name = self.expect_ident("parameter name")?;
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
                name,
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
        self.expect_punct("{")?;
        let mut body = Vec::new();
        while !self.is_punct("}") {
            if self.tok.kind == TokKind::Eof {
                return Err(self.error("unclosed block"));
            }
            body.push(self.parse_statement()?);
        }
        self.expect_punct("}")?;
        Ok(Block { body })
    }

    fn parse_if(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("if")?;
        let test = self.parse_expression()?;
        let then_block = self.parse_block()?;
        let else_branch = if self.eat_kw("else")? {
            if self.is_kw("if") {
                Some(Box::new(self.parse_if()?))
            } else {
                let pos = self.tok.pos;
                Some(Box::new(Stmt::Block(self.parse_block()?, pos)))
            }
        } else {
            None
        };
        Ok(Stmt::If(IfStmt {
            test,
            then_block,
            else_branch,
            pos: start.pos,
        }))
    }

    fn parse_while(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("while")?;
        let test = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::While(WhileStmt {
            test,
            body,
            pos: start.pos,
        }))
    }

    fn parse_for(&mut self) -> Result<Stmt, VeyError> {
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
        let body = self.parse_block()?;
        Ok(Stmt::For(ForStmt {
            pattern,
            ty,
            iter,
            body,
            pos: start.pos,
        }))
    }
    fn parse_repeat(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("repeat")?;
        let count = self.parse_expression()?;
        let body = self.parse_block()?;
        Ok(Stmt::Repeat(RepeatStmt {
            count,
            body,
            pos: start.pos,
        }))
    }

    fn parse_return(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("return")?;
        let value = if self.is_punct("}") || self.tok.kind == TokKind::Eof {
            None
        } else {
            Some(self.parse_expression()?)
        };
        Ok(Stmt::Return(ReturnStmt {
            value,
            pos: start.pos,
        }))
    }

    fn parse_try(&mut self) -> Result<Stmt, VeyError> {
        let start = self.expect_kw("try")?;
        let body = self.parse_block()?;
        self.expect_kw("catch")?;
        let name = if self.tok.kind == TokKind::Ident {
            Some(self.advance()?.value)
        } else {
            None
        };
        let catch = self.parse_block()?;
        Ok(Stmt::Try(TryStmt {
            body,
            name,
            catch,
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
        let alias = if self.eat_kw("as")? {
            self.expect_ident("module alias")?
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
            alias
        };
        if !is_ident(&alias) {
            return Err(self.error_at(start.pos, format!("invalid module alias '{}'", alias)));
        }
        Ok(Stmt::Use(UseStmt {
            path,
            alias,
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
        let expr = self.parse_expression()?;
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
        self.parse_pipe()
    }

    fn parse_pipe(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_ternary()?;
        while self.is_op("|>") {
            self.advance()?;
            let right = self.parse_ternary()?;
            let pos = right.pos();
            left = match right {
                Expr::Call(callee, mut args, optional, p) => {
                    args.insert(0, left);
                    Expr::Call(callee, args, optional, p)
                }
                other => Expr::Call(Box::new(other), vec![left], false, pos),
            };
        }
        Ok(left)
    }

    fn parse_ternary(&mut self) -> Result<Expr, VeyError> {
        let test = self.parse_nullish()?;
        if self.is_op("?") {
            let pos = self.tok.pos;
            self.advance()?;
            let cons = self.parse_expression()?;
            self.expect_punct(":")?;
            let alt = self.parse_expression()?;
            return Ok(Expr::Ternary(
                Box::new(test),
                Box::new(cons),
                Box::new(alt),
                pos,
            ));
        }
        Ok(test)
    }

    fn parse_nullish(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_or()?;
        while self.is_op("??") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_or()?;
            left = Expr::Nullish(Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    fn parse_or(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_and()?;
        while self.is_kw("or") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_and()?;
            left = Expr::Logical("||".into(), Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_equality()?;
        while self.is_kw("and") {
            let pos = self.tok.pos;
            self.advance()?;
            let right = self.parse_equality()?;
            left = Expr::Logical("&&".into(), Box::new(left), Box::new(right), pos);
        }
        Ok(left)
    }

    fn parse_equality(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_comparison()?;
        while self.is_op("==") || self.is_op("!=") {
            let op_tok = self.advance()?;
            let op = if op_tok.value == "==" { "===" } else { "!==" };
            let right = self.parse_comparison()?;
            left = Expr::Binary(op.into(), Box::new(left), Box::new(right), op_tok.pos);
        }
        Ok(left)
    }

    fn parse_comparison(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_range()?;
        while self.is_op("<") || self.is_op("<=") || self.is_op(">") || self.is_op(">=") {
            let op_tok = self.advance()?;
            let right = self.parse_range()?;
            left = Expr::Binary(op_tok.value, Box::new(left), Box::new(right), op_tok.pos);
        }
        Ok(left)
    }

    fn parse_range(&mut self) -> Result<Expr, VeyError> {
        let start = self.parse_additive()?;
        if self.is_op("..") {
            let pos = self.advance()?.pos;
            let end = self.parse_additive()?;
            return Ok(Expr::Range(Box::new(start), Box::new(end), pos));
        }
        Ok(start)
    }

    fn parse_additive(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_multiplicative()?;
        while self.is_op("+") || self.is_op("-") {
            let op = self.advance()?;
            let right = self.parse_multiplicative()?;
            left = Expr::Binary(op.value, Box::new(left), Box::new(right), op.pos);
        }
        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, VeyError> {
        let mut left = self.parse_power()?;
        while self.is_op("*") || self.is_op("/") || self.is_op("%") {
            let op = self.advance()?;
            let right = self.parse_power()?;
            left = Expr::Binary(op.value, Box::new(left), Box::new(right), op.pos);
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<Expr, VeyError> {
        let left = self.parse_unary()?;
        if self.is_op("**") {
            let pos = self.advance()?.pos;
            let right = self.parse_power()?;
            return Ok(Expr::Binary(
                "**".into(),
                Box::new(left),
                Box::new(right),
                pos,
            ));
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, VeyError> {
        if self.is_op("-") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Unary("-".into(), Box::new(self.parse_unary()?), pos));
        }
        if self.is_kw("not") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Unary("!".into(), Box::new(self.parse_unary()?), pos));
        }
        if self.is_kw("await") {
            let pos = self.advance()?.pos;
            return Ok(Expr::Await(Box::new(self.parse_unary()?), pos));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Expr, VeyError> {
        let mut expr = self.parse_primary()?;
        loop {
            if self.is_punct(".") {
                let pos = self.advance()?.pos;
                let name = self.expect_ident("property name")?;
                expr = Expr::Member(Box::new(expr), MemberProp::Name(name), false, pos);
            } else if self.is_op("?.") {
                let pos = self.advance()?.pos;
                if self.is_punct("(") {
                    let args = self.parse_args()?;
                    expr = Expr::Call(Box::new(expr), args, true, pos);
                } else if self.is_punct("[") {
                    self.advance()?;
                    let prop = self.parse_expression()?;
                    self.expect_punct("]")?;
                    expr = Expr::Member(
                        Box::new(expr),
                        MemberProp::Computed(Box::new(prop)),
                        true,
                        pos,
                    );
                } else {
                    let name = self.expect_ident("property name")?;
                    expr = Expr::Member(Box::new(expr), MemberProp::Name(name), true, pos);
                }
            } else if self.is_punct("[") {
                let pos = self.advance()?.pos;
                let prop = self.parse_expression()?;
                self.expect_punct("]")?;
                expr = Expr::Member(
                    Box::new(expr),
                    MemberProp::Computed(Box::new(prop)),
                    false,
                    pos,
                );
            } else if self.is_punct("(") {
                let pos = self.tok.pos;
                let args = self.parse_args()?;
                expr = Expr::Call(Box::new(expr), args, false, pos);
            } else if self.is_op("<") {
                // The call may name its type arguments, as in
                // `parse<number>(text)`. When the text after the `<` is not a
                // list of types it is a comparison instead.
                let saved = self.clone();
                match self.try_parse_type_arguments() {
                    Ok(Some(type_args)) if self.is_punct("(") => {
                        let pos = self.tok.pos;
                        let args = self.parse_args()?;
                        expr = Expr::CallGeneric(Box::new(expr), type_args, args, false, pos);
                    }
                    _ => {
                        *self = saved;
                        break;
                    }
                }
            } else {
                break;
            }
        }
        Ok(expr)
    }

    /// Read the type arguments of a call, if the text is a list of types.
    fn try_parse_type_arguments(&mut self) -> Result<Option<Vec<TypeExpr>>, VeyError> {
        if !self.is_op("<") {
            return Ok(None);
        }
        self.advance()?;
        let mut args = Vec::new();
        while !self.is_op(">") {
            if self.tok.kind == TokKind::Eof {
                return Ok(None);
            }
            match self.parse_type() {
                Ok(ty) => args.push(ty),
                Err(_) => return Ok(None),
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        if !self.is_op(">") {
            return Ok(None);
        }
        self.advance()?;
        Ok(Some(args))
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, VeyError> {
        self.expect_punct("(")?;
        let mut args = Vec::new();
        while !self.is_punct(")") {
            if self.is_op("...") {
                let pos = self.advance()?.pos;
                let value = self.parse_expression()?;
                args.push(Expr::Spread(Box::new(value), pos));
            } else {
                args.push(self.parse_expression()?);
            }
            if self.is_op("=") {
                return Err(self
                    .error("named arguments are only supported in markup")
                    .with_hint(
                        "use `Card title=\"value\"` inside markup, or pass the value in order",
                    ));
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    fn parse_primary(&mut self) -> Result<Expr, VeyError> {
        let token = self.tok.clone();

        match token.kind {
            TokKind::Number => {
                self.advance()?;
                return Ok(Expr::Number(token.number, token.pos));
            }
            TokKind::Str => {
                self.advance()?;
                return self.parse_string_token(&token);
            }
            TokKind::Ident => {
                let next = self.peek()?;
                if next.kind == TokKind::Op && next.value == "=>" {
                    self.advance()?;
                    self.advance()?;
                    let body = self.parse_arrow_body()?;
                    return Ok(Expr::Arrow(
                        vec![Param {
                            name: token.value,
                            ty: None,
                            default: None,
                            rest: false,
                            pos: token.pos,
                        }],
                        body,
                        token.pos,
                    ));
                }
                self.advance()?;
                return Ok(Expr::Ident(token.value, token.pos));
            }
            TokKind::Punct if token.value == "(" => return self.parse_group_or_arrow(),
            TokKind::Punct if token.value == "[" => return self.parse_array(),
            TokKind::Punct if token.value == "{" => return self.parse_object(),
            TokKind::Keyword if token.value == "fn" => {
                self.advance()?;
                let params = self.parse_params()?;
                let ret = if self.is_op("->") {
                    self.advance()?;
                    Some(self.parse_type()?)
                } else {
                    None
                };
                let body = self.parse_block()?;
                return Ok(Expr::Fn(params, body, ret, token.pos));
            }
            TokKind::Keyword if token.value == "match" => return self.parse_match(),
            TokKind::Keyword if token.value == "true" => {
                self.advance()?;
                return Ok(Expr::Bool(true, token.pos));
            }
            TokKind::Keyword if token.value == "false" => {
                self.advance()?;
                return Ok(Expr::Bool(false, token.pos));
            }
            TokKind::Keyword if token.value == "null" => {
                self.advance()?;
                return Ok(Expr::Null(token.pos));
            }
            TokKind::Keyword if token.value == "async" => {
                return Err(self
                    .reserved_word_error("an expression")
                    .with_hint("a function that uses `await` is already async"));
            }
            _ => {}
        }

        Err(self.error(format!("unexpected token '{}'", token.value)))
    }

    fn try_parse_typed_arrow(&mut self) -> Result<Option<Expr>, VeyError> {
        let pos = self.tok.pos;
        let saved = self.clone();
        let mut committed = false;
        let result: Result<Option<Expr>, VeyError> = (|| {
            self.expect_punct("(")?;
            let mut params = Vec::new();
            while !self.is_punct(")") {
                let param_pos = self.tok.pos;
                let rest = self.eat_op("...")?;
                let name = self.expect_ident("parameter name")?;
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
                    name,
                    ty,
                    default,
                    rest,
                    pos: param_pos,
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
            if !self.is_op("=>") {
                return Ok(None);
            }
            self.advance()?;
            committed = true;
            let body = self.parse_arrow_body()?;
            Ok(Some(Expr::Arrow(params, body, pos)))
        })();

        match result {
            Ok(Some(expr)) => Ok(Some(expr)),
            Err(err) if committed => Err(err),
            _ => {
                *self = saved;
                Ok(None)
            }
        }
    }

    fn parse_group_or_arrow(&mut self) -> Result<Expr, VeyError> {
        if let Some(expr) = self.try_parse_typed_arrow()? {
            return Ok(expr);
        }

        let pos = self.advance()?.pos;
        if self.is_punct(")") {
            self.advance()?;
            self.expect_op("=>")?;
            let body = self.parse_arrow_body()?;
            return Ok(Expr::Arrow(Vec::new(), body, pos));
        }

        let mut exprs = vec![self.parse_expression()?];
        while self.is_punct(",") {
            self.advance()?;
            exprs.push(self.parse_expression()?);
        }
        self.expect_punct(")")?;

        if self.is_op("=>") {
            self.advance()?;
            let mut params = Vec::new();
            for expr in exprs {
                match expr {
                    Expr::Ident(name, p) => params.push(Param {
                        name,
                        ty: None,
                        default: None,
                        rest: false,
                        pos: p,
                    }),
                    _ => return Err(self.error_at(pos, "arrow parameters must be identifiers")),
                }
            }
            let body = self.parse_arrow_body()?;
            return Ok(Expr::Arrow(params, body, pos));
        }

        if exprs.len() > 1 {
            return Err(self.error_at(pos, "comma expressions are not supported"));
        }
        Ok(exprs.remove(0))
    }

    fn parse_arrow_body(&mut self) -> Result<ArrowBody, VeyError> {
        if self.is_punct("{") {
            return Ok(ArrowBody::Block(self.parse_block()?));
        }
        Ok(ArrowBody::Expr(Box::new(self.parse_expression()?)))
    }

    fn parse_array(&mut self) -> Result<Expr, VeyError> {
        let pos = self.advance()?.pos;
        let mut elements = Vec::new();
        while !self.is_punct("]") {
            if self.is_op("...") {
                let spread_pos = self.advance()?.pos;
                let value = self.parse_expression()?;
                elements.push(Expr::Spread(Box::new(value), spread_pos));
            } else {
                elements.push(self.parse_expression()?);
            }
            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct("]")?;
        Ok(Expr::Array(elements, pos))
    }

    fn parse_object(&mut self) -> Result<Expr, VeyError> {
        let pos = self.advance()?.pos;
        let mut props = Vec::new();
        while !self.is_punct("}") {
            if self.is_op("...") {
                self.advance()?;
                props.push(ObjectProp::Spread(self.parse_expression()?));
            } else {
                let key_tok = self.tok.clone();
                let (key, shorthand) = match key_tok.kind {
                    TokKind::Ident | TokKind::Keyword => {
                        self.advance()?;
                        (key_tok.value, true)
                    }
                    TokKind::Str => {
                        self.advance()?;
                        if key_tok.parts.iter().any(|p| p.expr.is_some()) {
                            return Err(
                                self.error_at(key_tok.pos, "object keys cannot interpolate")
                            );
                        }
                        let text: String = key_tok.parts.iter().map(|p| p.text.as_str()).collect();
                        (text, false)
                    }
                    TokKind::Number => {
                        self.advance()?;
                        (key_tok.value, false)
                    }
                    _ => return Err(self.error("expected an object key")),
                };

                let value = if self.is_punct(":") {
                    self.advance()?;
                    self.parse_expression()?
                } else if shorthand {
                    Expr::Ident(key.clone(), key_tok.pos)
                } else {
                    return Err(self.error("expected ':' after object key"));
                };
                props.push(ObjectProp::Field(key, value));
            }

            if self.is_punct(",") {
                self.advance()?;
            } else {
                break;
            }
        }
        self.expect_punct("}")?;
        Ok(Expr::Object(props, pos))
    }

    fn parse_match(&mut self) -> Result<Expr, VeyError> {
        let pos = self.expect_kw("match")?.pos;
        let subject = self.parse_expression()?;
        self.expect_punct("{")?;
        let mut arms = Vec::new();
        while !self.is_punct("}") {
            let mut patterns = vec![self.parse_pattern()?];
            while self.is_op("|") {
                self.advance()?;
                patterns.push(self.parse_pattern()?);
            }
            self.expect_op("=>")?;
            let body = self.parse_expression()?;
            arms.push(MatchArm { patterns, body });
            if self.is_punct(",") {
                self.advance()?;
            }
        }
        self.expect_punct("}")?;
        Ok(Expr::Match(Box::new(subject), arms, pos))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, VeyError> {
        if self.tok.kind == TokKind::Ident && self.tok.value == "_" {
            self.advance()?;
            return Ok(Pattern::Wildcard);
        }

        if self.tok.kind == TokKind::Ident {
            let next = self.peek()?;
            if next.kind == TokKind::Punct && next.value == "." {
                let pos = self.tok.pos;
                let enum_name = self.advance()?.value;
                self.advance()?; // "."
                let variant = self.expect_ident("variant name")?;
                let mut bindings = Vec::new();
                if self.is_punct("(") {
                    self.advance()?;
                    while !self.is_punct(")") {
                        let bpos = self.tok.pos;
                        let first = self.expect_ident("binding name")?;
                        if self.is_punct(":") {
                            self.advance()?;
                            let second = self.expect_ident("binding name")?;
                            bindings.push(EnumBinding {
                                field: Some(first),
                                name: second,
                                pos: bpos,
                            });
                        } else {
                            bindings.push(EnumBinding {
                                field: None,
                                name: first,
                                pos: bpos,
                            });
                        }
                        if self.is_punct(",") {
                            self.advance()?;
                        } else {
                            break;
                        }
                    }
                    self.expect_punct(")")?;
                }
                return Ok(Pattern::Enum {
                    enum_name,
                    variant,
                    bindings,
                    pos,
                });
            }
        }

        let negative = self.eat_op("-")?;
        let token = self.tok.clone();

        if token.kind == TokKind::Number {
            self.advance()?;
            let start = Expr::Number(
                if negative {
                    -token.number
                } else {
                    token.number
                },
                token.pos,
            );
            if self.is_op("..") {
                self.advance()?;
                let end_neg = self.eat_op("-")?;
                let end_tok = self.tok.clone();
                if end_tok.kind != TokKind::Number {
                    return Err(self.error("expected a number after '..'"));
                }
                self.advance()?;
                let end = Expr::Number(
                    if end_neg {
                        -end_tok.number
                    } else {
                        end_tok.number
                    },
                    end_tok.pos,
                );
                return Ok(Pattern::Range(start, end));
            }
            return Ok(Pattern::Literal(start));
        }

        if negative {
            return Err(self.error("expected a number after '-'"));
        }

        match token.kind {
            TokKind::Str => {
                self.advance()?;
                Ok(Pattern::Literal(self.parse_string_token(&token)?))
            }
            TokKind::Keyword if token.value == "true" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Bool(true, token.pos)))
            }
            TokKind::Keyword if token.value == "false" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Bool(false, token.pos)))
            }
            TokKind::Keyword if token.value == "null" => {
                self.advance()?;
                Ok(Pattern::Literal(Expr::Null(token.pos)))
            }
            _ => Err(self.error("expected a literal, range or _ in match pattern")),
        }
    }

    fn parse_string_token(&self, token: &Token) -> Result<Expr, VeyError> {
        let mut parts = Vec::new();
        for part in &token.parts {
            if let Some((source, pos)) = &part.expr {
                match parse_interpolation(source, &self.file, *pos)? {
                    Some(expr) => parts.push(StrPart {
                        text: String::new(),
                        expr: Some(expr),
                    }),
                    // The braces hold text rather than an expression, for
                    // example a JSON object, so they stay literal.
                    None => parts.push(StrPart {
                        text: format!("{{{}}}", crate::lexer::process_escapes(source)),
                        expr: None,
                    }),
                }
            } else {
                parts.push(StrPart {
                    text: part.text.clone(),
                    expr: None,
                });
            }
        }
        Ok(Expr::String(parts, token.pos))
    }
}

fn parse_interpolation(source: &str, file: &str, pos: Pos) -> Result<Option<Expr>, VeyError> {
    if source.trim().is_empty() {
        // "{}" is plain text, not an interpolation.
        return Ok(None);
    }
    let mut sub = match Parser::with_pos(source, file, pos.line, pos.col) {
        Ok(parser) => parser,
        // The text is not Vey source at all, for example a JSON object.
        Err(_) => return Ok(None),
    };
    let expr = sub.parse_expression()?;
    if sub.tok.kind != TokKind::Eof {
        return Ok(None);
    }
    Ok(Some(expr))
}

fn mark_exported(stmt: &mut Stmt) {
    match stmt {
        Stmt::Var(v) => v.exported = true,
        Stmt::Fn(f) => f.exported = true,
        Stmt::Component(c) => c.exported = true,
        Stmt::Type(t) => t.exported = true,
        Stmt::Enum(e) => e.exported = true,
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
