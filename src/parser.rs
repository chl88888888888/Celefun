// Celefun 语法分析器（递归下降）

use crate::ast::*;
use crate::lexer::{Tok, Token};

#[derive(Debug, Clone)]
pub struct ParseError {
    pub msg: String,
    pub line: u32,
    pub col: u32,
}

pub fn parse(toks: &[Token]) -> Result<Program, ParseError> {
    let mut p = Parser { toks, i: 0 };
    p.parse_program()
}

struct Parser<'a> {
    toks: &'a [Token],
    i: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> &Token {
        &self.toks[self.i.min(self.toks.len() - 1)]
    }
    fn peek_n(&self, n: usize) -> &Token {
        &self.toks[(self.i + n).min(self.toks.len() - 1)]
    }
    fn bump(&mut self) {
        if self.i < self.toks.len() - 1 {
            self.i += 1;
        }
    }
    fn at_eof(&self) -> bool {
        matches!(self.peek().tok, Tok::Eof)
    }
    fn check(&self, t: &Tok) -> bool {
        &self.peek().tok == t
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.check(t) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn err(&self, msg: impl Into<String>) -> ParseError {
        let t = self.peek();
        ParseError { msg: msg.into(), line: t.line, col: t.col }
    }
    fn expect(&mut self, t: &Tok, what: &str) -> Result<(), ParseError> {
        if self.eat(t) {
            Ok(())
        } else {
            Err(self.err(format!("这里需要 {}", what)))
        }
    }
    fn expect_ident(&mut self) -> Result<(String, u32), ParseError> {
        let t = self.peek().clone();
        if let Tok::Ident(name) = t.tok {
            self.bump();
            Ok((name, t.line))
        } else {
            Err(self.err("这里需要一个名字"))
        }
    }

    fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut prog = Program::default();
        loop {
            if self.at_eof() {
                break;
            }
            let t = self.peek().clone();
            match t.tok {
                Tok::Determ => prog.main = Some(self.parse_main()?),
                Tok::Ident(name) => {
                    if matches!(self.peek_n(1).tok, Tok::LBrace) {
                        prog.chapters.push(self.parse_chapter()?);
                    } else if matches!(self.peek_n(1).tok, Tok::Semi) {
                        self.bump();
                        self.bump();
                        prog.imports.push(name);
                    } else {
                        return Err(self.err("顶层只允许：章定义 名字→↘…↘→，导入 名字;，或 决心"));
                    }
                }
                _ => return Err(self.err("顶层只允许：章定义、导入、决心")),
            }
        }
        Ok(prog)
    }

    fn parse_main(&mut self) -> Result<Vec<Stmt>, ParseError> {
        self.expect(&Tok::Determ, "决心")?;
        self.expect(&Tok::LBrace, "→↘")?;
        self.parse_stmts()
    }

    fn parse_stmts(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut out = Vec::new();
        loop {
            if self.eat(&Tok::RBrace) {
                return Ok(out);
            }
            if self.at_eof() {
                return Err(self.err("少了 ↘→"));
            }
            out.push(self.parse_stmt()?);
        }
    }

    fn parse_chapter(&mut self) -> Result<Chapter, ParseError> {
        let (name, line) = self.expect_ident()?;
        self.expect(&Tok::LBrace, "→↘")?;
        let mut funcs = Vec::new();
        loop {
            if self.eat(&Tok::RBrace) {
                break;
            }
            if self.at_eof() {
                return Err(self.err("章的 ↘→ 不见了"));
            }
            funcs.push(self.parse_func()?);
        }
        Ok(Chapter { name, funcs, line })
    }

    fn parse_func(&mut self) -> Result<Func, ParseError> {
        let (name, line) = self.expect_ident()?;
        self.expect(&Tok::Colon, ":")?;
        let ret = self.parse_type()?;
        self.expect(&Tok::LBrace, "→↘")?;
        let mut params = Vec::new();
        let mut ffi = false;
        if !self.eat(&Tok::RBrace) {
            loop {
                if self.eat(&Tok::Pico8) {
                    ffi = true;
                } else {
                    let (pn, _) = self.expect_ident()?;
                    self.expect(&Tok::Colon, ":")?;
                    let pt = self.parse_type()?;
                    params.push((pn, pt));
                }
                if self.eat(&Tok::Sep) {
                    continue;
                }
                self.expect(&Tok::RBrace, "↘→")?;
                break;
            }
        }
        self.expect(&Tok::LBrace, "→↘")?;
        let body = self.parse_stmts()?;
        Ok(Func { name, ret, params, body, line, ffi })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let t = self.peek().clone();
        match t.tok {
            Tok::TInt => {
                self.bump();
                Ok(Type::Int)
            }
            Tok::TFloat => {
                self.bump();
                Ok(Type::Float)
            }
            Tok::TBool => {
                self.bump();
                Ok(Type::Bool)
            }
            Tok::TJelly => {
                self.bump();
                self.expect(&Tok::LBrace, "→↘")?;
                let inner = self.parse_type()?;
                self.expect(&Tok::RBrace, "↘→")?;
                Ok(Type::Jelly(Box::new(inner)))
            }
            _ => Err(self.err("这里需要一个类型：草莓 / 金草莓 / 磁带 / 果冻块→↘…↘→")),
        }
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        let t = self.peek().clone();
        match t.tok {
            Tok::Declare => self.parse_decl(),
            Tok::Door => self.parse_if(),
            Tok::Loop => self.parse_loop(),
            Tok::Brk => {
                self.bump();
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Break { line: t.line })
            }
            Tok::Cont => {
                self.bump();
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Continue { line: t.line })
            }
            Tok::Goto => {
                self.bump();
                let (label, _) = self.expect_ident()?;
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Goto { label, line: t.line })
            }
            Tok::Label => {
                self.bump();
                let (name, _) = self.expect_ident()?;
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Label { name, line: t.line })
            }
            Tok::Exit => {
                self.bump();
                let v = self.parse_opt_expr()?;
                Ok(Stmt::Exit { value: v, line: t.line })
            }
            Tok::Ret => {
                self.bump();
                let v = self.parse_opt_expr()?;
                Ok(Stmt::Return { value: v, line: t.line })
            }
            Tok::Panic => {
                self.bump();
                let v = if self.eat(&Tok::LBrace) {
                    let e = self.parse_expr()?;
                    self.expect(&Tok::RBrace, "↘→")?;
                    Some(e)
                } else {
                    None
                };
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Panic { value: v, line: t.line })
            }
            Tok::Ident(name) => self.parse_ident_stmt(name, t.line),
            Tok::Inc | Tok::Dec => {
                let e = self.parse_expr()?;
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::ExprStmt { expr: e, line: t.line })
            }
            _ => Err(self.err("这不是一个合法语句的开头")),
        }
    }

    fn parse_opt_expr(&mut self) -> Result<Option<Expr>, ParseError> {
        if self.check(&Tok::Semi) {
            self.bump();
            Ok(None)
        } else {
            let e = self.parse_expr()?;
            self.expect(&Tok::Semi, ";")?;
            Ok(Some(e))
        }
    }

    fn parse_decl(&mut self) -> Result<Stmt, ParseError> {
        let line = self.peek().line;
        self.expect(&Tok::Declare, "旗帜")?;
        let (name, _) = self.expect_ident()?;
        self.expect(&Tok::Colon, ":")?;
        let ty = self.parse_type()?;
        let init = if self.eat(&Tok::Assign) { Some(self.parse_expr()?) } else { None };
        self.expect(&Tok::Semi, ";")?;
        Ok(Stmt::Decl { name, ty, init, line })
    }

    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let line = self.peek().line;
        self.expect(&Tok::Door, "心门")?;
        self.expect(&Tok::LBrace, "→↘")?;
        let cond = self.parse_expr()?;
        self.expect(&Tok::RBrace, "↘→")?;
        self.expect(&Tok::LBrace, "→↘")?;
        let then_body = self.parse_stmts()?;
        let mut else_body = Vec::new();
        if self.eat(&Tok::NotOpen) {
            self.expect(&Tok::LBrace, "→↘")?;
            else_body = self.parse_stmts()?;
        }
        Ok(Stmt::If { cond, then_body, else_body, line })
    }

    fn parse_loop(&mut self) -> Result<Stmt, ParseError> {
        let line = self.peek().line;
        self.expect(&Tok::Loop, "凌波")?;
        self.expect(&Tok::LBrace, "→↘")?;
        let body = self.parse_stmts()?;
        Ok(Stmt::Loop { body, line })
    }

    fn parse_ident_stmt(&mut self, name: String, line: u32) -> Result<Stmt, ParseError> {
        self.bump();
        if self.eat(&Tok::LBrace) {
            let mut args = Vec::new();
            if !self.eat(&Tok::RBrace) {
                loop {
                    args.push(self.parse_expr()?);
                    if self.eat(&Tok::Sep) {
                        continue;
                    }
                    self.expect(&Tok::RBrace, "↘→")?;
                    break;
                }
            }
            if self.eat(&Tok::Assign) {
                if args.len() != 1 {
                    return Err(self.err("给数组元素赋值时，下标只能有一个"));
                }
                let value = self.parse_expr()?;
                self.expect(&Tok::Semi, ";")?;
                let idx = args.pop().unwrap();
                Ok(Stmt::Assign { target: LValue::Index(name, Box::new(idx)), value, line })
            } else {
                self.expect(&Tok::Semi, ";")?;
                Ok(Stmt::Call { name, args, line })
            }
        } else if self.eat(&Tok::Assign) {
            let value = self.parse_expr()?;
            self.expect(&Tok::Semi, ";")?;
            Ok(Stmt::Assign { target: LValue::Name(name), value, line })
        } else {
            Err(self.err("语句只能是：调用 名字→↘参数↘→; 或 赋值 名字→←值;"))
        }
    }

    // ---- 表达式：优先级从低到高 ----

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }
    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_and()?;
        while self.eat(&Tok::Or) {
            let r = self.parse_and()?;
            e = Expr::Binary(BinOp::Or, Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_cmp()?;
        while self.eat(&Tok::And) {
            let r = self.parse_cmp()?;
            e = Expr::Binary(BinOp::And, Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn parse_cmp(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_add()?;
        loop {
            let op = match self.peek().tok {
                Tok::Eq => BinOp::Eq,
                Tok::Ne => BinOp::Ne,
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let r = self.parse_add()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn parse_add(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_mul()?;
        loop {
            let op = match self.peek().tok {
                Tok::Add => BinOp::Add,
                Tok::Sub => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let r = self.parse_mul()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn parse_mul(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_unary()?;
        loop {
            let op = match self.peek().tok {
                Tok::Mul => BinOp::Mul,
                Tok::Div => BinOp::Div,
                Tok::Mod => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let r = self.parse_unary()?;
            e = Expr::Binary(op, Box::new(e), Box::new(r));
        }
        Ok(e)
    }
    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        let op = match self.peek().tok {
            Tok::Neg => Some(UnOp::Neg),
            Tok::Not => Some(UnOp::Not),
            Tok::Inc => Some(UnOp::Inc),
            Tok::Dec => Some(UnOp::Dec),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.parse_unary()?;
            return Ok(Expr::Unary(op, Box::new(e)));
        }
        self.parse_postfix()
    }
    fn parse_postfix(&mut self) -> Result<Expr, ParseError> {
        let mut e = self.parse_primary()?;
        while self.check(&Tok::LBrace) {
            let name = match &e {
                Expr::Name(n) => n.clone(),
                _ => break,
            };
            self.bump();
            let mut args = Vec::new();
            if !self.eat(&Tok::RBrace) {
                loop {
                    args.push(self.parse_expr()?);
                    if self.eat(&Tok::Sep) {
                        continue;
                    }
                    self.expect(&Tok::RBrace, "↘→")?;
                    break;
                }
            }
            e = Expr::Apply(name, args);
        }
        Ok(e)
    }
    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.peek().clone();
        match t.tok {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Int(v))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Float(v))
            }
            Tok::True => {
                self.bump();
                Ok(Expr::Bool(true))
            }
            Tok::False => {
                self.bump();
                Ok(Expr::Bool(false))
            }
            Tok::Ident(name) => {
                self.bump();
                Ok(Expr::Name(name))
            }
            Tok::LBrace => {
                self.bump();
                let e = self.parse_expr()?;
                self.expect(&Tok::RBrace, "↘→")?;
                Ok(Expr::Group(Box::new(e)))
            }
            _ => Err(self.err("这里需要一个表达式")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_src(src: &str) -> Program {
        let toks = tokenize(src).unwrap();
        parse(&toks).unwrap()
    }

    #[test]
    fn minimal_main() {
        let p = parse_src("决心 →↘ 旗帜 x:草莓 →← 0; ↘→");
        let body = p.main.expect("要有决心");
        assert_eq!(body.len(), 1);
    }

    #[test]
    fn chapter_and_import() {
        let src = "第九章 →↘ 第一面:草莓 →↘n:草莓↘→ →↘ 超级弹 n; ↘→ ↘→ 第九章第一面;";
        let p = parse_src(src);
        assert_eq!(p.chapters.len(), 1);
        assert_eq!(p.chapters[0].funcs.len(), 1);
        assert_eq!(p.chapters[0].funcs[0].params.len(), 1);
        assert_eq!(p.imports, vec!["第九章第一面".to_string()]);
    }

    #[test]
    fn array_type_and_index_assign() {
        let p = parse_src("决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘0↘→ →← 1; ↘→");
        let body = p.main.expect("要有决心");
        assert_eq!(body.len(), 2);
        match &body[0] {
            Stmt::Decl { ty, .. } => assert_eq!(*ty, Type::Jelly(Box::new(Type::Int))),
            _ => panic!("第一条应该是声明"),
        }
        match &body[1] {
            Stmt::Assign { target: LValue::Index(name, _), .. } => assert_eq!(name, "a"),
            _ => panic!("第二条应该是下标赋值"),
        }
    }

    #[test]
    fn if_else() {
        let src = "决心 →↘ 心门 →↘1 ←→ 1↘→ →↘ 狼跳; ↘→ 没开 →↘ 奥欻; ↘→ ↘→";
        let p = parse_src(src);
        let body = p.main.expect("要有决心");
        match &body[0] {
            Stmt::If { else_body, .. } => assert_eq!(else_body.len(), 1),
            _ => panic!("应该是心门"),
        }
    }

    #[test]
    fn loop_stmt() {
        let src = "决心 →↘ 凌波 →↘ 狼跳; ↘→ ↘→";
        let p = parse_src(src);
        let body = p.main.expect("要有决心");
        match &body[0] {
            Stmt::Loop { .. } => {}
            _ => panic!("应该是凌波"),
        }
    }

    #[test]
    fn missing_semi_is_error() {
        let toks = tokenize("决心 →↘ 旗帜 x:草莓 →← 0 ↘→").unwrap();
        assert!(parse(&toks).is_err());
    }
}



