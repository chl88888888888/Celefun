// Celefun 的 C99 代码生成（M2b）
// 输入是语义分析后的 HIR，输出可以直接交给 gcc / clang 的 C 源码。

use crate::ast::{BinOp, UnOp};
use crate::hir::{arr_c_name, Expr, ExprKind, Func, Program, Stmt, Ty};

const RUNTIME: &str = r#"
// ---- Celefun runtime: dynamic arrays (auto-grow on write, 0 on out-of-range read) ----
#ifndef CF_ARR_TYPES_DEFINED
#define CF_ARR_TYPES_DEFINED
typedef struct { int64_t* data; size_t len; size_t cap; } cf_arr_i64;
typedef struct { double*  data; size_t len; size_t cap; } cf_arr_f64;
#endif

static inline void cf_arr_i64_set(cf_arr_i64* a, int64_t i, int64_t v) {
    if (i < 0) { fprintf(stderr, "index must not be negative: %lld\n", (long long)i); exit(1); }
    size_t idx = (size_t)i;
    if (idx >= a->cap) {
        size_t cap = a->cap ? a->cap : 8;
        while (cap <= idx) cap *= 2;
        a->data = (int64_t*)realloc(a->data, cap * sizeof(int64_t));
        if (!a->data) { fprintf(stderr, "out of memory\n"); exit(1); }
        for (size_t k = a->cap; k < cap; k++) a->data[k] = 0;
        a->cap = cap;
    }
    if (idx >= a->len) a->len = idx + 1;
    a->data[idx] = v;
}
static inline int64_t cf_arr_i64_get(const cf_arr_i64* a, int64_t i) {
    if (i < 0) return 0;
    size_t idx = (size_t)i;
    if (idx >= a->len) return 0;
    return a->data[idx];
}

static inline void cf_arr_f64_set(cf_arr_f64* a, int64_t i, double v) {
    if (i < 0) { fprintf(stderr, "index must not be negative: %lld\n", (long long)i); exit(1); }
    size_t idx = (size_t)i;
    if (idx >= a->cap) {
        size_t cap = a->cap ? a->cap : 8;
        while (cap <= idx) cap *= 2;
        a->data = (double*)realloc(a->data, cap * sizeof(double));
        if (!a->data) { fprintf(stderr, "out of memory\n"); exit(1); }
        for (size_t k = a->cap; k < cap; k++) a->data[k] = 0;
        a->cap = cap;
    }
    if (idx >= a->len) a->len = idx + 1;
    a->data[idx] = v;
}
static inline double cf_arr_f64_get(const cf_arr_f64* a, int64_t i) {
    if (i < 0) return 0;
    size_t idx = (size_t)i;
    if (idx >= a->len) return 0;
    return a->data[idx];
}
static inline cf_arr_i64* cf_arr_i64_new(void) {
    cf_arr_i64* a = (cf_arr_i64*)calloc(1, sizeof(cf_arr_i64));
    if (!a) { fprintf(stderr, "out of memory\n"); exit(1); }
    return a;
}
static inline cf_arr_f64* cf_arr_f64_new(void) {
    cf_arr_f64* a = (cf_arr_f64*)calloc(1, sizeof(cf_arr_f64));
    if (!a) { fprintf(stderr, "out of memory\n"); exit(1); }
    return a;
}
// ---- Celefun 标准库 L1：日记（标准输出）与 望远镜（标准输入）----
static inline void cf_print_i64(FILE* f, int64_t v) { fprintf(f, "%lld\n", (long long)v); }
static inline void cf_print_f64(FILE* f, double v) { fprintf(f, "%g\n", v); }
static inline void cf_print_bool(FILE* f, bool v) { fprintf(f, "%s\n", v ? "true" : "false"); }

static inline FILE* cf_open_path(const cf_arr_i64* path) {
    if (!path || path->len == 0) return stdout;
    char* s = (char*)malloc(path->len + 1);
    if (!s) { fprintf(stderr, "out of memory\n"); exit(1); }
    for (size_t i = 0; i < path->len; i++) s[i] = (char)path->data[i];
    s[path->len] = 0;
    FILE* f = fopen(s, "wb");
    if (!f) { fprintf(stderr, "cannot open output file: %s\n", s); free(s); exit(1); }
    free(s);
    return f;
}

static inline void cf_put_utf8(FILE* f, int64_t cp) {
    uint32_t c = (uint32_t)cp;
    if (c < 0x80) {
        fputc((int)c, f);
    } else if (c < 0x800) {
        fputc((int)(0xC0 | (c >> 6)), f);
        fputc((int)(0x80 | (c & 0x3F)), f);
    } else if (c < 0x10000) {
        fputc((int)(0xE0 | (c >> 12)), f);
        fputc((int)(0x80 | ((c >> 6) & 0x3F)), f);
        fputc((int)(0x80 | (c & 0x3F)), f);
    } else {
        fputc((int)(0xF0 | (c >> 18)), f);
        fputc((int)(0x80 | ((c >> 12) & 0x3F)), f);
        fputc((int)(0x80 | ((c >> 6) & 0x3F)), f);
        fputc((int)(0x80 | (c & 0x3F)), f);
    }
}
static inline void cf_print_utf8(FILE* f, const cf_arr_i64* a) {
    for (size_t i = 0; i < a->len; i++) cf_put_utf8(f, a->data[i]);
    fputc('\n', f);
}

// 按 token 读：跳过空白，读一个空白分隔的记号再解析（int 用 strtoll）
static inline bool cf_read_i64(int64_t* out) {
    int c = fgetc(stdin);
    while (c == ' ' || c == '\t' || c == '\n' || c == '\r') c = fgetc(stdin);
    if (c == EOF) return false;
    char buf[128];
    int n = 0;
    while (c != EOF && c != ' ' && c != '\t' && c != '\n' && c != '\r') {
        if (n < 127) buf[n++] = (char)c;
        c = fgetc(stdin);
    }
    if (c != EOF) ungetc(c, stdin);
    buf[n] = 0;
    char* end = NULL;
    long long v = strtoll(buf, &end, 10);
    if (end == buf) return false;
    *out = (int64_t)v;
    return true;
}
// 按 token 读浮点（strtod）
static inline bool cf_read_f64(double* out) {
    int c = fgetc(stdin);
    while (c == ' ' || c == '\t' || c == '\n' || c == '\r') c = fgetc(stdin);
    if (c == EOF) return false;
    char buf[128];
    int n = 0;
    while (c != EOF && c != ' ' && c != '\t' && c != '\n' && c != '\r') {
        if (n < 127) buf[n++] = (char)c;
        c = fgetc(stdin);
    }
    if (c != EOF) ungetc(c, stdin);
    buf[n] = 0;
    char* end = NULL;
    double v = strtod(buf, &end);
    if (end == buf) return false;
    *out = v;
    return true;
}static inline bool cf_read_line_utf8(cf_arr_i64* a) {
    int c = fgetc(stdin);
    if (c == EOF) return false;
    a->len = 0;
    while (c != EOF && c != '\n') {
        if (c == '\r') {
            /* 忽略 CR */
        } else if (c < 0x80) {
            cf_arr_i64_set(a, (int64_t)a->len, (int64_t)c);
        } else {
            uint32_t cp = 0;
            int extra = 0;
            if ((c & 0xE0) == 0xC0) { extra = 1; cp = (uint32_t)(c & 0x1F); }
            else if ((c & 0xF0) == 0xE0) { extra = 2; cp = (uint32_t)(c & 0x0F); }
            else if ((c & 0xF8) == 0xF0) { extra = 3; cp = (uint32_t)(c & 0x07); }
            else { extra = 0; cp = (uint32_t)c; }
            for (int i = 0; i < extra; i++) {
                int d = fgetc(stdin);
                if (d == EOF || (d & 0xC0) != 0x80) break;
                cp = (cp << 6) | (uint32_t)(d & 0x3F);
            }
            cf_arr_i64_set(a, (int64_t)a->len, (int64_t)cp);
        }
        c = fgetc(stdin);
    }
    return true;
}"#;

pub fn generate(prog: &Program) -> String {
    let mut s = String::new();
    s.push_str("// generated by the Celefun compiler (M2b)\n");
    s.push_str("#include <stdint.h>\n");
    s.push_str("#include <stdbool.h>\n");
    s.push_str("#include <stddef.h>\n");
    s.push_str("#include <stdio.h>\n");
    s.push_str("#include <stdlib.h>\n");
    s.push_str(RUNTIME);
    s.push('\n');

    for f in &prog.funcs {
        s.push_str(&proto(f));
        s.push('\n');
    }
    if !prog.funcs.is_empty() {
        s.push('\n');
    }
    for f in &prog.funcs {
        gen_func(&mut s, f);
    }

    if !prog.main.is_empty() {
        s.push_str("int main(void) {\n");
        for st in &prog.main {
            gen_stmt(&mut s, st, 1);
        }
        s.push_str("    return 0;\n}\n");
    }
    s
}

fn proto(f: &Func) -> String {
    let params: Vec<String> = f.params.iter().map(|(_, t)| t.c_type()).collect();
    let link = if f.ffi { "" } else { "static " };
    format!(
        "{}{} {}({});",
        link,
        f.ret.c_type(),
        f.cname,
        if params.is_empty() { "void".to_string() } else { params.join(", ") }
    )
}

fn param_list(f: &Func) -> String {
    let params: Vec<String> = f
        .params
        .iter()
        .map(|(n, t)| format!("{} {}", t.c_type(), n))
        .collect();
    if params.is_empty() {
        "void".to_string()
    } else {
        params.join(", ")
    }
}

fn gen_func(s: &mut String, f: &Func) {
    let link = if f.ffi { "" } else { "static " };
    s.push_str(&format!("{}{} {}({}) {{\n", link, f.ret.c_type(), f.cname, param_list(f)));
    for st in &f.body {
        gen_stmt(s, st, 1);
    }
    if matches!(f.ret, Ty::Arr(_)) {
        s.push_str("    return 0;\n");
    } else {
        s.push_str(&format!("    return ({})0;\n", f.ret.c_name()));
    }
    s.push_str("}\n\n");
}

fn pad(lvl: usize) -> String {
    "    ".repeat(lvl)
}

fn gen_stmt(s: &mut String, st: &Stmt, lvl: usize) {
    let p = pad(lvl);
    match st {
        Stmt::Decl { cname, ty, init, .. } => match ty {
            Ty::Arr(elem) => match init {
                Some(e) => s.push_str(&format!("{}{}* {} = {};\n", p, arr_c_name(elem), cname, gen_expr(e))),
                None => s.push_str(&format!("{}{}* {} = {}_new();\n", p, arr_c_name(elem), cname, arr_c_name(elem))),
            },
            _ => match init {
                Some(e) => {
                    s.push_str(&format!("{}{} {} = {};\n", p, ty.c_name(), cname, gen_expr(e)));
                }
                None => {
                    let zero = if *ty == Ty::Bool { "false" } else { "0" };
                    s.push_str(&format!("{}{} {} = {};\n", p, ty.c_name(), cname, zero));
                }
            },
        },
        Stmt::Assign { cname, value, .. } => {
            s.push_str(&format!("{}{} = {};\n", p, cname, gen_expr(value)));
        }
        Stmt::AssignIndex { cname, elem_ty, idx, value, .. } => {
            s.push_str(&format!(
                "{}{}_set({}, (int64_t)({}), {});\n",
                p,
                arr_c_name(elem_ty),
                cname,
                gen_expr(idx),
                gen_expr(value)
            ));
        }
        Stmt::If { cond, then_body, else_body } => {
            s.push_str(&format!("{}if ({}) {{\n", p, gen_expr(cond)));
            for x in then_body {
                gen_stmt(s, x, lvl + 1);
            }
            if else_body.is_empty() {
                s.push_str(&format!("{}}}\n", p));
            } else {
                s.push_str(&format!("{}}} else {{\n", p));
                for x in else_body {
                    gen_stmt(s, x, lvl + 1);
                }
                s.push_str(&format!("{}}}\n", p));
            }
        }
        Stmt::Loop { body } => {
            s.push_str(&format!("{}for (;;) {{\n", p));
            for x in body {
                gen_stmt(s, x, lvl + 1);
            }
            s.push_str(&format!("{}}}\n", p));
        }
        Stmt::Break => s.push_str(&format!("{}break;\n", p)),
        Stmt::Continue => s.push_str(&format!("{}continue;\n", p)),
        Stmt::Label(n) => s.push_str(&format!("{}cf_label_{}:;\n", p, mangle_label(n))),
        Stmt::Goto(n) => s.push_str(&format!("{}goto cf_label_{};\n", p, mangle_label(n))),
        Stmt::Exit(v) => match v {
            Some(e) => s.push_str(&format!("{}exit((int)({}));\n", p, gen_expr(e))),
            None => s.push_str(&format!("{}exit(0);\n", p)),
        },
        Stmt::Return(v) => match v {
            Some(e) => s.push_str(&format!("{}return {};\n", p, gen_expr(e))),
            None => s.push_str(&format!("{}return;\n", p)),
        },
        Stmt::Panic(v) => {
            match v {
                Some(e) => s.push_str(&format!(
                    "{}fprintf(stderr, \"panic: %lld\\n\", (long long)({}));\n",
                    p,
                    gen_expr(e)
                )),
                None => s.push_str(&format!("{}fprintf(stderr, \"panic\\n\");\n", p)),
            }
            s.push_str(&format!("{}exit(1);\n", p));
        }
        Stmt::CallStmt(e) => s.push_str(&format!("{}{};\n", p, gen_expr(e))),
        Stmt::ExprStmt(e) => s.push_str(&format!("{}(void)({});\n", p, gen_expr(e))),
        Stmt::Print { value, utf8, path } => {
            let f = match path {
                Some(p) => format!("cf_open_path({})", gen_expr(p)),
                None => "stdout".to_string(),
            };
            let v = gen_expr(value);
            if *utf8 {
                s.push_str(&format!("{}cf_print_utf8({}, {});\n", p, f, v));
            } else {
                let call = match value.ty {
                    Ty::Float => "cf_print_f64",
                    Ty::Bool => "cf_print_bool",
                    _ => "cf_print_i64",
                };
                s.push_str(&format!("{}{}({}, {});\n", p, call, f, v));
            }
        }
        Stmt::Read { cname, ty } => {
            let call = if *ty == Ty::Float { "cf_read_f64" } else { "cf_read_i64" };
            s.push_str(&format!("{}(void){}(&{});\n", p, call, cname));
        }
    }
}

fn gen_expr(e: &Expr) -> String {
    match &e.kind {
        ExprKind::Int(v) => format!("INT64_C({})", v),
        ExprKind::Float(v) => {
            let t = format!("{:?}", v);
            if t.contains('.') || t.contains('e') || t.contains('E') {
                t
            } else {
                format!("{}.0", t)
            }
        }
        ExprKind::Bool(b) => if *b { "true".to_string() } else { "false".to_string() },
        ExprKind::Var(n) => n.clone(),
        ExprKind::Call { cname, args } => {
            let a: Vec<String> = args.iter().map(gen_expr).collect();
            format!("{}({})", cname, a.join(", "))
        }
        ExprKind::Index { cname, elem_ty, idx, .. } => {
            format!("{}_get({}, (int64_t)({}))", arr_c_name(elem_ty), cname, gen_expr(idx))
        }
        ExprKind::Cast(x) => format!("((double)({}))", gen_expr(x)),
        ExprKind::Len(x) => format!("((int64_t)(({})->len))", gen_expr(x)),
        ExprKind::ReadLine(n) => format!("cf_read_line_utf8({})", n),
        ExprKind::Unary(op, x) => {
            let sx = gen_expr(x);
            match op {
                UnOp::Neg => format!("(-({}))", sx),
                UnOp::Not => format!("(!({}))", sx),
                UnOp::Inc => format!("(++({}))", sx),
                UnOp::Dec => format!("(--({}))", sx),
            }
        }
        ExprKind::Binary(op, l, r) => {
            let a = gen_expr(l);
            let b = gen_expr(r);
            let o = match op {
                BinOp::Add => "+",
                BinOp::Sub => "-",
                BinOp::Mul => "*",
                BinOp::Div => "/",
                BinOp::Mod => "%",
                BinOp::Eq => "==",
                BinOp::Ne => "!=",
                BinOp::Lt => "<",
                BinOp::Gt => ">",
                BinOp::Le => "<=",
                BinOp::Ge => ">=",
                BinOp::And => "&&",
                BinOp::Or => "||",
            };
            format!("(({}) {} ({}))", a, o, b)
        }
    }
}

// 生成给 C 用的头文件：只包含被 PICO-8 标记过的面
pub fn generate_header(prog: &Program) -> String {
    let mut s = String::new();
    s.push_str("// generated by the Celefun compiler (L4)\n");
    s.push_str("#ifndef CELEFUN_GENERATED_H\n#define CELEFUN_GENERATED_H\n\n");
    s.push_str("#include <stdint.h>\n#include <stdbool.h>\n#include <stddef.h>\n\n");
    s.push_str("#ifndef CF_ARR_TYPES_DEFINED\n#define CF_ARR_TYPES_DEFINED\n");
    s.push_str("typedef struct { int64_t* data; size_t len; size_t cap; } cf_arr_i64;\n");
    s.push_str("typedef struct { double*  data; size_t len; size_t cap; } cf_arr_f64;\n");
    s.push_str("#endif\n\n");
    for f in &prog.funcs {
        if f.ffi {
            s.push_str(&format!("// Celefun: {}\n", f.full_name));
            s.push_str(&proto(f));
            s.push('\n');
        }
    }
    s.push_str("\n#endif\n");
    s
}
fn mangle_label(n: &str) -> String {
    let mut s = String::new();
    for ch in n.chars() {
        s.push_str(&format!("{:x}_", ch as u32));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;
    use crate::parser::parse;
    use crate::sema;

    fn gen_src(src: &str) -> String {
        let toks = tokenize(src).unwrap();
        let ast = parse(&toks).unwrap();
        let hir = sema::check(&ast).unwrap();
        generate(&hir)
    }

    #[test]
    fn hello_c() {
        let c = gen_src("决心 →↘ 心 45; ↘→");
        assert!(c.contains("int main(void)"));
        assert!(c.contains("exit((int)(INT64_C(45)))"));
    }

    #[test]
    fn func_proto_and_call() {
        let src = "第九章 →↘ 第一面:草莓 →↘n:草莓↘→ →↘ 超级弹 n; ↘→ ↘→ 第九章第一面; 决心 →↘ 心 第九章第一面 →↘1↘→; ↘→";
        let c = gen_src(src);
        assert!(c.contains("int64_t cff_"));
        assert!(c.contains("cfv_"));
    }

    #[test]
    fn array_runtime() {
        let src = "决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘0↘→ →← 1; ↘→";
        let c = gen_src(src);
        assert!(c.contains("cf_arr_i64* cfv_"));
        assert!(c.contains("cf_arr_i64_new()"));
        assert!(c.contains("cf_arr_i64_set"));
    }

    #[test]
    fn f64_and_promotion() {
        let c = gen_src("决心 →↘ 旗帜 x:金草莓 →← 1 ↑ 1.5; ↘→");
        assert!(c.contains("double cfv_"));
        assert!(c.contains("1.5"));
        assert!(c.contains("((double)(INT64_C(1)))"));
    }
}












