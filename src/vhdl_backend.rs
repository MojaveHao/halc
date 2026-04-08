use crate::ast::*;
use crate::backend::Backend;

pub struct VhdlBackend;

impl VhdlBackend {
    pub fn new() -> Self {
        VhdlBackend
    }

    /// 返回 VHDL 类型声明，例如 `STD_LOGIC` 或 `STD_LOGIC_VECTOR(7 downto 0)`
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
                    // 非数字宽度（参数化表达式）
                    format!("STD_LOGIC_VECTOR({} - 1 downto 0)", w_str)
                }
            }
            None => "STD_LOGIC".to_string(),
        }
    }

    /// 将类似 Verilog 的字面量转换为 VHDL 字面量或函数调用
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
                        // 如果所有位都是 '0'，用 (others => '0') 更简洁
                        if val_str.chars().all(|c| c == '0') {
                            "(others => '0')".to_string()
                        } else if val_str.chars().all(|c| c == '1') {
                            "(others => '1')".to_string()
                        } else {
                            format!("\"{}\"", val_str)
                        }
                    }
                }
                "h" => {
                    if val_str.chars().all(|c| c == '0') {
                        "(others => '0')".to_string()
                    } else {
                        format!("x\"{}\"", val_str)
                    }
                }
                "d" => {
                    if width == 1 {
                        match val_str {
                            "0" => "'0'".to_string(),
                            "1" => "'1'".to_string(),
                            _ => format!("'{}'", val_str),
                        }
                    } else {
                        // 对十进制 0 也使用 (others => '0')
                        if val_str == "0" {
                            "(others => '0')".to_string()
                        } else {
                            format!("std_logic_vector(to_unsigned({}, {}))", val_str, width)
                        }
                    }
                }
                "o" => {
                    if val_str.chars().all(|c| c == '0') {
                        "(others => '0')".to_string()
                    } else {
                        format!("o\"{}\"", val_str)
                    }
                }
                _ => s.to_string(),
            }
        } else {
            // 无基数前缀的纯数字，默认为十进制
            if let Ok(v) = s.parse::<i64>() {
                if v == 0 {
                    "'0'".to_string()
                } else if v == 1 {
                    "'1'".to_string()
                } else {
                    // 无宽度信息时保留原样（这种情况很少出现在正确代码中）
                    s.to_string()
                }
            } else {
                s.to_string()
            }
        }
    }
}

impl Backend for VhdlBackend {
    fn generate(&self, modules: &[Module]) -> Result<String, String> {
        let mut output = String::new();
        for (i, module) in modules.iter().enumerate() {
            if i > 0 {
                output.push_str("\n\n");
            }
            output.push_str(&self.gen_module(module)?);
        }
        Ok(output)
    }
}

impl VhdlBackend {
    fn gen_module(&self, module: &Module) -> Result<String, String> {
        let mut lines = Vec::new();

        // 库声明
        lines.push("library IEEE;".to_string());
        lines.push("use IEEE.STD_LOGIC_1164.ALL;".to_string());
        lines.push("use IEEE.NUMERIC_STD.ALL;".to_string());
        lines.push("".to_string());

        // 实体
        lines.push(format!("entity {} is", module.name));
        if !module.ports.is_empty() {
            lines.push("    port (".to_string());
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

        // 架构
        lines.push(format!("architecture Behavioral of {} is", module.name));

        // 内部信号声明
        for wire in &module.wires {
            lines.push(format!(
                "    signal {} : {};",
                wire.name,
                self.get_type(&wire.width)
            ));
        }
        for reg in &module.regs {
            lines.push(format!(
                "    signal {} : {};",
                reg.name,
                self.get_type(&reg.width)
            ));
        }

        lines.push("begin".to_string());

        // 连续赋值（并发信号赋值）
        for assign in &module.assigns {
            lines.push(format!(
                "    {} <= {};",
                self.gen_assign_target(&assign.target),
                self.gen_expr(&assign.expr)
            ));
        }

        // 进程（时序/组合逻辑）
        for proc in &module.processes {
            lines.push(self.gen_process(proc)?);
        }

        // 子模块实例化
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
            AssignTarget::Slice(name, high, low) => {
                if high == low {
                    format!("{}({})", name, high)
                } else {
                    format!("{}({} downto {})", name, high, low)
                }
            }
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
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        let l = self.gen_expr_unsigned(e1);   // 生成可安全用于 unsigned() 的表达式
                        let r = self.gen_expr_unsigned(e2);
                        let op_str = match op {
                            BinaryOp::Add => "+",
                            BinaryOp::Sub => "-",
                            BinaryOp::Mul => "*",
                            _ => unreachable!(),
                        };
                        format!("std_logic_vector({} {} {})", l, op_str, r)
                    }
                    BinaryOp::Shl => {
                        format!("std_logic_vector(shift_left(unsigned({}), {}))", l, r)
                    }
                    BinaryOp::Shr => {
                        format!("std_logic_vector(shift_right(unsigned({}), {}))", l, r)
                    }
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
                    "{} when {} else {}",
                    self.gen_expr(t),
                    self.gen_expr(c),
                    self.gen_expr(e)
                )
            }
        }
    }

    /// 生成 process 语句，正确处理时钟边沿和异步复位
    fn gen_process(&self, proc: &Process) -> Result<String, String> {
        // 分类敏感性列表
        let mut posedge_signals = Vec::new();
        let mut negedge_signals = Vec::new();
        let mut level_signals = Vec::new();

        for sens in &proc.sensitivity {
            match sens {
                Sensitivity::PosEdge(s) => posedge_signals.push(s.clone()),
                Sensitivity::NegEdge(s) => negedge_signals.push(s.clone()),
                Sensitivity::Level(s) => level_signals.push(s.clone()),
            }
        }

        // 构建敏感性列表字符串
        let mut sens_list = Vec::new();
        sens_list.extend(posedge_signals.iter().cloned());
        sens_list.extend(negedge_signals.iter().cloned());
        sens_list.extend(level_signals.iter().cloned());
        let sens_str = sens_list.join(", ");

        // 判断是否为时序逻辑（有边沿敏感）
        let has_edges = !posedge_signals.is_empty() || !negedge_signals.is_empty();

        let body = if has_edges {
            // 简单约定：如果同时存在上升沿和下降沿，将第一个上升沿视为时钟，第一个下降沿视为异步复位
            // 更精确的分析应来自前端，这里提供一个健壮的默认模板
            let clk_sig = posedge_signals
                .first()
                .or_else(|| negedge_signals.first())
                .cloned();
            let rst_sig = if posedge_signals.len() >= 2 {
                posedge_signals.get(1).cloned()
            } else if !negedge_signals.is_empty() && !posedge_signals.is_empty() {
                // 有上升沿也有下降沿，将下降沿视为复位
                negedge_signals.first().cloned()
            } else {
                None
            };

            if let Some(clk) = clk_sig {
                let clock_cond = if posedge_signals.contains(&clk) {
                    format!("rising_edge({})", clk)
                } else {
                    format!("falling_edge({})", clk)
                };

                let (reset_stmts, clock_stmts) =
                    self.partition_reset_clock_body(&proc.body, rst_sig.as_ref());

                let mut lines = Vec::new();
                let indent = "        ";

                if let Some(rst) = rst_sig {
                    // 异步复位模板
                    let rst_cond = if posedge_signals.contains(&rst) {
                        format!("{} = '1'", rst)
                    } else {
                        format!("{} = '0'", rst)
                    };
                    lines.push(format!("{}if {} then", indent, rst_cond));
                    if !reset_stmts.is_empty() {
                        lines.push(self.gen_statements(&reset_stmts, &format!("{}    ", indent)));
                    } else {
                        lines.push(format!("{}    null; -- reset branch", indent));
                    }
                    lines.push(format!("{}elsif {} then", indent, clock_cond));
                } else {
                    lines.push(format!("{}if {} then", indent, clock_cond));
                }

                if !clock_stmts.is_empty() {
                    lines.push(self.gen_statements(&clock_stmts, &format!("{}    ", indent)));
                } else {
                    lines.push(format!("{}    null; -- clocked branch", indent));
                }
                lines.push(format!("{}end if;", indent));
                lines.join("\n")
            } else {
                // 只有边沿但无信号？退化为组合逻辑
                self.gen_statements(&proc.body, "        ")
            }
        } else {
            // 纯组合逻辑 process
            self.gen_statements(&proc.body, "        ")
        };

        Ok(format!(
            "    process({})\n    begin\n{}{}    end process;",
            sens_str,
            body,
            if body.is_empty() { "" } else { "\n" }
        ))
    }

    /// 从过程体中分离复位分支和时钟分支
    /// 返回 (复位语句列表, 时钟语句列表)
    fn partition_reset_clock_body(
        &self,
        stmts: &[Statement],
        rst_sig: Option<&String>,
    ) -> (Vec<Statement>, Vec<Statement>) {
        if rst_sig.is_none() {
            return (Vec::new(), stmts.to_vec());
        }
        let rst = rst_sig.unwrap();

        // 尝试识别常见的 if (rst_condition) ... else ... 模式
        if stmts.len() == 1 {
            if let Statement::If(cond, then_stmts, else_stmts) = &stmts[0] {
                if self.is_reset_condition(cond, rst) {
                    return (then_stmts.clone(), else_stmts.clone());
                }
            }
        }
        // 默认：所有语句都放在时钟分支，复位分支为空
        (Vec::new(), stmts.to_vec())
    }

    /// 判断表达式是否为复位条件（rst = '1' 或 rst = '0' 或 not rst 等）
    fn is_reset_condition(&self, expr: &Expr, rst: &str) -> bool {
        match expr {
            Expr::Ident(s) => s == rst, // 直接写 rst，视为高有效复位
            Expr::UnaryOp(UnaryOp::Not, e) => {
                if let Expr::Ident(s) = e.as_ref() {
                    s == rst // not rst 视为低有效复位
                } else {
                    false
                }
            }
            Expr::BinaryOp(BinaryOp::Eq, left, right) => match (left.as_ref(), right.as_ref()) {
                (Expr::Ident(s), Expr::Literal(lit)) if s == rst => lit == "1",
                (Expr::Literal(lit), Expr::Ident(s)) if s == rst => lit == "1",
                _ => false,
            },
            Expr::BinaryOp(BinaryOp::Ne, left, right) => match (left.as_ref(), right.as_ref()) {
                (Expr::Ident(s), Expr::Literal(lit)) if s == rst => lit == "0",
                (Expr::Literal(lit), Expr::Ident(s)) if s == rst => lit == "0",
                _ => false,
            },
            _ => false,
        }
    }

    /// 生成顺序语句块，返回带缩进的 VHDL 代码
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
        lines.push(format!(
            "    {}: entity work.{}",
            inst.instance_name, inst.module_name
        ));
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

    /// 生成一个适合放入 unsigned() 的表达式（即已去除外层的 std_logic_vector 包装）
    fn gen_expr_unsigned(&self, expr: &Expr) -> String {
        match expr {
            Expr::Ident(s) => format!("unsigned({})", s),
            Expr::Literal(s) => {
                // 字面量不能直接用于 unsigned，需要先转为 unsigned 字面量或保留原转换
                // 但 VHDL 不允许 unsigned 字面量，所以保持 to_unsigned 调用
                let width = self.extract_width_from_literal(s).unwrap_or(32);
                let val_str = self.extract_value_from_literal(s);
                format!("to_unsigned({}, {})", val_str, width)
            }
            Expr::Slice(e, high, low) => {
                // 切片结果也是 std_logic_vector，需要 unsigned 转换
                format!("unsigned({})", self.gen_expr(expr))
            }
            Expr::Concat(_) => {
                format!("unsigned({})", self.gen_expr(expr))
            }
            Expr::UnaryOp(_, _) => {
                format!("unsigned({})", self.gen_expr(expr))
            }
            Expr::BinaryOp(op, e1, e2) => {
                // 如果内部已经是算术运算，直接递归生成 unsigned 表达式，避免重复包装
                match op {
                    BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul => {
                        let l = self.gen_expr_unsigned(e1);
                        let r = self.gen_expr_unsigned(e2);
                        let op_str = match op {
                            BinaryOp::Add => "+",
                            BinaryOp::Sub => "-",
                            BinaryOp::Mul => "*",
                            _ => unreachable!(),
                        };
                        format!("({} {} {})", l, op_str, r)
                    }
                    _ => {
                        format!("unsigned({})", self.gen_expr(expr))
                    }
                }
            }
            Expr::Cond(c, t, e) => {
                // 条件表达式结果也是 std_logic_vector
                format!("unsigned({})", self.gen_expr(expr))
            }
        }
    }

    /// 从字面量字符串中提取宽度（如 "8'd0" -> 8）
    fn extract_width_from_literal(&self, s: &str) -> Option<usize> {
        s.find('\'').and_then(|idx| s[..idx].parse::<usize>().ok())
    }

    /// 从字面量字符串中提取数值部分（如 "8'd0" -> "0"）
    fn extract_value_from_literal(&self, s: &str) -> String {
        if let Some(idx) = s.find('\'') {
            let radix_and_val = &s[idx + 1..];
            if radix_and_val.len() > 1 {
                radix_and_val[1..].to_string()
            } else {
                "0".to_string()
            }
        } else {
            s.to_string()
        }
    }
}
