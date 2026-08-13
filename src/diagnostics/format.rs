//! regex-engine/src/diagnostics/format.rs

pub fn bits_str(bs: &[bool]) -> String {
    let inner: String = bs.iter().map(|b| if *b { '1' } else { '0' }).collect();
    format!("[{}]", inner)
}


// -------------------------------
// Tests
// -------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_str_empty() {
        assert_eq!(bits_str(&[]), "[]");
    }

    #[test]
    fn bits_str_mixed() {
        assert_eq!(bits_str(&[false, true, true, false]), "[0110]");
    }
}
