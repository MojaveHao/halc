use crate::parser::SExpr;

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
    pub ports: Vec<Port>,
    pub wires: Vec<Signal>,
    pub regs: Vec<Signal>,
    pub processes: Vec<Process>,
    pub assigns: Vec<Assign>,
    pub instances: Vec<Instance>,
}

#[derive(Debug, Clone)]
pub struct Port {
    pub direction: PortDir,
    pub name: String,
    pub width: Option<String>, // None 表示 1
    pub reg: bool,             // 是否是 output reg
}

#[derive(Debug, Clone, PartialEq)]
pub enum PortDir {
    Input,
    Output,
    Inout,
}

#[derive(Debug, Clone)]
pub struct Signal {
    pub name: String,
    pub width: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Process {
    pub sensitivity: Vec<Sensitivity>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub enum Sensitivity {
    PosEdge(String),
    NegEdge(String),
    Level(String),
}

#[derive(Debug, Clone)]
pub enum Statement {
    BlockWrite(AssignTarget, Expr),
    NonBlockWrite(AssignTarget, Expr),
    If(Expr, Vec<Statement>, Vec<Statement>),
    // 可扩展更多语句
}

#[derive(Debug, Clone)]
pub struct Assign {
    pub target: AssignTarget,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct Instance {
    pub module_name: String,
    pub instance_name: String,
    pub port_map: PortMap,
}

#[derive(Debug, Clone)]
pub enum PortMap {
    ByName(Vec<(String, Expr)>),
    ByPosition(Vec<Expr>),
}

#[derive(Debug, Clone)]
pub enum AssignTarget {
    Signal(String, Option<usize>), // 信号名，可选位索引
    Slice(String, usize, usize),   // 切片 [high:low]
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(String),                // 数字字面量
    Ident(String),                  // 信号名
    Slice(Box<Expr>, usize, usize), // 位选或切片
    Concat(Vec<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),
    BinaryOp(BinaryOp, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>), // cond ? then : else
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    And, Or, Xor,
    Add, Sub, Mul,
    Eq, Ne, Lt, Le, Gt, Ge,
    Shl, Shr,
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub message: String,
    pub file: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn display(&self, source: &str) {
        let lines: Vec<&str> = source.lines().collect();
        let line_content = lines.get(self.span.line - 1).unwrap_or(&"");
        println!("error: {}", self.message);
        println!(" --> {}:{}:{}", self.file, self.span.line, self.span.column);
        println!(" |");
        println!("{} | {}", self.span.line, line_content);
        let mut indicator = String::new();
        for _ in 0..(self.span.column - 1) {
            indicator.push(' ');
        }
        indicator.push('^');
        println!(
            "{} | {}",
            " ".repeat(self.span.line.to_string().len()),
            indicator
        );
    }
}

// 从 S‑表达式构建 Module
pub fn parse_modules(sexprs: Vec<SExpr>) -> Result<Vec<Module>, String> {
    let mut modules = Vec::new();
    for sexpr in sexprs {
        if let SExpr::List(list) = sexpr {
            if let Some(SExpr::Atom(first)) = list.first() {
                if first == "module" {
                    modules.push(parse_module(list)?);
                } else {
                    return Err(format!("Unexpected top-level form: {}", first));
                }
            } else {
                return Err("Expected module definition".to_string());
            }
        } else {
            return Err("Expected list at top level".to_string());
        }
    }
    Ok(modules)
}

fn parse_module(list: Vec<SExpr>) -> Result<Module, String> {
    if list.len() < 3 {
        return Err("Invalid module definition".to_string());
    }
    let name = match &list[1] {
        SExpr::Atom(s) => s.clone(),
        _ => return Err("Module name must be an atom".to_string()),
    };
    let mut ports = Vec::new();
    let mut wires = Vec::new();
    let mut regs = Vec::new();
    let mut processes = Vec::new();
    let mut assigns = Vec::new();
    let mut instances = Vec::new();

    for form in &list[2..] {
        match form {
            SExpr::List(items) => {
                if items.is_empty() {
                    continue;
                }
                match &items[0] {
                    SExpr::Atom(s) => match s.as_str() {
                        "ports" => ports = parse_ports(items)?,
                        "wire" => wires.push(parse_signal(items)?),
                        "reg" => regs.push(parse_signal(items)?),
                        "process" => processes.push(parse_process(items)?),
                        "assign" => assigns.push(parse_assign(items)?),
                        "instance" => instances.push(parse_instance(items)?),
                        _ => return Err(format!("Unknown form: {}", s)),
                    },
                    _ => return Err("Expected keyword".to_string()),
                }
            }
            _ => return Err("Expected list form".to_string()),
        }
    }
    Ok(Module {
        name,
        ports,
        wires,
        regs,
        processes,
        assigns,
        instances,
    })
}

fn parse_port(p: &[SExpr]) -> Result<Vec<Port>, String> {
    if p.len() < 2 {
        return Err("Invalid port declaration".to_string());
    }
    let mut direction = PortDir::Input;
    let mut reg = false;
    let mut names = Vec::new();
    let mut width = None;

    // 方向关键字
    match &p[0] {
        SExpr::Atom(s) => match s.as_str() {
            "input" => direction = PortDir::Input,
            "output" => direction = PortDir::Output,
            "inout" => direction = PortDir::Inout,
            _ => return Err(format!("Unknown port direction: {}", s)),
        },
        _ => return Err("Expected direction keyword".to_string()),
    }

    // 跳过方向，处理 reg 和端口名
    let mut iter = p[1..].iter();
    if let Some(next) = iter.next() {
        match next {
            SExpr::Atom(s) if s == "reg" => {
                reg = true;
                // 后续所有原子都是端口名（最后一个可能是宽度）
                for token in iter {
                    if let SExpr::Atom(name) = token {
                        names.push(name.clone());
                    } else {
                        return Err("Expected port name".to_string());
                    }
                }
            }
            SExpr::Atom(s) => {
                names.push(s.clone());
                for token in iter {
                    if let SExpr::Atom(name) = token {
                        names.push(name.clone());
                    } else {
                        return Err("Expected port name".to_string());
                    }
                }
            }
            _ => return Err("Expected port name".to_string()),
        }
    } else {
        return Err("Missing port name".to_string());
    }

    // 检查最后一个名称是否为宽度（数字或包含 ' 的宽度字面量）
    if let Some(last) = names.last() {
        if last.chars().next().map_or(false, |c| c.is_ascii_digit()) || last.contains('\'') {
            width = Some(last.clone());
            names.pop();
        }
    }

    // 为每个名称生成一个端口
    let mut ports = Vec::new();
    for name in names {
        ports.push(Port {
            direction: direction.clone(),
            name,
            width: width.clone(),
            reg,
        });
    }
    Ok(ports)
}

// 辅助函数：递归收集所有端口声明
fn collect_ports(sexprs: &[SExpr]) -> Result<Vec<Port>, String> {
    let mut ports = Vec::new();
    for sexpr in sexprs {
        match sexpr {
            SExpr::List(p) => {
                if let Some(SExpr::Atom(first)) = p.first() {
                    if matches!(first.as_str(), "input" | "output" | "inout") {
                        let new_ports = parse_port(p)?;
                        ports.extend(new_ports);
                    } else {
                        ports.extend(collect_ports(p)?);
                    }
                } else {
                    ports.extend(collect_ports(p)?);
                }
            }
            SExpr::Atom(_) => {}
        }
    }
    Ok(ports)
}

fn parse_ports(items: &[SExpr]) -> Result<Vec<Port>, String> {
    // items 是 (ports ...) 列表，第一个元素是 "ports"，所以从索引 1 开始处理
    collect_ports(&items[1..])
}

fn parse_signal(items: &[SExpr]) -> Result<Signal, String> {
    if items.len() < 2 {
        return Err("Missing signal name".to_string());
    }
    let name = match &items[1] {
        SExpr::Atom(s) => s.clone(),
        _ => return Err("Signal name must be atom".to_string()),
    };
    let width = if items.len() >= 3 {
        match &items[2] {
            SExpr::Atom(s) => Some(s.clone()), // 存储为字符串
            _ => return Err("Width must be atom".to_string()),
        }
    } else {
        None
    };
    Ok(Signal { name, width })
}

fn parse_process(items: &[SExpr]) -> Result<Process, String> {
    if items.len() < 3 {
        return Err("Invalid process".to_string());
    }
    let sensitivity = parse_sensitivity(&items[1])?;
    let body = parse_statements(&items[2..])?;
    Ok(Process { sensitivity, body })
}

fn parse_sensitivity(sexpr: &SExpr) -> Result<Vec<Sensitivity>, String> {
    match sexpr {
        SExpr::List(list) => {
            let mut sens = Vec::new();
            // 如果第一个元素是 "or"，则跳过它，处理剩余部分
            let items = if let Some(SExpr::Atom(first)) = list.first() {
                if first == "or" {
                    &list[1..]
                } else {
                    list.as_slice()
                }
            } else {
                list.as_slice()
            };
            for item in items {
                match item {
                    SExpr::List(sublist) => {
                        if sublist.len() != 2 {
                            return Err("Invalid sensitivity event".to_string());
                        }
                        match &sublist[0] {
                            SExpr::Atom(s) => match s.as_str() {
                                "posedge" => {
                                    if let SExpr::Atom(sig) = &sublist[1] {
                                        sens.push(Sensitivity::PosEdge(sig.clone()));
                                    } else {
                                        return Err("Expected signal name".to_string());
                                    }
                                }
                                "negedge" => {
                                    if let SExpr::Atom(sig) = &sublist[1] {
                                        sens.push(Sensitivity::NegEdge(sig.clone()));
                                    } else {
                                        return Err("Expected signal name".to_string());
                                    }
                                }
                                _ => return Err(format!("Unknown event type: {}", s)),
                            },
                            _ => return Err("Expected event type".to_string()),
                        }
                    }
                    SExpr::Atom(sig) => {
                        sens.push(Sensitivity::Level(sig.clone()));
                    }
                    _ => return Err("Invalid sensitivity item".to_string()),
                }
            }
            Ok(sens)
        }
        _ => Err("Sensitivity list must be a list".to_string()),
    }
}

fn parse_statements(sexprs: &[SExpr]) -> Result<Vec<Statement>, String> {
    let mut stmts = Vec::new();
    for sexpr in sexprs {
        match sexpr {
            SExpr::List(list) => {
                if list.is_empty() {
                    continue;
                }
                match &list[0] {
                    SExpr::Atom(s) => match s.as_str() {
                        "block-write" => {
                            if list.len() != 3 {
                                return Err("Invalid block-write".to_string());
                            }
                            let target = parse_assign_target(&list[1])?;
                            let expr = parse_expr(&list[2])?;
                            stmts.push(Statement::BlockWrite(target, expr));
                        }
                        "nb-write" => {
                            if list.len() != 3 {
                                return Err("Invalid nb-write".to_string());
                            }
                            let target = parse_assign_target(&list[1])?;
                            let expr = parse_expr(&list[2])?;
                            stmts.push(Statement::NonBlockWrite(target, expr));
                        }
                        "if" => {
                            if list.len() < 3 {
                                return Err("Invalid if statement".to_string());
                            }
                            let cond = parse_expr(&list[1])?;
                            // then 分支（第二个元素）
                            let then_stmt = parse_statements(&[list[2].clone()])?;
                            let else_stmt = if list.len() >= 4 {
                                // else 分支（第三个元素）
                                parse_statements(&[list[3].clone()])?
                            } else {
                                vec![]
                            };
                            stmts.push(Statement::If(cond, then_stmt, else_stmt));
                        }
                        "begin" => {
                            // 将 begin 块内的语句展开
                            let inner_stmts = parse_statements(&list[1..])?;
                            stmts.extend(inner_stmts);
                        }
                        _ => return Err(format!("Unknown statement: {}", s)),
                    },
                    _ => return Err("Expected statement keyword".to_string()),
                }
            }
            _ => return Err("Statement must be a list".to_string()),
        }
    }
    Ok(stmts)
}

fn parse_assign_target(sexpr: &SExpr) -> Result<AssignTarget, String> {
    match sexpr {
        SExpr::List(list) => {
            if list.len() >= 2 {
                match &list[0] {
                    SExpr::Atom(s) if s == "signal" => {
                        let name = match &list[1] {
                            SExpr::Atom(n) => n.clone(),
                            _ => return Err("Expected signal name".to_string()),
                        };
                        let idx = if list.len() >= 3 {
                            match &list[2] {
                                SExpr::Atom(n) => {
                                    Some(n.parse::<usize>().map_err(|_| "Invalid index")?)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        Ok(AssignTarget::Signal(name, idx))
                    }
                    SExpr::Atom(s) if s == "slice" => {
                        if list.len() != 4 {
                            return Err("Invalid slice: (slice sig high low)".to_string());
                        }
                        let name = match &list[1] {
                            SExpr::Atom(n) => n.clone(),
                            _ => return Err("Expected signal name".to_string()),
                        };
                        let high = match &list[2] {
                            SExpr::Atom(n) => n.parse::<usize>().map_err(|_| "Invalid high")?,
                            _ => return Err("Expected high index".to_string()),
                        };
                        let low = match &list[3] {
                            SExpr::Atom(n) => n.parse::<usize>().map_err(|_| "Invalid low")?,
                            _ => return Err("Expected low index".to_string()),
                        };
                        Ok(AssignTarget::Slice(name, high, low))
                    }
                    _ => Err("Unknown assign target".to_string()),
                }
            } else {
                Err("Invalid assign target".to_string())
            }
        }
        SExpr::Atom(s) => Ok(AssignTarget::Signal(s.clone(), None)),
        _ => Err("Invalid assign target".to_string()),
    }
}

fn parse_expr(sexpr: &SExpr) -> Result<Expr, String> {
    match sexpr {
        SExpr::Atom(s) => {
            // 尝试解析为数字字面量，否则为标识符
            if s.chars().next().map_or(false, |c| c.is_ascii_digit()) || s.contains('\'') {
                Ok(Expr::Literal(s.clone()))
            } else {
                Ok(Expr::Ident(s.clone()))
            }
        }
        SExpr::List(list) => {
            if list.is_empty() {
                return Err("Empty expression".to_string());
            }
            match &list[0] {
                SExpr::Atom(op) => {
                    match op.as_str() {
                        "slice" => {
                            if list.len() != 4 {
                                return Err("Invalid slice expression".to_string());
                            }
                            let expr = parse_expr(&list[1])?;
                            let high = match &list[2] {
                                SExpr::Atom(n) => n.parse::<usize>().map_err(|_| "Invalid high")?,
                                _ => return Err("Expected high index".to_string()),
                            };
                            let low = match &list[3] {
                                SExpr::Atom(n) => n.parse::<usize>().map_err(|_| "Invalid low")?,
                                _ => return Err("Expected low index".to_string()),
                            };
                            Ok(Expr::Slice(Box::new(expr), high, low))
                        }
                        "concat" => {
                            let mut exprs = Vec::new();
                            for e in &list[1..] {
                                exprs.push(parse_expr(e)?);
                            }
                            Ok(Expr::Concat(exprs))
                        }
                        "signal" => {
                            if list.len() < 2 {
                                return Err("signal requires at least a name".to_string());
                            }
                            let name = match &list[1] {
                                SExpr::Atom(s) => s.clone(),
                                _ => return Err("signal name must be an atom".to_string()),
                            };
                            if list.len() >= 3 {
                                let idx = match &list[2] {
                                    SExpr::Atom(s) => {
                                        s.parse::<usize>().map_err(|_| "Invalid index")?
                                    }
                                    _ => return Err("index must be a number".to_string()),
                                };
                                Ok(Expr::Slice(Box::new(Expr::Ident(name)), idx, idx))
                            } else {
                                Ok(Expr::Ident(name))
                            }
                        }
                        "not" => {
                            if list.len() != 2 {
                                return Err("not requires one argument".to_string());
                            }
                            Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(parse_expr(&list[1])?)))
                        }
                        "and" | "or" | "xor" | "+" | "-" | "*" | "==" | "!=" | ">" | "<" | ">="
                        | "<=" | "<<" | ">>" => {
                            if list.len() != 3 {
                                return Err("Binary operator requires two arguments".to_string());
                            }
                            let op = match op.as_str() {
                                "and" => BinaryOp::And,
                                "or" => BinaryOp::Or,
                                "xor" => BinaryOp::Xor,
                                "+" => BinaryOp::Add,
                                "-" => BinaryOp::Sub,
                                "*" => BinaryOp::Mul,
                                "==" => BinaryOp::Eq,
                                "!=" => BinaryOp::Ne,
                                ">" => BinaryOp::Gt,
                                "<" => BinaryOp::Lt,
                                ">=" => BinaryOp::Ge,
                                "<=" => BinaryOp::Le,
                                "<<" => BinaryOp::Shl,
                                ">>" => BinaryOp::Shr,
                                _ => unreachable!(),
                            };
                            Ok(Expr::BinaryOp(
                                op,
                                Box::new(parse_expr(&list[1])?),
                                Box::new(parse_expr(&list[2])?),
                            ))
                        }
                        "cond" => {
                            // (cond (condition then) (else else_expr))
                            if list.len() != 3 {
                                return Err("cond requires two branches".to_string());
                            }
                            let cond_branch = match &list[1] {
                                SExpr::List(l) if l.len() == 2 => {
                                    (parse_expr(&l[0])?, parse_expr(&l[1])?)
                                }
                                _ => return Err("Invalid condition branch".to_string()),
                            };
                            let else_branch = match &list[2] {
                                SExpr::List(l)
                                    if l.len() == 2
                                        && matches!(&l[0], SExpr::Atom(s) if s == "else") =>
                                {
                                    parse_expr(&l[1])?
                                }
                                _ => return Err("Invalid else branch".to_string()),
                            };
                            Ok(Expr::Cond(
                                Box::new(cond_branch.0),
                                Box::new(cond_branch.1),
                                Box::new(else_branch),
                            ))
                        }
                        _ => {
                            // 函数调用？暂时不支持，当作标识符处理
                            // 但这里可能是子表达式
                            Err(format!("Unknown operator: {}", op))
                        }
                    }
                }
                _ => Err("Expected operator".to_string()),
            }
        }
    }
}

fn parse_assign(items: &[SExpr]) -> Result<Assign, String> {
    if items.len() != 3 {
        return Err("Invalid assign".to_string());
    }
    let target = parse_assign_target(&items[1])?;
    let expr = parse_expr(&items[2])?;
    Ok(Assign { target, expr })
}

fn parse_instance(items: &[SExpr]) -> Result<Instance, String> {
    if items.len() < 4 {
        return Err("Invalid instance".to_string());
    }
    let module_name = match &items[1] {
        SExpr::Atom(s) => s.clone(),
        _ => return Err("Module name must be atom".to_string()),
    };
    let instance_name = match &items[2] {
        SExpr::Atom(s) => s.clone(),
        _ => return Err("Instance name must be atom".to_string()),
    };
    let port_map = match &items[3] {
        SExpr::List(list) => {
            // 如果列表非空且第一个元素是 port-map，则跳过该关键字
            let map_items = if !list.is_empty() {
                match &list[0] {
                    SExpr::Atom(s) if s == "port-map" => &list[1..],
                    _ => list.as_slice(),
                }
            } else {
                &[]
            };
            let mut by_name = Vec::new();
            for mapping in map_items {
                match mapping {
                    SExpr::List(m) => {
                        if m.len() != 2 {
                            return Err("Invalid port mapping".to_string());
                        }
                        let port = match &m[0] {
                            SExpr::Atom(s) => s.clone(),
                            _ => return Err("Port name must be atom".to_string()),
                        };
                        let expr = parse_expr(&m[1])?;
                        by_name.push((port, expr));
                    }
                    _ => return Err("Port map must be list of pairs".to_string()),
                }
            }
            PortMap::ByName(by_name)
        }
        _ => return Err("Port map must be a list".to_string()),
    };
    Ok(Instance {
        module_name,
        instance_name,
        port_map,
    })
}
