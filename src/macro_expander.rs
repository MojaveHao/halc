use crate::error::{Diagnostic, Span};
use crate::parser::SExpr;
use std::collections::HashMap;

/// 宏定义
struct MacroDef {
    params: Vec<String>,
    body: SExpr,
}

/// 宏展开上下文，存储宏定义和当前模块信息（如端口列表，此处简化未实现）
pub struct MacroContext {
    macros: HashMap<String, MacroDef>,
    file_name: String,
}

impl MacroContext {
    pub fn new(file_name: &str) -> Self {
        MacroContext {
            macros: HashMap::new(),
            file_name: file_name.to_string(),
        }
    }

    /// 注册一个宏定义
    fn define_macro(&mut self, name: &str, params: Vec<String>, body: SExpr) {
        self.macros
            .insert(name.to_string(), MacroDef { params, body });
    }

    /// 判断是否为宏调用
    fn is_macro(&self, name: &str) -> bool {
        self.macros.contains_key(name)
    }

    /// 展开宏调用，返回展开后的 S‑表达式
    fn expand_macro(&self, name: &str, args: &[SExpr], call_span: Span) -> Result<SExpr, Diagnostic> {
        let def = self.macros.get(name).unwrap();
        if args.len() != def.params.len() {
            return Err(Diagnostic::new(
                format!(
                    "Macro {} expects {} arguments, got {}",
                    name,
                    def.params.len(),
                    args.len()
                ),
                self.file_name.clone(),
                call_span,
            ));
        }
        // 构建参数绑定
        let mut bindings = HashMap::new();
        for (param, arg) in def.params.iter().zip(args.iter()) {
            bindings.insert(param.clone(), arg.clone());
        }
        // 展开宏体
        self.expand_sexpr(&def.body, Some(&bindings), call_span)
    }

    /// 递归展开 S‑表达式，支持参数替换和编译时函数
    fn expand_sexpr(
        &self,
        sexpr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        context_span: Span, // 用于新生成表达式的默认位置（如调用点）
    ) -> Result<SExpr, Diagnostic> {
        match sexpr {
            SExpr::Atom(s, span) => {
                // 如果是参数，则替换为绑定的值
                if let Some(b) = bindings {
                    if let Some(val) = b.get(s) {
                        return Ok(val.clone());
                    }
                }
                // 否则保持原样
                Ok(SExpr::Atom(s.clone(), *span))
            }
            SExpr::List(list, span) => {
                if list.is_empty() {
                    return Ok(SExpr::List(vec![], *span));
                }
                // 第一个元素可能是宏名、编译时函数或普通表达式
                match &list[0] {
                    SExpr::Atom(first, _) => {
                        // 1. 检查是否为宏调用（且不是编译时函数）
                        if self.is_macro(first) && !first.ends_with('!') {
                            // 展开参数（先展开参数中的宏和函数）
                            let mut expanded_args = Vec::new();
                            for arg in &list[1..] {
                                expanded_args.push(self.expand_sexpr(arg, bindings, context_span)?);
                            }
                            return self.expand_macro(first, &expanded_args, *span);
                        }
                        // 2. 检查是否为编译时函数（以 '!' 结尾）
                        if first.ends_with('!') {
                            return self.eval_builtin(first, &list[1..], bindings, *span);
                        }
                        // 3. 普通列表，递归展开每个元素
                        let mut new_list = Vec::new();
                        for item in list {
                            new_list.push(self.expand_sexpr(item, bindings, context_span)?);
                        }
                        Ok(SExpr::List(new_list, *span))
                    }
                    _ => {
                        // 第一个元素不是原子，递归展开
                        let mut new_list = Vec::new();
                        for item in list {
                            new_list.push(self.expand_sexpr(item, bindings, context_span)?);
                        }
                        Ok(SExpr::List(new_list, *span))
                    }
                }
            }
        }
    }

    /// 执行编译时内置函数
    fn eval_builtin(
        &self,
        name: &str,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        match name {
            "expil!" => {
                if args.len() != 1 {
                    return Err(Diagnostic::new(
                        "expil! expects 1 argument".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                self.expand_sexpr(&args[0], bindings, span)
            }
            "between!" => self.builtin_between(args, bindings, span),
            "foreach!" => self.builtin_foreach(args, bindings, span),
            "str!" => self.builtin_str(args, bindings, span),
            "add!" => self.builtin_add(args, bindings, span),
            "if!" => self.builtin_if(args, bindings, span),
            "eval!" => self.builtin_eval(args, bindings, span),
            "let!" => self.builtin_let(args, bindings, span),

            // 算术与比较运算符（均以 ! 结尾）
            "+!" => {
                if args.len() < 2 {
                    return Err(Diagnostic::new(
                        "+! requires at least two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let mut sum = 0i64;
                for arg in args {
                    sum += self.eval_integer(arg, bindings, span)?;
                }
                Ok(SExpr::Atom(sum.to_string(), span))
            }
            "-!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "-! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom((a - b).to_string(), span))
            }
            "*!" => {
                if args.len() < 2 {
                    return Err(Diagnostic::new(
                        "*! requires at least two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let mut product = 1i64;
                for arg in args {
                    product *= self.eval_integer(arg, bindings, span)?;
                }
                Ok(SExpr::Atom(product.to_string(), span))
            }
            "/!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "/! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                if b == 0 {
                    return Err(Diagnostic::new(
                        "Division by zero in /!".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                Ok(SExpr::Atom((a / b).to_string(), span))
            }
            "%!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "%! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                if b == 0 {
                    return Err(Diagnostic::new(
                        "Modulo by zero in %!".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                Ok(SExpr::Atom((a % b).to_string(), span))
            }
            "==!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "==! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a == b { "1" } else { "0" }.to_string(), span))
            }
            "!=!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "!=! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a != b { "1" } else { "0" }.to_string(), span))
            }
            "<!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "<! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a < b { "1" } else { "0" }.to_string(), span))
            }
            "<=!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        "<=! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a <= b { "1" } else { "0" }.to_string(), span))
            }
            ">!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        ">! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a > b { "1" } else { "0" }.to_string(), span))
            }
            ">=!" => {
                if args.len() != 2 {
                    return Err(Diagnostic::new(
                        ">=! requires exactly two arguments".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
                let a = self.eval_integer(&args[0], bindings, span)?;
                let b = self.eval_integer(&args[1], bindings, span)?;
                Ok(SExpr::Atom(if a >= b { "1" } else { "0" }.to_string(), span))
            }

            _ => Err(Diagnostic::new(
                format!("Unknown builtin function: {}", name),
                self.file_name.clone(),
                span,
            )),
        }
    }

    // between! start end -> 生成整数列表 (start, start+1, ..., end-1)
    fn builtin_between(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::new(
                "between! expects 2 arguments".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        let start = self.eval_integer(&args[0], bindings, span)?;
        let end = self.eval_integer(&args[1], bindings, span)?;
        let mut list = Vec::new();
        for i in start..end {
            list.push(SExpr::Atom(i.to_string(), span));
        }
        Ok(SExpr::List(list, span))
    }

    // foreach! list [var] body
    // 遍历列表，将当前元素绑定到 var（默认 it），然后展开 body
    fn builtin_foreach(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() < 2 || args.len() > 3 {
            return Err(Diagnostic::new(
                "foreach! expects 2 or 3 arguments: (foreach! list [var] body)".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        let list_expr = &args[0];
        let var = if args.len() == 3 {
            match &args[1] {
                SExpr::Atom(s, _) => s.clone(),
                _ => {
                    return Err(Diagnostic::new(
                        "foreach! var must be an atom".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
            }
        } else {
            "it".to_string()
        };
        let body_expr = if args.len() == 3 { &args[2] } else { &args[1] };

        let list = self.eval_list(list_expr, bindings, span)?;
        let mut result = Vec::new();
        for item in list {
            let mut new_bindings = bindings.cloned().unwrap_or_default();
            new_bindings.insert(var.clone(), item);
            let expanded = self.expand_sexpr(body_expr, Some(&new_bindings), span)?;
            // 如果 expanded 是 (begin ...) 列表，则将其内容展平
            if let SExpr::List(inner, _) = &expanded {
                if let Some(SExpr::Atom(first, _)) = inner.first() {
                    if first == "begin" {
                        for sub in &inner[1..] {
                            result.push(sub.clone());
                        }
                        continue;
                    }
                }
            }
            result.push(expanded);
        }
        // 将结果拼接成一个列表
        Ok(SExpr::List(result, span))
    }

    // let! ((var val) ...) body...
    fn builtin_let(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() < 2 {
            return Err(Diagnostic::new(
                "let! expects at least 2 arguments: (let! bindings body ...)".to_string(),
                self.file_name.clone(),
                span,
            ));
        }

        // 解析绑定列表
        let bindings_list = match &args[0] {
            SExpr::List(list, _) => list,
            _ => {
                return Err(Diagnostic::new(
                    "let! first argument must be a list of bindings".to_string(),
                    self.file_name.clone(),
                    span,
                ));
            }
        };

        // 构造新的绑定环境（继承外部绑定）
        let mut new_bindings = bindings.cloned().unwrap_or_default();
        for binding in bindings_list {
            let (var, val_expr) = match binding {
                SExpr::List(inner, _) if inner.len() == 2 => {
                    let var = match &inner[0] {
                        SExpr::Atom(s, _) => s.clone(),
                        _ => {
                            return Err(Diagnostic::new(
                                "let! binding variable must be an atom".to_string(),
                                self.file_name.clone(),
                                span,
                            ));
                        }
                    };
                    (var, &inner[1])
                }
                _ => {
                    return Err(Diagnostic::new(
                        "let! binding must be a list of (var expr)".to_string(),
                        self.file_name.clone(),
                        span,
                    ));
                }
            };

            // 在当前未添加新绑定的环境中求值 val_expr（不允许递归引用同层绑定）
            // 注意：这里应该使用旧的 bindings 环境，防止 let 绑定之间互相引用。
            let val = self.expand_sexpr(val_expr, bindings, span)?;
            new_bindings.insert(var, val);
        }

        // 展开所有主体表达式（索引 1..）
        let mut result = Vec::new();
        for body_expr in &args[1..] {
            let expanded = self.expand_sexpr(body_expr, Some(&new_bindings), span)?;
            // 若展开结果是 (begin ...) 列表，则展平其内部
            if let SExpr::List(inner, _) = &expanded {
                if let Some(SExpr::Atom(first, _)) = inner.first() {
                    if first == "begin" {
                        for sub in &inner[1..] {
                            result.push(sub.clone());
                        }
                        continue;
                    }
                }
            }
            result.push(expanded);
        }

        Ok(SExpr::List(result, span))
    }

    // str! symbol -> 将符号转为字符串（原子）
    fn builtin_str(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new(
                "str! expects 1 argument".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        let atom = self.eval_atom(&args[0], bindings, span)?;
        Ok(SExpr::Atom(atom, span))
    }

    // add! str int -> 字符串拼接整数，返回新符号
    fn builtin_add(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() != 2 {
            return Err(Diagnostic::new(
                "add! expects 2 arguments".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        let s = self.eval_string(&args[0], bindings, span)?;
        let n = self.eval_integer(&args[1], bindings, span)?;
        Ok(SExpr::Atom(format!("{}{}", s, n), span))
    }

    // if! cond then else
    fn builtin_if(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() != 3 {
            return Err(Diagnostic::new(
                "if! expects 3 arguments".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        let cond = self.eval_bool(&args[0], bindings, span)?;
        if cond {
            self.expand_sexpr(&args[1], bindings, span)
        } else {
            self.expand_sexpr(&args[2], bindings, span)
        }
    }

    // eval! expr -> 直接返回 expr 的展开结果（不添加额外包装）
    fn builtin_eval(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<SExpr, Diagnostic> {
        if args.len() != 1 {
            return Err(Diagnostic::new(
                "eval! expects 1 argument".to_string(),
                self.file_name.clone(),
                span,
            ));
        }
        self.expand_sexpr(&args[0], bindings, span)
    }

    // 辅助求值函数：将 S‑表达式求值为整数
    fn eval_integer(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<i64, Diagnostic> {
        let expanded = self.expand_sexpr(expr, bindings, span)?;
        match expanded {
            SExpr::Atom(s, _) => s
                .parse::<i64>()
                .map_err(|_| Diagnostic::new(
                    format!("Expected integer, got: {}", s),
                    self.file_name.clone(),
                    span,
                )),
            _ => Err(Diagnostic::new(
                "Expected integer".to_string(),
                self.file_name.clone(),
                span,
            )),
        }
    }

    // 求值为字符串（原子）
    fn eval_string(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<String, Diagnostic> {
        let expanded = self.expand_sexpr(expr, bindings, span)?;
        match expanded {
            SExpr::Atom(s, _) => Ok(s),
            _ => Err(Diagnostic::new(
                "Expected string/symbol".to_string(),
                self.file_name.clone(),
                span,
            )),
        }
    }

    // 求值为原子（符号或字符串）
    fn eval_atom(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<String, Diagnostic> {
        let expanded = self.expand_sexpr(expr, bindings, span)?;
        match expanded {
            SExpr::Atom(s, _) => Ok(s),
            _ => Err(Diagnostic::new(
                "Expected atom".to_string(),
                self.file_name.clone(),
                span,
            )),
        }
    }

    // 求值为布尔值（0/false 视为假，非0/true 视为真）
    fn eval_bool(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<bool, Diagnostic> {
        let expanded = self.expand_sexpr(expr, bindings, span)?;
        match expanded {
            SExpr::Atom(s, _) => {
                if s == "0" || s == "false" {
                    Ok(false)
                } else {
                    Ok(true)
                }
            }
            _ => Ok(true), // 非原子视为真
        }
    }

    // 求值为 S‑表达式列表
    fn eval_list(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
        span: Span,
    ) -> Result<Vec<SExpr>, Diagnostic> {
        let expanded = self.expand_sexpr(expr, bindings, span)?;
        match expanded {
            SExpr::List(list, _) => Ok(list),
            _ => Err(Diagnostic::new(
                "Expected list".to_string(),
                self.file_name.clone(),
                span,
            )),
        }
    }
}

/// 对展开后的 S‑表达式列表进行后处理，展平模块定义中的嵌套列表
fn flatten_sexprs(sexprs: Vec<SExpr>) -> Vec<SExpr> {
    sexprs.into_iter().map(flatten_sexpr).collect()
}

/// 递归展平单个 S‑表达式
fn flatten_sexpr(sexpr: SExpr) -> SExpr {
    match sexpr {
        SExpr::List(list, span) => {
            // 如果是模块定义，则展平其主体
            if list.len() >= 3 {
                if let Some(SExpr::Atom(first, _)) = list.first() {
                    if first == "module" {
                        let mut new_list = Vec::new();
                        new_list.push(list[0].clone());
                        new_list.push(list[1].clone());
                        // 处理主体（从索引2开始）
                        let body = flatten_module_body(&list[2..]);
                        new_list.extend(body);
                        return SExpr::List(new_list, span);
                    }
                }
            }
            // 其他列表递归展平内部元素
            SExpr::List(list.into_iter().map(flatten_sexpr).collect(), span)
        }
        _ => sexpr,
    }
}

/// 展平模块主体，将嵌套列表中的元素提升到顶层
fn flatten_module_body(body: &[SExpr]) -> Vec<SExpr> {
    let mut result = Vec::new();
    for item in body {
        match item {
            SExpr::List(inner, _) => {
                // 检查第一个元素是否为原子
                if let Some(first) = inner.first() {
                    match first {
                        SExpr::Atom(_, _) => {
                            // 原子开头，保留原样
                            result.push(item.clone());
                        }
                        SExpr::List(_, _) => {
                            // 非原子开头，递归展平内部
                            let flattened = flatten_module_body(inner);
                            result.extend(flattened);
                        }
                    }
                }
            }
            _ => {
                // 非列表元素（如原子）保留原样
                result.push(item.clone());
            }
        }
    }
    result
}

/// 主展开函数：对顶层 S‑表达式列表进行宏展开，返回新的 S‑表达式列表
pub fn expand(sexprs: Vec<SExpr>, file_name: &str) -> Result<Vec<SExpr>, Diagnostic> {
    let mut context = MacroContext::new(file_name);
    // 第一遍：收集所有顶层宏定义，并移除它们（宏定义不保留在最终代码中）
    let mut remaining = Vec::new();
    for sexpr in sexprs {
        match &sexpr {
            SExpr::List(list, span) => {
                if let Some(SExpr::Atom(first, _)) = list.first() {
                    if first == "macro" {
                        // 解析宏定义
                        if list.len() < 4 {
                            return Err(Diagnostic::new(
                                "Invalid macro definition".to_string(),
                                file_name.to_string(),
                                *span,
                            ));
                        }
                        let name = match &list[1] {
                            SExpr::Atom(s, _) => s.clone(),
                            _ => {
                                return Err(Diagnostic::new(
                                    "Macro name must be an atom".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        let params = match &list[2] {
                            SExpr::List(p, _) => {
                                let mut params_vec = Vec::new();
                                for param in p {
                                    match param {
                                        SExpr::Atom(s, _) => params_vec.push(s.clone()),
                                        _ => {
                                            return Err(Diagnostic::new(
                                                "Macro parameters must be atoms".to_string(),
                                                file_name.to_string(),
                                                *span,
                                            ));
                                        }
                                    }
                                }
                                params_vec
                            }
                            _ => {
                                return Err(Diagnostic::new(
                                    "Macro parameters must be a list".to_string(),
                                    file_name.to_string(),
                                    *span,
                                ));
                            }
                        };
                        let body = &list[3];
                        context.define_macro(&name, params, body.clone());
                        // 宏定义不放入最终输出
                        continue;
                    }
                }
            }
            _ => {}
        }
        remaining.push(sexpr);
    }
    // 第二遍：展开所有剩余的 S‑表达式
    let mut result = Vec::new();
    for sexpr in remaining {
        // 使用顶层表达式的 span 作为上下文默认 span
        let default_span = match &sexpr {
            SExpr::Atom(_, span) => *span,
            SExpr::List(_, span) => *span,
        };
        let expanded = context.expand_sexpr(&sexpr, None, default_span)?;
        result.push(expanded);
    }
    let result = flatten_sexprs(result);
    Ok(result)
}