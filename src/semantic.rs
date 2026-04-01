use crate::ast::*;
use std::collections::HashSet;

pub fn check_module(module: &Module) -> Result<(), String> {
    // 检查端口名称唯一性
    let mut port_names = HashSet::new();
    for port in &module.ports {
        if port_names.contains(&port.name) {
            return Err(format!("Duplicate port name: {}", port.name));
        }
        port_names.insert(port.name.clone());
    }

    // 检查内部信号名称不冲突
    let mut sig_names = HashSet::new();
    for wire in &module.wires {
        if sig_names.contains(&wire.name) {
            return Err(format!("Duplicate wire: {}", wire.name));
        }
        sig_names.insert(wire.name.clone());
    }
    for reg in &module.regs {
        if sig_names.contains(&reg.name) {
            return Err(format!("Duplicate reg: {}", reg.name));
        }
        sig_names.insert(reg.name.clone());
    }

    // 检查 assign 和 process 中使用的信号是否已声明
    for assign in &module.assigns {
        check_expr_signals(&assign.expr, &port_names, &sig_names)?;
        check_assign_target(&assign.target, &port_names, &sig_names)?;
    }
    for proc in &module.processes {
        for stmt in &proc.body {
            check_statement(stmt, &port_names, &sig_names)?;
        }
    }

    Ok(())
}

fn check_expr_signals(expr: &Expr, ports: &HashSet<String>, signals: &HashSet<String>) -> Result<(), String> {
    match expr {
        Expr::Ident(name) => {
            if !ports.contains(name) && !signals.contains(name) {
                return Err(format!("Undeclared signal: {}", name));
            }
        }
        Expr::Slice(e, _, _) => check_expr_signals(e, ports, signals)?,
        Expr::Concat(exprs) => {
            for e in exprs {
                check_expr_signals(e, ports, signals)?;
            }
        }
        Expr::UnaryOp(_, e) => check_expr_signals(e, ports, signals)?,
        Expr::BinaryOp(_, e1, e2) => {
            check_expr_signals(e1, ports, signals)?;
            check_expr_signals(e2, ports, signals)?;
        }
        Expr::Cond(c, t, e) => {
            check_expr_signals(c, ports, signals)?;
            check_expr_signals(t, ports, signals)?;
            check_expr_signals(e, ports, signals)?;
        }
        _ => {}
    }
    Ok(())
}

fn check_assign_target(target: &AssignTarget, ports: &HashSet<String>, signals: &HashSet<String>) -> Result<(), String> {
    match target {
        AssignTarget::Signal(name, _) => {
            if !ports.contains(name) && !signals.contains(name) {
                return Err(format!("Undeclared target: {}", name));
            }
        }
        AssignTarget::Slice(name, _, _) => {
            if !ports.contains(name) && !signals.contains(name) {
                return Err(format!("Undeclared target slice: {}", name));
            }
        }
    }
    Ok(())
}

fn check_statement(stmt: &Statement, ports: &HashSet<String>, signals: &HashSet<String>) -> Result<(), String> {
    match stmt {
        Statement::BlockWrite(target, expr) | Statement::NonBlockWrite(target, expr) => {
            check_assign_target(target, ports, signals)?;
            check_expr_signals(expr, ports, signals)?;
        }
        Statement::If(cond, then_body, else_body) => {
            check_expr_signals(cond, ports, signals)?;
            for s in then_body {
                check_statement(s, ports, signals)?;
            }
            for s in else_body {
                check_statement(s, ports, signals)?;
            }
        }
    }
    Ok(())
}