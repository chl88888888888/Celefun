// Celefun 语义分析
// * 收集 章 / 面 的签名，解析导入
// * 把 名字→↘…↘→ 解析成 函数调用 或 数组下标
// * 类型检查（数值会做 int -> f64 隐式提升），输出 HIR

use std::collections::{HashMap, HashSet};

use crate::ast::{self, BinOp, UnOp};
use crate::hir::{Expr, ExprKind, Func, Intr, Program, Stmt, Ty};

#[derive(Debug, Clone)]
pub struct SemaError {
    pub msg: String,
    pub line: u32,
}

fn err(msg: impl Into<String>, line: u32) -> SemaError {
    SemaError { msg: msg.into(), line }
}

struct Sig {
    cname: String,
    params: Vec<Ty>,
    ret: Ty,
}

pub fn check(prog: &ast::Program) -> Result<Program, SemaError> {
    let mut s = Sema::new();
    s.collect(prog)?;
    s.lower_program(prog)
}

struct Sema {
    by_full: HashMap<String, Sig>,
    short_to_full: HashMap<(String, String), String>,
    import_shorts: HashSet<String>,
    scopes: Vec<HashMap<String, Ty>>,
    loop_depth: usize,
    labels: HashSet<String>,
    cur_chapter: Option<String>,
    cur_func: Option<(String, Ty)>,
    line: u32,
}

impl Sema {
    fn new() -> Self {
        Sema {
            by_full: HashMap::new(),
            short_to_full: HashMap::new(),
            import_shorts: HashSet::new(),
            scopes: Vec::new(),
            loop_depth: 0,
            labels: HashSet::new(),
            cur_chapter: None,
            cur_func: None,
            line: 0,
        }
    }

    fn collect(&mut self, prog: &ast::Program) -> Result<(), SemaError> {
        for ch in &prog.chapters {
            for f in &ch.funcs {
                let full = format!("{}{}", ch.name, f.name);
                if self.by_full.contains_key(&full) {
                    return Err(err(format!("面重复定义：{}", full), f.line));
                }
                let mut params = Vec::new();
                for (_, t) in &f.params {
                    let pt = self.ty(t, f.line)?;
                    params.push(pt);
                }
                let ret = self.ty(&f.ret, f.line)?;
                self.by_full.insert(
                    full.clone(),
                    Sig { cname: mangle_func(&full), params, ret },
                );
                self.short_to_full
                    .insert((ch.name.clone(), f.name.clone()), full);
            }
        }
        for imp in &prog.imports {
            if !self.by_full.contains_key(imp) {
                if intrinsic_by_full(imp).is_none() { return Err(err(format!("导入不存在的面：{}", imp), 0)); }
            }
            if let Some(sh) = short_name(imp) {
                self.import_shorts.insert(sh);
            }
        }
        Ok(())
    }

    fn ty(&self, t: &ast::Type, line: u32) -> Result<Ty, SemaError> {
        match t {
            ast::Type::Int => Ok(Ty::Int),
            ast::Type::Float => Ok(Ty::Float),
            ast::Type::Bool => Ok(Ty::Bool),
            ast::Type::Jelly(inner) => {
                let e = self.ty(inner, line)?;
                match e {
                    Ty::Int | Ty::Float => Ok(Ty::Arr(Box::new(e))),
                    _ => Err(err("果冻块现在只能装 草莓(int) 或 金草莓(f64)", line)),
                }
            }
        }
    }

    fn lower_program(&mut self, prog: &ast::Program) -> Result<Program, SemaError> {
        let mut out = Program::default();

        for ch in &prog.chapters {
            for f in &ch.funcs {
                let full = format!("{}{}", ch.name, f.name);
                let (cname, params, ret) = {
                    let sig = self.by_full.get(&full).unwrap();
                    (sig.cname.clone(), sig.params.clone(), sig.ret.clone())
                };
                self.cur_chapter = Some(ch.name.clone());
                self.cur_func = Some((full.clone(), ret.clone()));
                self.scopes = vec![HashMap::new()];
                self.labels = collect_labels(&f.body);
                self.loop_depth = 0;

                let mut cparams = Vec::new();
                for ((pname, _), pty) in f.params.iter().zip(params.iter()) {
                    self.scopes[0].insert(pname.clone(), pty.clone());
                    cparams.push((mangle_var(pname), pty.clone()));
                }
                let body = self.lower_stmts(&f.body)?;
                out.funcs.push(Func {
                    cname,
                    full_name: full,
                    params: cparams,
                    ret,
                    body,
                    ffi: f.ffi,
                });
            }
        }

        self.cur_chapter = None;
        self.cur_func = None;
        self.scopes = vec![HashMap::new()];
        self.labels = HashSet::new();
        self.loop_depth = 0;
        if let Some(m) = &prog.main {
            self.labels = collect_labels(m);
            out.main = self.lower_stmts(m)?;
        }
        Ok(out)
    }

    fn lower_stmts(&mut self, stmts: &[ast::Stmt]) -> Result<Vec<Stmt>, SemaError> {
        let mut out = Vec::new();
        for s in stmts {
            out.push(self.lower_stmt(s)?);
        }
        Ok(out)
    }

    fn lower_stmt(&mut self, s: &ast::Stmt) -> Result<Stmt, SemaError> {
        match s {
            ast::Stmt::Decl { name, ty, init, line } => {
                self.line = *line;
                let t = self.ty(ty, *line)?;
                if self.scopes.last().unwrap().contains_key(name) {
                    return Err(err(format!("同一层里重复声明：{}", name), *line));
                }
                let init_h = match init {
                    Some(e) => {
                        let h = self.lower_expr(e)?;
                        Some(self.coerce(h, t.clone(), *line, "初始值")?)
                    }
                    None => None,
                };
                self.scopes.last_mut().unwrap().insert(name.clone(), t.clone());
                Ok(Stmt::Decl {
                    name: name.clone(),
                    cname: mangle_var(name),
                    ty: t,
                    init: init_h,
                })
            }

            ast::Stmt::Assign { target, value, line } => {
                self.line = *line;
                match target {
                    ast::LValue::Name(n) => {
                        let t = self
                            .lookup(n)
                            .ok_or_else(|| err(format!("未定义的变量：{}", n), *line))?;
                        let v = self.lower_expr(value)?;
                        let v = self.coerce(v, t.clone(), *line, "赋值")?;
                        Ok(Stmt::Assign { cname: mangle_var(n), ty: t, value: v })
                    }
                    ast::LValue::Index(n, idx) => {
                        let t = self
                            .lookup(n)
                            .ok_or_else(|| err(format!("未定义的变量：{}", n), *line))?;
                        let et = match &t {
                            Ty::Arr(et) => (**et).clone(),
                            _ => return Err(err(format!("{} 不是数组，不能取下标的赋值", n), *line)),
                        };
                        let idxh = self.lower_expr(idx)?;
                        let idxh = self.coerce(idxh, Ty::Int, *line, "下标")?;
                        let v = self.lower_expr(value)?;
                        let v = self.coerce(v, et.clone(), *line, "元素")?;
                        Ok(Stmt::AssignIndex {
                            cname: mangle_var(n),
                            arr_ty: t,
                            elem_ty: et,
                            idx: idxh,
                            value: v,
                        })
                    }
                }
            }

            ast::Stmt::If { cond, then_body, else_body, line } => {
                self.line = *line;
                let c = self.lower_expr(cond)?;
                if !matches!(c.ty, Ty::Bool | Ty::Int | Ty::Float) {
                    return Err(err("心门的条件要能当真假", *line));
                }
                self.push_scope();
                let t = self.lower_stmts(then_body)?;
                self.pop_scope();
                self.push_scope();
                let e = self.lower_stmts(else_body)?;
                self.pop_scope();
                Ok(Stmt::If { cond: c, then_body: t, else_body: e })
            }

            ast::Stmt::Loop { body, line } => {
                self.line = *line;
                self.push_scope();
                self.loop_depth += 1;
                let b = self.lower_stmts(body)?;
                self.loop_depth -= 1;
                self.pop_scope();
                Ok(Stmt::Loop { body: b })
            }

            ast::Stmt::Break { line } => {
                if self.loop_depth == 0 {
                    return Err(err("狼跳 只能出现在 凌波 里面", *line));
                }
                Ok(Stmt::Break)
            }
            ast::Stmt::Continue { line } => {
                if self.loop_depth == 0 {
                    return Err(err("奥欻 只能出现在 凌波 里面", *line));
                }
                Ok(Stmt::Continue)
            }

            ast::Stmt::Goto { label, line } => {
                if !self.labels.contains(label) {
                    return Err(err(format!("找不到标签：{}", label), *line));
                }
                Ok(Stmt::Goto(label.clone()))
            }
            ast::Stmt::Label { name, .. } => Ok(Stmt::Label(name.clone())),

            ast::Stmt::Exit { value, line } => {
                self.line = *line;
                let v = match value {
                    Some(e) => {
                        let h = self.lower_expr(e)?;
                        Some(self.coerce(h, Ty::Int, *line, "退出码")?)
                    }
                    None => None,
                };
                Ok(Stmt::Exit(v))
            }

            ast::Stmt::Return { value, line } => {
                self.line = *line;
                let (_, ret) = self
                    .cur_func
                    .clone()
                    .ok_or_else(|| err("超级弹 不能出现在 决心 里", *line))?;
                let v = match value {
                    Some(e) => {
                        let h = self.lower_expr(e)?;
                        Some(self.coerce(h, ret.clone(), *line, "返回值")?)
                    }
                    None => None,
                };
                Ok(Stmt::Return(v))
            }

            ast::Stmt::Panic { value, line } => {
                self.line = *line;
                let v = match value {
                    Some(e) => Some(self.lower_expr(e)?),
                    None => None,
                };
                Ok(Stmt::Panic(v))
            }

            ast::Stmt::Call { name, args, line } => {
                self.line = *line;
                if self.resolve_func(name).is_some() {
                    let e = self.lower_call(name, args, *line)?;
                    Ok(Stmt::CallStmt(e))
                } else if let Some(intr) = self.resolve_intrinsic(name) {
                    self.lower_intrinsic(intr, args, *line)
                } else {
                    Err(err(format!("找不到面：{}", name), *line))
                }
            }

            ast::Stmt::ExprStmt { expr, line } => {
                self.line = *line;
                let e = self.lower_expr(expr)?;
                Ok(Stmt::ExprStmt(e))
            }
        }
    }

    fn lower_call(
        &mut self,
        name: &str,
        args: &[ast::Expr],
        line: u32,
    ) -> Result<Expr, SemaError> {
        let (cname, params, ret) = {
            let sig = self
                .resolve_func(name)
                .ok_or_else(|| err(format!("找不到面：{}", name), line))?;
            (sig.cname.clone(), sig.params.clone(), sig.ret.clone())
        };
        if args.len() != params.len() {
            return Err(err(
                format!("{} 需要 {} 个参数，给了 {}", name, params.len(), args.len()),
                line,
            ));
        }
        let mut hargs = Vec::new();
        for (a, pt) in args.iter().zip(params.iter()) {
            let h = self.lower_expr(a)?;
            hargs.push(self.coerce(h, pt.clone(), line, "参数")?);
        }
        Ok(Expr { kind: ExprKind::Call { cname, args: hargs }, ty: ret })
    }

    fn lower_expr(&mut self, e: &ast::Expr) -> Result<Expr, SemaError> {
        match e {
            ast::Expr::Int(v) => Ok(Expr { kind: ExprKind::Int(*v), ty: Ty::Int }),
            ast::Expr::Float(v) => Ok(Expr { kind: ExprKind::Float(*v), ty: Ty::Float }),
            ast::Expr::Bool(b) => Ok(Expr { kind: ExprKind::Bool(*b), ty: Ty::Bool }),
            ast::Expr::Name(n) => {
                let t = self
                    .lookup(n)
                    .ok_or_else(|| err(format!("未定义的变量：{}", n), self.line))?;
                Ok(Expr { kind: ExprKind::Var(mangle_var(n)), ty: t })
            }
            ast::Expr::Apply(n, args) => {
                if let Some(t) = self.lookup(n) {
                    if let Ty::Arr(et) = &t {
                        if args.len() != 1 {
                            return Err(err("下标只能有一个", self.line));
                        }
                        let idx = self.lower_expr(&args[0])?;
                        let idx = self.coerce(idx, Ty::Int, self.line, "下标")?;
                        return Ok(Expr {
                            kind: ExprKind::Index {
                                cname: mangle_var(n),
                                arr_ty: t.clone(),
                                idx: Box::new(idx),
                                elem_ty: (**et).clone(),
                            },
                            ty: (**et).clone(),
                        });
                    }
                    return Err(err(format!("{} 是 {:?} 变量，不能这样调用", n, t), self.line));
                }
                if self.resolve_intrinsic(n) == Some(Intr::Read) {
                    if args.len() == 2 {
                        if let ast::Expr::Name(flag) = &args[1] {
                            if flag == "utf8" {
                                let line = self.line;
                                return self.lower_read_line(&args[0], line);
                            }
                        }
                    }
                }                if self.resolve_func(n).is_some() {
                    return self.lower_call(n, args, self.line);
                }
                if n == "果冻反抓" {
                    if args.len() != 1 {
                        return Err(err("果冻反抓 需要一个数组参数", self.line));
                    }
                    let a = self.lower_expr(&args[0])?;
                    if !matches!(a.ty, Ty::Arr(_)) {
                        return Err(err("果冻反抓 只作用于 果冻块(数组)", self.line));
                    }
                    return Ok(Expr { kind: ExprKind::Len(Box::new(a)), ty: Ty::Int });
                }                if self.resolve_intrinsic(n).is_some() {
                    return Err(err(format!("{} 只能当语句用", n), self.line));
                }
                Err(err(format!("找不到面：{}", n), self.line))
            }
            ast::Expr::Unary(op, x) => {
                let h = self.lower_expr(x)?;
                let ty = match op {
                    UnOp::Neg => match h.ty {
                        Ty::Int => Ty::Int,
                        Ty::Float => Ty::Float,
                        _ => return Err(err("取负只支持数值", self.line)),
                    },
                    UnOp::Not => {
                        self.need(&h.ty, &Ty::Bool, "非")?;
                        Ty::Bool
                    }
                    UnOp::Inc | UnOp::Dec => {
                        if !h.ty.is_num() {
                            return Err(err("自增 / 自减只支持数值", self.line));
                        }
                        if !matches!(h.kind, ExprKind::Var(_)) {
                            return Err(err("↑↑ / ↓↓ 只能作用在变量上", self.line));
                        }
                        h.ty.clone()
                    }
                };
                if matches!(op, UnOp::Neg) {
                    if let ExprKind::Int(v) = &h.kind {
                        if let Some(nv) = v.checked_neg() {
                            return Ok(Expr { kind: ExprKind::Int(nv), ty: Ty::Int });
                        }
                    }
                }                Ok(Expr { kind: ExprKind::Unary(*op, Box::new(h)), ty })
            }
            ast::Expr::Binary(op, l, r) => {
                let a = self.lower_expr(l)?;
                let b = self.lower_expr(r)?;
                self.bin_expr(*op, a, b, self.line)
            }
            ast::Expr::Group(x) => self.lower_expr(x),
        }
    }

    fn bin_expr(&mut self, op: BinOp, a: Expr, b: Expr, line: u32) -> Result<Expr, SemaError> {
        use BinOp::*;
        let (ty, a, b) = match op {
            Add | Sub | Mul | Div => {
                let (t, a2, b2) = self.unify_num(a, b, line, "算术运算")?;
                (t, a2, b2)
            }
            Mod => {
                if a.ty == Ty::Int && b.ty == Ty::Int {
                    (Ty::Int, a, b)
                } else {
                    return Err(err("取余只支持 草莓(int)", line));
                }
            }
            Eq | Ne => {
                if a.ty.is_num() && b.ty.is_num() {
                    let (_, a2, b2) = self.unify_num(a, b, line, "相等比较")?;
                    (Ty::Bool, a2, b2)
                } else if a.ty == b.ty && matches!(a.ty, Ty::Int | Ty::Bool) {
                    (Ty::Bool, a, b)
                } else {
                    return Err(err("相等 / 不等的两边类型不匹配", line));
                }
            }
            Lt | Gt | Le | Ge => {
                let (_, a2, b2) = self.unify_num(a, b, line, "大小比较")?;
                (Ty::Bool, a2, b2)
            }
            And | Or => {
                if a.ty == Ty::Bool && b.ty == Ty::Bool {
                    (Ty::Bool, a, b)
                } else {
                    return Err(err("且 / 或 只支持 磁带(bool)", line));
                }
            }
        };
        if let (ExprKind::Int(x), ExprKind::Int(y)) = (&a.kind, &b.kind) {
            let (x, y) = (*x, *y);
            match op {
                Add | Sub | Mul | Div | Mod => {
                    let r = match op {
                        Add => x.checked_add(y),
                        Sub => x.checked_sub(y),
                        Mul => x.checked_mul(y),
                        Div => if y == 0 { None } else { x.checked_div(y) },
                        _ => if y == 0 { None } else { x.checked_rem(y) },
                    };
                    if let Some(v) = r {
                        return Ok(Expr { kind: ExprKind::Int(v), ty: Ty::Int });
                    }
                }
                Eq | Ne | Lt | Gt | Le | Ge => {
                    let r = match op {
                        Eq => x == y,
                        Ne => x != y,
                        Lt => x < y,
                        Gt => x > y,
                        Le => x <= y,
                        _ => x >= y,
                    };
                    return Ok(Expr { kind: ExprKind::Bool(r), ty: Ty::Bool });
                }
                _ => {}
            }
        }        Ok(Expr { kind: ExprKind::Binary(op, Box::new(a), Box::new(b)), ty })
    }

    fn unify_num(
        &mut self,
        a: Expr,
        b: Expr,
        line: u32,
        what: &str,
    ) -> Result<(Ty, Expr, Expr), SemaError> {
        match (a.ty.clone(), b.ty.clone()) {
            (Ty::Int, Ty::Int) => Ok((Ty::Int, a, b)),
            (Ty::Float, Ty::Float) => Ok((Ty::Float, a, b)),
            (Ty::Int, Ty::Float) => {
                let a2 = self.coerce(a, Ty::Float, line, what)?;
                Ok((Ty::Float, a2, b))
            }
            (Ty::Float, Ty::Int) => {
                let b2 = self.coerce(b, Ty::Float, line, what)?;
                Ok((Ty::Float, a, b2))
            }
            _ => Err(err(format!("{} 只支持数值", what), line)),
        }
    }

    fn coerce(&mut self, e: Expr, want: Ty, line: u32, what: &str) -> Result<Expr, SemaError> {
        if e.ty == want {
            return Ok(e);
        }
        if e.ty == Ty::Int && want == Ty::Float {
            return Ok(Expr { kind: ExprKind::Cast(Box::new(e)), ty: Ty::Float });
        }
        Err(err(format!("{} 要 {:?}，给的是 {:?}", what, want, e.ty), line))
    }

    fn need(&self, got: &Ty, want: &Ty, what: &str) -> Result<(), SemaError> {
        if got == want {
            Ok(())
        } else {
            Err(err(format!("{} 要 {:?}，给的是 {:?}", what, want, got), self.line))
        }
    }

    fn resolve_intrinsic(&self, name: &str) -> Option<Intr> {
        if let Some(i) = intrinsic_by_full(name) {
            return Some(i);
        }
        if self.import_shorts.contains(name) {
            return intrinsic_by_short(name);
        }
        None
    }

    fn lower_read_line(&mut self, arg: &ast::Expr, line: u32) -> Result<Expr, SemaError> {
        match arg {
            ast::Expr::Name(n) => {
                let t = self
                    .lookup(n)
                    .ok_or_else(|| err(format!("未定义的变量：{}", n), line))?;
                if t != Ty::Arr(Box::new(Ty::Int)) {
                    return Err(err("望远镜 utf8 的第一个参数要是 草莓(i64) 数组", line));
                }
                Ok(Expr { kind: ExprKind::ReadLine(mangle_var(n)), ty: Ty::Bool })
            }
            _ => Err(err("望远镜 utf8 的第一个参数必须是一个数组变量", line)),
        }
    }
    fn lower_intrinsic(
        &mut self,
        intr: Intr,
        args: &[ast::Expr],
        line: u32,
    ) -> Result<Stmt, SemaError> {
        match intr {
            Intr::Print => {
                if args.is_empty() || args.len() > 3 {
                    return Err(err("日记 需要 1 到 3 个参数：值 [、utf8] [、文件路径]", line));
                }
                let mut utf8 = false;
                if args.len() >= 2 {
                    match &args[1] {
                        ast::Expr::Name(n) if n == "utf8" => utf8 = true,
                        _ => return Err(err("日记 的第二个参数只能是 utf8 这个标志", line)),
                    }
                }
                let mut path = None;
                if args.len() == 3 {
                    let p = self.lower_expr(&args[2])?;
                    if p.ty != Ty::Arr(Box::new(Ty::Int)) {
                        return Err(err("文件路径要是 草莓(i64) 数组（UTF-8 字节）", line));
                    }
                    path = Some(p);
                }
                let v = self.lower_expr(&args[0])?;
                if utf8 {
                    if v.ty != Ty::Arr(Box::new(Ty::Int)) {
                        return Err(err("加了 utf8 之后，第一个参数要是 草莓(i64) 数组", line));
                    }
                } else if !matches!(v.ty, Ty::Int | Ty::Float | Ty::Bool) {
                    return Err(err("日记 只能打印 草莓 / 金草莓 / 磁带（数组要加 utf8）", line));
                }
                Ok(Stmt::Print { value: v, utf8, path })
            }
            Intr::Read => {
                if args.len() == 2 {
                    if let ast::Expr::Name(flag) = &args[1] {
                        if flag == "utf8" {
                            let e = self.lower_read_line(&args[0], line)?;
                            return Ok(Stmt::ExprStmt(e));
                        }
                    }
                    return Err(err("望远镜 的第二个参数只能是 utf8 这个标志", line));
                }
                if args.len() != 1 {
                    return Err(err("望远镜 需要一个变量参数", line));
                }
                match &args[0] {
                    ast::Expr::Name(n) => {
                        let t = self
                            .lookup(n)
                            .ok_or_else(|| err(format!("未定义的变量：{}", n), line))?;
                        if !matches!(t, Ty::Int | Ty::Float) {
                            return Err(err("望远镜 只能读 草莓 / 金草莓", line));
                        }
                        Ok(Stmt::Read { cname: mangle_var(n), ty: t })
                    }
                    _ => Err(err("望远镜 的参数必须是一个变量", line)),
                }
            }
        }
    }
    fn resolve_func<'a>(&'a self, name: &str) -> Option<&'a Sig> {
        if let Some(s) = self.by_full.get(name) {
            return Some(s);
        }
        if let Some(ch) = &self.cur_chapter {
            if let Some(full) = self.short_to_full.get(&(ch.clone(), name.to_string())) {
                return self.by_full.get(full);
            }
        }
        if self.import_shorts.contains(name) {
            for (full, s) in &self.by_full {
                if short_name(full).as_deref() == Some(name) {
                    return Some(s);
                }
            }
        }
        None
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }
    fn pop_scope(&mut self) {
        self.scopes.pop();
    }
    fn lookup(&self, name: &str) -> Option<Ty> {
        for sc in self.scopes.iter().rev() {
            if let Some(t) = sc.get(name) {
                return Some(t.clone());
            }
        }
        None
    }
}

fn intrinsic_by_full(name: &str) -> Option<Intr> {
    match name {
        "第十章日记" => Some(Intr::Print),
        "第十章望远镜" => Some(Intr::Read),
        _ => None,
    }
}

fn intrinsic_by_short(name: &str) -> Option<Intr> {
    match name {
        "日记" => Some(Intr::Print),
        "望远镜" => Some(Intr::Read),
        _ => None,
    }
}
fn collect_labels(stmts: &[ast::Stmt]) -> HashSet<String> {
    let mut out = HashSet::new();
    for s in stmts {
        match s {
            ast::Stmt::Label { name, .. } => {
                out.insert(name.clone());
            }
            ast::Stmt::If { then_body, else_body, .. } => {
                out.extend(collect_labels(then_body));
                out.extend(collect_labels(else_body));
            }
            ast::Stmt::Loop { body, .. } => {
                out.extend(collect_labels(body));
            }
            _ => {}
        }
    }
    out
}

pub fn mangle_func(name: &str) -> String {
    let mut s = String::from("cff");
    for ch in name.chars() {
        s.push('_');
        s.push_str(&format!("{:x}", ch as u32));
    }
    s
}

pub fn mangle_var(name: &str) -> String {
    let mut s = String::from("cfv");
    for ch in name.chars() {
        s.push('_');
        s.push_str(&format!("{:x}", ch as u32));
    }
    s
}

fn short_name(full: &str) -> Option<String> {
    full.rfind('章').map(|i| full[i + '章'.len_utf8()..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;

    fn check_src(src: &str) -> Result<Program, SemaError> {
        let toks = tokenize(src).unwrap();
        let ast = parse(&toks).unwrap();
        check(&ast)
    }

    #[test]
    fn ok_scalars_and_functions() {
        let src = "第九章 →↘ 第一面:草莓 →↘n:草莓↘→ →↘ 超级弹 n; ↘→ ↘→ 第九章第一面; 决心 →↘ 心 第九章第一面 →↘1↘→; ↘→";
        assert!(check_src(src).is_ok());
    }

    #[test]
    fn f64_ok() {
        assert!(check_src("决心 →↘ 旗帜 x:金草莓 →← 1.5; ↘→").is_ok());
    }

    #[test]
    fn int_promotes_to_f64() {
        assert!(check_src("决心 →↘ 旗帜 x:金草莓 →← 1 ↑ 1.5; ↘→").is_ok());
        assert!(check_src("决心 →↘ 旗帜 x:金草莓 →← 3; ↘→").is_ok());
    }

    #[test]
    fn array_ok() {
        let src = "决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘0↘→ →← 1; ↘→";
        let p = check_src(src).unwrap();
        assert_eq!(p.main.len(), 2);
    }

    #[test]
    fn array_index_must_be_int() {
        let src = "决心 →↘ 旗帜 a:果冻块→↘草莓↘→; 旗帜 x:草莓 →← a →↘1.5↘→; ↘→";
        assert!(check_src(src).is_err());
    }

    #[test]
    fn bool_array_not_supported() {
        assert!(check_src("决心 →↘ 旗帜 a:果冻块→↘磁带↘→; ↘→").is_err());
    }

    #[test]
    fn undefined_var() {
        assert!(check_src("决心 →↘ 心 x; ↘→").is_err());
    }

    #[test]
    fn type_mismatch() {
        assert!(check_src("决心 →↘ 旗帜 b:磁带 →← 1; ↘→").is_err());
    }

    #[test]
    fn break_outside_loop() {
        assert!(check_src("决心 →↘ 狼跳; ↘→").is_err());
    }

    #[test]
    fn goto_missing_label() {
        assert!(check_src("决心 →↘ 金羽毛 哪里; ↘→").is_err());
    }
}







