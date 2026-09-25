//! regex-engine/src/frontend/parse.rs
//!
//! Recursive-descent parser: pattern string -> `ExtPat`
//! Modeled on `Text.Regex.PDeriv.Parse`'s:
//! - `p_ere`/`p_branch`/`p_exp`/`p_atom`/ `p_group`/`p_charclass`/`p_enum`/`p_bound` 
//! 
//! Differences from the reference:
//! - `\d`/`\w`/`\s`/`\D`/`\W`/`\S` are read as PCRE character-class shorthands
//!   (`Translate.hs`reads a bare `EEscape c` as the literal character `c`)
//! 
//! - Backreferences (`\1`..`\9`) and lookaround (`(?=`, `(?!`, `(?<=`, `(?<!`) are
//!   rejected here (not read as the reference's 3-digit octal-ASCII escape
//!   (`p_oct_ascii`) or literal digit). `\b`/`\B` parse into `ExtPat::WordBoundary`;
//!   `translate` rejects it, since a plain `Regex` can't represent a
//!   position-dependent assertion.
//!
//! - Named groups `(?<name>...)` / `(?P<name>...)` are accepted 
//! 
//! - `\xHH` (2-digit hex escape) is accepted

use super::alphabet;
use super::ext_pattern::ExtPat;

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Characters with regex meaning at top level 
/// - Reference's `specials = "^.[$()|*+?{\\"`) 
/// - `]` and `}` deliberately absent: only meaningful in context of already-open `[` / `{`
const SPECIALS: &str = "^.[$()|*+?{\\";

/// Parse a full pattern string into an `ExtPat`
/// Errors on any leftover unparsed suffix (e.g. an unmatched `)`)
pub fn parse_ext_pattern(s: &str) -> Result<ExtPat, String> {
    let mut chars = s.chars().peekable();
    let pat = parse_or(&mut chars)?;
    if let Some(c) = chars.peek() {
        return Err(format!("unexpected trailing character {:?} (unmatched ')'?)", c));
    }
    Ok(pat)
}

// -------------------------------
// a|b|c
// -------------------------------

fn parse_or(chars: &mut Chars) -> Result<ExtPat, String> {
    let mut alts = vec![parse_concat(chars)?];
    while chars.peek() == Some(&'|') {
        chars.next();
        alts.push(parse_concat(chars)?);
    }
    Ok(if alts.len() == 1 { alts.pop().unwrap() } else { ExtPat::Or(alts) })
}

// -------------------------------
// ab c  (implicit concatenation, stops at '|' or ')')
// -------------------------------

fn parse_concat(chars: &mut Chars) -> Result<ExtPat, String> {
    let mut parts = Vec::new();
    while let Some(&c) = chars.peek() {
        if c == '|' || c == ')' {
            break;
        }
        parts.push(parse_postfixed(chars)?);
    }
    match parts.len() {
        0 => Ok(ExtPat::Empty), // missing branch in "a|" or "(|b)"
        1 => Ok(parts.pop().unwrap()),
        _ => Ok(ExtPat::Concat(parts)),
    }
}

// -------------------------------
// atom postfixed by at most one of ? + * {m,n}
// like single (non-looping) p_post_anchor_or_atom application
// -------------------------------

fn parse_postfixed(chars: &mut Chars) -> Result<ExtPat, String> {
    let atom = parse_anchor_or_atom(chars)?;
    match chars.peek() {
        Some('?') => {
            chars.next();
            Ok(ExtPat::Opt(Box::new(atom), consume_lazy_marker(chars)))
        }
        Some('+') => {
            chars.next();
            Ok(ExtPat::Plus(Box::new(atom), consume_lazy_marker(chars)))
        }
        Some('*') => {
            chars.next();
            Ok(ExtPat::Star(Box::new(atom), consume_lazy_marker(chars)))
        }
        Some('{') => Ok(parse_bound(chars, atom)),
        _ => Ok(atom),
    }
}

/// Consumes trailing `?` (marking operator as lazy) if present
/// Returns whether operator is greedy (`true` unless a `?` followed)
fn consume_lazy_marker(chars: &mut Chars) -> bool {
    if chars.peek() == Some(&'?') {
        chars.next();
        false
    } else {
        true
    }
}

fn parse_anchor_or_atom(chars: &mut Chars) -> Result<ExtPat, String> {
    match chars.peek() {
        Some('^') => {
            chars.next();
            Ok(ExtPat::Carat)
        }
        Some('$') => {
            chars.next();
            Ok(ExtPat::Dollar)
        }
        _ => parse_atom(chars),
    }
}

fn parse_atom(chars: &mut Chars) -> Result<ExtPat, String> {
    match chars.next() {
        Some('(') => parse_group(chars),
        Some('[') => parse_charclass(chars),
        Some('.') => Ok(ExtPat::Dot),
        Some('\\') => parse_escape(chars),
        Some('ε') => Ok(ExtPat::Eps),
        Some('∅') => Ok(ExtPat::Never),
        Some(c) if SPECIALS.contains(c) => {
            Err(format!("unexpected special character {:?} here", c))
        }
        Some(c) => Ok(ExtPat::Char(c)),
        None => Err("unexpected end of pattern".to_string()),
    }
}

// -------------------------------
// {m,n} / {m,} / {m} only consumed if it's a genuine bound
// - a '{' that doesn't form one is left unconsumed
// (a bare stray '{' then fails at the next p_atom, since '{' is in `specials` )
// -------------------------------

fn parse_bound(chars: &mut Chars, atom: ExtPat) -> ExtPat {
    let mut trial = chars.clone();
    match try_parse_bound_spec(&mut trial) {
        Some((lo, hi, greedy)) => {
            *chars = trial;
            ExtPat::Bound(Box::new(atom), lo, hi, greedy)
        }
        None => atom, // '{' left unconsumed; next parse_atom will error on it
    }
}

fn try_parse_bound_spec(chars: &mut Chars) -> Option<(u32, Option<u32>, bool)> {
    if chars.next() != Some('{') {
        return None;
    }
    let lo = read_digits(chars)?;
    let hi: Option<u32> = if chars.peek() == Some(&',') {
        chars.next();
        read_digits(chars) // None here means unbounded "{m,}"
    } else {
        Some(lo) // "{m}" exact
    };
    if chars.next() != Some('}') {
        return None;
    }
    if let Some(h) = hi {
        if h < lo {
            return None;
        }
    }
    let greedy = if chars.peek() == Some(&'?') {
        chars.next();
        false
    } else {
        true
    };
    Some((lo, hi, greedy))
}

fn read_digits(chars: &mut Chars) -> Option<u32> {
    let mut s = String::new();
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            s.push(c);
            chars.next();
        } else {
            break;
        }
    }
    if s.is_empty() { None } else { s.parse().ok() }
}

// -------------------------------
// ( ... )  (?: ... )  (?<name> ... )  (?P<name> ... )
// (?= (?! (?<= (?<!  -- rejected
// -------------------------------

fn parse_group(chars: &mut Chars) -> Result<ExtPat, String> {
    if chars.peek() != Some(&'?') {
        let inner = parse_or(chars)?;
        expect_close_paren(chars)?;
        return Ok(ExtPat::Group(Box::new(inner)));
    }
    chars.next(); // consume '?'
    match chars.peek() {
        Some(':') => {
            chars.next();
            let inner = parse_or(chars)?;
            expect_close_paren(chars)?;
            Ok(ExtPat::GroupNonMarking(Box::new(inner)))
        }
        Some('=') => Err("lookahead '(?=...)' is not a regular-language construct -- unsupported".to_string()),
        Some('!') => Err("negative lookahead '(?!...)' is not a regular-language construct -- unsupported".to_string()),
        Some('P') => {
            chars.next();
            if chars.peek() == Some(&'=') {
                return Err("named backreference '(?P=...)' is not a regular-language construct -- unsupported".to_string());
            }
            expect_char(chars, '<')?;
            skip_group_name(chars)?;
            let inner = parse_or(chars)?;
            expect_close_paren(chars)?;
            Ok(ExtPat::Group(Box::new(inner)))
        }
        Some('<') => {
            chars.next();
            match chars.peek() {
                Some('=') => Err("lookbehind '(?<=...)' is not a regular-language construct -- unsupported".to_string()),
                Some('!') => Err("negative lookbehind '(?<!...)' is not a regular-language construct -- unsupported".to_string()),
                _ => {
                    skip_group_name(chars)?;
                    let inner = parse_or(chars)?;
                    expect_close_paren(chars)?;
                    Ok(ExtPat::Group(Box::new(inner)))
                }
            }
        }
        other => Err(format!("unsupported '(?{}' group syntax", other.map(|c| c.to_string()).unwrap_or_default())),
    }
}

fn skip_group_name(chars: &mut Chars) -> Result<(), String> {
    while let Some(&c) = chars.peek() {
        if c == '>' {
            chars.next();
            return Ok(());
        }
        chars.next();
    }
    Err("unterminated group name (expected '>')".to_string())
}

fn expect_close_paren(chars: &mut Chars) -> Result<(), String> {
    match chars.next() {
        Some(')') => Ok(()),
        other => Err(format!("expected ')', found {:?}", other)),
    }
}

fn expect_char(chars: &mut Chars, expected: char) -> Result<(), String> {
    match chars.next() {
        Some(c) if c == expected => Ok(()),
        other => Err(format!("expected {:?}, found {:?}", expected, other)),
    }
}

// -------------------------------
// \c - escapes outside a character class
// -------------------------------

fn parse_escape(chars: &mut Chars) -> Result<ExtPat, String> {
    let c = chars.next().ok_or_else(|| "unexpected end of pattern after '\\'".to_string())?;
    match c {
        'n' => Ok(ExtPat::Char('\n')),
        't' => Ok(ExtPat::Char('\t')),
        'r' => Ok(ExtPat::Char('\r')),
        'x' => parse_hex_escape(chars),
        '1'..='9' => Err(format!(
            "backreference '\\{}' is not a regular-language construct -- unsupported",
            c
        )),
        // Word boundary / non-boundary. `translate` rejects these, since a plain
        // `Regex` can't represent a position-dependent assertion. Only outside a
        // character class: `[\b]` is parsed by `parse_charclass`'s own escape
        // handling, unaffected by this.
        'b' => Ok(ExtPat::WordBoundary(true)),
        'B' => Ok(ExtPat::WordBoundary(false)),
        _ => Ok(ExtPat::Escape(c)),
    }
}

fn parse_hex_escape(chars: &mut Chars) -> Result<ExtPat, String> {
    Ok(ExtPat::Char(read_hex_byte(chars)? as char))
}

fn read_hex_byte(chars: &mut Chars) -> Result<u8, String> {
    let mut take_hex = || chars.next().filter(|c| c.is_ascii_hexdigit());
    let h1 = take_hex().ok_or_else(|| "expected 2 hex digits after '\\x'".to_string())?;
    let h2 = take_hex().ok_or_else(|| "expected 2 hex digits after '\\x'".to_string())?;
    u8::from_str_radix(&format!("{h1}{h2}"), 16).map_err(|e| e.to_string())
}

// -------------------------------
// [ ... ]  [^ ... ]
// -------------------------------

fn parse_charclass(chars: &mut Chars) -> Result<ExtPat, String> {
    let negated = if chars.peek() == Some(&'^') {
        chars.next();
        true
    } else {
        false
    };
    let members = parse_class_enum(chars)?;
    Ok(if negated { ExtPat::NoneOf(members) } else { ExtPat::Any(members) })
}

fn parse_class_enum(chars: &mut Chars) -> Result<Vec<char>, String> {
    let mut members = Vec::new();
    // Reference (p_enum): a ']' or '-' immediately after '[' / '[^' is a literal member
    // not the terminator / a range operator 
    // Otherwise "[]f-z]" (']' plus the range 'f'..'z') could never be written
    match chars.peek() {
        Some(&']') => {
            chars.next();
            members.push(']');
        }
        Some(&'-') => {
            chars.next();
            members.push('-');
        }
        _ => {}
    }

    loop {
        match chars.peek() {
            Some(&']') => {
                chars.next();
                break;
            }
            None => return Err("unterminated character class (expected ']')".to_string()),
            _ => members.extend(parse_one_class_member(chars)?),
        }
    }
    if members.is_empty() {
        return Err("empty character class '[]'".to_string());
    }
    Ok(members)
}

enum ClassAtom {
    Single(char),
    Multi(Vec<char>),
}

fn parse_one_class_member(chars: &mut Chars) -> Result<Vec<char>, String> {
    let start = parse_class_atom(chars)?;
    let lo = match start {
        ClassAtom::Multi(cs) => return Ok(cs), // \d \w \s ... : never a range endpoint
        ClassAtom::Single(c) => c,
    };
    if chars.peek() == Some(&'-') {
        let mut trial = chars.clone();
        trial.next(); // consume '-' in the trial
        if trial.peek() != Some(&']') && trial.peek().is_some() {
            // range: commit the '-', then parse the end char
            chars.next();
            let hi = parse_class_range_end(chars)?;
            if hi < lo {
                return Err(format!("invalid range '{}-{}' in character class (end before start)", lo, hi));
            }
            return Ok((lo..=hi).collect());
        }
        // '-' immediately before ']' (or end of input): literal trailing dash
    }
    Ok(vec![lo])
}

fn parse_class_atom(chars: &mut Chars) -> Result<ClassAtom, String> {
    match chars.next() {
        Some('\\') => {
            let c = chars
                .next()
                .ok_or_else(|| "unexpected end of pattern after '\\' in character class".to_string())?;
            match c {
                'n' => Ok(ClassAtom::Single('\n')),
                't' => Ok(ClassAtom::Single('\t')),
                'r' => Ok(ClassAtom::Single('\r')),
                'x' => Ok(ClassAtom::Single(read_hex_byte(chars)? as char)),
                'd' => Ok(ClassAtom::Multi(alphabet::digit_chars())),
                'D' => Ok(ClassAtom::Multi(alphabet::complement(&alphabet::digit_chars()))),
                'w' => Ok(ClassAtom::Multi(alphabet::word_chars())),
                'W' => Ok(ClassAtom::Multi(alphabet::complement(&alphabet::word_chars()))),
                's' => Ok(ClassAtom::Multi(alphabet::space_chars())),
                'S' => Ok(ClassAtom::Multi(alphabet::complement(&alphabet::space_chars()))),
                other => Ok(ClassAtom::Single(other)), // \] \- \\ \^ etc: literal
            }
        }
        Some(c) => Ok(ClassAtom::Single(c)),
        None => Err("unterminated character class (expected ']')".to_string()),
    }
}

fn parse_class_range_end(chars: &mut Chars) -> Result<char, String> {
    match parse_class_atom(chars)? {
        ClassAtom::Single(c) => Ok(c),
        ClassAtom::Multi(_) => {
            Err("a character-class shorthand (\\d, \\w, \\s, ...) cannot be a range endpoint".to_string())
        }
    }
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> ExtPat {
        parse_ext_pattern(s).unwrap_or_else(|e| panic!("parse({:?}) failed: {}", s, e))
    }

    #[test]
    fn single_char() {
        assert_eq!(p("a"), ExtPat::Char('a'));
    }

    #[test]
    fn concat_two_chars() {
        assert_eq!(p("ab"), ExtPat::Concat(vec![ExtPat::Char('a'), ExtPat::Char('b')]));
    }

    #[test]
    fn alternation() {
        assert_eq!(p("a|b"), ExtPat::Or(vec![ExtPat::Char('a'), ExtPat::Char('b')]));
    }

    #[test]
    fn plain_group() {
        assert_eq!(p("(a)"), ExtPat::Group(Box::new(ExtPat::Char('a'))));
    }

    #[test]
    fn non_marking_group() {
        assert_eq!(p("(?:a)"), ExtPat::GroupNonMarking(Box::new(ExtPat::Char('a'))));
    }

    #[test]
    fn named_group_angle_syntax_drops_the_name() {
        assert_eq!(p("(?<foo>a)"), ExtPat::Group(Box::new(ExtPat::Char('a'))));
    }

    #[test]
    fn named_group_python_syntax_drops_the_name() {
        assert_eq!(p("(?P<foo>a)"), ExtPat::Group(Box::new(ExtPat::Char('a'))));
    }

    #[test]
    fn lookahead_is_rejected() {
        assert!(parse_ext_pattern("a(?=b)").is_err());
    }

    #[test]
    fn negative_lookahead_is_rejected() {
        assert!(parse_ext_pattern("a(?!b)").is_err());
    }

    #[test]
    fn lookbehind_is_rejected() {
        assert!(parse_ext_pattern("(?<=a)b").is_err());
    }

    #[test]
    fn negative_lookbehind_is_rejected() {
        assert!(parse_ext_pattern("(?<!a)b").is_err());
    }

    #[test]
    fn word_boundary_parses() {
        assert_eq!(p(r"\b"), ExtPat::WordBoundary(true));
        assert_eq!(p(r"\B"), ExtPat::WordBoundary(false));
    }

    #[test]
    fn backreference_is_rejected() {
        assert!(parse_ext_pattern(r"(a)\1").is_err());
    }

    #[test]
    fn named_backreference_is_rejected_with_a_clear_message() {
        let err = parse_ext_pattern(r"(?P<var1>a).(?P=var1)").unwrap_err();
        assert!(err.contains("backreference"), "error should say backreference, got: {}", err);
    }

    #[test]
    fn dot_star_plus_opt() {
        assert_eq!(p("."), ExtPat::Dot);
        assert_eq!(p("a*"), ExtPat::Star(Box::new(ExtPat::Char('a')), true));
        assert_eq!(p("a+"), ExtPat::Plus(Box::new(ExtPat::Char('a')), true));
        assert_eq!(p("a?"), ExtPat::Opt(Box::new(ExtPat::Char('a')), true));
    }

    #[test]
    fn lazy_variants_carry_greedy_false() {
        assert_eq!(p("a*?"), ExtPat::Star(Box::new(ExtPat::Char('a')), false));
        assert_eq!(p("a+?"), ExtPat::Plus(Box::new(ExtPat::Char('a')), false));
        assert_eq!(p("a??"), ExtPat::Opt(Box::new(ExtPat::Char('a')), false));
    }

    #[test]
    fn anchors() {
        assert_eq!(p("^a$"), ExtPat::Concat(vec![ExtPat::Carat, ExtPat::Char('a'), ExtPat::Dollar]));
    }

    #[test]
    fn bound_exact() {
        assert_eq!(p("a{3}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 3, Some(3), true));
    }

    #[test]
    fn bound_range() {
        assert_eq!(p("a{2,4}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, Some(4), true));
    }

    #[test]
    fn bound_unbounded() {
        assert_eq!(p("a{2,}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, None, true));
    }

    #[test]
    fn bound_lazy() {
        assert_eq!(p("a{2,4}?"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, Some(4), false));
    }

    #[test]
    fn invalid_bound_falls_back_and_then_errors_on_stray_brace() {
        // "{9,2}" (hi < lo) isn't a valid bound spec
        // '{' is left unconsumed for the next atom, which then fails
        // becuase '{' is a special character with no literal-fallback meaning
        assert!(parse_ext_pattern("a{9,2}").is_err());
    }

    #[test]
    fn simple_char_class() {
        assert_eq!(p("[abc]"), ExtPat::Any(vec!['a', 'b', 'c']));
    }

    #[test]
    fn negated_char_class() {
        assert_eq!(p("[^abc]"), ExtPat::NoneOf(vec!['a', 'b', 'c']));
    }

    #[test]
    fn char_class_range() {
        assert_eq!(p("[a-c]"), ExtPat::Any(vec!['a', 'b', 'c']));
    }

    #[test]
    fn char_class_leading_bracket_is_literal() {
        // "[]a]" = the set {']', 'a'} -- a ']' right after '[' is a literal member
        // not the terminator (reference's p_enum)
        assert_eq!(p("[]a]"), ExtPat::Any(vec![']', 'a']));
    }

    #[test]
    fn char_class_trailing_dash_is_literal() {
        assert_eq!(p("[a-]"), ExtPat::Any(vec!['a', '-']));
    }

    #[test]
    fn char_class_digit_shorthand_expands_inline() {
        assert_eq!(p(r"[\da-f]"), ExtPat::Any({
            let mut v = alphabet::digit_chars();
            v.extend(['a', 'b', 'c', 'd', 'e', 'f']);
            v
        }));
    }

    #[test]
    fn char_class_shorthand_cannot_be_range_endpoint() {
        assert!(parse_ext_pattern(r"[a-\d]").is_err());
    }

    #[test]
    fn eps_and_never_tokens() {
        assert_eq!(p("ε"), ExtPat::Eps);
        assert_eq!(p("∅"), ExtPat::Never);
        assert_eq!(p("aεb"), ExtPat::Concat(vec![ExtPat::Char('a'), ExtPat::Eps, ExtPat::Char('b')]));
    }

    #[test]
    fn hex_escape() {
        assert_eq!(p(r"\x41"), ExtPat::Char('A'));
    }

    #[test]
    fn escape_shorthands_are_kept_symbolic_for_translate() {
        assert_eq!(p(r"\d"), ExtPat::Escape('d'));
        assert_eq!(p(r"\w"), ExtPat::Escape('w'));
        assert_eq!(p(r"\s"), ExtPat::Escape('s'));
    }

    #[test]
    fn punctuation_escape() {
        assert_eq!(p(r"\."), ExtPat::Escape('.'));
    }

    #[test]
    fn unterminated_class_is_an_error() {
        assert!(parse_ext_pattern("[abc").is_err());
    }

    #[test]
    fn unmatched_close_paren_is_an_error() {
        assert!(parse_ext_pattern("a)").is_err());
    }

    #[test]
    fn unmatched_open_paren_is_an_error() {
        assert!(parse_ext_pattern("(a").is_err());
    }

    // A real dataset-style pattern: simplified IP-octet-group shape
    #[test]
    fn realistic_nested_pattern_shape() {
        let ep = p(r"(\d{1,3}\.){3}\d{1,3}");
        match ep {
            ExtPat::Concat(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[0], ExtPat::Bound(_, 3, Some(3), true)));
                assert!(matches!(parts[1], ExtPat::Bound(_, 1, Some(3), true)));
            }
            other => panic!("expected Concat, got {:?}", other),
        }
    }
}
