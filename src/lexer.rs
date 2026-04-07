use crate::error::{Diagnostic, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    LParen(Span),
    RParen(Span),
    Ident(String, Span),
    Number(String, Span),
    String(String, Span),
    Eof(Span),
}

/// 词法分析器，维护扫描状态
struct Lexer<'a> {
    source: &'a str,
    chars: Vec<char>,          // 字符数组，便于按索引操作
    file_name: String,         // 文件名（用于错误报告）
    pos: usize,                // 当前字节偏移
    index: usize,              // 当前字符索引
    line: usize,               // 当前行号（从1开始）
    column: usize,             // 当前列号（从1开始）
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str, file_name: &str) -> Self {
        Lexer {
            source,
            chars: source.chars().collect(),
            file_name: file_name.to_string(),
            pos: 0,
            index: 0,
            line: 1,
            column: 1,
        }
    }

    /// 查看当前字符（不消费）
    fn peek(&self) -> Option<&char> {
        self.chars.get(self.index)
    }

    /// 消费一个字符，更新位置信息
    fn advance(&mut self) -> Option<char> {
        let ch = *self.peek()?;
        self.index += 1;
        if ch == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        self.pos += ch.len_utf8();
        Some(ch)
    }

    /// 跳过空白字符和注释
    fn skip_whitespace_and_comments(&mut self) {
        while let Some(&ch) = self.peek() {
            if ch.is_whitespace() {
                self.advance();
            } else if ch == ';' {
                // 行注释：跳过直到行尾或文件尾
                while let Some(&c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    /// 解析字符串字面量，若未闭合则返回错误
    fn read_string(&mut self, start_byte: usize, start_line: usize, start_col: usize) -> Result<Token, Diagnostic> {
        self.advance(); // 消费起始引号
        let mut content = String::new();

        while let Some(&ch) = self.peek() {
            if ch == '"' {
                self.advance(); // 消费闭合引号
                let span = Span::new(start_byte, self.pos, start_line, start_col);
                return Ok(Token::String(content, span));
            }
            content.push(ch);
            self.advance();
        }

        // 遇到 EOF 仍未闭合
        let span = Span::new(start_byte, self.pos, start_line, start_col);
        Err(Diagnostic::new(
            format!("unclosed string literal: `{}...`", content),
            self.file_name.clone(),
            span,
        ))
    }

    /// 解析标识符或数字，返回带 Span 的 Token
    fn read_ident_or_number(&mut self) -> Token {
        let start_byte = self.pos;
        let start_line = self.line;
        let start_col = self.column;
        let mut text = String::new();

        while let Some(&ch) = self.peek() {
            if ch.is_whitespace() || ch == '(' || ch == ')' || ch == ';' {
                break;
            }
            text.push(ch);
            self.advance();
        }

        let span = Span::new(start_byte, self.pos, start_line, start_col);

        // 如果第一个字符是数字，则认为是数字字面量
        if text.chars().next().map_or(false, |c| c.is_ascii_digit()) {
            Token::Number(text, span)
        } else {
            Token::Ident(text, span)
        }
    }

    /// 主解析函数，返回 Token 列表或带位置信息的错误
    fn tokenize(&mut self) -> Result<Vec<Token>, Diagnostic> {
        let mut tokens = Vec::new();

        while let Some(&_) = self.peek() {
            self.skip_whitespace_and_comments();

            let Some(&ch) = self.peek() else { break };

            match ch {
                '(' => {
                    let start_byte = self.pos;
                    let start_line = self.line;
                    let start_col = self.column;
                    self.advance();
                    let span = Span::new(start_byte, self.pos, start_line, start_col);
                    tokens.push(Token::LParen(span));
                }
                ')' => {
                    let start_byte = self.pos;
                    let start_line = self.line;
                    let start_col = self.column;
                    self.advance();
                    let span = Span::new(start_byte, self.pos, start_line, start_col);
                    tokens.push(Token::RParen(span));
                }
                '"' => {
                    let start_byte = self.pos;
                    let start_line = self.line;
                    let start_col = self.column;
                    let string_token = self.read_string(start_byte, start_line, start_col)?;
                    tokens.push(string_token);
                }
                _ => {
                    // 非法字符检查（可选，增强错误体验）
                    if !ch.is_ascii_graphic() && !ch.is_whitespace() {
                        let span = Span::new(self.pos, self.pos + ch.len_utf8(), self.line, self.column);
                        return Err(Diagnostic::new(
                            format!("unexpected character `{}`", ch),
                            self.file_name.clone(),
                            span,
                        ));
                    }

                    let token = self.read_ident_or_number();
                    tokens.push(token);
                }
            }
        }

        // 文件结束，生成 Eof token，位置为文件末尾
        let eof_span = Span::new(self.pos, self.pos, self.line, self.column);
        tokens.push(Token::Eof(eof_span));
        Ok(tokens)
    }
}

/// 对外接口：对源码进行词法分析，返回 Token 列表或带位置信息的错误
pub fn tokenize(source: &str, file_name: &str) -> Result<Vec<Token>, Diagnostic> {
    let mut lexer = Lexer::new(source, file_name);
    lexer.tokenize()
}