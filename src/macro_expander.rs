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
}

impl MacroContext {
    pub fn new() -> Self {
        MacroContext {
            macros: HashMap::new(),
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
    fn expand_macro(&self, name: &str, args: &[SExpr]) -> Result<SExpr, String> {
        let def = self.macros.get(name).unwrap();
        if args.len() != def.params.len() {
            return Err(format!(
                "Macro {} expects {} arguments, got {}",
                name,
                def.params.len(),
                args.len()
            ));
        }
        // 构建参数绑定
        let mut bindings = HashMap::new();
        for (param, arg) in def.params.iter().zip(args.iter()) {
            bindings.insert(param.clone(), arg.clone());
        }
        // 展开宏体
        self.expand_sexpr(&def.body, Some(&bindings))
    }

    /// 递归展开 S‑表达式，支持参数替换和编译时函数
    fn expand_sexpr(
        &self,
        sexpr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        match sexpr {
            SExpr::Atom(s) => {
                // 如果是参数，则替换为绑定的值
                if let Some(b) = bindings {
                    if let Some(val) = b.get(s) {
                        return Ok(val.clone());
                    }
                }
                // 否则保持原样
                Ok(SExpr::Atom(s.clone()))
            }
            SExpr::List(list) => {
                if list.is_empty() {
                    return Ok(SExpr::List(vec![]));
                }
                // 第一个元素可能是宏名、编译时函数或普通表达式
                match &list[0] {
                    SExpr::Atom(first) => {
                        // 1. 检查是否为宏调用（且不是编译时函数）
                        if self.is_macro(first) && !first.ends_with('!') {
                            // 展开参数（先展开参数中的宏和函数）
                            let mut expanded_args = Vec::new();
                            for arg in &list[1..] {
                                expanded_args.push(self.expand_sexpr(arg, bindings)?);
                            }
                            return self.expand_macro(first, &expanded_args);
                        }
                        // 2. 检查是否为编译时函数（以 '!' 结尾）
                        if first.ends_with('!') {
                            return self.eval_builtin(first, &list[1..], bindings);
                        }
                        // 3. 普通列表，递归展开每个元素
                        let mut new_list = Vec::new();
                        for item in list {
                            new_list.push(self.expand_sexpr(item, bindings)?);
                        }
                        Ok(SExpr::List(new_list))
                    }
                    _ => {
                        // 第一个元素不是原子，递归展开
                        let mut new_list = Vec::new();
                        for item in list {
                            new_list.push(self.expand_sexpr(item, bindings)?);
                        }
                        Ok(SExpr::List(new_list))
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
    ) -> Result<SExpr, String> {
        match name {
            "expil!" => {
                if args.len() != 1 {
                    return Err("expil! expects 1 argument".to_string());
                }
                self.expand_sexpr(&args[0], bindings)
            }
            "between!" => self.builtin_between(args, bindings),
            "foreach!" => self.builtin_foreach(args, bindings),
            "str!" => self.builtin_str(args, bindings),
            "add!" => self.builtin_add(args, bindings),
            "if!" => self.builtin_if(args, bindings),
            "eval!" => self.builtin_eval(args, bindings),

            // 算术与比较运算符（均以 ! 结尾）
            "+!" => {
                if args.len() < 2 {
                    return Err("+! requires at least two arguments".to_string());
                }
                let mut sum = 0i64;
                for arg in args {
                    sum += self.eval_integer(arg, bindings)?;
                }
                Ok(SExpr::Atom(sum.to_string()))
            }
            "-!" => {
                if args.len() != 2 {
                    return Err("-! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom((a - b).to_string()))
            }
            "*!" => {
                if args.len() < 2 {
                    return Err("*! requires at least two arguments".to_string());
                }
                let mut product = 1i64;
                for arg in args {
                    product *= self.eval_integer(arg, bindings)?;
                }
                Ok(SExpr::Atom(product.to_string()))
            }
            "/!" => {
                if args.len() != 2 {
                    return Err("/! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                if b == 0 {
                    return Err("Division by zero in /!".to_string());
                }
                Ok(SExpr::Atom((a / b).to_string()))
            }
            "%!" => {
                if args.len() != 2 {
                    return Err("%! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                if b == 0 {
                    return Err("Modulo by zero in %!".to_string());
                }
                Ok(SExpr::Atom((a % b).to_string()))
            }
            "==!" => {
                if args.len() != 2 {
                    return Err("==! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a == b { "1" } else { "0" }.to_string()))
            }
            "!=!" => {
                if args.len() != 2 {
                    return Err("!=! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a != b { "1" } else { "0" }.to_string()))
            }
            "<!" => {
                if args.len() != 2 {
                    return Err("<! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a < b { "1" } else { "0" }.to_string()))
            }
            "<=!" => {
                if args.len() != 2 {
                    return Err("<=! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a <= b { "1" } else { "0" }.to_string()))
            }
            ">!" => {
                if args.len() != 2 {
                    return Err(">! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a > b { "1" } else { "0" }.to_string()))
            }
            ">=!" => {
                if args.len() != 2 {
                    return Err(">=! requires exactly two arguments".to_string());
                }
                let a = self.eval_integer(&args[0], bindings)?;
                let b = self.eval_integer(&args[1], bindings)?;
                Ok(SExpr::Atom(if a >= b { "1" } else { "0" }.to_string()))
            }

            _ => Err(format!("Unknown builtin function: {}", name)),
        }
    }

    // between! start end -> 生成整数列表 (start, start+1, ..., end-1)
    fn builtin_between(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() != 2 {
            return Err("between! expects 2 arguments".to_string());
        }
        let start = self.eval_integer(&args[0], bindings)?;
        let end = self.eval_integer(&args[1], bindings)?;
        let mut list = Vec::new();
        for i in start..end {
            list.push(SExpr::Atom(i.to_string()));
        }
        Ok(SExpr::List(list))
    }

    // foreach! list [var] body
    // 遍历列表，将当前元素绑定到 var（默认 it），然后展开 body
    // foreach! list [var] body
    // 遍历列表，将当前元素绑定到 var（默认 it），然后展开 body
    fn builtin_foreach(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() < 2 || args.len() > 3 {
            return Err(
                "foreach! expects 2 or 3 arguments: (foreach! list [var] body)".to_string(),
            );
        }
        let list_expr = &args[0];
        let var = if args.len() == 3 {
            match &args[1] {
                SExpr::Atom(s) => s.clone(),
                _ => return Err("foreach! var must be an atom".to_string()),
            }
        } else {
            "it".to_string()
        };
        let body_expr = if args.len() == 3 { &args[2] } else { &args[1] };

        let list = self.eval_list(list_expr, bindings)?;
        let mut result = Vec::new();
        for item in list {
            let mut new_bindings = bindings.cloned().unwrap_or_default();
            new_bindings.insert(var.clone(), item);
            let expanded = self.expand_sexpr(body_expr, Some(&new_bindings))?;
            // 如果 expanded 是 (begin ...) 列表，则将其内容展平
            if let SExpr::List(inner) = &expanded {
                if let Some(SExpr::Atom(first)) = inner.first() {
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
        Ok(SExpr::List(result))
    }

    // str! symbol -> 将符号转为字符串（原子）
    fn builtin_str(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() != 1 {
            return Err("str! expects 1 argument".to_string());
        }
        let atom = self.eval_atom(&args[0], bindings)?;
        Ok(SExpr::Atom(atom))
    }

    // add! str int -> 字符串拼接整数，返回新符号
    fn builtin_add(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() != 2 {
            return Err("add! expects 2 arguments".to_string());
        }
        let s = self.eval_string(&args[0], bindings)?;
        let n = self.eval_integer(&args[1], bindings)?;
        Ok(SExpr::Atom(format!("{}{}", s, n)))
    }

    // if! cond then else
    fn builtin_if(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() != 3 {
            return Err("if! expects 3 arguments".to_string());
        }
        let cond = self.eval_bool(&args[0], bindings)?;
        if cond {
            self.expand_sexpr(&args[1], bindings)
        } else {
            self.expand_sexpr(&args[2], bindings)
        }
    }

    // eval! expr -> 直接返回 expr 的展开结果（不添加额外包装）
    fn builtin_eval(
        &self,
        args: &[SExpr],
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<SExpr, String> {
        if args.len() != 1 {
            return Err("eval! expects 1 argument".to_string());
        }
        self.expand_sexpr(&args[0], bindings)
    }

    // 辅助求值函数：将 S‑表达式求值为整数
    fn eval_integer(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<i64, String> {
        let expanded = self.expand_sexpr(expr, bindings)?;
        match expanded {
            SExpr::Atom(s) => s
                .parse::<i64>()
                .map_err(|_| format!("Expected integer, got: {}", s)),
            _ => Err("Expected integer".to_string()),
        }
    }

    // 求值为字符串（原子）
    fn eval_string(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<String, String> {
        let expanded = self.expand_sexpr(expr, bindings)?;
        match expanded {
            SExpr::Atom(s) => Ok(s),
            _ => Err("Expected string/symbol".to_string()),
        }
    }

    // 求值为原子（符号或字符串）
    fn eval_atom(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<String, String> {
        let expanded = self.expand_sexpr(expr, bindings)?;
        match expanded {
            SExpr::Atom(s) => Ok(s),
            _ => Err("Expected atom".to_string()),
        }
    }

    // 求值为布尔值（0/false 视为假，非0/true 视为真）
    fn eval_bool(
        &self,
        expr: &SExpr,
        bindings: Option<&HashMap<String, SExpr>>,
    ) -> Result<bool, String> {
        let expanded = self.expand_sexpr(expr, bindings)?;
        match expanded {
            SExpr::Atom(s) => {
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
    ) -> Result<Vec<SExpr>, String> {
        let expanded = self.expand_sexpr(expr, bindings)?;
        match expanded {
            SExpr::List(list) => Ok(list),
            _ => Err("Expected list".to_string()),
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
        SExpr::List(list) => {
            // 如果是模块定义，则展平其主体
            if list.len() >= 3 {
                if let Some(SExpr::Atom(first)) = list.first() {
                    if first == "module" {
                        let mut new_list = Vec::new();
                        new_list.push(list[0].clone());
                        new_list.push(list[1].clone());
                        // 处理主体（从索引2开始）
                        let body = flatten_module_body(&list[2..]);
                        new_list.extend(body);
                        return SExpr::List(new_list);
                    }
                }
            }
            // 其他列表递归展平内部元素
            SExpr::List(list.into_iter().map(flatten_sexpr).collect())
        }
        _ => sexpr,
    }
}

/// 展平模块主体，将嵌套列表中的元素提升到顶层
fn flatten_module_body(body: &[SExpr]) -> Vec<SExpr> {
    let mut result = Vec::new();
    for item in body {
        match item {
            SExpr::List(inner) => {
                // 检查第一个元素是否为原子
                if let Some(first) = inner.first() {
                    match first {
                        SExpr::Atom(_) => {
                            // 原子开头，保留原样
                            result.push(item.clone());
                        }
                        SExpr::List(_) => {
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
pub fn expand(sexprs: Vec<SExpr>) -> Result<Vec<SExpr>, String> {
    let mut context = MacroContext::new();
    // 第一遍：收集所有顶层宏定义，并移除它们（宏定义不保留在最终代码中）
    let mut remaining = Vec::new();
    for sexpr in sexprs {
        match &sexpr {
            SExpr::List(list) => {
                if let Some(SExpr::Atom(first)) = list.first() {
                    if first == "macro" {
                        // 解析宏定义
                        if list.len() < 4 {
                            return Err("Invalid macro definition".to_string());
                        }
                        let name = match &list[1] {
                            SExpr::Atom(s) => s.clone(),
                            _ => return Err("Macro name must be an atom".to_string()),
                        };
                        let params = match &list[2] {
                            SExpr::List(p) => {
                                let mut params_vec = Vec::new();
                                for param in p {
                                    match param {
                                        SExpr::Atom(s) => params_vec.push(s.clone()),
                                        _ => {
                                            return Err("Macro parameters must be atoms".to_string());
                                        }
                                    }
                                }
                                params_vec
                            }
                            _ => return Err("Macro parameters must be a list".to_string()),
                        };
                        let body = &list[3];
                        // 确保 body 是 expil! 包裹的形式？但根据语法，可以是任意表达式，我们直接存储 body
                        // 如果 body 是 (expil! ...)，我们提取内部内容（expil! 标记可能在宏展开时处理，但我们先保留）
                        // 简化：直接存储整个 body
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
        let expanded = context.expand_sexpr(&sexpr, None)?;
        result.push(expanded);
    }
    let result = flatten_sexprs(result);
    Ok(result)
}
