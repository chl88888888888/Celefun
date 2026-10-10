// Celefun 集成测试：同一份 .celefun 走不同工具链编成 exe 再跑，校验退出码。
//
// 覆盖：
//   gcc      Strawberry GCC（MinGW）
//   clang    LLVM clang（MSVC 目标）
//   cl       MSVC cl.exe
//   llvm-ir  先 clang -emit-llvm 生成 .ll，再用 clang 把 .ll 编成 exe
//
// clang / cl / llvm-ir 需要 vcvars64.bat（见 VCVARS）。

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use celefun::{codegen_c, lexer, parser, sema};

const VCVARS: &str = r"D:\daima\vs\VC\Auxiliary\Build\vcvars64.bat";

const CASES: &[(&str, &str, i32)] = &[
    ("arith", "决心 →↘ 心 1 ↑ 2 ↗ 3 ↓ 4; ↘→", 3),
    ("modulo", "决心 →↘ 心 17 ↙ 5; ↘→", 2),
    ("incdec", "决心 →↘ 旗帜 x:草莓 →← 5; ↑↑x; ↓↓x; ↑↑x; 心 x; ↘→", 6),
    ("neg", "决心 →↘ 心 ↖ 7 ↑ 10; ↘→", 3),
    ("compare", "决心 →↘ 心门 →↘3 ←← 3 ↘↘ 4 → 3↘→ →↘ 心 1; ↘→ 没开 →↘ 心 0; ↘→ ↘→", 1),
    ("bool", "决心 →↘ 旗帜 a:磁带 →← true ↘↘ false ↙↙ true; 心门 →↘a↘→ →↘ 心 1; ↘→ 没开 →↘ 心 0; ↘→ ↘→", 1),
    ("ifelse", "决心 →↘ 旗帜 x:草莓 →← 1; 心门 →↘x → 2↘→ →↘ 心 10; ↘→ 没开 →↘ 心 20; ↘→ ↘→", 20),
    ("loop_break_continue", "决心 →↘ 旗帜 s:草莓 →← 0; 旗帜 i:草莓 →← 0; 凌波 →↘ 心门 →↘i →→ 10↘→ →↘ 狼跳; ↘→ ↑↑i; 心门 →↘i ↙ 2 ←→ 0↘→ →↘ s →← s ↑ i; ↘→ ↘→ 心 s; ↘→", 30),
    ("goto_label", "决心 →↘ 旗帜 i:草莓 →← 0; Badeline 头; ↑↑i; 心门 →↘i ← 3↘→ →↘ 金羽毛 头; ↘→ 心 i; ↘→", 3),
    ("array_grow", "决心 →↘ 旗帜 带:果冻块→↘草莓↘→; 带 →↘100000↘→ →← 7; 心 带 →↘100000↘→; ↘→", 7),
    ("array_read_oob", "决心 →↘ 旗帜 带:果冻块→↘草莓↘→; 心 带 →↘5↘→; ↘→", 0),
    ("array_sum", "决心 →↘ 旗帜 a:果冻块→↘草莓↘→; 旗帜 i:草莓 →← 0; 凌波 →↘ 心门 →↘i →→ 5↘→ →↘ 狼跳; ↘→ a →↘i↘→ →← i ↗ i; ↑↑i; ↘→ 心 a →↘4↘→; ↘→", 16),
    ("f64_eq", "决心 →↘ 旗帜 x:金草莓 →← 1.5 ↑ 2.5; 心门 →↘x ←→ 4.0↘→ →↘ 心 1; ↘→ 没开 →↘ 心 0; ↘→ ↘→", 1),
    ("array_f64", "决心 →↘ 旗帜 a:果冻块→↘金草莓↘→; a →↘3↘→ →← 2.5 ↗ 2.0; 心门 →↘a →↘3↘→ ←→ 5.0↘→ →↘ 心 1; ↘→ 没开 →↘ 心 0; ↘→ ↘→", 1),
    ("fib_rec", "第九章 →↘ 第一面:草莓 →↘n:草莓↘→ →↘ 心门 →↘n ←← 1↘→ →↘ 超级弹 n; ↘→ 超级弹 第一面 →↘n ↓ 1↘→ ↑ 第一面 →↘n ↓ 2↘→; ↘→ ↘→ 第九章第一面; 决心 →↘ 心 第一面 →↘10↘→; ↘→", 55),
    ("func_two_params", "第九章 →↘ 第一面:草莓 →↘a:草莓、b:草莓↘→ →↘ 超级弹 a ↑ b; ↘→ ↘→ 第九章第一面; 决心 →↘ 心 第一面 →↘40、2↘→; ↘→", 42),
    ("jelly_len", "决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘5↘→ →← 1; 心 果冻反抓 →↘a↘→; ↘→", 6),
    ("array_param", "第九章 →↘ 求和:草莓 →↘a:果冻块→↘草莓↘→、n:草莓↘→ →↘ 旗帜 s:草莓 →← 0; 旗帜 i:草莓 →← 0; 凌波 →↘ 心门 →↘i →→ n↘→ →↘ 狼跳; ↘→ s →← s ↑ a →↘i↘→; ↑↑i; ↘→ 超级弹 s; ↘→ ↘→ 第九章求和; 决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘0↘→ →← 1; a →↘1↘→ →← 2; a →↘2↘→ →← 3; 心 求和 →↘a、3↘→; ↘→", 6),
    ("array_return", "第九章 →↘ 造:果冻块→↘草莓↘→ →↘n:草莓↘→ →↘ 旗帜 a:果冻块→↘草莓↘→; 旗帜 i:草莓 →← 0; 凌波 →↘ 心门 →↘i →→ n↘→ →↘ 狼跳; ↘→ a →↘i↘→ →← i ↗ i; ↑↑i; ↘→ 超级弹 a; ↘→ ↘→ 第九章造; 决心 →↘ 旗帜 b:果冻块→↘草莓↘→ →← 造 →↘5↘→; 心 b →↘4↘→; ↘→", 16),
];

fn it_dir(tc: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("it-programs")
        .join(tc);
    fs::create_dir_all(&d).unwrap();
    d
}

fn write_c_files(dir: &PathBuf) {
    for (name, src, _) in CASES {
        let toks = lexer::tokenize(src)
            .unwrap_or_else(|e| panic!("{} 词法错误 {}:{}: {}", name, e.line, e.col, e.msg));
        let ast = parser::parse(&toks)
            .unwrap_or_else(|e| panic!("{} 语法错误 {}:{}: {}", name, e.line, e.col, e.msg));
        let hir = sema::check(&ast)
            .unwrap_or_else(|e| panic!("{} 语义错误 {}: {}", name, e.line, e.msg));
        fs::write(dir.join(format!("{}.c", name)), codegen_c::generate(&hir)).unwrap();
    }
}

fn run_bat(dir: &PathBuf, bat_name: &str, body: &str) {
    let mut bat = String::new();
    bat.push_str("@echo off\r\n");
    bat.push_str("cd /d \"%~dp0\"\r\n");
    bat.push_str("setlocal enabledelayedexpansion\r\n");
    bat.push_str(body);
    let bat_path = dir.join(bat_name);
    fs::write(&bat_path, bat).unwrap();
    let out = Command::new("cmd")
        .arg("/c")
        .arg(&bat_path)
        .output()
        .expect("cmd 跑不起来");
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();

    let mut got: HashMap<String, i32> = HashMap::new();
    for line in stdout.lines() {
        let p: Vec<&str> = line.split_whitespace().collect();
        if p.len() >= 3 && p[0] == "CASE" {
            if p[2] == "FAIL" {
                panic!("[{}] {} 编译/链接失败\n{}", bat_name, p[1], stdout);
            }
            if p.len() == 4 && p[2] == "EXIT" {
                got.insert(p[1].to_string(), p[3].parse().unwrap());
            }
        }
    }
    let mut missing: Vec<&str> = Vec::new();
    for (name, _, want) in CASES {
        match got.get(*name) {
            Some(v) => assert_eq!(*v, *want, "[{}] {} 期望退出码 {}，实际 {}", bat_name, name, want, v),
            None => missing.push(name),
        }
    }
    assert!(missing.is_empty(), "[{}] 这些用例没出结果: {:?}\n{}", bat_name, missing, stdout);
}

fn blocks(tc: &str, compile: &str) -> String {
    let mut b = String::new();
    for (name, _, _) in CASES {
        let c = format!("{}.c", name);
        let exe = format!("{}_{}.exe", tc, name);
        let skip = format!("{}_skip_{}", tc, name);
        b.push_str(&compile.replace("{C}", &c).replace("{EXE}", &exe));
        b.push_str(" >nul 2>&1\r\n");
        b.push_str(&format!("if errorlevel 1 (echo CASE {} FAIL & goto :{})\r\n", name, skip));
        b.push_str(&format!("{}\r\n", exe));
        b.push_str(&format!("echo CASE {} EXIT !errorlevel!\r\n", name));
        b.push_str(&format!(":{}\r\n", skip));
    }
    b
}

#[test]
fn gcc_all() {
    let dir = it_dir("gcc");
    write_c_files(&dir);
    let body = blocks("gcc", "gcc -std=c99 -O2 -o {EXE} {C}");
    run_bat(&dir, "gcc_all.bat", &body);
}

#[test]
fn clang_all() {
    let dir = it_dir("clang");
    write_c_files(&dir);
    let mut body = format!("call \"{}\" >nul 2>&1\r\n", VCVARS);
    body.push_str(&blocks("clang", "clang -std=c99 -O2 -o {EXE} {C}"));
    run_bat(&dir, "clang_all.bat", &body);
}

#[test]
fn msvc_cl_all() {
    let dir = it_dir("cl");
    write_c_files(&dir);
    let mut body = format!("call \"{}\" >nul 2>&1\r\n", VCVARS);
    body.push_str(&blocks("cl", "cl /nologo /utf-8 /std:c11 /Fe:{EXE} /Fo:{EXE}.obj {C}"));
    run_bat(&dir, "cl_all.bat", &body);
}

#[test]
fn llvm_ir_all() {
    let dir = it_dir("llvm");
    write_c_files(&dir);
    let mut body = format!("call \"{}\" >nul 2>&1\r\n", VCVARS);
    for (name, _, _) in CASES {
        let c = format!("{}.c", name);
        let ll = format!("llvm_{}.ll", name);
        let exe = format!("llvm_{}.exe", name);
        let skip = format!("llvm_skip_{}", name);
        body.push_str(&format!("clang -std=c99 -O2 -S -emit-llvm -o {} {}\r\n", ll, c));
        body.push_str(&format!("if errorlevel 1 (echo CASE {} FAIL & goto :{})\r\n", name, skip));
        body.push_str(&format!("clang -O2 -o {} {}\r\n", exe, ll));
        body.push_str(&format!("if errorlevel 1 (echo CASE {} FAIL & goto :{})\r\n", name, skip));
        body.push_str(&format!("{}\r\n", exe));
        body.push_str(&format!("echo CASE {} EXIT !errorlevel!\r\n", name));
        body.push_str(&format!(":{}\r\n", skip));
    }
    run_bat(&dir, "llvm_all.bat", &body);
}


// ---------- 标准库 L1：日记（输出） / 望远镜（输入） ----------

fn build_exe(dir: &PathBuf, name: &str, src: &str) -> PathBuf {
    let toks = lexer::tokenize(src).unwrap_or_else(|e| panic!("{} 词法错误 {}:{}: {}", name, e.line, e.col, e.msg));
    let ast = parser::parse(&toks).unwrap_or_else(|e| panic!("{} 语法错误 {}:{}: {}", name, e.line, e.col, e.msg));
    let hir = sema::check(&ast).unwrap_or_else(|e| panic!("{} 语义错误 {}: {}", name, e.line, e.msg));
    let c_path = dir.join(format!("{}.c", name));
    let exe_path = dir.join(format!("{}.exe", name));
    fs::write(&c_path, codegen_c::generate(&hir)).unwrap();
    let st = Command::new("gcc")
        .arg("-std=c99")
        .arg("-O2")
        .arg("-o")
        .arg(&exe_path)
        .arg(&c_path)
        .status()
        .expect("需要 gcc");
    assert!(st.success(), "{} 的 C 编译失败", name);
    exe_path
}

fn run_with_stdin(exe: &PathBuf, input: &str, cwd: &PathBuf) -> (i32, String) {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new(exe)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("跑不起来");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code().unwrap_or(-1), String::from_utf8_lossy(&out.stdout).to_string())
}

#[test]
fn stdlib_io() {
    let dir = it_dir("io");

    let p1 = "第十章日记;\n决心 →↘ 日记 →↘42↘→; 心 0; ↘→";
    let exe = build_exe(&dir, "print_i64", p1);
    let (code, out) = run_with_stdin(&exe, "", &dir);
    assert_eq!(code, 0);
    assert_eq!(out.replace("\r\n", "\n"), "42\n");

    let p2 = "第十章日记;\n决心 →↘ 旗帜 s:果冻块→↘草莓↘→; s →↘0↘→ →← 72; s →↘1↘→ →← 105; s →↘2↘→ →← 33; 日记 →↘s、utf8↘→; 心 0; ↘→";
    let exe = build_exe(&dir, "print_utf8", p2);
    let (code, out) = run_with_stdin(&exe, "", &dir);
    assert_eq!(code, 0);
    assert_eq!(out.replace("\r\n", "\n"), "Hi!\n");

    let p3 = "第十章望远镜;\n决心 →↘ 旗帜 x:草莓 →← 0; 望远镜 →↘x↘→; 心 x; ↘→";
    let exe = build_exe(&dir, "read_i64", p3);
    let (code, _) = run_with_stdin(&exe, "7\n", &dir);
    assert_eq!(code, 7);

    let p4 = "第十章望远镜;\n决心 →↘ 旗帜 x:金草莓 →← 0.0; 望远镜 →↘x↘→; 心门 →↘x → 2.5↘→ →↘ 心 1; ↘→ 没开 →↘ 心 0; ↘→ ↘→";
    let exe = build_exe(&dir, "read_f64", p4);
    let (code, _) = run_with_stdin(&exe, "3.5\n", &dir);
    assert_eq!(code, 1);

    let p5 = "第十章日记;\n决心 →↘ 旗帜 s:果冻块→↘草莓↘→; s →↘0↘→ →← 65; s →↘1↘→ →← 66; 旗帜 p:果冻块→↘草莓↘→; p →↘0↘→ →← 95; p →↘1↘→ →← 111; p →↘2↘→ →← 117; p →↘3↘→ →← 116; p →↘4↘→ →← 46; p →↘5↘→ →← 116; p →↘6↘→ →← 120; p →↘7↘→ →← 116; 日记 →↘s、utf8、p↘→; 心 0; ↘→";
    let exe = build_exe(&dir, "print_file", p5);
    let (code, _) = run_with_stdin(&exe, "", &dir);
    assert_eq!(code, 0);
    assert_eq!(fs::read_to_string(dir.join("_out.txt")).unwrap(), "AB\n");
}



// ---------- L4：多文件库 + C FFI ----------

#[test]
fn multi_file_and_ffi() {
    let dir = it_dir("l4");
    let bin = env!("CARGO_BIN_EXE_celefun");

    // 多文件：库文件贡献章 / 面，主文件提供决心
    let lib_src = "第九章 →↘ 求和:草莓 →↘a:果冻块→↘草莓↘→、n:草莓↘→ →↘ 旗帜 s:草莓 →← 0; 旗帜 i:草莓 →← 0; 凌波 →↘ 心门 →↘i →→ n↘→ →↘ 狼跳; ↘→ s →← s ↑ a →↘i↘→; ↑↑i; ↘→ 超级弹 s; ↘→ ↘→";
    let main_src = "第九章求和; 决心 →↘ 旗帜 a:果冻块→↘草莓↘→; a →↘0↘→ →← 1; a →↘1↘→ →← 2; a →↘2↘→ →← 3; 心 求和 →↘a、3↘→; ↘→";
    fs::write(dir.join("lib.celefun"), lib_src).unwrap();
    fs::write(dir.join("main.celefun"), main_src).unwrap();
    let st = Command::new(bin)
        .current_dir(&dir)
        .arg("run")
        .arg("main.celefun")
        .arg("lib.celefun")
        .arg("--cc")
        .arg("gcc")
        .status()
        .unwrap();
    assert_eq!(st.code().unwrap_or(-1), 6, "多文件编译 / 运行不对");

    // FFI：PICO-8 标记的面导出到 .h，且不是 static
    let ffi_src = "第九章 →↘ 加法:草莓 →↘a:草莓、b:草莓、PICO-8↘→ →↘ 超级弹 a ↑ b; ↘→ ↘→ 第九章加法;";
    fs::write(dir.join("ffi.celefun"), ffi_src).unwrap();
    let st = Command::new(bin)
        .current_dir(&dir)
        .arg("build")
        .arg("ffi.celefun")
        .arg("--cc")
        .arg("gcc")
        .status()
        .unwrap();
    assert!(st.success(), "FFI 库生成失败");

    let header = fs::read_to_string(dir.join("ffi.celefun.h")).unwrap();
    assert!(
        header.contains("int64_t cff_7b2c_4e5d_7ae0_52a0_6cd5(int64_t, int64_t)"),
        "头文件里没有 FFI 原型:\n{}",
        header
    );
    assert!(!header.contains("static int64_t cff_"), "FFI 面不该是 static");

    // 用 C 调它
    let caller = "#include \"ffi.celefun.h\"\nint main(void) { return (int)cff_7b2c_4e5d_7ae0_52a0_6cd5(40, 2); }\n";
    fs::write(dir.join("caller.c"), caller).unwrap();
    let st = Command::new("gcc")
        .current_dir(&dir)
        .args(["-std=c99", "-O2", "-o", "caller.exe", "caller.c", "ffi.celefun.c"])
        .status()
        .unwrap();
    assert!(st.success(), "C 与 FFI 链接失败");
    let st = Command::new(dir.join("caller.exe")).status().unwrap();
    assert_eq!(st.code().unwrap_or(-1), 42, "C 调 FFI 的结果不对");
}
