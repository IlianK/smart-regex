//! Safe wrapper around Google's RE2
//! small C shim in `csrc/re2_shim.cpp`/`.h` over RE2's C++ API 
//! (RE2 has no official C API of its own)

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

// RE2 objects are documented as thread-safe and logically immutable once constructed 
// (re2/re2.h, "RE2 objects are thread-safe and logically immutable")
unsafe impl Send for Re2 {}
unsafe impl Sync for Re2 {}

impl Re2 {
    /// Compiles `pattern`. `posix` selects RE2's POSIX leftmost-longest mode
    /// over its default leftmost-first (Perl-like) mode -- both modes still
    /// accept `\s`/`\d`/`\w`/`\b`, since posix mode also enables
    /// `perl_classes`/`word_boundary` (see csrc/re2_shim.cpp), so this stays
    /// a disambiguation-policy switch, not a syntax-dialect one.
    /// `case_insensitive` is a separate option because RE2's `(?i)` inline
    /// group is unavailable in posix_syntax mode.
    pub fn new(pattern: &str, posix: bool, case_insensitive: bool) -> Result<Self, String> {
        let mut error: *mut c_char = std::ptr::null_mut();
        let handle = unsafe {
            re2_new(
                pattern.as_ptr().cast(),
                pattern.len(),
                posix as c_int,
                case_insensitive as c_int,
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

    /// Unanchored substring search -- this project's dataset "search"
    /// semantics (docs/testing/DATASETS.md).
    pub fn is_match(&self, text: &str) -> bool {
        unsafe { re2_partial_match(self.handle, text.as_ptr().cast(), text.len()) != 0 }
    }

    /// Anchored, whole-string match -- this project's CLI/`parse_pattern`
    /// membership semantics.
    pub fn full_match(&self, text: &str) -> bool {
        unsafe { re2_full_match(self.handle, text.as_ptr().cast(), text.len()) != 0 }
    }

    /// Unanchored search, returning the byte range of the overall match.
    /// Unlike `is_match`/`full_match` (both booleans, which never
    /// distinguish leftmost-first from POSIX leftmost-longest -- they
    /// accept the same language), the returned span does: it is where the
    /// `posix` flag passed to `new` actually shows up.
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
        let re = Re2::new("ab*c", false, false).unwrap();
        assert!(re.full_match("abc"));
        assert!(re.is_match("xxabcyy"));
        assert!(!re.full_match("xxabcyy"));
        assert!(!re.is_match("xyz"));
    }

    #[test]
    fn rejects_invalid_pattern() {
        assert!(Re2::new("a(", false, false).is_err());
    }

    /// The whole reason this wrapper exposes a `posix` flag and `find`
    /// (span) alongside the boolean `is_match`/`full_match`: whole-string
    /// membership can't tell leftmost-first and POSIX leftmost-longest
    /// apart -- both accept the same language, so `is_match`/`full_match`
    /// always agree between them. Only the match *span* differs: on `a|ab`
    /// against "ab", RE2's default leftmost-first mode reports the first
    /// alternative that matches ("a", span (0,1)), while POSIX
    /// leftmost-longest mode reports the longest overall match ("ab", span
    /// (0,2)). If this ever agrees, the `posix_syntax`/`longest_match`
    /// options wired through the C shim have stopped doing anything.
    #[test]
    fn posix_mode_prefers_longest_match() {
        let greedy = Re2::new("a|ab", false, false).unwrap();
        let posix = Re2::new("a|ab", true, false).unwrap();
        assert_eq!(greedy.find("ab"), Some((0, 1)));
        assert_eq!(posix.find("ab"), Some((0, 2)));
    }
}
