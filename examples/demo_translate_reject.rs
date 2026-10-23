//! examples/demo_translate_reject.rs
//!
//! cargo run --release --example demo_translate_reject
//!
//! One hand-authored pattern per frontend rejection cause, each verified
//! against the real `parse_pcre_rule` error rather than assumed.
//! Causes: backreference, lookaround, word-boundary, flag-m, flag-x,
//! POSIX class, bound-too-large, anchor.
//!
//! Replaces `demo_canonical_reject.rs`: no more real-corpus scan, and
//! POSIX class / bound-too-large now get their own named cause instead
//! of falling into a catch-all "other" -- `translate.rs`/`parse.rs`
//! already produce distinct messages for both.
//!
//! Writes the full table to reports/demos/translate_reject.txt.

#[path = "common/thesis_figures.rs"]
mod common;

use std::fmt::Write as _;

use regex_engine::frontend::parse_pcre_rule;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cause {
    Backreference,
    Lookaround,
    WordBoundary,
    FlagMultiline,
    FlagExtended,
    PosixClass,
    BoundTooLarge,
    Anchor,
}

impl Cause {
    fn label(&self) -> &'static str {
        match self {
            Cause::Backreference => "backreference",
            Cause::Lookaround => "lookaround",
            Cause::WordBoundary => "word-boundary",
            Cause::FlagMultiline => "flag-m",
            Cause::FlagExtended => "flag-x",
            Cause::PosixClass => "posix-class",
            Cause::BoundTooLarge => "bound-too-large",
            Cause::Anchor => "anchor",
        }
    }

    /// `multiline`'s own message names "anchor" too, so it (and every
    /// other needle) is checked before the generic `Anchor` catch-all.
    fn classify(err: &str) -> Option<Cause> {
        let e = err.to_lowercase();
        if e.contains("backreference") {
            Some(Cause::Backreference)
        } else if e.contains("lookahead") || e.contains("lookbehind") {
            Some(Cause::Lookaround)
        } else if e.contains("word boundary") {
            Some(Cause::WordBoundary)
        } else if e.contains("multiline") {
            Some(Cause::FlagMultiline)
        } else if e.contains("extended") {
            Some(Cause::FlagExtended)
        } else if e.contains("posix class") {
            Some(Cause::PosixClass)
        } else if e.contains("exceeds this frontend's cap") {
            Some(Cause::BoundTooLarge)
        } else if e.contains("anchor") {
            Some(Cause::Anchor)
        } else {
            None
        }
    }
}

struct Example {
    cause: Cause,
    pattern: &'static str,
}

const EXAMPLES: &[Example] = &[
    Example { cause: Cause::Backreference, pattern: r"(a)\1" },
    Example { cause: Cause::Lookaround, pattern: r"a(?=b)" },
    Example { cause: Cause::WordBoundary, pattern: r"a\bb" },
    Example { cause: Cause::FlagMultiline, pattern: "/abc/m" },
    Example { cause: Cause::FlagExtended, pattern: "/a b/x" },
    Example { cause: Cause::PosixClass, pattern: "[[:alpha:]]" },
    Example { cause: Cause::BoundTooLarge, pattern: "a{1001}" },
    Example { cause: Cause::Anchor, pattern: "^abc" },
];

fn main() {
    let mut buf = String::new();
    writeln!(buf, "One hand-authored example per frontend rejection cause.\n").unwrap();

    let mut passed = 0usize;
    for ex in EXAMPLES {
        let ok = match parse_pcre_rule(ex.pattern) {
            Ok(_) => {
                writeln!(
                    buf,
                    "[{}] FAIL: pattern {:?} was accepted, not rejected",
                    ex.cause.label(),
                    ex.pattern
                )
                .unwrap();
                false
            }
            Err(err) => {
                let got = Cause::classify(&err);
                let ok = got == Some(ex.cause);
                writeln!(buf, "[{}] {}", ex.cause.label(), if ok { "pass" } else { "FAIL" }).unwrap();
                writeln!(buf, "  pattern: {}", ex.pattern).unwrap();
                writeln!(buf, "  error:   {}", err).unwrap();
                if !ok {
                    writeln!(buf, "  classified as {:?}, expected {:?}", got, ex.cause).unwrap();
                }
                ok
            }
        };
        if ok {
            passed += 1;
        }
        writeln!(buf).unwrap();
    }

    writeln!(buf, "{}/{} causes correctly demonstrated", passed, EXAMPLES.len()).unwrap();
    common::report("translate_reject", &buf);
    if passed != EXAMPLES.len() {
        std::process::exit(1);
    }
}
