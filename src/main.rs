//! Demo application for regex-engine

use regex_engine::{ParseTree, basic::Regex, flatten, match_deriv, match_naive, match_pderiv, parse_posix};

fn main() {
    println!("╔═══════════════════════════════════════════╗");
    println!("║        Regex Engine Demo                  ║");
    println!("╚═══════════════════════════════════════════╝\n");
    

    // ========== PART 1: BASIC MATCHING ==========
    println!("┌─────────── BASIC MATCHING ───────────┐");
    println!("│ Three algorithms:                    │");
    println!("│   n = naive (exponential)            │");
    println!("│   d = Brzozowski derivatives (DFA)   │");
    println!("│   p = Antimirov partial (NFA)        │");
    println!("└──────────────────────────────────────┘\n");
    
    let r = Regex::star(Regex::lit('a'));
    demo_basic_matching(&r, "a*", &["", "a", "aa", "aaa", "ab"]);
    


    // ========== PART 2: POSIX PARSING ==========
    println!("┌──────────── POSIX PARSING ───────────┐");
    println!("│ POSIX = longest leftmost match       │");
    println!("└──────────────────────────────────────┘\n");
    

    // Example 1: Paper example
    println!("▶ Example 1: (a + ab)(b + ε)");
    let r1 = Regex::seq(
        Regex::alt(Regex::lit('a'), Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        Regex::alt(Regex::lit('b'), Regex::Eps),
    );
    demo_posix_parse(&r1, "(a + ab)(b + ε)", "ab");
    

    // Example 2: Ambiguous pattern
    println!("▶ Example 2: (a + b + ab)*");
    println!("  (POSIX prefers [ab] over [a, b])");
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


///
/// HELPER FUNCTIONS
///
pub fn demo_basic_matching(r: &Regex, expr_str: &str, inputs: &[&str]) {
    println!("Expression: {}", expr_str);
    for input in inputs {
        let naive = match_naive(input, r);
        let deriv = match_deriv(input, r);
        let pderiv = match_pderiv(input, r);
        println!("  \"{:4}\" → n={} d={} p={}", input, naive, deriv, pderiv);
    }
    println!();
}

pub fn demo_posix_parse(r: &Regex, expr_str: &str, input: &str) -> Option<ParseTree> {
    println!("  Input: \"{}\"", input);
    
    match parse_posix(input, r) {
        Some(tree) => {
            println!("  - Parse tree: {}", tree);
            println!("  - String:     \"{}\"\n", flatten(&tree));
            Some(tree)
        }
        None => {
            println!("  [X] No match!\n");
            None
        }
    }
}

pub fn print_parse_tree(tree: &ParseTree) {
    println!("  - Paper notation: {}", tree);
    println!("  - Debug:          {:?}", tree);
    println!("  - Flattened:      \"{}\"", flatten(tree));
}