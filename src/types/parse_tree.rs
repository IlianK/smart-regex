//! Parse tree representation (values v)

use std::fmt;

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

/// Flattens a parse tree to the underlying word (string)
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


// ============================================================================
// Tests for ParseTree and flatten
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
 
    // flatten: private recursive logic
    #[test]
    fn flatten_char_unit() {
        assert_eq!(flatten(&ParseTree::Char('z')), "z");
    }
 
    #[test]
    fn flatten_pair_unit() {
        let t = ParseTree::Pair(
            Box::new(ParseTree::Char('a')),
            Box::new(ParseTree::Char('b')),
        );
        assert_eq!(flatten(&t), "ab");
    }
 
    #[test]
    fn flatten_left_unit() {
        assert_eq!(flatten(&ParseTree::Left(Box::new(ParseTree::Char('x')))), "x");
    }
 
    #[test]
    fn flatten_right_unit() {
        assert_eq!(flatten(&ParseTree::Right(Box::new(ParseTree::Char('y')))), "y");
    }
 
    #[test]
    fn flatten_star_unit() {
        let t = ParseTree::Star(vec![ParseTree::Char('a'), ParseTree::Char('b')]);
        assert_eq!(flatten(&t), "ab");
    }
 
    // Display
    #[test]
    fn display_pair_unit() {
        let t = ParseTree::Pair(
            Box::new(ParseTree::Char('a')),
            Box::new(ParseTree::Char('b')),
        );
        assert_eq!(format!("{}", t), "(a, b)");
    }
}
