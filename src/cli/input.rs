//! regex-engine/src/cli/input.rs
//! 
//! Convert user-provided regex string into internal Regex enum

use regex_engine::types::Regex;

pub fn parse_regex_string(s: &str) -> Result<Regex, String> {
    let mut chars = s.chars().peekable();
    parse_alt(&mut chars)
}

fn parse_alt(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Regex, String> {
    let mut left = parse_seq(chars)?;
    while let Some(&c) = chars.peek() {
        if c == '|' || c == '+' {
            chars.next();
            let right = parse_seq(chars)?;
            left = Regex::alt(left, right);
        } else {
            break;
        }
    }
    Ok(left)
}

fn parse_seq(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Regex, String> {
    let mut left = parse_atom(chars)?;
    while let Some(&c) = chars.peek() {
        if c == '|' || c == '+' || c == ')' {
            break;
        }
        let right = parse_atom(chars)?;
        left = Regex::seq(left, right);
    }
    Ok(left)
}

fn parse_atom(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<Regex, String> {
    match chars.next() {
        Some('ε') => Ok(Regex::Eps),
        Some('∅') => Ok(Regex::Phi),
        Some('(') => {
            let inner = parse_alt(chars)?;
            match chars.next() {
                Some(')') => {
                    let mut r = inner;
                    if chars.peek() == Some(&'*') {
                        chars.next();
                        r = Regex::star(r);
                    }
                    Ok(r)
                }
                _ => Err("Expected ')'".to_string()),
            }
        }
        Some(c) if c.is_alphabetic() => {
            let mut r = Regex::lit(c);
            if chars.peek() == Some(&'*') {
                chars.next();
                r = Regex::star(r);
            }
            Ok(r)
        }
        Some(c) => Err(format!("Unexpected character: {}", c)),
        None => Err("Unexpected end of input".to_string()),
    }
}