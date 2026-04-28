//! Optional debugging for POSIX parser

/// Debug flag - enabled via environment variable REGEX_DEBUG=1
pub fn debug_enabled() -> bool {
    std::env::var("REGEX_DEBUG").is_ok()
}

/// Debug macro that only prints when REGEX_DEBUG is set
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if $crate::posix::debug::debug_enabled() {
            eprintln!($($arg)*);
        }
    };
}