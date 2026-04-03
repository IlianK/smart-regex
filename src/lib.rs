/// regex_engine/src/lib.rs

/*  
Three matchers:
1. Naive recursive matcher (exponential time, backtracking)
2. Brzozowski derivative matcher (DFA)
3. Antimirov partial derivatives matcher (NFA)
*/

// ----------------------------------------------
// Data Types
// ----------------------------------------------

// Regex as Rust enum (Box for recursive variants -> heap allocation)
// ENBF grammar: r,s ::= x | epsilon | phi | r+s | r.s | r*
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Regex {
    /// Empty language: L(Phi) = {}
    Phi,
    /// Empty word: L(Eps) = {epsilon}
    Eps,
    /// Single character: L(Lit('a')) = {"a"}
    Lit(char),
    /// Sequence: L(Seq(r,s)) = { v++w | v in L(r), w in L(s) }
    Seq(Box<Regex>, Box<Regex>),
    /// Alternative: L(Alt(r,s)) = L(r) union L(s)
    Alt(Box<Regex>, Box<Regex>),
    /// Kleene star: L(Star(r)) = {epsilon} union L(r)·L(Star(r))
    Star(Box<Regex>),
}

// Constructors for better readability
// Regex::seq(r, s) instead of Regex::Seq(Box::new(r), Box::new(s))
impl Regex {
    pub fn seq(r: Regex, s: Regex) -> Regex {
        Regex::Seq(Box::new(r), Box::new(s))
    }
    pub fn alt(r: Regex, s: Regex) -> Regex {
        Regex::Alt(Box::new(r), Box::new(s))
    }
    pub fn star(r: Regex) -> Regex {
        Regex::Star(Box::new(r))
    }
    pub fn lit(c: char) -> Regex {
        Regex::Lit(c)
    }
}

// ---------------------------------------------
// Nullability
// ---------------------------------------------

// Decides whether epsilon is in L(r) (by recursion over r)
pub fn nullable(r: &Regex) -> bool {
    match r {
        Regex::Phi       => false, // n(Phi)
        Regex::Eps       => true,  // n(Eps)
        Regex::Lit(_)    => false, // n(Lit x)
        Regex::Alt(r, s) => nullable(r) || nullable(s), // n(r+s) = n(r) || n(s)
        Regex::Seq(r, s) => nullable(r) && nullable(s), // n(r.s) = n(r) && n(s)
        Regex::Star(_)   => true, // n(r*)
    }
}

// ---------------------------------------------
// Brozozowski derivatives
// ---------------------------------------------

// Computes derivative of r based on character x
// L(deriv(r, x)) = x \ L(r) = { w | x · w in L(r) }
pub fn deriv(r: &Regex, x: char) -> Regex {
    match r {
        Regex::Phi    => Regex::Phi, // d(Phi, y) = Phi
        Regex::Eps    => Regex::Phi, // d(eps, y) = Phi
        
        // d(x, y) = if x==y then epsilon else Phi
        Regex::Lit(c) => {
            if *c == x { Regex::Eps } else { Regex::Phi } 
        }

        // d(r+s, y) = d(r,y) + d(s,y)
        Regex::Alt(r, s) => {
            Regex::alt(deriv(r, x), deriv(s, x)) 
        }

        // d(r.s, y) = if n(r) then d(r,y).s + d(s,y) else d(r,y).s
        Regex::Seq(r, s) => {
            let dr = Regex::seq(deriv(r, x), *s.clone());
            if nullable(r) { 
                Regex::alt(dr, deriv(s, x))
            } else {                  
                dr
            }
        }

        // d(r*, y)  = d(r,y) . r*
        Regex::Star(r) => { 
            Regex::seq(deriv(r, x), Regex::star(*r.clone()))
        }
    }
}


// ---------------------------------------------
// Simplification
// ---------------------------------------------

// Algebraic laws for simplification 
// Without expression size grows unboundedly under repeated derivation
pub fn simplify(r: Regex) -> Regex {
    match r {
        Regex::Seq(r, s) => {
            let r = simplify(*r);
            let s = simplify(*s);
            match (&r, &s) {
                (Regex::Phi, _) => Regex::Phi, // Phi . r = Phi  (annihilator)
                (_, Regex::Phi) => Regex::Phi, // r . Phi = Phi  (annihilator)
                (Regex::Eps, _) => s,           // Eps . r = r    (identity)
                (_, Regex::Eps) => r,           // r . Eps = r    (identity)
                _               => Regex::seq(r, s),
            }
        }
        Regex::Alt(r, s) => {
            let r = simplify(*r);
            let s = simplify(*s);
            match (&r, &s) {
                (Regex::Phi, _) => s,  // Phi + r = r  (identity)
                (_, Regex::Phi) => r,  // r + Phi = r  (identity)
                _ if r == s     => r,  // r + r   = r  (idempotency)
                _               => Regex::alt(r, s),
            }
        }
        Regex::Star(r) => {
            let r = simplify(*r);
            match r {
                Regex::Eps => Regex::Eps, // Eps* = Eps  (already nullable, no new words)
                Regex::Phi => Regex::Eps, // Phi* = Eps  (L(Phi*) = {epsilon})
                r          => Regex::star(r),
            }
        }
        other => other, // Phi, Eps, Lit are already in normal form
    }
}


// ---------------------------------------------
// Brzozowski Matcher
// ---------------------------------------------

// Match input against r using Brzozowski derivatives
// For each character of input derive current expression and simplify
// Nullability check decides result
// Implicitly builds a DFA on the fly -> every simplified derivative is a DFA state

// Complexity: O(|w| * |r|^2) (with simplification)
//             |w| = input length and |r| = expression size
//             (|r|^2 factor from the equality checks inside simplify) 
pub fn match_deriv(input: &str, r: &Regex) -> bool {
    let mut current = r.clone();
    for c in input.chars() {
        current = simplify(deriv(&current, c));
    }
    nullable(&current)
}


// ---------------------------------------------
// Antimirov partial derivatives
// ---------------------------------------------

use std::collections::HashSet;

// Normalizes Eps . r to r (Antimirov property)
// Guarantee that partial derivatives remain subexpressions of original 
fn smart_seq(r: Regex, s: &Regex) -> Regex {
    match r {
        Regex::Eps => s.clone(),
        r          => Regex::seq(r, s.clone()),
    }
}

// Computes set of partial derivatives of r based on x
// Returns expression set representing NFA states
// L(r1) union ... union L(rn) = x \ L(r)  where {r1,...,rn} = pd(r,x)
pub fn pderiv(r: &Regex, x: char) -> HashSet<Regex> {
    match r {
        Regex::Phi    => HashSet::new(), // pd(Phi, y) = {}
        Regex::Eps    => HashSet::new(), // pd(eps, y) = {}

        // pd(x, y)   = if x==y then {eps} else {}
        Regex::Lit(c) => {
            let mut set = HashSet::new();
            if *c == x { set.insert(Regex::Eps); }
            set
        }

        // pd(r+s, y) = pd(r,y) union pd(s,y)
        Regex::Alt(r, s) => {
            let mut set = pderiv(r, x);
            set.extend(pderiv(s, x));
            set
        }

        // pd(r.s, y) = { r'.s | r' in pd(r,y) }
        //              union (if n(r) then pd(s,y) else {})
        Regex::Seq(r, s) => {
            let mut set: HashSet<Regex> = pderiv(r, x)
                .into_iter()
                // Attach s to each r' in pd(r,y) to get r'.s
                .map(|r_prime| smart_seq(r_prime, s))
                .collect();

            // If r is nullable, x could come from s instead -> pd(s,y)
            if nullable(r) {
                set.extend(pderiv(s, x));
            }
            set
        }

        // pd(r*, y)  = { r'.r* | r' in pd(r,y) }
        Regex::Star(r) => {
            pderiv(r, x)
                .into_iter()
                // Attach r* to each r' in pd(r,y) to get r'.r*
                .map(|r_prime| smart_seq(r_prime, &Regex::star(*r.clone())))
                .collect()
        }
    }
}


// ---------------------------------------------
// Antimirov Matcher
// ---------------------------------------------
 
// Matches input against r using Antimirov partial derivatives
// Maintains set of active expressions as NFA state set
// Returns true if at least one active expression is nullable in the end
//
// -> state set is bounded by the subexpressions of the original
// -> number of distinct states is finite
// -> guarantees termination and linear-time matching in input length
pub fn match_pderiv(input: &str, r: &Regex) -> bool {
    let mut states: HashSet<Regex> = HashSet::new();
    states.insert(r.clone());

    for c in input.chars() {
        let mut next_states: HashSet<Regex> = HashSet::new();
        for state in &states {
            next_states.extend(pderiv(state, c));
        }
        states = next_states;
        if states.is_empty() {
            return false; // dead state: no path forward
        }
    }

    states.iter().any(|r| nullable(r))
}

// ---------------------------------------------
// Naive recursive matcher
// ---------------------------------------------

// Check whether subword w[i..j] is in L(r)
// Complexity: exponential in worst case -> all possible splits of input are explored
fn match_naive_range(word: &[char], i: usize, j: usize, r: &Regex) -> bool {
    match r {
        // L(Phi) = {} -> never matches
        Regex::Phi => false,

        // L(Eps) = {epsilon} -> matches only the empty subword
        Regex::Eps => i == j,

        // L(Lit(c)) = {"c"} -> subword must be exactly one char equal to c
        Regex::Lit(c) => j == i + 1 && word.get(i) == Some(c),

        // L(Alt(r,s)) = L(r) union L(s) -> try both on the same subword
        Regex::Alt(r, s) => {
            match_naive_range(word, i, j, r) || match_naive_range(word, i, j, s)
        }

        // L(Seq(r,s)) = { v++w | v in L(r), w in L(s) }
        // Try all split points k: w[i..k] for r, w[k..j] for s
        Regex::Seq(r, s) => {
            (i..=j).any(|k| {
                match_naive_range(word, i, k, r) && match_naive_range(word, k, j, s)
            })
        }

        // L(Star(r)) = {epsilon} union L(r)·L(Star(r))
        // Either empty (i==j), or one chunk from r followed by more from r* (recursive call with same r)
        // k starts at i+1 -> guarantee progress and avoid infinite recursion
        Regex::Star(r) => {
            i == j
            || (i + 1..=j).any(|k| {
                match_naive_range(word, i, k, r)
                && match_naive_range(word, k, j, &Regex::Star(Box::new(*r.clone())))
            })
        }
    }
}

// Matches full input against r by calling match_naive_range with full range
// Sets i=0 and j=length to cover full input
// match w r = matchR w (0, length w) r
pub fn match_naive(input: &str, r: &Regex) -> bool {
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    match_naive_range(&chars, 0, len, r)
}