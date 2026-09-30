//! regex-engine/src/frontend/parse.rs
//!
//! Recursive-descent parser: pattern string -> `ExtPat`.
//!
//! Differences from `Text.Regex.PDeriv.Parse`:
//! - `\d`/`\w`/`\s` and their negations are read as class shorthands.
//! - Backreferences and lookaround are rejected.
//! - Named groups `(?<name>...)` / `(?P<name>...)` are accepted (name dropped).
//! - `\xHH` hex escapes are accepted.
//! - `\b` / `\B` parse to `ExtPat::WordBoundary`, which `translate` rejects.
//! - A trailing `?` after `?`, `+`, `*`, or `{...}` is accepted and discarded.

use super::alphabet;
use super::ext_pattern::ExtPat;

type Chars<'a> = std::iter::Peekable<std::str::Chars<'a>>;

/// Characters with regex meaning outside a character class. `]` and `}`
/// are absent: they only matter inside an already-open `[` or `{`.
const SPECIALS: &str = "^.[$()|*+?{\\";

pub fn parse_ext_pattern(s: &str) -> Result<ExtPat, String> {
    let mut chars = s.chars().peekable();
    let pat = parse_or(&mut chars)?;
    if let Some(c) = chars.peek() {
        return Err(format!("unexpected trailing character {:?} (unmatched ')'?)", c));
    }
    Ok(pat)
}

// a|b|c

fn parse_or(chars: &mut Chars) -> Result<ExtPat, String> {
    let mut alts = vec![parse_concat(chars)?];
    while chars.peek() == Some(&'|') {
        chars.next();
        alts.push(parse_concat(chars)?);
    }
    Ok(if alts.len() == 1 { alts.pop().unwrap() } else { ExtPat::Or(alts) })
}

// ab c  (stops at '|' or ')')

fn parse_concat(chars: &mut Chars) -> Result<ExtPat, String> {
    let mut parts = Vec::new();
    while let Some(&c) = chars.peek() {
        if c == '|' || c == ')' {
            break;
        }
        parts.push(parse_postfixed(chars)?);
    }
    match parts.len() {
        0 => Ok(ExtPat::Empty),
        1 => Ok(parts.pop().unwrap()),
        _ => Ok(ExtPat::Concat(parts)),
    }
}

// atom postfixed by at most one of ? + * {m,n}

fn parse_postfixed(chars: &mut Chars) -> Result<ExtPat, String> {
    let atom = parse_anchor_or_atom(chars)?;
    match chars.peek() {
        Some('?') => {
            chars.next();
            discard_lazy_marker(chars);
            Ok(ExtPat::Opt(Box::new(atom)))
        }
        Some('+') => {
            chars.next();
            discard_lazy_marker(chars);
            Ok(ExtPat::Plus(Box::new(atom)))
        }
        Some('*') => {
            chars.next();
            discard_lazy_marker(chars);
            Ok(ExtPat::Star(Box::new(atom)))
        }
        Some('{') => Ok(parse_bound(chars, atom)),
        _ => Ok(atom),
    }
}

/// Consumes a trailing `?` marking a quantifier lazy, if present. The
/// marker has no effect on the lowered `Regex`, so it is not recorded.
fn discard_lazy_marker(chars: &mut Chars) {
    if chars.peek() == Some(&'?') {
        chars.next();
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

// {m,n} / {m,} / {m}. A '{' that does not form a valid bound is left
// unconsumed, and the next `parse_atom` then errors on it.

fn parse_bound(chars: &mut Chars, atom: ExtPat) -> ExtPat {
    let mut trial = chars.clone();
    match try_parse_bound_spec(&mut trial) {
        Some((lo, hi)) => {
            *chars = trial;
            ExtPat::Bound(Box::new(atom), lo, hi)
        }
        None => atom,
    }
}

fn try_parse_bound_spec(chars: &mut Chars) -> Option<(u32, Option<u32>)> {
    if chars.next() != Some('{') {
        return None;
    }
    let lo = read_digits(chars)?;
    let hi: Option<u32> = if chars.peek() == Some(&',') {
        chars.next();
        read_digits(chars)
    } else {
        Some(lo)
    };
    if chars.next() != Some('}') {
        return None;
    }
    if let Some(h) = hi {
        if h < lo {
            return None;
        }
    }
    discard_lazy_marker(chars);
    Some((lo, hi))
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

// ( ... )  (?: ... )  (?<name> ... )  (?P<name> ... )
// (?= (?! (?<= (?<!  -- rejected

fn parse_group(chars: &mut Chars) -> Result<ExtPat, String> {
    if chars.peek() != Some(&'?') {
        let inner = parse_or(chars)?;
        expect_close_paren(chars)?;
        return Ok(ExtPat::Group(Box::new(inner)));
    }
    chars.next();
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

// \c outside a character class

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

// [ ... ]  [^ ... ]

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
    // A ']' or '-' immediately after '[' / '[^' is a literal member, so
    // that "[]f-z]" and "[-a]" can be written.
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
            _ => {
                if let Some(name) = posix_class_name(chars) {
                    return Err(format!(
                        "POSIX class '[:{name}:]' is not supported inside a character class \
                         (it would otherwise be silently misread as the literal characters \
                         '[', ':', and the class name); use an explicit range or a shorthand \
                         (\\d, \\w, \\s and their negations) instead"
                    ));
                }
                members.extend(parse_one_class_member(chars)?)
            }
        }
    }
    if members.is_empty() {
        return Err("empty character class '[]'".to_string());
    }
    Ok(members)
}

/// Looks ahead (without consuming) for a `[:name:]` POSIX class at the
/// current position, e.g. the `[:alpha:]` in `[[:alpha:]]`. Returns the
/// name if found. This frontend does not implement POSIX classes; without
/// this check, `[:alpha:]` would be silently read as the literal members
/// `[`, `:`, `a`, `l`, `p`, `h` followed by a literal `]` -- accepted, and
/// translated into something that denotes almost nothing like what the
/// pattern author meant, with no error and no caveat to catch it.
fn posix_class_name(chars: &Chars) -> Option<String> {
    let mut trial = chars.clone();
    if trial.next() != Some('[') || trial.next() != Some(':') {
        return None;
    }
    let mut name = String::new();
    loop {
        match trial.next() {
            Some(':') if trial.next() == Some(']') => return Some(name),
            Some(c) if c.is_ascii_lowercase() => name.push(c),
            _ => return None,
        }
    }
}

enum ClassAtom {
    Single(char),
    Multi(Vec<char>),
}

fn parse_one_class_member(chars: &mut Chars) -> Result<Vec<char>, String> {
    let start = parse_class_atom(chars)?;
    let lo = match start {
        ClassAtom::Multi(cs) => return Ok(cs),
        ClassAtom::Single(c) => c,
    };
    if chars.peek() == Some(&'-') {
        let mut trial = chars.clone();
        trial.next();
        if trial.peek() != Some(&']') && trial.peek().is_some() {
            chars.next();
            let hi = parse_class_range_end(chars)?;
            if hi < lo {
                return Err(format!("invalid range '{}-{}' in character class (end before start)", lo, hi));
            }
            return Ok((lo..=hi).collect());
        }
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
                // Inside a class, PCRE never reads a digit-following-backslash
                // as a backreference (backreferences don't exist in classes),
                // so `\0`-`\7` are unambiguous: up to three octal digits are
                // read to make one code point. `\8`/`\9` are not octal digits,
                // so PCRE reads them as the literal characters '8'/'9' --
                // that's already what the `other` fallthrough below does.
                '0'..='7' => Ok(ClassAtom::Single(read_class_octal(c, chars))),
                other => Ok(ClassAtom::Single(other)),
            }
        }
        Some(c) => Ok(ClassAtom::Single(c)),
        None => Err("unterminated character class (expected ']')".to_string()),
    }
}

/// Reads up to two further octal digits after an already-consumed first
/// octal digit `first`, and returns the resulting code point (0-255).
/// Three octal digits can specify up to 511; clamped to 255 rather than
/// wrapped, since this frontend has no character above 255 to represent.
fn read_class_octal(first: char, chars: &mut Chars) -> char {
    let mut value = first.to_digit(8).expect("caller only passes '0'..='7'");
    for _ in 0..2 {
        match chars.peek().and_then(|c| c.to_digit(8)) {
            Some(d) => {
                value = value * 8 + d;
                chars.next();
            }
            None => break,
        }
    }
    value.min(255) as u8 as char
}

fn parse_class_range_end(chars: &mut Chars) -> Result<char, String> {
    match parse_class_atom(chars)? {
        ClassAtom::Single(c) => Ok(c),
        ClassAtom::Multi(_) => {
            Err("a character-class shorthand (\\d, \\w, \\s, ...) cannot be a range endpoint".to_string())
        }
    }
}

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
    fn posix_class_inside_brackets_is_rejected_not_silently_misread() {
        let err = parse_ext_pattern("[[:alpha:]]").expect_err("should be rejected");
        assert!(err.contains("[:alpha:]"), "error should name the class: {err}");
    }

    #[test]
    fn posix_class_embedded_after_other_members_is_also_rejected() {
        let err = parse_ext_pattern("[a[:digit:]z]").expect_err("should be rejected");
        assert!(err.contains("[:digit:]"), "error should name the class: {err}");
    }

    #[test]
    fn ordinary_bracket_class_with_a_leading_open_bracket_char_still_works() {
        // A literal '[' as a class member (not a POSIX class) must still parse.
        assert_eq!(
            p("[\\[ab]"),
            ExtPat::Any(vec!['[', 'a', 'b'])
        );
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
        assert_eq!(p("a*"), ExtPat::Star(Box::new(ExtPat::Char('a'))));
        assert_eq!(p("a+"), ExtPat::Plus(Box::new(ExtPat::Char('a'))));
        assert_eq!(p("a?"), ExtPat::Opt(Box::new(ExtPat::Char('a'))));
    }

    #[test]
    fn lazy_markers_are_accepted_and_dropped() {
        assert_eq!(p("a*?"), p("a*"));
        assert_eq!(p("a+?"), p("a+"));
        assert_eq!(p("a??"), p("a?"));
        assert_eq!(p("a{2,4}?"), p("a{2,4}"));
    }

    #[test]
    fn anchors() {
        assert_eq!(p("^a$"), ExtPat::Concat(vec![ExtPat::Carat, ExtPat::Char('a'), ExtPat::Dollar]));
    }

    #[test]
    fn bound_exact() {
        assert_eq!(p("a{3}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 3, Some(3)));
    }

    #[test]
    fn bound_range() {
        assert_eq!(p("a{2,4}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, Some(4)));
    }

    #[test]
    fn bound_unbounded() {
        assert_eq!(p("a{2,}"), ExtPat::Bound(Box::new(ExtPat::Char('a')), 2, None));
    }

    #[test]
    fn invalid_bound_falls_back_and_then_errors_on_stray_brace() {
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
    fn octal_escape_in_class_reads_up_to_three_digits() {
        // Real SpamAssassin data: [\042\223\224\262\263\271]. \042 (octal)
        // is '"', not the three literal characters '0', '4', '2'.
        assert_eq!(p("[\\042]"), ExtPat::Any(vec!['"']));
        assert_eq!(p("[\\101]"), ExtPat::Any(vec!['A'])); // octal 101 = 'A'
    }

    #[test]
    fn octal_8_and_9_are_not_octal_digits() {
        // Per PCRE: \8 and \9 inside a class are the literal characters
        // '8' and '9', not the start of an octal escape.
        assert_eq!(p("[\\8\\9]"), ExtPat::Any(vec!['8', '9']));
    }

    #[test]
    fn char_class_leading_bracket_is_literal() {
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

    #[test]
    fn realistic_nested_pattern_shape() {
        let ep = p(r"(\d{1,3}\.){3}\d{1,3}");
        match ep {
            ExtPat::Concat(parts) => {
                assert_eq!(parts.len(), 2);
                assert!(matches!(parts[0], ExtPat::Bound(_, 3, Some(3))));
                assert!(matches!(parts[1], ExtPat::Bound(_, 1, Some(3))));
            }
            other => panic!("expected Concat, got {:?}", other),
        }
    }
}