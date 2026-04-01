use std::error::Error;
use std::fs;
use std::time::Instant;

mod ast;
mod backend;
mod lexer;
mod macro_expander;
mod parser;
mod semantic;
mod verilog_backend;
pub mod vhdl_backend;

use backend::Backend;
use verilog_backend::VerilogBackend;
use vhdl_backend::VhdlBackend; // 引入 VhdlBackend

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = Instant::now();
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        // 更新了 usage 提示
        eprintln!("usage: halc <input.hal> [--vhdl]");
        std::process::exit(1);
    }

    let input_file = &args[1];

    // 判断是否传入了 --vhdl 参数
    let use_vhdl = args.iter().any(|arg| arg == "--vhdl");

    // 根据选择的 Backend 替换不同的文件后缀
    let output_file = if use_vhdl {
        input_file.replace(".hal", ".vhd")
    } else {
        input_file.replace(".hal", ".v")
    };

    // 顶部摘要
    println!("halc: compiling '{}'", input_file);

    // 步骤追踪（使用标准的缩进和进度标记）
    println!("  [1/4] parsing s-expressions...");
    let source = fs::read_to_string(input_file)?;
    let tokens = lexer::tokenize(&source)?;
    let sexprs = parser::parse_sexprs(tokens)?;

    println!("  [2/4] expanding macros...");
    let expanded = macro_expander::expand(sexprs)?;

    println!("  [3/4] analyzing semantics...");
    let modules = ast::parse_modules(expanded)?;
    for module in &modules {
        semantic::check_module(module)?;
    }

    // 根据选择执行相应的 Backend 生成逻辑
    if use_vhdl {
        println!("  [4/4] emitting vhdl...");
        let backend = VhdlBackend::new();
        let output = backend.generate(&modules)?;
        fs::write(&output_file, output)?;
    } else {
        println!("  [4/4] emitting verilog...");
        let backend = VerilogBackend::new();
        let output = backend.generate(&modules)?;
        fs::write(&output_file, output)?;
    }

    // 底部总结
    let elapsed = start_time.elapsed();
    println!("build complete: -> {} ({:?})", output_file, elapsed);

    Ok(())
}