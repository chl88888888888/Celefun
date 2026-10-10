# 1-celefun-rust

## 文件夹作用
本文件夹是「Celefun」——一门关键字取自《蔚蓝》(Celeste) 操作技巧与实体、运算符取自蔚蓝八方向冲刺的**整蛊编程语言**——的编译器工程。

整蛊指的是**语言外观荒诞**（满屏蔚蓝梗、箭头当标点），不是语义坑人。
设计红线：**零隐藏语义** —— 无随机、无隐式延迟、无「偷偷改结果」的全局量。

- 实现语言：Rust（文件夹名已按工作区契约标注语言）
- 源文件扩展名：`.celefun`（UTF-8 纯文本）
- 括号：→↘（左）与 ↘→（右）；条件 = 心门 / 没开；跳转 = 金羽毛 / Badeline；终止 = 踩刺
- 标点：、 分隔，; 结束，: 类型标注；写入 = →←（二者合一），相等 = ←→
- 注释：↑↑↑ ... ↓↓↓
- 类型：草莓 = i64，金草莓 = f64，磁带 = bool；布尔字面量 true / false
- 容器：果冻块 数组（引用语义，越界写自动扩容、越界读返回 0）；长度 = 果冻反抓
- 模块与函数：章 = 模块，面 = 函数；面可以收数组参数、可以返回数组；返回 = 超级弹
- 循环：只有 凌波；break = 狼跳，continue = 奥欻
- 变量：旗帜 名字:类型 →← 值;（声明）；写入直接用 名字 →← 值;
- 标准库：第十章（要导入）；日记 = 标准输出，望远镜 = 标准输入
- 多文件：多个 .celefun 一起编译，库文件只出章 / 面，主文件出决心
- C FFI：面用 →↘PICO-8↘→ 标记就导出，并生成头文件
- 编译方式：前端 Rust 生成 C99，交给 gcc / clang / cl / LLVM；生成的 C 是纯 ASCII
- 产物：原生可执行文件（或 FFI 库的 .c / .h）

## 目录说明
| 路径 | 作用 |
| --- | --- |
| `立项计划书.md` | 立项计划、语言设计、里程碑、风险 |
| `标准库设计.md` | 标准库与 FFI 的分期与 API |
| `src/` | 编译器实现：lexer / parser / sema / hir / codegen_c |
| `examples/` | .celefun 示例（含库文件、FFI 库） |
| `tests/` | 集成测试（真调工具链编译运行） |

## 当前状态
`cargo test` 共 30 项（24 单元 + 6 集成）。

集成测试覆盖：
- 19 个 .celefun 分别用 gcc / clang / cl(MSVC) / LLVM IR 编成 exe 再跑，校验退出码
- 标准库：打印数字、UTF-8 打印、读入 i64 / f64、输出到文件
- L4：多文件库编译、FFI 面导出、C 头文件、C 侧链接并调用

支持：草莓(i64)、金草莓(f64，int 自动提升)、磁带(bool)、果冻块(数组，引用语义)、章 / 面、导入、心门 / 没开、凌波、狼跳 / 奥欻、金羽毛 / Badeline、超级弹、踩刺、心、日记、望远镜、果冻反抓、面收数组参数 / 返回数组、多文件库、C FFI。

还没做：优化。标准库到此为止——不做数学 / 容器 / 字符串库。

## 用法

    cargo build
    cargo test
    cargo run -- check examples/example.celefun
    cargo run -- run   examples/print.celefun
    cargo run -- run   examples/uselib.celefun examples/mathlib.celefun
    cargo run -- build examples/ffi_add.celefun [--cc gcc|clang|cl|llvm]

多文件：库文件只贡献章 / 面，主文件提供决心（只能有一个决心）。

C FFI：面参数表里加 PICO-8 就会导出，并生成 <主文件>.h：

    第九章 →↘
        加法:草莓 →↘a:草莓、b:草莓、PICO-8↘→ →↘
            超级弹 a ↑ b;
        ↘→
    ↘→

    // C 侧
    #include "ffi_add.celefun.h"
    return (int)cff_00000000(40, 2);   // 实际名字见头文件

