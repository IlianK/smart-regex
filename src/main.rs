//! Demo application for regex-engine

use regex_engine::basic::Regex;
use regex_engine::demo::{demo_basic_matching, demo_posix_parse};

fn main() { 
    println!("\n----------- REGEX DEMO -----------\n");


    // 1: BASIC MATCHING 
    println!("BASIC MATCHING");
    println!("- n = naive (exponential)");
    println!("- d = Brzozowski derivatives (DFA)");
    println!("- p = Antimirov partial (NFA)\n");
    
    let r = Regex::star(Regex::lit('a'));
    demo_basic_matching(&r, "a*", &["", "a", "aa", "aaa", "ab"]);
    

    // 2: POSIX PARSING
    println!("\nPOSIX PARSING (longest (leftmost) match)\n");
    
    // Example 1: Paper example
    println!("▶ Example 1: (a + ab)(b + ε)");
    let r1 = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    );
    demo_posix_parse(&r1, "(a + ab)(b + ε)", "ab");
    

    // Example 2: Ambiguous pattern
    println!("▶ Example 2: (a + b + ab)*");
    let r2 = Regex::star(Regex::alt(
        Regex::lit('a'),
        Regex::alt(Regex::lit('b'), Regex::seq(Regex::lit('a'), Regex::lit('b')))
    ));
    demo_posix_parse(&r2, "(a + b + ab)*", "ab");
    

    // Example 3: Kleene star
    println!("▶ Example 3: a*");
    let r3 = Regex::star(Regex::lit('a'));
    demo_posix_parse(&r3, "a*", "aaa");
    

    // Example 4: Empty matches (ε*)
    println!("▶ Example 4: ε* (empty matches)");
    let r4 = Regex::star(Regex::Eps);
    demo_posix_parse(&r4, "ε*", "");
    
    
    // Example 5: (ε + a)*
    println!("▶ Example 5: (ε + a)*");
    let r5 = Regex::star(Regex::alt(Regex::Eps, Regex::lit('a')));
    demo_posix_parse(&r5, "(ε + a)*", "a");
}