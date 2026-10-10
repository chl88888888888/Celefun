use std::env;
use std::fs;
use std::process::{exit, Command};

use celefun::ast::Program;
use celefun::{codegen_c, lexer, parser, sema};

fn usage() -> ! {
    eprintln!("用法:");
    eprintln!("  celefun check 主文件.celefun [库文件.celefun ...]");
    eprintln!("  celefun build 主文件.celefun [库文件.celefun ...] [-o 输出.exe] [--cc gcc|clang|cl|llvm]");
    eprintln!("  celefun run   主文件.celefun [库文件.celefun ...] [--cc gcc|clang|cl|llvm]");
    eprintln!();
    eprintln!("  多个文件会合并：库文件只贡献「章 / 面」，主文件提供「决心」。");
    eprintln!("  面用 →↘PICO-8↘→ 标记时会导出成 C FFI，并生成 主文件.h");
    exit(2);
}

// MSVC / LLVM(MSVC 目标) 需要的环境脚本；可用环境变量覆盖
fn vcvars() -> String {
    env::var("CELEFUN_VCVARS")
        .unwrap_or_else(|_| r"D:\daima\vs\VC\Auxiliary\Build\vcvars64.bat".to_string())
}

fn run_cmd(mut cmd: Command) -> Result<(), String> {
    match cmd.status() {
        Ok(st) if st.success() => Ok(()),
        Ok(st) => Err(format!("编译器失败，退出码 {:?}", st.code())),
        Err(e) => Err(format!("叫不动编译器: {}", e)),
    }
}

fn build_simple(prog: &str, c_path: &str, exe_path: &str) -> Result<(), String> {
    let mut cmd = Command::new(prog);
    cmd.arg("-std=c99").arg("-O2").arg("-o").arg(exe_path).arg(c_path);
    run_cmd(cmd)
}

fn build_bat(body: &str, bat_path: &str) -> Result<(), String> {
    fs::write(bat_path, body).map_err(|e| e.to_string())?;
    let mut cmd = Command::new("cmd");
    cmd.arg("/c").arg(bat_path);
    let r = run_cmd(cmd);
    let _ = fs::remove_file(bat_path);
    r
}

fn build_cl(c_path: &str, exe_path: &str) -> Result<(), String> {
    let bat = format!("{}.cl.bat", c_path);
    let body = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\ncl /nologo /utf-8 /std:c11 /O2 /Fe:\"{}\" /Fo:\"{}.obj\" \"{}\"\r\nexit /b %errorlevel%\r\n",
        vcvars(),
        exe_path,
        exe_path,
        c_path
    );
    build_bat(&body, &bat)
}

fn build_llvm(c_path: &str, exe_path: &str) -> Result<(), String> {
    let bat = format!("{}.llvm.bat", c_path);
    let ll = format!("{}.ll", c_path);
    let body = format!(
        "@echo off\r\ncall \"{}\" >nul 2>&1\r\nclang -std=c99 -O2 -S -emit-llvm -o \"{}\" \"{}\" || exit /b 1\r\nclang -O2 -o \"{}\" \"{}\" || exit /b 1\r\nexit /b 0\r\n",
        vcvars(),
        ll,
        c_path,
        exe_path,
        ll
    );
    build_bat(&body, &bat)
}

// 读一个 .celefun 文件，切成 AST
fn load(path: &str) -> Result<Program, String> {
    let src = fs::read_to_string(path).map_err(|e| format!("读不到文件 {}: {}", path, e))?;
    let toks = lexer::tokenize(&src)
        .map_err(|e| format!("{} 词法错误 {}:{}: {}", path, e.line, e.col, e.msg))?;
    let ast = parser::parse(&toks)
        .map_err(|e| format!("{} 语法错误 {}:{}: {}", path, e.line, e.col, e.msg))?;
    Ok(ast)
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        usage();
    }
    let cmd = args[1].clone();
    let entry = args[2].clone();
    let mut out: Option<String> = None;
    let mut cc = String::from("gcc");
    let mut libs: Vec<String> = Vec::new();
    let mut i = 3;
    while i < args.len() {
        if args[i] == "-o" && i + 1 < args.len() {
            out = Some(args[i + 1].clone());
            i += 2;
        } else if args[i] == "--cc" && i + 1 < args.len() {
            cc = args[i + 1].clone();
            i += 2;
        } else if args[i].starts_with('-') {
            eprintln!("不认识的参数: {}", args[i]);
            exit(2);
        } else {
            libs.push(args[i].clone());
            i += 1;
        }
    }

    let mut prog = match load(&entry) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{}", e);
            exit(1);
        }
    };
    for lib in &libs {
        let p = match load(lib) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("{}", e);
                exit(1);
            }
        };
        if p.main.is_some() {
            if prog.main.is_some() {
                eprintln!("只能有一个 决心（{} 里又出现了一个）", lib);
                exit(1);
            }
            prog.main = p.main;
        }
        prog.chapters.extend(p.chapters);
        prog.imports.extend(p.imports);
    }

    let hir = match sema::check(&prog) {
        Ok(h) => h,
        Err(e) => {
            if e.line > 0 {
                eprintln!("语义错误 {}: {}", e.line, e.msg);
            } else {
                eprintln!("语义错误: {}", e.msg);
            }
            exit(1);
        }
    };

    match cmd.as_str() {
        "check" => {
            println!("检查通过");
            println!("面: {} 个", hir.funcs.len());
            let ffi = hir.funcs.iter().filter(|f| f.ffi).count();
            if ffi > 0 {
                println!("FFI 面: {} 个", ffi);
            }
            if !hir.main.is_empty() {
                println!("决心: {} 条语句", hir.main.len());
            }
        }
        "build" | "run" => {
            let c_src = codegen_c::generate(&hir);
            let h_src = codegen_c::generate_header(&hir);
            let c_path = format!("{}.c", entry);
            let h_path = format!("{}.h", entry);
            if let Err(e) = fs::write(&c_path, c_src) {
                eprintln!("写不了 C 文件: {}", e);
                exit(1);
            }
            if let Err(e) = fs::write(&h_path, h_src) {
                eprintln!("写不了头文件: {}", e);
                exit(1);
            }
            if hir.main.is_empty() {
                println!("没有 决心，只生成 C 和头文件（不链接 exe）");
                println!("C 文件: {}", c_path);
                println!("头文件: {}", h_path);
                exit(0);
            }
            let exe_path = out.clone().unwrap_or_else(|| format!("{}.exe", entry));
            let r = match cc.as_str() {
                "gcc" => build_simple("gcc", &c_path, &exe_path),
                "clang" => build_simple("clang", &c_path, &exe_path),
                "cl" => build_cl(&c_path, &exe_path),
                "llvm" => build_llvm(&c_path, &exe_path),
                other => Err(format!("不认识的 --cc: {}（支持 gcc / clang / cl / llvm）", other)),
            };
            if let Err(e) = r {
                eprintln!("{}", e);
                exit(1);
            }
            if cmd == "run" {
                let run_path = fs::canonicalize(&exe_path).unwrap_or_else(|_| exe_path.clone().into());
                let st = Command::new(run_path).status().expect("跑不起来");
                exit(st.code().unwrap_or(1));
            } else {
                println!("生成: {} （用 {}）", exe_path, cc);
                println!("头文件: {}", h_path);
            }
        }
        _ => usage(),
    }
}



