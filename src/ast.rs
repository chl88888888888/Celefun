// Celefun 语法树（AST）

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,              // 草莓
    Float,            // 金草莓
    Bool,             // 磁带
    Jelly(Box<Type>), // 果冻块→↘元素类型↘→
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp { Add, Sub, Mul, Div, Mod, Eq, Ne, Lt, Gt, Le, Ge, And, Or }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnOp { Neg, Not, Inc, Dec }

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Name(String),
    // 名字→↘参数↘→：函数调用或数组下标，交给语义分析区分
    Apply(String, Vec<Expr>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Group(Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LValue {
    Name(String),
    Index(String, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Decl { name: String, ty: Type, init: Option<Expr>, line: u32 },
    Assign { target: LValue, value: Expr, line: u32 },
    If { cond: Expr, then_body: Vec<Stmt>, else_body: Vec<Stmt>, line: u32 },
    Loop { body: Vec<Stmt>, line: u32 },
    Break { line: u32 },
    Continue { line: u32 },
    Goto { label: String, line: u32 },
    Label { name: String, line: u32 },
    Exit { value: Option<Expr>, line: u32 },
    Return { value: Option<Expr>, line: u32 },
    Panic { value: Option<Expr>, line: u32 },
    Call { name: String, args: Vec<Expr>, line: u32 },
    ExprStmt { expr: Expr, line: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Func {
    pub name: String,
    pub ret: Type,
    pub params: Vec<(String, Type)>,
    pub body: Vec<Stmt>,
    pub line: u32,
    pub ffi: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub name: String,
    pub funcs: Vec<Func>,
    pub line: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Program {
    pub chapters: Vec<Chapter>,
    pub imports: Vec<String>,
    pub main: Option<Vec<Stmt>>,
}



