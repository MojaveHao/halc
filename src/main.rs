use std::error::Error;
use std::fs;
use std::time::Instant;

mod ast;
mod backend;
mod error;
mod lexer;
mod macro_expander;
mod parser;
mod semantic;
mod verilog_backend;
pub mod vhdl_backend;

use backend::Backend;
use verilog_backend::VerilogBackend;
use vhdl_backend::VhdlBackend;

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = Instant::now();
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("usage: halc <input.hal> [--vhdl]");
        std::process::exit(1);
    }

    let input_file = &args[1];
    let use_vhdl = args.iter().any(|arg| arg == "--vhdl");

    let output_file = if use_vhdl {
        input_file.replace(".hal", ".vhd")
    } else {
        input_file.replace(".hal", ".v")
    };

    println!("halc: compiling '{}'", input_file);

    let step_start = Instant::now();
    print!("  [1/4] parsing s-expressions...");
    let source = fs::read_to_string(input_file)?;
    let tokens = lexer::tokenize(&source, input_file).unwrap_or_else(|diag| {
        diag.display(&source);
        std::process::exit(2);
    });
    let sexprs = parser::parse_sexprs(tokens, input_file).unwrap_or_else(|diag| {
        diag.display(&source);
        std::process::exit(3);
    });
    println!(" done in {:.3} s", step_start.elapsed().as_secs_f64());

    let step_start = Instant::now();
    print!("  [2/4] expanding macros...");
    let expanded = macro_expander::expand(sexprs, input_file).unwrap_or_else(|diag| {
        diag.display(&source);
        std::process::exit(3);
    });
    println!(" done in {:.3} s", step_start.elapsed().as_secs_f64());

    let step_start = Instant::now();
    print!("  [3/4] analyzing semantics...");
    let modules = ast::parse_modules(expanded, input_file).unwrap_or_else(|diag| {
        diag.display(&source);
        std::process::exit(3);
    });
    for module in &modules {
        semantic::check_module(module)?;
    }
    println!(" done in {:.3} s", step_start.elapsed().as_secs_f64());

    let step_start = Instant::now();
    if use_vhdl {
        print!("  [4/4] emitting vhdl...");
        let backend = VhdlBackend::new();
        let output = backend.generate(&modules)?;
        fs::write(&output_file, output)?;
    } else {
        print!("  [4/4] emitting verilog...");
        let backend = VerilogBackend::new();
        let output = backend.generate(&modules)?;
        fs::write(&output_file, output)?;
    }
    println!(" done in {:.3} s", step_start.elapsed().as_secs_f64());

    // 底部总结
    let elapsed = start_time.elapsed();
    println!("build complete: -> {} ({:.3} s)", output_file, elapsed.as_secs_f64());

    Ok(())
}
