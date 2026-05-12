//! Compare recursive and loop versions

use regex_engine::basic::Regex;
use regex_engine::posix::{parse_recursive, parse_loop};
use std::io::Write;

fn compare(input: &str, r: &Regex, label: &str) {
    println!("\n▶ Testing: {}", label);
    println!("  Input: \"{}\"", input);
    println!("  Regex: {:?}", r);
    
    let rec_result = parse_recursive(input, r);
    let loop_result = parse_loop(input, r);
    
    println!("  Recursive: {:?}", rec_result);
    println!("  Loop:      {:?}", loop_result);
    
    assert_eq!(rec_result, loop_result, "Results differ for {}", label);
    println!("  ✓ Equal");
}

#[test]
fn test_recursive_vs_loop() {
    let r_lit = Regex::lit('a');
    compare("a", &r_lit, "Literal 'a'");
    
    let r_seq = Regex::seq(Regex::lit('a'), Regex::lit('b'));
    compare("ab", &r_seq, "Sequence a·b");
    
    let r_star = Regex::star(Regex::lit('a'));
    compare("", &r_star, "Star empty");
    compare("aaa", &r_star, "Star three");
    
    let r_alt = Regex::alt(
        Regex::lit('a'),
        Regex::seq(Regex::lit('a'), Regex::lit('b'))
    );
    compare("ab", &r_alt, "Alternation (a + ab)");
    
    let r_eps = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    compare("a", &r_eps, "(ε + a)*");
}

#[test]
fn test_recursive_vs_loop_no_match() {
    let r = Regex::lit('a');
    
    let rec_result = parse_recursive("b", &r);
    let loop_result = parse_loop("b", &r);
    
    assert_eq!(rec_result, loop_result);
    assert!(rec_result.is_none());
    
    println!("\n✓ Both return None for non-matching input");
}

// ============================================================================
// Separate tests for stack behavior
// ============================================================================

#[test]
fn test_recursive_stack_limit() {
    let r = Regex::star(Regex::lit('a'));
    
    println!("\n=== Finding Recursive Stack Limit ===\n");
    
    let mut length = 100;
    let step = 50;
    
    while length <= 2000 {
        let input = "a".repeat(length);
        
        print!("Length {}: ", length);
        std::io::stdout().flush().unwrap();
        
        let result = std::panic::catch_unwind(|| {
            let _ = parse_recursive(&input, &r);
        });
        
        if result.is_ok() {
            println!("✓ OK");
            length += step;
        } else {
            println!("CRASHED at {} characters", length);
            println!("\n=== Result ===");
            println!("Recursive stack limit: ~{} characters", length - step);
            return;
        }
    }
    
    println!("Recursive survived up to {} characters", length);
}

#[test]
fn test_loop_no_stack_limit() {
    let r = Regex::star(Regex::lit('a'));
    
    println!("\n=== Verifying Loop Has No Stack Limit ===\n");
    
    // Test increasingly large inputs
    for &length in &[1000, 5000, 10000, 50000] {
        let input = "a".repeat(length);
        
        print!("Length {}: ", length);
        std::io::stdout().flush().unwrap();
        
        let result = std::panic::catch_unwind(|| {
            let _ = parse_loop(&input, &r);
        });
        
        if result.is_ok() {
            println!("✓ OK");
        } else {
            panic!("Loop crashed at length {} (UNEXPECTED!)", length);
        }
    }
    
    println!("\n✓ Loop handles all tested lengths (heap allocation, no stack limit)");
}