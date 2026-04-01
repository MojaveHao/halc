use crate::ast::*;
use crate::backend::Backend;

pub struct VerilogBackend;

impl VerilogBackend {
    pub fn new() -> Self {
        VerilogBackend
    }
}

impl Backend for VerilogBackend {
    fn generate(&self, modules: &[Module]) -> Result<String, String> {
        let mut output = String::new();
        for module in modules {
            output.push_str(&self.gen_module(module)?);
            output.push('\n');
        }
        Ok(output)
    }
}

impl VerilogBackend {
    fn gen_module(&self, module: &Module) -> Result<String, String> {
        let mut lines = Vec::new();
        lines.push(format!("module {} (", module.name));
        // 端口列表
        let port_names: Vec<&str> = module.ports.iter().map(|p| p.name.as_str()).collect();
        lines.push(format!("    {} );", port_names.join(", ")));

        for port in &module.ports {
            let mut parts = Vec::new();
            parts.push(match port.direction {
                PortDir::Input => "input".to_string(),
                PortDir::Output => "output".to_string(),
                PortDir::Inout => "inout".to_string(),
            });
            if port.reg {
                parts.push("reg".to_string());
            }
            if let Some(w_str) = &port.width {
                if let Ok(w) = w_str.parse::<usize>() {
                    parts.push(format!("[{}:0]", w - 1));
                } else {
                    return Err(format!("Invalid width '{}' for port {}", w_str, port.name));
                }
            }
            parts.push(port.name.clone());
            lines.push(format!("    {};", parts.join(" ")));
        }

        for wire in &module.wires {
            let mut parts = vec!["wire".to_string()];
            if let Some(w_str) = &wire.width {
                if let Ok(w) = w_str.parse::<usize>() {
                    parts.push(format!("[{}:0]", w - 1));
                } else {
                    // 如果宽度不是数字，可能是未展开的符号，报错
                    return Err(format!(
                        "Invalid width '{}' for signal {}",
                        w_str, wire.name
                    ));
                }
            }
            parts.push(wire.name.clone());
            lines.push(format!("    {};", parts.join(" ")));
        }

        for reg in &module.regs {
            let mut parts = vec!["reg".to_string()];
            if let Some(w_str) = &reg.width {
                if let Ok(w) = w_str.parse::<usize>() {
                    parts.push(format!("[{}:0]", w - 1));
                } else {
                    // 若宽度不是数字（如宏未展开），直接使用原字符串
                    parts.push(format!("[{}:0]", w_str));
                }
            }
            parts.push(reg.name.clone());
            lines.push(format!("    {};", parts.join(" ")));
        }

        // 连续赋值
        for assign in &module.assigns {
            lines.push(format!(
                "    assign {} = {};",
                self.gen_assign_target(&assign.target),
                self.gen_expr(&assign.expr)
            ));
        }

        // 进程
        for proc in &module.processes {
            lines.push(self.gen_process(proc)?);
        }

        // 子模块实例化
        for inst in &module.instances {
            lines.push(self.gen_instance(inst)?);
        }

        lines.push("endmodule".to_string());
        Ok(lines.join("\n"))
    }

    fn gen_assign_target(&self, target: &AssignTarget) -> String {
        match target {
            AssignTarget::Signal(name, None) => name.clone(),
            AssignTarget::Signal(name, Some(idx)) => format!("{}[{}]", name, idx),
            AssignTarget::Slice(name, high, low) => format!("{}[{}:{}]", name, high, low),
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(s) => s.clone(),
            Expr::Ident(s) => s.clone(),
            Expr::Slice(e, high, low) => {
                if high == low {
                    format!("{}[{}]", self.gen_expr(e), high)
                } else {
                    format!("{}[{}:{}]", self.gen_expr(e), high, low)
                }
            }
            Expr::Concat(exprs) => {
                let parts: Vec<String> = exprs.iter().map(|e| self.gen_expr(e)).collect();
                format!("{{{}}}", parts.join(", "))
            }
            Expr::UnaryOp(op, e) => {
                let op_str = match op {
                    UnaryOp::Not => "~",
                };
                format!("{}({})", op_str, self.gen_expr(e))
            }
            Expr::BinaryOp(op, e1, e2) => {
                let op_str = match op {
                    BinaryOp::And => "&",
                    BinaryOp::Or => "|",
                    BinaryOp::Xor => "^",
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Eq => "==",
                    BinaryOp::Ne => "!=",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::Shl => "<<",
                    BinaryOp::Shr => ">>",
                };
                format!("({} {} {})", self.gen_expr(e1), op_str, self.gen_expr(e2))
            }
            Expr::Cond(c, t, e) => {
                format!(
                    "({} ? {} : {})",
                    self.gen_expr(c),
                    self.gen_expr(t),
                    self.gen_expr(e)
                )
            }
        }
    }

    fn gen_process(&self, proc: &Process) -> Result<String, String> {
        let sens_list: Vec<String> = proc
            .sensitivity
            .iter()
            .map(|s| match s {
                Sensitivity::PosEdge(sig) => format!("posedge {}", sig),
                Sensitivity::NegEdge(sig) => format!("negedge {}", sig),
                Sensitivity::Level(sig) => sig.clone(),
            })
            .collect();
        let sens_str = sens_list.join(" or ");
        let body = self.gen_statements(&proc.body);
        Ok(format!(
            "    always @({}) begin\n{}{}    end",
            sens_str,
            body,
            if body.is_empty() { "" } else { "\n" }
        ))
    }

    fn gen_statements(&self, stmts: &[Statement]) -> String {
        let mut lines = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::BlockWrite(target, expr) => {
                    lines.push(format!(
                        "        {} = {};",
                        self.gen_assign_target(target),
                        self.gen_expr(expr)
                    ));
                }
                Statement::NonBlockWrite(target, expr) => {
                    lines.push(format!(
                        "        {} <= {};",
                        self.gen_assign_target(target),
                        self.gen_expr(expr)
                    ));
                }
                Statement::If(cond, then_body, else_body) => {
                    lines.push(format!("        if ({}) begin", self.gen_expr(cond)));
                    lines.extend(
                        self.gen_statements(then_body)
                            .lines()
                            .map(|l| format!("    {}", l)),
                    );
                    lines.push("        end".to_string());
                    if !else_body.is_empty() {
                        lines.push("        else begin".to_string());
                        lines.extend(
                            self.gen_statements(else_body)
                                .lines()
                                .map(|l| format!("    {}", l)),
                        );
                        lines.push("        end".to_string());
                    }
                }
            }
        }
        lines.join("\n")
    }

    fn gen_instance(&self, inst: &Instance) -> Result<String, String> {
        let mut lines = Vec::new();
        lines.push(format!("    {} {} (", inst.module_name, inst.instance_name));
        match &inst.port_map {
            PortMap::ByName(map) => {
                for (i, (port, expr)) in map.iter().enumerate() {
                    let comma = if i == map.len() - 1 { "" } else { "," };
                    lines.push(format!(
                        "        .{}({}){}",
                        port,
                        self.gen_expr(expr),
                        comma
                    ));
                }
            }
            PortMap::ByPosition(exprs) => {
                for (i, expr) in exprs.iter().enumerate() {
                    let comma = if i == exprs.len() - 1 { "" } else { "," };
                    lines.push(format!("        {}{}", self.gen_expr(expr), comma));
                }
            }
        }
        lines.push("    );".to_string());
        Ok(lines.join("\n"))
    }
}
