use crate::ast::*;
use crate::backend::Backend;

pub struct VerilogBackend;

impl VerilogBackend {
    pub fn new() -> Self {
        VerilogBackend
    }

    /// 将宽度字符串转换为 Verilog 向量范围（如 "8" -> "[7:0]"）
    fn format_width(width: Option<&String>) -> String {
        if let Some(w) = width {
            if let Ok(n) = w.parse::<usize>() {
                if n > 1 {
                    format!("[{}:0]", n - 1)
                } else {
                    String::new()
                }
            } else {
                // 非数字宽度（如参数化表达式）原样保留
                format!("[{}:0]", w)
            }
        } else {
            String::new()
        }
    }
}

impl Backend for VerilogBackend {
    fn generate(&self, modules: &[Module]) -> Result<String, String> {
        let mut output = String::new();
        for (i, module) in modules.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }
            output.push_str(&self.gen_module(module)?);
        }
        Ok(output)
    }
}

impl VerilogBackend {
    fn gen_module(&self, module: &Module) -> Result<String, String> {
        let mut lines = Vec::new();

        // 模块头部：端口列表 (ANSI-C 风格)
        let port_list = self.gen_port_list(module);
        lines.push(format!("module {} {}", module.name, port_list));

        // Wire 声明
        for wire in &module.wires {
            let width_str = Self::format_width(wire.width.as_ref());
            lines.push(format!("    wire{}{};",
                               if width_str.is_empty() { " ".to_string() } else { format!(" {} ", width_str) },
                               wire.name));
        }

        // Reg 声明（包括非端口 reg）
        for reg in &module.regs {
            let width_str = Self::format_width(reg.width.as_ref());
            lines.push(format!("    reg{}{};",
                               if width_str.is_empty() { " ".to_string() } else { format!(" {} ", width_str) },
                               reg.name));
        }

        // 连续赋值
        for assign in &module.assigns {
            lines.push(format!("    assign {} = {};",
                               self.gen_assign_target(&assign.target),
                               self.gen_expr(&assign.expr)));
        }

        // 进程（always 块）
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

    /// 生成 ANSI-C 风格的端口列表，例如：
    /// (input clk, input [7:0] addr, output reg [15:0] data)
    fn gen_port_list(&self, module: &Module) -> String {
        if module.ports.is_empty() {
            return "();".to_string();
        }

        let port_strings: Vec<String> = module
            .ports
            .iter()
            .map(|p| {
                let mut parts = Vec::new();
                // 方向
                parts.push(match p.direction {
                    PortDir::Input => "input",
                    PortDir::Output => "output",
                    PortDir::Inout => "inout",
                }.to_string());

                // output reg 需要 reg 关键字
                if p.direction == PortDir::Output && p.reg {
                    parts.push("reg".to_string());
                }

                // 宽度
                let width_str = Self::format_width(p.width.as_ref());
                if !width_str.is_empty() {
                    parts.push(width_str);
                }

                // 端口名
                parts.push(p.name.clone());

                parts.join(" ")
            })
            .collect();

        format!("(\n    {}\n);", port_strings.join(",\n    "))
    }

    fn gen_assign_target(&self, target: &AssignTarget) -> String {
        match target {
            AssignTarget::Signal(name, None) => name.clone(),
            AssignTarget::Signal(name, Some(idx)) => format!("{}[{}]", name, idx),
            AssignTarget::Slice(name, high, low) => {
                if high == low {
                    format!("{}[{}]", name, high)
                } else {
                    format!("{}[{}:{}]", name, high, low)
                }
            }
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
                // 避免不必要的括号，简单处理为 ~expr
                format!("~{}", self.gen_expr(e))
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
        let sens_items: Vec<String> = proc
            .sensitivity
            .iter()
            .map(|s| match s {
                Sensitivity::PosEdge(sig) => format!("posedge {}", sig),
                Sensitivity::NegEdge(sig) => format!("negedge {}", sig),
                Sensitivity::Level(sig) => sig.clone(),
            })
            .collect();

        let sens_str = if sens_items.len() == 1 {
            sens_items[0].clone()
        } else {
            sens_items.join(" or ")
        };

        let body_str = self.gen_statements(&proc.body);
        let mut lines = vec![format!("    always @({}) begin", sens_str)];
        for stmt_line in body_str.lines() {
            lines.push(format!("        {}", stmt_line));
        }
        lines.push("    end".to_string());
        Ok(lines.join("\n"))
    }

    /// 生成语句块，返回字符串，每行语句已去除外层缩进，由调用者添加缩进
    fn gen_statements(&self, stmts: &[Statement]) -> String {
        let mut out = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::BlockWrite(target, expr) => {
                    out.push(format!("{} = {};",
                                     self.gen_assign_target(target),
                                     self.gen_expr(expr)));
                }
                Statement::NonBlockWrite(target, expr) => {
                    out.push(format!("{} <= {};",
                                     self.gen_assign_target(target),
                                     self.gen_expr(expr)));
                }
                Statement::If(cond, then_body, else_body) => {
                    out.push(format!("if ({}) begin", self.gen_expr(cond)));
                    // 嵌套缩进：在已有基础上再加一级
                    for line in self.gen_statements(then_body).lines() {
                        out.push(format!("    {}", line));
                    }
                    if !else_body.is_empty() {
                        out.push("end else begin".to_string());
                        for line in self.gen_statements(else_body).lines() {
                            out.push(format!("    {}", line));
                        }
                    }
                    out.push("end".to_string());
                }
            }
        }
        out.join("\n")
    }

    fn gen_instance(&self, inst: &Instance) -> Result<String, String> {
        let mut lines = vec![format!("    {} {} (", inst.module_name, inst.instance_name)];
        match &inst.port_map {
            PortMap::ByName(map) => {
                for (i, (port, expr)) in map.iter().enumerate() {
                    let comma = if i == map.len() - 1 { "" } else { "," };
                    lines.push(format!("        .{}({}){}", port, self.gen_expr(expr), comma));
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
