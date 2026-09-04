//! regex-engine/src/frontend/alphabet.rs
//!

/// The full alphabet: printable ASCII plus tab/newline/CR. 98 characters.
pub fn alphabet() -> Vec<char> {
    let mut v: Vec<char> = (0x20u8..=0x7E).map(|b| b as char).collect();
    v.push('\t');
    v.push('\n');
    v.push('\r');
    v
}

/// `\d`: ASCII digits.
pub fn digit_chars() -> Vec<char> {
    ('0'..='9').collect()
}

/// `\w`: ASCII letters, digits, underscore.
pub fn word_chars() -> Vec<char> {
    let mut v: Vec<char> = ('a'..='z').chain('A'..='Z').chain('0'..='9').collect();
    v.push('_');
    v
}

/// `\s`: space, tab, newline, CR.
pub fn space_chars() -> Vec<char> {
    vec![' ', '\t', '\n', '\r']
}

/// The alphabet minus `of` -- used for `\D`/`\W`/`\S` and negated classes
/// `[^...]`.
pub fn complement(of: &[char]) -> Vec<char> {
    alphabet().into_iter().filter(|c| !of.contains(c)).collect()
}

// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabet_size() {
        // 0x20..=0x7E is 95 chars, plus \t \n \r
        assert_eq!(alphabet().len(), 98);
    }

    #[test]
    fn digit_chars_are_0_to_9() {
        assert_eq!(digit_chars(), ('0'..='9').collect::<Vec<_>>());
    }

    #[test]
    fn word_chars_include_underscore() {
        assert!(word_chars().contains(&'_'));
        assert!(word_chars().contains(&'a'));
        assert!(word_chars().contains(&'Z'));
        assert!(word_chars().contains(&'5'));
        assert!(!word_chars().contains(&'-'));
    }

    #[test]
    fn complement_excludes_given_set() {
        let comp = complement(&digit_chars());
        for d in digit_chars() {
            assert!(!comp.contains(&d));
        }
        assert!(comp.contains(&'a'));
        assert_eq!(comp.len(), alphabet().len() - digit_chars().len());
    }

    #[test]
    fn complement_of_alphabet_is_empty() {
        assert!(complement(&alphabet()).is_empty());
    }
}
