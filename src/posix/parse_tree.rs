//! Parse tree representation (values v)

use std::fmt;

/// Parse tree representing a structured match of a regular expression
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseTree {
    /// Empty parse tree (for epsilon)
    Empty,

    /// Literal character
    Char(char),

    /// Pair for concatenation (v1, v2)
    Pair(Box<ParseTree>, Box<ParseTree>),

    /// Left injection for alternative (Left v)
    Left(Box<ParseTree>),

    /// Right injection for alternative (Right v)
    Right(Box<ParseTree>),

    /// List for Kleene star iterations [v1, v2, ..., vn]
    Star(Vec<ParseTree>),
}

/// Paper-style formatting (compact, mathematical notation)
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

impl ParseTree {
    pub fn debug_rust(&self) -> String {
        format!("{:?}", self)
    }
}

/// Flattens a parse tree to the underlying word (string) |v|
/// Example: v = Right((a,b)), |v| = "ab"
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