//! src/types/regex.rs
//! 
//! Regular expression data type definition

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Regex {
    /// Empty language: L(Phi) = {}
    Phi,

    /// Empty word: L(Eps) = {epsilon}
    Eps,

    /// Single character: L(Lit('a')) = {"a"}
    Lit(char),

    /// Sequence: L(Seq(r,s)) = { v++w | v in L(r), w in L(s) }
    Seq(Box<Regex>, Box<Regex>),

    /// Alternative: L(Alt(r,s)) = L(r) union L(s)
    Alt(Box<Regex>, Box<Regex>),
    
    /// Kleene star: L(Star(r)) = {epsilon} union L(r)·L(Star(r))
    Star(Box<Regex>),
}

impl Regex {
    pub fn seq(r: Regex, s: Regex) -> Regex {
        Regex::Seq(Box::new(r), Box::new(s))
    }

    pub fn alt(r: Regex, s: Regex) -> Regex {
        Regex::Alt(Box::new(r), Box::new(s))
    }

    pub fn star(r: Regex) -> Regex {
        Regex::Star(Box::new(r))
    }

    pub fn lit(c: char) -> Regex {
        Regex::Lit(c)
    }
}

/// Paper notation, matching `ARegex`'s `Display` minus the bit-codes:
/// `φ`/`ε` for `Phi`/`Eps`, `+` for `Alt` (this repo's own demo labels
/// already write e.g. `(a+ab)(b+ε)`), juxtaposition for `Seq`, `(...)*`
/// for `Star`. Parens are added only where precedence would otherwise
/// change the reading -- around an `Alt` nested under `Seq`, and around
/// anything but a bare atom nested under `Star`.
impl std::fmt::Display for Regex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Regex::Phi => write!(f, "φ"),
            Regex::Eps => write!(f, "ε"),
            Regex::Lit(c) => write_lit(f, *c),
            Regex::Seq(r, s) => {
                write_seq_operand(f, r)?;
                write_seq_operand(f, s)
            }
            Regex::Alt(r, s) => write!(f, "{r} + {s}"),
            Regex::Star(r) => {
                if matches!(r.as_ref(), Regex::Phi | Regex::Eps | Regex::Lit(_)) {
                    write!(f, "{r}*")
                } else {
                    write!(f, "({r})*")
                }
            }
        }
    }
}

fn write_seq_operand(f: &mut std::fmt::Formatter<'_>, r: &Regex) -> std::fmt::Result {
    if matches!(r, Regex::Alt(..)) {
        write!(f, "({r})")
    } else {
        write!(f, "{r}")
    }
}

/// A literal that is itself one of this `Display`'s own metacharacters
/// (`(`, `)`, `+`, `*`, or the escape marker `\` itself) would
/// otherwise be unreadable from a structural use of the same
/// character. A literal control character (real corpus patterns do
/// contain raw `\x0D`/`\x0A` -- CR/LF -- matching network line
/// endings) would otherwise be written straight into the output,
/// breaking a one-row-per-pattern CSV or text file. Escaped the same
/// way `char::escape_default` already renders unprintable characters
/// elsewhere in Rust, so both look familiar rather than inventing a
/// second convention.
fn write_lit(f: &mut std::fmt::Formatter<'_>, c: char) -> std::fmt::Result {
    match c {
        '(' | ')' | '+' | '*' | '\\' => write!(f, "\\{c}"),
        c if c.is_control() => write!(f, "{}", c.escape_default()),
        c => write!(f, "{c}"),
    }
}

// -------------------------------
// Tests for Regex
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lit_constructor() {
        assert_eq!(Regex::lit('a'), Regex::Lit('a'));
    }

    #[test]
    fn seq_constructor() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        assert_eq!(r, Regex::Seq(Box::new(Regex::Lit('a')), Box::new(Regex::Lit('b'))));
    }

    #[test]
    fn alt_constructor() {
        let r = Regex::alt(Regex::lit('a'), Regex::Eps);
        assert_eq!(r, Regex::Alt(Box::new(Regex::Lit('a')), Box::new(Regex::Eps)));
    }

    #[test]
    fn star_constructor() {
        let r = Regex::star(Regex::lit('a'));
        assert_eq!(r, Regex::Star(Box::new(Regex::Lit('a'))));
    }

    #[test]
    fn clone_and_eq() {
        let r = Regex::seq(Regex::star(Regex::lit('a')), Regex::lit('b'));
        assert_eq!(r.clone(), r);
    }

    #[test]
    fn phi_and_eps_are_distinct() {
        assert_ne!(Regex::Phi, Regex::Eps);
    }

    #[test]
    fn display_atoms() {
        assert_eq!(format!("{}", Regex::Phi), "φ");
        assert_eq!(format!("{}", Regex::Eps), "ε");
        assert_eq!(format!("{}", Regex::lit('a')), "a");
    }

    #[test]
    fn display_seq_is_juxtaposition() {
        let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
        assert_eq!(format!("{}", r), "ab");
    }

    #[test]
    fn display_alt_uses_plus() {
        let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
        assert_eq!(format!("{}", r), "a + b");
    }

    #[test]
    fn display_seq_wraps_alt_operand() {
        let r = Regex::seq(Regex::lit('a'), Regex::alt(Regex::lit('b'), Regex::lit('c')));
        assert_eq!(format!("{}", r), "a(b + c)");
    }

    #[test]
    fn display_star_atom_has_no_redundant_parens() {
        assert_eq!(format!("{}", Regex::star(Regex::lit('a'))), "a*");
    }

    #[test]
    fn display_star_wraps_non_atomic_body() {
        let r = Regex::star(Regex::seq(Regex::lit('a'), Regex::lit('b')));
        assert_eq!(format!("{}", r), "(ab)*");
    }

    #[test]
    fn display_escapes_own_metacharacters() {
        assert_eq!(format!("{}", Regex::lit('(')), "\\(");
        assert_eq!(format!("{}", Regex::lit(')')), "\\)");
        assert_eq!(format!("{}", Regex::lit('+')), "\\+");
        assert_eq!(format!("{}", Regex::lit('*')), "\\*");
        assert_eq!(format!("{}", Regex::lit('\\')), "\\\\");
    }

    #[test]
    fn display_escapes_control_characters() {
        // Real corpus patterns match literal CR/LF (network line endings);
        // printed raw, these would break a one-row-per-pattern CSV or
        // text file instead of reading as a character.
        assert_eq!(format!("{}", Regex::lit('\r')), "\\r");
        assert_eq!(format!("{}", Regex::lit('\n')), "\\n");
        assert_eq!(format!("{}", Regex::lit('\t')), "\\t");
    }

    #[test]
    fn display_matches_this_repos_own_demo_labels() {
        // examples/demo_parse.rs names this case "(a+ab)(b+ε)".
        let r = Regex::seq(
            Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
            Regex::alt(Regex::lit('b'), Regex::Eps),
        );
        assert_eq!(format!("{}", r), "(a + ab)(b + ε)");
    }
}