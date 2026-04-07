use crate::ast::*;
use crate::backend::Backend;

pub struct VhdlBackend;

impl VhdlBackend {
    pub fn new() -> Self {
        VhdlBackend
    }

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
                    format!("STD_LOGIC_VECTOR({} - 1 downto 0)", w_str)
                }
            }
            None => "STD_LOGIC".to_string(),
        }
    }

    fn convert_literal(&self, s: &str) -> String {
        if let Some(idx) = s.find('\'') {
            let width_str = &s[..idx];
            let radix_and_val = &s[idx + 1..];
            if radix_and_val.is_empty() {
                return s.to_string();
            }
            let radix = &radix_and_val[0..1].to_lowercase();
            let val_str = &radix_and_val[1..];
            let width = width_str.parse::<usize>().unwrap_or(32);
            match radix.as_str() {
                "b" => {
                    if width == 1 {
                        format!("'{}'", val_str)
                    } else {
                        format!("\"{}\"", val_str)
                    }
                }
                "h" => format!("x\"{}\"", val_str),
                "d" => format!("std_logic_vector(to_unsigned({}, {}))", val_str, width),
                "o" => format!("o\"{}\"", val_str),
                _ => s.to_string(),
            }
        } else {
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
        lines.push("library IEEE;".to_string());
        lines.push("use IEEE.STD_LOGIC_1164.ALL;".to_string());
        lines.push("use IEEE.NUMERIC_STD.ALL;".to_string());
        lines.push("".to_string());

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

        lines.push(format!("architecture Behavioral of {} is", module.name));
        for wire in &module.wires {
            lines.push(format!("    signal {} : {};", wire.name, self.get_type(&wire.width)));
        }
        for reg in &module.regs {
            lines.push(format!("    signal {} : {};", reg.name, self.get_type(&reg.width)));
        }
        lines.push("begin".to_string());

        for assign in &module.assigns {
            lines.push(format!(
                "    {} <= {};",
                self.gen_assign_target(&assign.target),
                self.gen_expr(&assign.expr)
            ));
        }

        for proc in &module.processes {
            lines.push(self.gen_process(proc)?);
        }

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
                format!("({})", parts.join(" & "))
            }
            Expr::UnaryOp(op, e) => {
                let op_str = match op {
                    UnaryOp::Not => "not",
                };
                format!("{} {}", op_str, self.gen_expr(e))
            }
            Expr::BinaryOp(op, e1, e2) => {
                let l = self.gen_expr(e1);
                let r = self.gen_expr(e2);
                match op {
                    // 算术运算：需要转换为 unsigned 进行运算，再转回 std_logic_vector
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        let op_str = match op {
                            BinaryOp::Add => "+",
                            BinaryOp::Sub => "-",
                            BinaryOp::Mul => "*",
                            _ => unreachable!(),
                        };
                        format!("std_logic_vector(unsigned({}) {} unsigned({}))", l, op_str, r)
                    }
                    BinaryOp::Shl => {
                        format!("std_logic_vector(shift_left(unsigned({}), {}))", l, r)
                    }
                    BinaryOp::Shr => {
                        format!("std_logic_vector(shift_right(unsigned({}), {}))", l, r)
                    }
                    // 逻辑运算和比较运算：直接使用
                    _ => {
                        let op_str = match op {
                            BinaryOp::And => "and",
                            BinaryOp::Or => "or",
                            BinaryOp::Xor => "xor",
                            BinaryOp::Eq => "=",
                            BinaryOp::Ne => "/=",
                            BinaryOp::Lt => "<",
                            BinaryOp::Le => "<=",
                            BinaryOp::Gt => ">",
                            BinaryOp::Ge => ">=",
                            _ => unreachable!(),
                        };
                        format!("({} {} {})", l, op_str, r)
                    }
                }
            }
            Expr::Cond(c, t, e) => {
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
        let mut clk_edge = None;
        let mut rst_edge = None;
        let mut level_signals = Vec::new();

        for sens in &proc.sensitivity {
            match sens {
                Sensitivity::PosEdge(s) => {
                    let sig = s.to_lowercase();
                    if sig.contains("clk") || sig.contains("clock") {
                        clk_edge = Some((s.clone(), true));
                    } else if sig.contains("rst") || sig.contains("reset") {
                        rst_edge = Some((s.clone(), true));
                    } else {
                        level_signals.push(s.clone());
                    }
                }
                Sensitivity::NegEdge(s) => {
                    let sig = s.to_lowercase();
                    if sig.contains("clk") || sig.contains("clock") {
                        clk_edge = Some((s.clone(), false));
                    } else if sig.contains("rst") || sig.contains("reset") {
                        rst_edge = Some((s.clone(), false));
                    } else {
                        level_signals.push(s.clone());
                    }
                }
                Sensitivity::Level(s) => {
                    level_signals.push(s.clone());
                }
            }
        }

        let mut sensitivity_list = Vec::new();
        if let Some((clk, _)) = &clk_edge {
            sensitivity_list.push(clk.clone());
        }
        if let Some((rst, _)) = &rst_edge {
            sensitivity_list.push(rst.clone());
        }
        sensitivity_list.extend(level_signals);
        let sens_str = sensitivity_list.join(", ");

        let indent = "        ";
        let body_code = if clk_edge.is_some() {
            let (clk_sig, clk_pos) = clk_edge.as_ref().unwrap();
            let clock_cond = if *clk_pos {
                format!("rising_edge({})", clk_sig)
            } else {
                format!("falling_edge({})", clk_sig)
            };

            let (reset_stmts, clock_stmts) = self.extract_reset_and_clock_bodies(&proc.body, rst_edge.as_ref());

            let mut lines = Vec::new();

            if let Some((rst_sig, rst_pos)) = rst_edge {
                let reset_cond = if rst_pos {
                    format!("{} = '1'", rst_sig)
                } else {
                    format!("{} = '0'", rst_sig)
                };
                lines.push(format!("{}if {} then", indent, reset_cond));
                if !reset_stmts.is_empty() {
                    lines.push(self.gen_statements(&reset_stmts, &format!("{}    ", indent)));
                } else {
                    lines.push(format!("{}    -- No reset assignments provided", indent));
                }
                lines.push(format!("{}elsif {} then", indent, clock_cond));
            } else {
                lines.push(format!("{}if {} then", indent, clock_cond));
            }

            if !clock_stmts.is_empty() {
                lines.push(self.gen_statements(&clock_stmts, &format!("{}    ", indent)));
            } else {
                lines.push(format!("{}    -- No clocked statements", indent));
            }
            lines.push(format!("{}end if;", indent));

            lines.join("\n")
        } else {
            self.gen_statements(&proc.body, indent)
        };

        Ok(format!(
            "    process({})\n    begin\n{}{}    end process;",
            sens_str,
            body_code,
            if body_code.is_empty() { "" } else { "\n" }
        ))
    }

    fn extract_reset_and_clock_bodies(
        &self,
        stmts: &[Statement],
        rst_edge: Option<&(String, bool)>,
    ) -> (Vec<Statement>, Vec<Statement>) {
        if stmts.len() != 1 {
            return (Vec::new(), stmts.to_vec());
        }
        match &stmts[0] {
            Statement::If(cond, then_stmts, else_stmts) => {
                let is_reset_condition = match (cond, rst_edge) {
                    (Expr::UnaryOp(UnaryOp::Not, e), Some((rst_sig, pos))) => {
                        if let Expr::Ident(s) = e.as_ref() {
                            s == rst_sig && !pos
                        } else {
                            false
                        }
                    }
                    (Expr::BinaryOp(BinaryOp::Eq, e1, e2), Some((rst_sig, pos))) => {
                        let (left, right) = (e1.as_ref(), e2.as_ref());
                        match (left, right) {
                            (Expr::Ident(s), Expr::Literal(lit)) if s == rst_sig => {
                                if *pos {
                                    lit == "1"
                                } else {
                                    lit == "0"
                                }
                            }
                            (Expr::Literal(lit), Expr::Ident(s)) if s == rst_sig => {
                                if *pos {
                                    lit == "1"
                                } else {
                                    lit == "0"
                                }
                            }
                            _ => false,
                        }
                    }
                    _ => false,
                };
                if is_reset_condition {
                    (then_stmts.clone(), else_stmts.clone())
                } else {
                    (Vec::new(), stmts.to_vec())
                }
            }
            _ => (Vec::new(), stmts.to_vec()),
        }
    }

    fn gen_statements(&self, stmts: &[Statement], indent: &str) -> String {
        let mut lines = Vec::new();
        for stmt in stmts {
            match stmt {
                Statement::BlockWrite(target, expr) => {
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