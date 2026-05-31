//! Demo: Basic matching with all three matchers (naive, deriv, pderiv)
//!
//! Run: cargo run --example demo_match

use regex_engine::types::Regex;
use regex_engine::matchers::{match_naive, match_deriv, match_pderiv};

// ============================================================================
// Test Case Structure
// ============================================================================

struct TestCase {
    pattern: &'static str,
    regex: Regex,
    inputs: Vec<&'static str>,
}

impl TestCase {
    fn new(pattern: &'static str, regex: Regex, inputs: Vec<&'static str>) -> Self {
        Self { pattern, regex, inputs }
    }
    
    fn run(&self) {
        println!("\nPattern: {}", self.pattern);
        println!("{:-<40}", "");
        println!("{:10} | {:6} | {:6} | {:6}", "Input", "naive", "deriv", "pderiv");
        println!("{:-<10}-+-{:-<6}-+-{:-<6}-+-{:-<6}", "", "", "", "");
        
        for input in &self.inputs {
            let naive = match_naive(input, &self.regex);
            let deriv = match_deriv(input, &self.regex);
            let pderiv = match_pderiv(input, &self.regex);
            println!("{:10} | {:6} | {:6} | {:6}", input, naive, deriv, pderiv);
        }
    }
}

// ============================================================================
// Test Case Definitions
// ============================================================================

// Literal: a
fn test_case_literal() -> TestCase {
    let r = Regex::lit('a');
    let inputs = vec!["a", "b", "", "aa"];
    TestCase::new("a", r, inputs)
}

/// Sequence: a·b
fn test_case_sequence() -> TestCase {
    let r = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    let inputs = vec!["ab", "a", "b", "abc"];
    TestCase::new("a·b", r, inputs)
}

// Alternation: a|b
fn test_case_alt() -> TestCase {
    let r = Regex::alt(Regex::lit('a'), Regex::lit('b'));
    let inputs = vec!["a", "b", "c", "ab"];
    TestCase::new("a|b", r, inputs)
}

// Kleene star: a*
fn test_case_star() -> TestCase {
    let r = Regex::star(Regex::lit('a'));
    let inputs = vec!["", "a", "aa", "aaa", "ab"];
    TestCase::new("a*", r, inputs)
}

// Alternation star: (a|b)*
fn test_case_alt_star() -> TestCase {
    let r = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    let inputs = vec!["", "a", "b", "ab", "ba", "abc"];
    TestCase::new("(a|b)*", r, inputs)
}


// ============================================================================
// Main
// ============================================================================

fn main() {
    println!("=========================================");
    println!("Basic Matching Demo");
    println!("  n = naive (exponential)");
    println!("  d = Brzozowski derivatives (DFA)");
    println!("  p = Antimirov partial (NFA)");
    println!("=========================================");
    
    test_case_literal().run();
    test_case_sequence().run();
    test_case_alt().run();
    test_case_star().run();
    test_case_alt_star().run();
    
    println!();
}