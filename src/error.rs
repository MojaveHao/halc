#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// 创建一个新的诊断信息
    pub fn new(message: String, file: String, span: Span) -> Self {
        Diagnostic {
            message,
            file,
            span,
        }
    }

    /// 漂亮地显示错误信息（Clang 风格，显示上一行和当前行）
    pub fn display(&self, source: &str) {
        let lines: Vec<&str> = source.lines().collect();
        let line_idx = self.span.line.saturating_sub(1);
        let current_line = lines.get(line_idx).unwrap_or(&"");
        let prev_line = if line_idx > 0 { lines.get(line_idx - 1) } else { None };

        eprintln!("error: {}", self.message);
        eprintln!("  --> {}:{}:{}", self.file, self.span.line, self.span.column);
        eprintln!("   |");

        let current_line_num = self.span.line;
        let prev_line_num = current_line_num - 1;
        let max_width = if prev_line.is_some() {
            std::cmp::max(current_line_num.to_string().len(), prev_line_num.to_string().len())
        } else {
            current_line_num.to_string().len()
        };

        if let Some(prev) = prev_line {
            eprintln!("{:>width$} | {}", prev_line_num, prev, width = max_width);
        }
        eprintln!("{:>width$} | {}", current_line_num, current_line, width = max_width);

        // 指示箭头
        let indent = max_width + 3 + (self.span.column - 1);
        eprintln!("{:>indent$}^", "", indent = indent + 1); // +1 因为 ^ 本身占一位
        eprintln!("   |");
    }

    /// 简单版本（只显示一行，不带代码高亮）
    pub fn display_simple(&self) {
        eprintln!(
            "{}:{}:{}: error: {}",
            self.file, self.span.line, self.span.column, self.message
        );
    }
}

/// 方便创建 Span 的辅助函数
impl Span {
    pub fn new(start: usize, end: usize, line: usize, column: usize) -> Self {
        Span {
            start,
            end,
            line,
            column,
        }
    }

    /// 从字符索引创建 Span
    pub fn from_char_range(start: usize, end: usize, source: &str) -> Self {
        let (line, column) = Self::pos_to_line_col(start, source);
        Span {
            start,
            end,
            line,
            column,
        }
    }

    fn pos_to_line_col(pos: usize, source: &str) -> (usize, usize) {
        let mut line = 1;
        let mut column = 1;
        let mut current_pos = 0;

        for ch in source.chars() {
            if current_pos >= pos {
                break;
            }
            if ch == '\n' {
                line += 1;
                column = 1;
            } else {
                column += 1;
            }
            current_pos += ch.len_utf8();
        }

        (line, column)
    }
}

/// 方便的错误创建宏
#[macro_export]
macro_rules! error {
    ($file:expr, $span:expr, $($arg:tt)*) => {
        Err(format!($($arg)*)).map_err(|msg| {
            crate::error::Diagnostic::new(msg, $file.to_string(), $span)
        })
    };
}