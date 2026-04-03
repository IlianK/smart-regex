/// regex_engine/src/main.rs

/*  
Demo of three matchers:
- Step-by-step derivation of example
- Side-by-side comparison table
*/

use regex_engine::{Regex, match_deriv, match_naive, match_pderiv, nullable, deriv, simplify};

fn main() {
    println!("---------------------------------------------");
    println!(" Regex Engine -- Three Matchers");
    println!("---------------------------------------------\n");

    // a*
    let a_star = Regex::star(Regex::lit('a'));
    println!("Expression: a*");
    demo("",    &a_star);
    demo("a",   &a_star);
    demo("aaa", &a_star);
    demo("ab",  &a_star);
    println!();

    // (a|b)*
    let ab_star = Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')));
    println!("Expression: (a|b)*");
    demo("",     &ab_star);
    demo("abba", &ab_star);
    demo("abc",  &ab_star);
    println!();

    // a·b·c
    let abc = Regex::seq(
        Regex::seq(Regex::lit('a'), Regex::lit('b')),
        Regex::lit('c'),
    );
    println!("Expression: a·b·c");
    demo("abc",  &abc);
    demo("ab",   &abc);
    demo("abcd", &abc);
    println!();

    // (a·a)* -> accept strings with even number of a's
    let even_a = Regex::star(Regex::seq(Regex::lit('a'), Regex::lit('a')));
    println!("Expression: (a·a)* -- accepts even number of a's");
    demo("",     &even_a);
    demo("aa",   &even_a);
    demo("aaaa", &even_a);
    demo("a",    &even_a);
    demo("aaa",  &even_a);
    println!();

    // Step-by-step derivation of d(a*, "aa")
    println!("Step-by-step: d(a*, \"aa\")");
    let r0 = Regex::star(Regex::lit('a'));
    println!("  r0 = {:?}", r0);
    let r1 = simplify(deriv(&r0, 'a'));
    println!("  r1 = deriv(r0, 'a') = {:?}", r1);
    let r2 = simplify(deriv(&r1, 'a'));
    println!("  r2 = deriv(r1, 'a') = {:?}", r2);
    println!("  nullable(r2) = {} --> match: {}", nullable(&r2), nullable(&r2));
    println!();

    // Comparison table of three matchers
    println!("Comparison of all three matchers:");
    let test_cases: Vec<(&str, Regex)> = vec![
        ("aaa",  Regex::star(Regex::lit('a'))),
        ("ab",   Regex::seq(Regex::lit('a'), Regex::lit('b'))),
        ("abba", Regex::star(Regex::alt(Regex::lit('a'), Regex::lit('b')))),
        ("xyz",  Regex::star(Regex::lit('a'))),
    ];

    println!("  {:<12} {:<30} {:>8} {:>12} {:>12}",
        "Input", "Regex", "Naive", "Deriv", "PDeriv");
    
    println!("  {}", "-".repeat(76));
    for (input, r) in &test_cases {
        let n = match_naive(input, r);
        let d = match_deriv(input, r);
        let p = match_pderiv(input, r);
    
        println!("  {:<12} {:<30} {:>8} {:>12} {:>12}",
            input, format!("{:?}", r), n, d, p);
    }
}


// Runs three matchers on given input and expression
fn demo(input: &str, r: &Regex) {
    let naive  = match_naive(input, r);
    let deriv  = match_deriv(input, r);
    let pderiv = match_pderiv(input, r);

    let status = if naive == deriv && deriv == pderiv { "ok" } else { "MISMATCH" };

    println!("  {:>6}  naive={} deriv={} pderiv={}  {}",
        format!("{:?}", input), naive, deriv, pderiv, status);
}