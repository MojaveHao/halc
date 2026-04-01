#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LParen,         // '('
    RParen,         // ')'
    Ident(String),  // 标识符，如 module, input, clk, add!
    Number(String), // 数字字面量，如 42, 4'b1010
    String(String), // 字符串，如 "data"
    Eof,
}

pub fn tokenize(source: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = source.chars().peekable();

    while let Some(&ch) = chars.peek() {
        match ch {
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            ';' => {
                // 行注释，忽略到行尾
                while let Some(&c) = chars.peek() {
                    if c == '\n' {
                        break;
                    }
                    chars.next();
                }
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            c if c == '"' => {
                chars.next();
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                chars.next(); // 关闭引号
                tokens.push(Token::String(s));
            }
            _ => {
                // 标识符或数字
                let mut ident = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == ';' {
                        break;
                    }
                    ident.push(c);
                    chars.next();
                }
                // 简单判断：如果第一个字符是数字或数字后跟'b/h/d，则为数字
                if ident.chars().next().map_or(false, |c| c.is_ascii_digit()) {
                    tokens.push(Token::Number(ident));
                } else {
                    tokens.push(Token::Ident(ident));
                }
            }
        }
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}
