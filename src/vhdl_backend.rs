use crate::ast::*;
use crate::backend::Backend;

pub struct VhdlBackend;

impl VhdlBackend {
    pub fn new() -> Self {
        VhdlBackend
    }

    // 辅助函数：根据位宽生成 VHDL 的 STD_LOGIC 或 STD_LOGIC_VECTOR
    fn get_type(&self, width: &Option<String>) -> String {
        match width {
            Some(w_str) => {
                if let Ok(w) = w_str.parse::<usize>() {
                    if w == 1 {
                        "STD_LOGIC".to_string()
                    } else {
                        format!("STD_LOGIC_VECTOR({} downto 0)", w - 1)
                    }
                } else {
                    // 若宽度不是数字（如宏），直接带入
                    format!("STD_LOGIC_VECTOR({} - 1 downto 0)", w_str)
                }
            }
            None => "STD_LOGIC".to_string(),
        }
    }

    // 辅助函数：将 Verilog 风格字面量转为 VHDL 风格字面量
    fn convert_literal(&self, s: &str) -> String {
        // 查找 Verilog 的位宽分隔符 "'"
        if let Some(idx) = s.find('\'') {
            let width_str = &s[..idx];
            let radix_and_val = &s[idx + 1..];

            if radix_and_val.is_empty() {
                return s.to_string(); // 异常保护
            }

            let radix = &radix_and_val[0..1].to_lowercase();
            let val_str = &radix_and_val[1..];

            // 解析位宽，如果解析失败默认给个 32
            let width = width_str.parse::<usize>().unwrap_or(32);

            match radix.as_str() {
                "b" => {
                    if width == 1 {
                        format!("'{}'", val_str) // 单比特: '1'
                    } else {
                        format!("\"{}\"", val_str) // 多比特: "1010"
                    }
                }
                "h" => {
                    // 十六进制: x"FF"
                    // 注意：VHDL的十六进制字符串位宽必须是4的倍数并且和目标匹配
                    // 理想情况下这里应该做补零对齐，这里先做基础转换
                    format!("x\"{}\"", val_str)
                }
                "d" => {
                    // 十进制: 必须借用 numeric_std 库转换
                    // 例如: 8'd255 -> std_logic_vector(to_unsigned(255, 8))
                    format!("std_logic_vector(to_unsigned({}, {}))", val_str, width)
                }
                "o" => {
                    // 八进制: o"77"
                    format!("o\"{}\"", val_str)
                }
                _ => s.to_string(),
            }
        } else {
            // 没有找到 "'" 说明可能是纯数字（如 "0"）或者只是个字符串
            // 在 VHDL 中，纯数字会被当作 integer。如果是给 std_logic 赋值可能会报错，
            // 但如果是在 for 循环索引等场景则是对的。先原样返回。
            s.to_string()
        }
    }
}

impl Backend for VhdlBackend {
    fn generate(&self, modules: &[Module]) -> Result<String, String> {
        let mut output = String::new();
        for module in modules {
            output.push_str(&self.gen_module(module)?);
            output.push_str("\n\n");
        }
        Ok(output)
    }
}

impl VhdlBackend {
    fn gen_module(&self, module: &Module) -> Result<String, String> {
        let mut lines = Vec::new();

        // 1. 引入 VHDL 标准库
        lines.push("library IEEE;".to_string());
        lines.push("use IEEE.STD_LOGIC_1164.ALL;".to_string());
        lines.push("use IEEE.NUMERIC_STD.ALL;".to_string());
        lines.push("".to_string());

        // 2. 实体 (Entity) 声明
        lines.push(format!("entity {} is", module.name));
        if !module.ports.is_empty() {
            lines.push("    Port (".to_string());
            for (i, port) in module.ports.iter().enumerate() {
                let dir = match port.direction {
                    PortDir::Input => "in",
                    PortDir::Output => "out",
                    PortDir::Inout => "inout",
                };
                let typ = self.get_type(&port.width);
                let semi = if i == module.ports.len() - 1 { "" } else { ";" };
                lines.push(format!("        {} : {} {}{}", port.name, dir, typ, semi));
            }
            lines.push("    );".to_string());
        }
        lines.push(format!("end {};", module.name));
        lines.push("".to_string());

        // 3. 架构 (Architecture) 声明
        lines.push(format!("architecture Behavioral of {} is", module.name));

        // 信号声明 (包含 wires 和 regs)
        for wire in &module.wires {
            lines.push(format!("    signal {} : {};", wire.name, self.get_type(&wire.width)));
        }
        for reg in &module.regs {
            lines.push(format!("    signal {} : {};", reg.name, self.get_type(&reg.width)));
        }

        lines.push("begin".to_string());

        // 连续赋值 (Assign)
        for assign in &module.assigns {
            lines.push(format!(
                "    {} <= {};",
                self.gen_assign_target(&assign.target),
                self.gen_expr(&assign.expr)
            ));
        }

        // 进程 (Process)
        for proc in &module.processes {
            lines.push(self.gen_process(proc)?);
        }

        // 实例化 (Instance)
        for inst in &module.instances {
            lines.push(self.gen_instance(inst)?);
        }

        lines.push("end Behavioral;".to_string());
        Ok(lines.join("\n"))
    }

    fn gen_assign_target(&self, target: &AssignTarget) -> String {
        match target {
            AssignTarget::Signal(name, None) => name.clone(),
            AssignTarget::Signal(name, Some(idx)) => format!("{}({})", name, idx),
            AssignTarget::Slice(name, high, low) => format!("{}({} downto {})", name, high, low),
        }
    }

    fn gen_expr(&self, expr: &Expr) -> String {
        match expr {
            Expr::Literal(s) => self.convert_literal(s),
            Expr::Ident(s) => s.clone(),
            Expr::Slice(e, high, low) => {
                if high == low {
                    format!("{}({})", self.gen_expr(e), high)
                } else {
                    format!("{}({} downto {})", self.gen_expr(e), high, low)
                }
            }
            Expr::Concat(exprs) => {
                let parts: Vec<String> = exprs.iter().map(|e| self.gen_expr(e)).collect();
                format!("({})", parts.join(" & ")) // VHDL 用 & 做拼接
            }
            Expr::UnaryOp(op, e) => {
                let op_str = match op {
                    UnaryOp::Not => "not",
                };
                format!("{} {}", op_str, self.gen_expr(e))
            }
            Expr::BinaryOp(op, e1, e2) => {
                let op_str = match op {
                    BinaryOp::And => "and",
                    BinaryOp::Or => "or",
                    BinaryOp::Xor => "xor",
                    BinaryOp::Add => "+",
                    BinaryOp::Sub => "-",
                    BinaryOp::Mul => "*",
                    BinaryOp::Eq => "=",
                    BinaryOp::Ne => "/=",
                    BinaryOp::Lt => "<",
                    BinaryOp::Le => "<=",
                    BinaryOp::Gt => ">",
                    BinaryOp::Ge => ">=",
                    BinaryOp::Shl => "sll", // 或者 shift_left 根据库的兼容性
                    BinaryOp::Shr => "srl",
                };
                format!("({} {} {})", self.gen_expr(e1), op_str, self.gen_expr(e2))
            }
            Expr::Cond(c, t, e) => {
                // VHDL-2008 支持条件表达式
                format!(
                    "({} when {} else {})",
                    self.gen_expr(t),
                    self.gen_expr(c),
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
                Sensitivity::PosEdge(sig) => sig.clone(),
                Sensitivity::NegEdge(sig) => sig.clone(),
                Sensitivity::Level(sig) => sig.clone(),
            })
            .collect();

        let sens_str = sens_list.join(", ");
        let body = self.gen_statements(&proc.body, "        ");

        Ok(format!(
            "    process({})\n    begin\n{}    end process;",
            sens_str,
            body
        ))
    }

    fn gen_statements(&self, stmts: &[Statement], indent: &str) -> String {
        let mut lines = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::BlockWrite(target, expr) => {
                    // VHDL中 := 通常用于 variable，<= 用于 signal。
                    // 此处严格区分以保留 AST 的语义意图。
                    lines.push(format!(
                        "{}{} := {};",
                        indent,
                        self.gen_assign_target(target),
                        self.gen_expr(expr)
                    ));
                }
                Statement::NonBlockWrite(target, expr) => {
                    lines.push(format!(
                        "{}{} <= {};",
                        indent,
                        self.gen_assign_target(target),
                        self.gen_expr(expr)
                    ));
                }
                Statement::If(cond, then_body, else_body) => {
                    // 默认这里的条件已经能在 VHDL 环境中求值为 boolean 或是处理后的 std_logic。
                    lines.push(format!("{}if {} then", indent, self.gen_expr(cond)));
                    lines.push(self.gen_statements(then_body, &format!("{}    ", indent)));

                    if !else_body.is_empty() {
                        lines.push(format!("{}else", indent));
                        lines.push(self.gen_statements(else_body, &format!("{}    ", indent)));
                    }
                    lines.push(format!("{}end if;", indent));
                }
            }
        }
        if lines.is_empty() {
            String::new()
        } else {
            lines.join("\n") + "\n"
        }
    }

    fn gen_instance(&self, inst: &Instance) -> Result<String, String> {
        let mut lines = Vec::new();
        // VHDL direct instantiation (entity work.xxx)
        lines.push(format!("    {}: entity work.{}", inst.instance_name, inst.module_name));
        lines.push("        port map (".to_string());

        match &inst.port_map {
            PortMap::ByName(map) => {
                for (i, (port, expr)) in map.iter().enumerate() {
                    let comma = if i == map.len() - 1 { "" } else { "," };
                    lines.push(format!(
                        "            {} => {}{}",
                        port,
                        self.gen_expr(expr),
                        comma
                    ));
                }
            }
            PortMap::ByPosition(exprs) => {
                for (i, expr) in exprs.iter().enumerate() {
                    let comma = if i == exprs.len() - 1 { "" } else { "," };
                    lines.push(format!("            {}{}", self.gen_expr(expr), comma));
                }
            }
        }
        lines.push("        );".to_string());
        Ok(lines.join("\n"))
    }
}