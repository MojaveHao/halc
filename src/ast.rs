use crate::error::{Diagnostic, Span};
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
    Signal(String, Option<usize>),
    Slice(String, usize, usize),
}

#[derive(Debug, Clone)]
pub enum Expr {
    Literal(String),
    Ident(String),
    Slice(Box<Expr>, usize, usize),
    Concat(Vec<Expr>),
    UnaryOp(UnaryOp, Box<Expr>),
    BinaryOp(BinaryOp, Box<Expr>, Box<Expr>),
    Cond(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug, Clone)]
pub enum BinaryOp {
    And,
    Or,
    Xor,
    Add,
    Sub,
    Mul,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Shl,
    Shr,
}

// 从 S‑表达式构建 Module（带位置信息）
pub fn parse_modules(sexprs: Vec<SExpr>, file_name: &str) -> Result<Vec<Module>, Diagnostic> {
    let mut modules = Vec::new();
    for sexpr in sexprs {
        match sexpr {
            SExpr::List(list, span) => {
                if let Some(SExpr::Atom(first, _)) = list.first() {
                    if first == "module" {
                        modules.push(parse_module(list, span, file_name)?);
                    } else {
                        return Err(Diagnostic::new(
                            format!("Unexpected top-level form: {}", first),
                            file_name.to_string(),
                            span,
                        ));
                    }
                } else {
                    return Err(Diagnostic::new(
                        "Expected module definition".to_string(),
                        file_name.to_string(),
                        span,
                    ));
                }
            }
            SExpr::Atom(_, span) => {
                return Err(Diagnostic::new(
                    "Expected list at top level".to_string(),
                    file_name.to_string(),
                    span,
                ));
            }
        }
    }
    Ok(modules)
}

fn parse_module(list: Vec<SExpr>, span: Span, file_name: &str) -> Result<Module, Diagnostic> {
    if list.len() < 3 {
        return Err(Diagnostic::new(
            "Invalid module definition: too few elements".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let name = match &list[1] {
        SExpr::Atom(s, _) => s.clone(),
        _ => {
            return Err(Diagnostic::new(
                "Module name must be an atom".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    };
    let mut ports = Vec::new();
    let mut wires = Vec::new();
    let mut regs = Vec::new();
    let mut processes = Vec::new();
    let mut assigns = Vec::new();
    let mut instances = Vec::new();

    for form in &list[2..] {
        match form {
            SExpr::List(items, item_span) => {
                if items.is_empty() {
                    continue;
                }
                match &items[0] {
                    SExpr::Atom(s, _) => match s.as_str() {
                        "ports" => ports = parse_ports(items, *item_span, file_name)?,
                        "wire" => wires.push(parse_signal(items, *item_span, file_name)?),
                        "reg" => regs.push(parse_signal(items, *item_span, file_name)?),
                        "process" => processes.push(parse_process(items, *item_span, file_name)?),
                        "assign" => assigns.push(parse_assign(items, *item_span, file_name)?),
                        "instance" => instances.push(parse_instance(items, *item_span, file_name)?),
                        _ => {
                            return Err(Diagnostic::new(
                                format!("Unknown form: {}", s),
                                file_name.to_string(),
                                *item_span,
                            ));
                        }
                    },
                    _ => {
                        return Err(Diagnostic::new(
                            "Expected keyword".to_string(),
                            file_name.to_string(),
                            *item_span,
                        ));
                    }
                }
            }
            SExpr::Atom(_, atom_span) => {
                return Err(Diagnostic::new(
                    "Expected list form".to_string(),
                    file_name.to_string(),
                    *atom_span,
                ));
            }
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

fn parse_port(p: &[SExpr], span: Span, file_name: &str) -> Result<Vec<Port>, Diagnostic> {
    if p.len() < 2 {
        return Err(Diagnostic::new(
            "Invalid port declaration".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let mut direction = PortDir::Input;
    let mut reg = false;
    let mut names = Vec::new();
    let mut width = None;

    // 方向关键字
    match &p[0] {
        SExpr::Atom(s, _) => match s.as_str() {
            "input" => direction = PortDir::Input,
            "output" => direction = PortDir::Output,
            "inout" => direction = PortDir::Inout,
            _ => {
                return Err(Diagnostic::new(
                    format!("Unknown port direction: {}", s),
                    file_name.to_string(),
                    span,
                ));
            }
        },
        _ => {
            return Err(Diagnostic::new(
                "Expected direction keyword".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    }

    // 跳过方向，处理 reg 和端口名
    let mut iter = p[1..].iter();
    if let Some(next) = iter.next() {
        match next {
            SExpr::Atom(s, _) if s == "reg" => {
                reg = true;
                for token in iter {
                    match token {
                        SExpr::Atom(name, _) => names.push(name.clone()),
                        _ => {
                            return Err(Diagnostic::new(
                                "Expected port name".to_string(),
                                file_name.to_string(),
                                span,
                            ));
                        }
                    }
                }
            }
            SExpr::Atom(s, _) => {
                names.push(s.clone());
                for token in iter {
                    match token {
                        SExpr::Atom(name, _) => names.push(name.clone()),
                        _ => {
                            return Err(Diagnostic::new(
                                "Expected port name".to_string(),
                                file_name.to_string(),
                                span,
                            ));
                        }
                    }
                }
            }
            _ => {
                return Err(Diagnostic::new(
                    "Expected port name".to_string(),
                    file_name.to_string(),
                    span,
                ));
            }
        }
    } else {
        return Err(Diagnostic::new(
            "Missing port name".to_string(),
            file_name.to_string(),
            span,
        ));
    }

    // 检查最后一个名称是否为宽度
    if let Some(last) = names.last() {
        if last.chars().next().map_or(false, |c| c.is_ascii_digit()) || last.contains('\'') {
            width = Some(last.clone());
            names.pop();
        }
    }

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

fn collect_ports(sexprs: &[SExpr], file_name: &str) -> Result<Vec<Port>, Diagnostic> {
    let mut ports = Vec::new();
    for sexpr in sexprs {
        match sexpr {
            SExpr::List(p, sub_span) => {
                if let Some(SExpr::Atom(first, _)) = p.first() {
                    if matches!(first.as_str(), "input" | "output" | "inout") {
                        let new_ports = parse_port(p, *sub_span, file_name)?;
                        ports.extend(new_ports);
                    } else {
                        ports.extend(collect_ports(p, file_name)?);
                    }
                } else {
                    ports.extend(collect_ports(p, file_name)?);
                }
            }
            SExpr::Atom(_, _) => {}
        }
    }
    Ok(ports)
}

fn parse_ports(items: &[SExpr], span: Span, file_name: &str) -> Result<Vec<Port>, Diagnostic> {
    collect_ports(&items[1..], file_name)
}

fn parse_signal(items: &[SExpr], span: Span, file_name: &str) -> Result<Signal, Diagnostic> {
    if items.len() < 2 {
        return Err(Diagnostic::new(
            "Missing signal name".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let name = match &items[1] {
        SExpr::Atom(s, _) => s.clone(),
        _ => {
            return Err(Diagnostic::new(
                "Signal name must be atom".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    };
    let width = if items.len() >= 3 {
        match &items[2] {
            SExpr::Atom(s, _) => Some(s.clone()),
            _ => {
                return Err(Diagnostic::new(
                    "Width must be atom".to_string(),
                    file_name.to_string(),
                    span,
                ));
            }
        }
    } else {
        None
    };
    Ok(Signal { name, width })
}

fn parse_process(items: &[SExpr], span: Span, file_name: &str) -> Result<Process, Diagnostic> {
    if items.len() < 3 {
        return Err(Diagnostic::new(
            "Invalid process".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let sensitivity = parse_sensitivity(&items[1], file_name)?;
    let body = parse_statements(&items[2..], file_name)?;
    Ok(Process { sensitivity, body })
}

fn parse_sensitivity(sexpr: &SExpr, file_name: &str) -> Result<Vec<Sensitivity>, Diagnostic> {
    match sexpr {
        SExpr::List(list, span) => {
            let mut sens = Vec::new();
            let items = if let Some(SExpr::Atom(first, _)) = list.first() {
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
                    SExpr::List(sublist, sub_span) => {
                        if sublist.len() != 2 {
                            return Err(Diagnostic::new(
                                "Invalid sensitivity event".to_string(),
                                file_name.to_string(),
                                *sub_span,
                            ));
                        }
                        match &sublist[0] {
                            SExpr::Atom(s, _) => match s.as_str() {
                                "posedge" => {
                                    if let SExpr::Atom(sig, _) = &sublist[1] {
                                        sens.push(Sensitivity::PosEdge(sig.clone()));
                                    } else {
                                        return Err(Diagnostic::new(
                                            "Expected signal name".to_string(),
                                            file_name.to_string(),
                                            *sub_span,
                                        ));
                                    }
                                }
                                "negedge" => {
                                    if let SExpr::Atom(sig, _) = &sublist[1] {
                                        sens.push(Sensitivity::NegEdge(sig.clone()));
                                    } else {
                                        return Err(Diagnostic::new(
                                            "Expected signal name".to_string(),
                                            file_name.to_string(),
                                            *sub_span,
                                        ));
                                    }
                                }
                                _ => {
                                    return Err(Diagnostic::new(
                                        format!("Unknown event type: {}", s),
                                        file_name.to_string(),
                                        *sub_span,
                                    ));
                                }
                            },
                            _ => {
                                return Err(Diagnostic::new(
                                    "Expected event type".to_string(),
                                    file_name.to_string(),
                                    *sub_span,
                                ));
                            }
                        }
                    }
                    SExpr::Atom(sig, atom_span) => {
                        sens.push(Sensitivity::Level(sig.clone()));
                    }
                }
            }
            Ok(sens)
        }
        SExpr::Atom(_, span) => {
            Err(Diagnostic::new(
                "Sensitivity list must be a list".to_string(),
                file_name.to_string(),
                *span,
            ))
        }
    }
}

fn parse_statements(sexprs: &[SExpr], file_name: &str) -> Result<Vec<Statement>, Diagnostic> {
    let mut stmts = Vec::new();
    for sexpr in sexprs {
        match sexpr {
            SExpr::List(list, span) => {
                if list.is_empty() {
                    continue;
                }
                match &list[0] {
                    SExpr::Atom(s, _) => match s.as_str() {
                        "block-write" => {
                            if list.len() != 3 {
                                return Err(Diagnostic::new(
                                    "Invalid block-write".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let target = parse_assign_target(&list[1], file_name)?;
                            let expr = parse_expr(&list[2], file_name)?;
                            stmts.push(Statement::BlockWrite(target, expr));
                        }
                        "nb-write" => {
                            if list.len() != 3 {
                                return Err(Diagnostic::new(
                                    "Invalid nb-write".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let target = parse_assign_target(&list[1], file_name)?;
                            let expr = parse_expr(&list[2], file_name)?;
                            stmts.push(Statement::NonBlockWrite(target, expr));
                        }
                        "if" => {
                            if list.len() < 3 {
                                return Err(Diagnostic::new(
                                    "Invalid if statement".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let cond = parse_expr(&list[1], file_name)?;
                            let then_stmt = parse_statements(&[list[2].clone()], file_name)?;
                            let else_stmt = if list.len() >= 4 {
                                parse_statements(&[list[3].clone()], file_name)?
                            } else {
                                vec![]
                            };
                            stmts.push(Statement::If(cond, then_stmt, else_stmt));
                        }
                        "begin" => {
                            let inner_stmts = parse_statements(&list[1..], file_name)?;
                            stmts.extend(inner_stmts);
                        }
                        _ => {
                            return Err(Diagnostic::new(
                                format!("Unknown statement: {}", s),
                                file_name.to_string(),
                                *span,
                            ));
                        }
                    },
                    _ => {
                        return Err(Diagnostic::new(
                            "Expected statement keyword".to_string(),
                            file_name.to_string(),
                            *span,
                        ));
                    }
                }
            }
            SExpr::Atom(_, span) => {
                return Err(Diagnostic::new(
                    "Statement must be a list".to_string(),
                    file_name.to_string(),
                    *span,
                ));
            }
        }
    }
    Ok(stmts)
}

fn parse_assign_target(sexpr: &SExpr, file_name: &str) -> Result<AssignTarget, Diagnostic> {
    match sexpr {
        SExpr::List(list, span) => {
            if list.len() >= 2 {
                match &list[0] {
                    SExpr::Atom(s, _) if s == "signal" => {
                        let name = match &list[1] {
                            SExpr::Atom(n, _) => n.clone(),
                            _ => {
                                return Err(Diagnostic::new(
                                    "Expected signal name".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        let idx = if list.len() >= 3 {
                            match &list[2] {
                                SExpr::Atom(n, _) => {
                                    Some(n.parse::<usize>().map_err(|_| {
                                        Diagnostic::new(
                                            "Invalid index".to_string(),
                                            file_name.to_string(),
                                            *span,
                                        )
                                    })?)
                                }
                                _ => None,
                            }
                        } else {
                            None
                        };
                        Ok(AssignTarget::Signal(name, idx))
                    }
                    SExpr::Atom(s, _) if s == "slice" => {
                        if list.len() != 4 {
                            return Err(Diagnostic::new(
                                "Invalid slice: (slice sig high low)".to_string(),
                                file_name.to_string(),
                                *span,
                            ));
                        }
                        let name = match &list[1] {
                            SExpr::Atom(n, _) => n.clone(),
                            _ => {
                                return Err(Diagnostic::new(
                                    "Expected signal name".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        let high = match &list[2] {
                            SExpr::Atom(n, _) => n.parse::<usize>().map_err(|_| {
                                Diagnostic::new("Invalid high".to_string(), file_name.to_string(), *span)
                            })?,
                            _ => {
                                return Err(Diagnostic::new(
                                    "Expected high index".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        let low = match &list[3] {
                            SExpr::Atom(n, _) => n.parse::<usize>().map_err(|_| {
                                Diagnostic::new("Invalid low".to_string(), file_name.to_string(), *span)
                            })?,
                            _ => {
                                return Err(Diagnostic::new(
                                    "Expected low index".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        Ok(AssignTarget::Slice(name, high, low))
                    }
                    _ => {
                        Err(Diagnostic::new(
                            "Unknown assign target".to_string(),
                            file_name.to_string(),
                            *span,
                        ))
                    }
                }
            } else {
                Err(Diagnostic::new(
                    "Invalid assign target".to_string(),
                    file_name.to_string(),
                    *span,
                ))
            }
        }
        SExpr::Atom(s, span) => Ok(AssignTarget::Signal(s.clone(), None)),
    }
}

fn parse_expr(sexpr: &SExpr, file_name: &str) -> Result<Expr, Diagnostic> {
    match sexpr {
        SExpr::Atom(s, _span) => {
            if s.chars().next().map_or(false, |c| c.is_ascii_digit()) || s.contains('\'') {
                Ok(Expr::Literal(s.clone()))
            } else {
                Ok(Expr::Ident(s.clone()))
            }
        }
        SExpr::List(list, span) => {
            if list.is_empty() {
                return Err(Diagnostic::new(
                    "Empty expression".to_string(),
                    file_name.to_string(),
                    *span,
                ));
            }
            match &list[0] {
                SExpr::Atom(op, _) => {
                    match op.as_str() {
                        "slice" => {
                            if list.len() != 4 {
                                return Err(Diagnostic::new(
                                    "Invalid slice expression".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let expr = parse_expr(&list[1], file_name)?;
                            let high = match &list[2] {
                                SExpr::Atom(n, _) => n.parse::<usize>().map_err(|_| {
                                    Diagnostic::new("Invalid high".to_string(), file_name.to_string(), *span)
                                })?,
                                _ => {
                                    return Err(Diagnostic::new(
                                        "Expected high index".to_string(),
                                        file_name.to_string(),
                                        *span,
                                    ));
                                }
                            };
                            let low = match &list[3] {
                                SExpr::Atom(n, _) => n.parse::<usize>().map_err(|_| {
                                    Diagnostic::new("Invalid low".to_string(), file_name.to_string(), *span)
                                })?,
                                _ => {
                                    return Err(Diagnostic::new(
                                        "Expected low index".to_string(),
                                        file_name.to_string(),
                                        *span,
                                    ));
                                }
                            };
                            Ok(Expr::Slice(Box::new(expr), high, low))
                        }
                        "concat" => {
                            let mut exprs = Vec::new();
                            for e in &list[1..] {
                                exprs.push(parse_expr(e, file_name)?);
                            }
                            Ok(Expr::Concat(exprs))
                        }
                        "signal" => {
                            if list.len() < 2 {
                                return Err(Diagnostic::new(
                                    "signal requires at least a name".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let name = match &list[1] {
                                SExpr::Atom(s, _) => s.clone(),
                                _ => {
                                    return Err(Diagnostic::new(
                                        "signal name must be an atom".to_string(),
                                        file_name.to_string(),
                                        *span,
                                    ));
                                }
                            };
                            if list.len() >= 3 {
                                let idx = match &list[2] {
                                    SExpr::Atom(s, _) => s.parse::<usize>().map_err(|_| {
                                        Diagnostic::new("Invalid index".to_string(), file_name.to_string(), *span)
                                    })?,
                                    _ => {
                                        return Err(Diagnostic::new(
                                            "index must be a number".to_string(),
                                            file_name.to_string(),
                                            *span,
                                        ));
                                    }
                                };
                                Ok(Expr::Slice(Box::new(Expr::Ident(name)), idx, idx))
                            } else {
                                Ok(Expr::Ident(name))
                            }
                        }
                        "not" => {
                            if list.len() != 2 {
                                return Err(Diagnostic::new(
                                    "not requires one argument".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            Ok(Expr::UnaryOp(UnaryOp::Not, Box::new(parse_expr(&list[1], file_name)?)))
                        }
                        "and" | "or" | "xor" | "+" | "-" | "*" | "==" | "!=" | ">" | "<" | ">="
                        | "<=" | "<<" | ">>" => {
                            if list.len() != 3 {
                                return Err(Diagnostic::new(
                                    "Binary operator requires two arguments".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
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
                                Box::new(parse_expr(&list[1], file_name)?),
                                Box::new(parse_expr(&list[2], file_name)?),
                            ))
                        }
                        "cond" => {
                            if list.len() != 3 {
                                return Err(Diagnostic::new(
                                    "cond requires two branches".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                            let cond_branch = match &list[1] {
                                SExpr::List(l, sub_span) if l.len() == 2 => {
                                    (parse_expr(&l[0], file_name)?, parse_expr(&l[1], file_name)?)
                                }
                                _ => {
                                    return Err(Diagnostic::new(
                                        "Invalid condition branch".to_string(),
                                        file_name.to_string(),
                                        *span,
                                    ));
                                }
                            };
                            let else_branch = match &list[2] {
                                SExpr::List(l, sub_span)
                                if l.len() == 2 && matches!(&l[0], SExpr::Atom(s, _) if s == "else") =>
                                    {
                                        parse_expr(&l[1], file_name)?
                                    }
                                _ => {
                                    return Err(Diagnostic::new(
                                        "Invalid else branch".to_string(),
                                        file_name.to_string(),
                                        *span,
                                    ));
                                }
                            };
                            Ok(Expr::Cond(
                                Box::new(cond_branch.0),
                                Box::new(cond_branch.1),
                                Box::new(else_branch),
                            ))
                        }
                        _ => {
                            Err(Diagnostic::new(
                                format!("Unknown operator: {}", op),
                                file_name.to_string(),
                                *span,
                            ))
                        }
                    }
                }
                _ => {
                    Err(Diagnostic::new(
                        "Expected operator".to_string(),
                        file_name.to_string(),
                        *span,
                    ))
                }
            }
        }
    }
}

fn parse_assign(items: &[SExpr], span: Span, file_name: &str) -> Result<Assign, Diagnostic> {
    if items.len() != 3 {
        return Err(Diagnostic::new(
            "Invalid assign".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let target = parse_assign_target(&items[1], file_name)?;
    let expr = parse_expr(&items[2], file_name)?;
    Ok(Assign { target, expr })
}

fn parse_instance(items: &[SExpr], span: Span, file_name: &str) -> Result<Instance, Diagnostic> {
    if items.len() < 4 {
        return Err(Diagnostic::new(
            "Invalid instance".to_string(),
            file_name.to_string(),
            span,
        ));
    }
    let module_name = match &items[1] {
        SExpr::Atom(s, _) => s.clone(),
        _ => {
            return Err(Diagnostic::new(
                "Module name must be atom".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    };
    let instance_name = match &items[2] {
        SExpr::Atom(s, _) => s.clone(),
        _ => {
            return Err(Diagnostic::new(
                "Instance name must be atom".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    };
    let port_map = match &items[3] {
        SExpr::List(list, _) => {
            let map_items = if !list.is_empty() {
                match &list[0] {
                    SExpr::Atom(s, _) if s == "port-map" => &list[1..],
                    _ => list.as_slice(),
                }
            } else {
                &[]
            };
            let mut by_name = Vec::new();
            for mapping in map_items {
                match mapping {
                    SExpr::List(m, sub_span) => {
                        if m.len() != 2 {
                            return Err(Diagnostic::new(
                                "Invalid port mapping".to_string(),
                                file_name.to_string(),
                                *sub_span,
                            ));
                        }
                        let port = match &m[0] {
                            SExpr::Atom(s, _) => s.clone(),
                            _ => {
                                return Err(Diagnostic::new(
                                    "Port name must be atom".to_string(),
                                    file_name.to_string(),
                                    *sub_span,
                                ));
                            }
                        };
                        let expr = parse_expr(&m[1], file_name)?;
                        by_name.push((port, expr));
                    }
                    _ => {
                        return Err(Diagnostic::new(
                            "Port map must be list of pairs".to_string(),
                            file_name.to_string(),
                            span,
                        ));
                    }
                }
            }
            PortMap::ByName(by_name)
        }
        _ => {
            return Err(Diagnostic::new(
                "Port map must be a list".to_string(),
                file_name.to_string(),
                span,
            ));
        }
    };
    Ok(Instance {
        module_name,
        instance_name,
        port_map,
    })
}