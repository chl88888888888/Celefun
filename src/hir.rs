// Celefun 的高层中间表示（HIR）：语义分析之后、代码生成之前

use crate::ast::{BinOp, UnOp};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Int,          // 草莓
    Float,        // 金草莓
    Bool,         // 磁带
    Arr(Box<Ty>), // 果冻块→↘元素类型↘→
}

impl Ty {
    pub fn is_num(&self) -> bool {
        matches!(self, Ty::Int | Ty::Float)
    }
    pub fn c_name(&self) -> &'static str {
        match self {
            Ty::Int => "int64_t",
            Ty::Float => "double",
            Ty::Bool => "bool",
            Ty::Arr(_) => "cf_arr",
        }
    }
}

impl Ty {
    // 数组在 C 里是「指向堆上结构体的指针」，所以类型名带星号
    pub fn c_type(&self) -> String {
        match self {
            Ty::Arr(elem) => format!("{}*", arr_c_name(elem)),
            other => other.c_name().to_string(),
        }
    }
}

// 数组的 C 结构体名
pub fn arr_c_name(elem: &Ty) -> &'static str {
    match elem {
        Ty::Float => "cf_arr_f64",
        _ => "cf_arr_i64",
    }
}

// 内建面（标准库 L1）：直接落到 C 的输入输出
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intr {
    Print, // 第十章日记
    Read,  // 第十章望远镜
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprKind {
    Int(i64),
    Float(f64),
    Bool(bool),
    Var(String),
    Call { cname: String, args: Vec<Expr> },
    Index { cname: String, arr_ty: Ty, idx: Box<Expr>, elem_ty: Ty },
    Cast(Box<Expr>),
    Len(Box<Expr>),
    ReadLine(String), // 望远镜 →↘数组、utf8↘→：读一行，返回 磁带
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Ty,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Decl {
        name: String,
        cname: String,
        ty: Ty,
        init: Option<Expr>,
    },
    Assign {
        cname: String,
        ty: Ty,
        value: Expr,
    },
    AssignIndex {
        cname: String,
        arr_ty: Ty,
        elem_ty: Ty,
        idx: Expr,
        value: Expr,
    },
    If {
        cond: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    Loop {
        body: Vec<Stmt>,
    },
    Break,
    Continue,
    Label(String),
    Goto(String),
    Exit(Option<Expr>),
    Return(Option<Expr>),
    Panic(Option<Expr>),
    CallStmt(Expr),
    ExprStmt(Expr),
    Print { value: Expr, utf8: bool, path: Option<Expr> },
    Read { cname: String, ty: Ty },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    pub cname: String,
    pub full_name: String,
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    pub body: Vec<Stmt>,
    pub ffi: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub funcs: Vec<Func>,
    pub main: Vec<Stmt>,
}



