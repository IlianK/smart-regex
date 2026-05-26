//! Parse tree representation (values v)

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseTree {
    Empty,
    Char(char),
    Pair(Box<ParseTree>, Box<ParseTree>),
    Left(Box<ParseTree>),
    Right(Box<ParseTree>),
    Star(Vec<ParseTree>),
}

impl fmt::Display for ParseTree {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseTree::Empty => write!(f, "()"),
            ParseTree::Char(c) => write!(f, "{}", c),
            ParseTree::Pair(l, r) => write!(f, "({}, {})", l, r),
            ParseTree::Left(v) => write!(f, "Left {}", v),
            ParseTree::Right(v) => write!(f, "Right {}", v),
            ParseTree::Star(vs) => {
                let inner: Vec<String> = vs.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", inner.join(", "))
            }
        }
    }
}

pub fn flatten(v: &ParseTree) -> String {
    match v {
        ParseTree::Empty => String::new(),
        ParseTree::Char(c) => c.to_string(),
        ParseTree::Pair(v1, v2) => format!("{}{}", flatten(v1), flatten(v2)),
        ParseTree::Left(v) => flatten(v),
        ParseTree::Right(v) => flatten(v),
        ParseTree::Star(vs) => vs.iter().map(flatten).collect(),
    }
}