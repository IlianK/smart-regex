//! Optional debugging for POSIX parser

use std::sync::atomic::{AtomicUsize, Ordering};
use std::cell::RefCell;

static INDENT: AtomicUsize = AtomicUsize::new(0);
thread_local! {
    static STEP: RefCell<usize> = const { RefCell::new(0) };
}

pub fn debug_enabled() -> bool {
    std::env::var("REGEX_DEBUG").is_ok()
}

pub fn indent_inc() {
    INDENT.fetch_add(2, Ordering::Relaxed);
}

pub fn indent_dec() {
    INDENT.fetch_sub(2, Ordering::Relaxed);
}

pub fn indent() -> String {
    " ".repeat(INDENT.load(Ordering::Relaxed))
}

pub fn step_inc() -> usize {
    STEP.with(|s| {
        let mut val = s.borrow_mut();
        *val += 1;
        *val
    })
}

pub fn step_reset() {
    STEP.with(|s| {
        *s.borrow_mut() = 0;
    });
}

#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if $crate::posix::debug::debug_enabled() {
            eprint!("{}", $crate::posix::debug::indent());
            eprintln!($($arg)*);
        }
    };
}

#[macro_export]
macro_rules! debug_step {
    ($label:expr) => {
        if $crate::posix::debug::debug_enabled() {
            let step = $crate::posix::debug::step_inc();
            eprintln!("\n{}[Step {}] {}", $crate::posix::debug::indent(), step, $label);
        }
    };
}