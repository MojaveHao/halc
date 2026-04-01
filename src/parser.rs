use crate::lexer::Token;

#[derive(Debug, Clone, PartialEq)]
pub enum SExpr {
    Atom(String),
    List(Vec<SExpr>),
}

pub fn parse_sexprs(tokens: Vec<Token>) -> Result<Vec<SExpr>, String> {
    let mut iter = tokens.into_iter().peekable();
    let mut sexprs = Vec::new();
    while let Some(token) = iter.next() {
        match token {
            Token::LParen => {
                let list = parse_list(&mut iter)?;
                sexprs.push(SExpr::List(list));
            }
            Token::RParen => return Err("Unexpected ')'".to_string()),
            Token::Ident(s) | Token::Number(s) | Token::String(s) => {
                sexprs.push(SExpr::Atom(s));
            }
            Token::Eof => break,
        }
    }
    Ok(sexprs)
}

fn parse_list(iter: &mut std::iter::Peekable<std::vec::IntoIter<Token>>) -> Result<Vec<SExpr>, String> {
    let mut list = Vec::new();
    while let Some(token) = iter.peek() {
        match token {
            Token::RParen => {
                iter.next();
                return Ok(list);
            }
            Token::LParen => {
                iter.next();
                let sub = parse_list(iter)?;
                list.push(SExpr::List(sub));
            }
            Token::Ident(s) | Token::Number(s) | Token::String(s) => {
                list.push(SExpr::Atom(s.clone()));
                iter.next();
            }
            Token::Eof => return Err("Unclosed '('".to_string()),
        }
    }
    Err("Unclosed '('".to_string())
}