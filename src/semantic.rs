use crate::ast::*;
use std::collections::HashSet;

pub fn check_module(module: &Module) -> Result<(), String> {
    // ========== 第一步：收集并分类所有标识符 ==========
    let mut ports = HashSet::new(); // 所有端口名
    let mut input_ports = HashSet::new(); // 输入端口名
    let mut output_ports = HashSet::new(); // 输出端口名
    let mut inout_ports = HashSet::new(); // 双向端口名
    let mut reg_set = HashSet::new(); // 可过程赋值的信号（reg 或 output reg 端口）
    let mut wire_set = HashSet::new(); // 可连续赋值的信号（wire 或 output 非 reg 端口）

    // 处理端口
    for port in &module.ports {
        if !ports.insert(port.name.clone()) {
            return Err(format!("Duplicate port name: {}", port.name));
        }
        match port.direction {
            PortDir::Input => {
                input_ports.insert(port.name.clone());
                // 输入端口不能被赋值，不加入 reg_set 或 wire_set
            }
            PortDir::Output => {
                output_ports.insert(port.name.clone());
                if port.reg {
                    reg_set.insert(port.name.clone());
                } else {
                    wire_set.insert(port.name.clone());
                }
            }
            PortDir::Inout => {
                inout_ports.insert(port.name.clone());
                // inout 端口通常视为 wire，允许连续赋值
                wire_set.insert(port.name.clone());
            }
        }
    }

    // 处理内部 wire
    let mut wire_names = HashSet::new();
    for wire in &module.wires {
        // 检查与端口重名
        if ports.contains(&wire.name) {
            return Err(format!(
                "Wire '{}' conflicts with a port of the same name",
                wire.name
            ));
        }
        if !wire_names.insert(wire.name.clone()) {
            return Err(format!("Duplicate wire: {}", wire.name));
        }
        wire_set.insert(wire.name.clone());
    }

    // 处理内部 reg
    let mut reg_names = HashSet::new();
    for reg in &module.regs {
        if ports.contains(&reg.name) {
            return Err(format!(
                "Reg '{}' conflicts with a port of the same name",
                reg.name
            ));
        }
        if !reg_names.insert(reg.name.clone()) {
            return Err(format!("Duplicate reg: {}", reg.name));
        }
        // reg 不应出现在 wire_set 中
        if wire_set.contains(&reg.name) {
            return Err(format!("Reg '{}' already declared as wire", reg.name));
        }
        reg_set.insert(reg.name.clone());
    }

    // 构建所有已声明信号的集合（用于表达式中的标识符检查）
    let all_signals: HashSet<String> = wire_set
        .union(&reg_set)
        .cloned()
        .chain(ports.iter().cloned())
        .collect();

    // ========== 第二步：检查连续赋值 (assign) ==========
    for assign in &module.assigns {
        // 检查赋值目标（assign 左侧必须是 wire 集合中的信号，且不能是 input）
        check_assign_target(&assign.target, &input_ports, &wire_set, true)?;
        // 检查右侧表达式中的信号
        check_expr_signals(&assign.expr, &all_signals)?;
    }

    // ========== 第三步：检查进程 (process) ==========
    for proc in &module.processes {
        // 检查敏感列表中的信号是否已声明
        for sens in &proc.sensitivity {
            let sig = match sens {
                Sensitivity::PosEdge(s) | Sensitivity::NegEdge(s) | Sensitivity::Level(s) => s,
            };
            if !all_signals.contains(sig) {
                return Err(format!("Undeclared signal in sensitivity list: '{}'", sig));
            }
        }

        // 检查过程体中的每一条语句
        for stmt in &proc.body {
            check_statement(stmt, &all_signals, &input_ports, &wire_set, &reg_set)?;
        }
    }

    // ========== 第四步：检查子模块实例化 ==========
    for inst in &module.instances {
        match &inst.port_map {
            PortMap::ByName(map) => {
                for (port_name, expr) in map {
                    // 注意：这里不检查 port_name 是否存在，因为它是目标模块的端口，可能在后续才检查
                    check_expr_signals(expr, &all_signals)?;
                }
            }
            PortMap::ByPosition(exprs) => {
                for expr in exprs {
                    check_expr_signals(expr, &all_signals)?;
                }
            }
        }
    }

    // 可选：检查多重驱动？过于复杂，暂略

    Ok(())
}

// 检查表达式中引用的所有标识符是否已声明
fn check_expr_signals(expr: &Expr, declared: &HashSet<String>) -> Result<(), String> {
    match expr {
        Expr::Literal(_) => {}
        Expr::Ident(name) => {
            if !declared.contains(name) {
                return Err(format!("Undeclared identifier: '{}'", name));
            }
        }
        Expr::Slice(e, _, _) => check_expr_signals(e, declared)?,
        Expr::Concat(exprs) => {
            for e in exprs {
                check_expr_signals(e, declared)?;
            }
        }
        Expr::UnaryOp(_, e) => check_expr_signals(e, declared)?,
        Expr::BinaryOp(_, e1, e2) => {
            check_expr_signals(e1, declared)?;
            check_expr_signals(e2, declared)?;
        }
        Expr::Cond(c, t, e) => {
            check_expr_signals(c, declared)?;
            check_expr_signals(t, declared)?;
            check_expr_signals(e, declared)?;
        }
    }
    Ok(())
}

// 检查赋值目标（用于 assign 或过程赋值语句）
// is_continuous: true 表示连续赋值 (assign)，false 表示过程赋值
fn check_assign_target(
    target: &AssignTarget,
    input_ports: &HashSet<String>,
    allowed_set: &HashSet<String>, // 对于 assign 是 wire_set，对于过程是 reg_set
    is_continuous: bool,
) -> Result<(), String> {
    let name = match target {
        AssignTarget::Signal(n, _) => n,
        AssignTarget::Slice(n, _, _) => n,
    };

    // 禁止对输入端口赋值（无论连续还是过程）
    if input_ports.contains(name) {
        return Err(format!("Cannot assign to input port '{}'", name));
    }

    // 检查目标类型是否匹配赋值上下文
    if !allowed_set.contains(name) {
        let expected = if is_continuous {
            "wire (or non-reg output port)"
        } else {
            "reg (or output reg port)"
        };
        return Err(format!(
            "Assignment target '{}' must be a {} in this context",
            name, expected
        ));
    }

    Ok(())
}

// 检查过程体内的语句
fn check_statement(
    stmt: &Statement,
    declared: &HashSet<String>,
    input_ports: &HashSet<String>,
    wire_set: &HashSet<String>,
    reg_set: &HashSet<String>,
) -> Result<(), String> {
    match stmt {
        Statement::BlockWrite(target, expr) | Statement::NonBlockWrite(target, expr) => {
            // 过程赋值目标必须是 reg 集合中的信号
            check_assign_target(target, input_ports, reg_set, false)?;
            check_expr_signals(expr, declared)?;
        }
        Statement::If(cond, then_body, else_body) => {
            check_expr_signals(cond, declared)?;
            for s in then_body {
                check_statement(s, declared, input_ports, wire_set, reg_set)?;
            }
            for s in else_body {
                check_statement(s, declared, input_ports, wire_set, reg_set)?;
            }
        }
    }
    Ok(())
}
