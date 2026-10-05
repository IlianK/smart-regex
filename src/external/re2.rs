//! src/external/re2.rs
//!
//! Safe wrapper over Google's RE2, via the C shim in
//! `csrc/re2_shim.{h,cpp}` (RE2 has no C API of its own).

use std::ffi::{c_char, c_int, CStr};

#[repr(C)]
struct Re2HandleOpaque {
    _private: [u8; 0],
}

extern "C" {
    fn re2_new(
        pattern: *const c_char,
        pattern_len: usize,
        posix: c_int,
        case_insensitive: c_int,
        dot_all: c_int,
        error_out: *mut *mut c_char,
    ) -> *mut Re2HandleOpaque;
    fn re2_free(handle: *mut Re2HandleOpaque);
    fn re2_free_error(error: *mut c_char);
    fn re2_partial_match(handle: *const Re2HandleOpaque, text: *const c_char, text_len: usize) -> c_int;
    fn re2_full_match(handle: *const Re2HandleOpaque, text: *const c_char, text_len: usize) -> c_int;
    fn re2_partial_match_span(
        handle: *const Re2HandleOpaque,
        text: *const c_char,
        text_len: usize,
        match_start: *mut usize,
        match_end: *mut usize,
    ) -> c_int;
}

pub struct Re2 {
    handle: *mut Re2HandleOpaque,
}

// RE2 objects are thread-safe and logically immutable once constructed.
unsafe impl Send for Re2 {}
unsafe impl Sync for Re2 {}

impl Re2 {
    /// Compile `pattern`. `posix` selects RE2's POSIX leftmost-longest
    /// mode over its default leftmost-first mode. Both modes still accept
    /// `\s`/`\d`/`\w`/`\b`, since the shim also enables
    /// `perl_classes`/`word_boundary`, so this is a disambiguation switch,
    /// not a syntax one. `case_insensitive` is a separate option because
    /// RE2's `(?i)` inline group is unavailable in posix_syntax mode.
    /// `dot_all` sets RE2's `dot_nl` option, so `.` also matches `\n`,
    /// matching this crate's own `s`-flag handling (`strip_dot_newline`).
    pub fn new(pattern: &str, posix: bool, case_insensitive: bool, dot_all: bool) -> Result<Self, String> {
        let mut error: *mut c_char = std::ptr::null_mut();
        let handle = unsafe {
            re2_new(
                pattern.as_ptr().cast(),
                pattern.len(),
                posix as c_int,
                case_insensitive as c_int,
                dot_all as c_int,
                &mut error,
            )
        };
        if handle.is_null() {
            let msg = if error.is_null() {
                "unknown RE2 compilation error".to_string()
            } else {
                let msg = unsafe { CStr::from_ptr(error) }.to_string_lossy().into_owned();
                unsafe { re2_free_error(error) };
                msg
            };
            return Err(msg);
        }
        Ok(Re2 { handle })
    }

    /// Unanchored substring search (the dataset's "search" semantics).
    pub fn is_match(&self, text: &str) -> bool {
        unsafe { re2_partial_match(self.handle, text.as_ptr().cast(), text.len()) != 0 }
    }

    /// Anchored, whole-string match (the CLI's default semantics).
    pub fn full_match(&self, text: &str) -> bool {
        unsafe { re2_full_match(self.handle, text.as_ptr().cast(), text.len()) != 0 }
    }

    /// Unanchored search, returning the match's byte range. The two
    /// boolean methods above cannot distinguish leftmost-first from POSIX
    /// leftmost-longest (same language); only the span can, so this is
    /// where the `posix` flag passed to `new` becomes observable.
    pub fn find(&self, text: &str) -> Option<(usize, usize)> {
        let mut start = 0usize;
        let mut end = 0usize;
        let found = unsafe { re2_partial_match_span(self.handle, text.as_ptr().cast(), text.len(), &mut start, &mut end) };
        (found != 0).then_some((start, end))
    }
}

impl Drop for Re2 {
    fn drop(&mut self) {
        unsafe { re2_free(self.handle) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_and_matches() {
        let re = Re2::new("ab*c", false, false, false).unwrap();
        assert!(re.full_match("abc"));
        assert!(re.is_match("xxabcyy"));
        assert!(!re.full_match("xxabcyy"));
        assert!(!re.is_match("xyz"));
    }

    #[test]
    fn rejects_invalid_pattern() {
        assert!(Re2::new("a(", false, false, false).is_err());
    }

    /// On `a|ab` against "ab": leftmost-first reports "a" (span (0,1)),
    /// POSIX leftmost-longest reports "ab" (span (0,2)). If these ever
    /// agree, the posix_syntax/longest_match options wired through the
    /// shim have stopped doing anything.
    #[test]
    fn posix_mode_prefers_longest_match() {
        let greedy = Re2::new("a|ab", false, false, false).unwrap();
        let posix = Re2::new("a|ab", true, false, false).unwrap();
        assert_eq!(greedy.find("ab"), Some((0, 1)));
        assert_eq!(posix.find("ab"), Some((0, 2)));
    }

    /// Without `dot_all`, `.` does not cross `\n` (RE2's own default);
    /// with it, `.+` can span a newline.
    #[test]
    fn dot_all_flag_lets_dot_cross_newline() {
        let without = Re2::new("a.+b", false, false, false).unwrap();
        let with = Re2::new("a.+b", false, false, true).unwrap();
        assert!(!without.is_match("a\nb"));
        assert!(with.is_match("a\nb"));
    }
}