// parser.rs 完整修改

use crate::error::{Diagnostic, Span};
use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    Atom(String, Span),   // 原子携带它的位置
    List(Vec<SExpr>, Span), // 列表携带整个列表的位置（左括号到右括号）
}

pub struct Parser {
    tokens: Vec<Token>,
    file_name: String,
    index: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, file_name: &str) -> Self {
        Parser {
            tokens,
            file_name: file_name.to_string(),
            index: 0,
        }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.index).cloned();
        self.index += 1;
        tok
    }

    /// 解析所有顶层 S 表达式
    pub fn parse_sexprs(&mut self) -> Result<Vec<SExpr>, Diagnostic> {
        let mut sexprs = Vec::new();
        while let Some(tok) = self.peek() {
            match tok {
                Token::Eof(_) => break,
                Token::LParen(_) => {
                    let list = self.parse_list()?;
                    sexprs.push(list);
                }
                Token::RParen(span) => {
                    return Err(Diagnostic::new(
                        "unexpected ')'".to_string(),
                        self.file_name.clone(),
                        *span,
                    ));
                }
                Token::Ident(_, span) | Token::Number(_, span) | Token::String(_, span) => {
                    let (atom, atom_span) = match self.advance().unwrap() {
                        Token::Ident(s, sp) => (s, sp),
                        Token::Number(s, sp) => (s, sp),
                        Token::String(s, sp) => (s, sp),
                        _ => unreachable!(),
                    };
                    sexprs.push(SExpr::Atom(atom, atom_span));
                }
            }
        }
        Ok(sexprs)
    }

    /// 解析一个列表：从 '(' 开始到匹配的 ')'
    fn parse_list(&mut self) -> Result<SExpr, Diagnostic> {
        // 消费左括号，获取其 Span
        let lparen_span = match self.advance() {
            Some(Token::LParen(span)) => span,
            _ => {
                return Err(Diagnostic::new(
                    "expected '('".to_string(),
                    self.file_name.clone(),
                    Span::new(0, 0, 1, 1), // fallback, shouldn't happen
                ));
            }
        };

        let mut elements = Vec::new();
        let mut closing_span = lparen_span; // 临时值

        while let Some(tok) = self.peek() {
            match tok {
                Token::RParen(span) => {
                    closing_span = *span;
                    self.advance(); // 消费 ')'
                    let list_span = Span::new(lparen_span.start, closing_span.end, lparen_span.line, lparen_span.column);
                    return Ok(SExpr::List(elements, list_span));
                }
                Token::LParen(_) => {
                    let sub = self.parse_list()?;
                    elements.push(sub);
                }
                Token::Ident(_, _) | Token::Number(_, _) | Token::String(_, _) => {
                    let (atom, atom_span) = match self.advance().unwrap() {
                        Token::Ident(s, sp) => (s, sp),
                        Token::Number(s, sp) => (s, sp),
                        Token::String(s, sp) => (s, sp),
                        _ => unreachable!(),
                    };
                    elements.push(SExpr::Atom(atom, atom_span));
                }
                Token::Eof(span) => {
                    return Err(Diagnostic::new(
                        "unclosed '('".to_string(),
                        self.file_name.clone(),
                        *span,
                    ));
                }
            }
        }

        Err(Diagnostic::new(
            "unclosed '('".to_string(),
            self.file_name.clone(),
            lparen_span,
        ))
    }
}

/// 对外接口：解析 Token 流，返回 S 表达式列表或带位置的错误
pub fn parse_sexprs(tokens: Vec<Token>, file_name: &str) -> Result<Vec<SExpr>, Diagnostic> {
    let mut parser = Parser::new(tokens, file_name);
    parser.parse_sexprs()
}