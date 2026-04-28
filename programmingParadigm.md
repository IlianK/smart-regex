Overview

    Background on regular expressions

    Derivatives

    Partial derivatives

    Disambiguation and matching policies (POSIX vs Greedy)

    POSIX parser

Further topics (yet to be added):

    Efficient implementations


---

Background
Syntax

In EBNF Syntax:


r,s ::= x | y | z | ...         Symbols aka letters taken from a finite alphabet
    |  epsilon                  Empty word
    |  phi                      Empty language
    |  r + r                    Alternatives
    |  r . r                    Sequence aka concatenation
    |  r*                       Kleene star

u,v,w ::= epsilon               empty word
      |  w . w                  concatenation

Sigma denotes the finite set of alphabet symbols.

Regular expressions are popular formalism to specify (infinitely many) patterns of input.

For example,

 (open . (read + write)* . close)*

specifies the valid access patterns of a resource usage policy.

We assume that open, read, write, close are the primitive events (symbols) which will be recorded during a program run.
Semantics

L(r) denotes the set of words represented by the regular expression r.

Standard (denotational) formulation of L(r):

L(x) = { x }

L(epsilon) = { epsilon }

L(phi) = { }

L(r + s) = { w | w in L(r) or w in L(s) }

L(r . s) = { v . w | v in L(r) and w in L(s) }

L(r*) = { epsilon } cap { w_1 . ... . w_n | w_1, ..., w_n in L(r) and n >=1 }

We say that r matches w if w in L(r).



slide 3/8
* help? contents? 

---


Simple Haskell matcher

We represent regular expressions via the following data type.

data R = Phi
       | Eps
       | L Char
       | Seq R R
       | Choice R R
       | Star R deriving (Show, Eq,Ord)

The following matcher follows L(r) and checks if a (sub)expression matches a (sub)word.

-- matchR [w_0,...,w_n] (i,j+1) r yields true if r matches the subword [w_i,..,w_j]
matchR :: [Char] -> (Int, Int) -> R -> Bool
matchR w (i,j) Eps = i == j
matchR w (i,j) (L c) =
    j == i+1 &&
    (if 0 <= i && i < length w
     then w !! i == c
     else False)
matchR w (i,j) (Choice r1 r2) =
     matchR w (i,j) r1 || matchR w (i,j) r2
matchR w (i,j) (Seq r1 r2) =
     or $
     map (\k -> matchR w (i,k) r1 && matchR w (k,j) r2) [i..j]
matchR w (i,j) (Star r) =
     i == j
     ||
     (or $
      map (\k -> matchR w (i,k) r && matchR w (k,j) (Star r)) [i+1..j])


-- match w r yields true if w in L(r)
match :: [Char] -> R -> Bool
match w r = matchR w (0,length w) r



slide 4/8
* help? contents? 

---


Derivative-based matcher

We consider an alternative method to carry out the membership test based on Brzozowski https://en.wikipedia.org/wiki/Brzozowski_derivative. He introduced a symbolic method to construct an automata from a regular expression based on the concept of derivatives.
Regular expression derivatives

Given some expression r and a symbol x, we obtain the derivative of r w.r.t. x, written d(r,x), by taking way the leading symbol x from r.

In semantic terms, d(r,x) can be described as follows:

L(d(r,x)) = x \ L(r)

    x  L(r) denotes the left quotient, i.e. the language { w | x . w in L(r)}.
        We write . to denote concatenation. In some exposition this is left silent, i.e. x w.

    Hence, the derivative d(r,x) denotes the set of all words from L(R) where the leading symbol x has been removed.

Thus, it is easy to solve the word problem. Let w be a word consisting of symbols x1 . x2 .... xn-1 . xn.

Compute

d(r,x1) = r1
d(r1,x2) = r2
...
d(rn-1,xn) = rn

That is, we repeatidely build the derivative of r w.r.t symbols xi.

Check if the final expression rn is nullable. An expression s is nullable if epsilon in L(s).
Formalizing nullability and the derivative operation

It is surprisingly simply to decide nullability by observing the structure of regular expression.

We write n(r) to denote the nullability test which yields a Boolean value (true/false).

n(x) where xi is a symbol never holds.

n(epsilon) always holds.

n(phi) never holds.

n(r + s) holds iff n(r) holds or n(s) holds.

n(r . s) holds iff n(r) holds and n(s) holds.

n(r*) always holds.

A similar approach (definition by structural recursion) works for the derivative operation.

We write d(r,x) to denote the derivative obtained by extracting the leading symbol x from expression r. For each derivative, we wish that the following holds: L(d(r,x)) = x \ L(r).

x \ L(r) denotes the left quotient of L(r) by the symbol x and equals { w | x.w in L(r) }.

As in case of the nullability test, the derivative operation is defined by observing the structure of regular expression patterns. Instead of a Boolean value, we yield another expression (the derivative).

d(x,y) =   either epsilon if x == y or phi otherwise

d(epsilon,y) = phi

d(phi,y)     = phi

d(r + s, y)  = d(r,y) + d(s,y)

d(r . s, y)  =  if n(r)
                then d(r,y) . s +  d(s,y)
                else d(r,y) . s

d(r*, y)     = d(r,y) . r*

Examples

Let’s consider some examples to understand the workings of the derivative and nullability function.

We write r -x-> d(r,x) to denote application of the derivative on some expression r with respect to symbol x.

       x*
-x->   d(x*,x)
       = d(x,x) . x*
       = epsilon . x*

-x->   epsilon . x*
       = d(epsilon,x) . x* + d(x*,x)     -- n(epsilon) yields true
       = phi . x* + epsilon . x*

So repeated applicaton of the derivative on x*$ for input string "x.x" yieldsphi . x* + epsilon . x*`. Let’s carry out the nullability function on the final expression.

    n(phi . x* + epsilon . x*)
    = n(phi .x*) or n(epsilon . x*)
    = (n(phi) and n(x*)) or (n(epsilon) and n(x*))
    = (false and true) or (true and true)
    = false or true
    = true

The final expression phi . x* + epsilon . x* is nullable. Hence, we can argue that expression x* matches the input word “x.x”.
Implementation in Haskell

-- Yield True if epsilon is part of the language, otherwise, we find False.
nullable :: R -> Bool
nullable Phi = False
nullable Eps = True
nullable L{} = False
nullable (Choice r s) = nullable r || nullable s
nullable (Seq r s) = nullable r && nullable s
nullable Star{} = True


deriv :: Char -> R -> R
deriv _ Eps = Phi
deriv _ Phi = Phi
deriv x (L y)
  | x == y    = Eps
  | otherwise = Phi
deriv x (Choice r s) = Choice (deriv x r) (deriv x s)
deriv x (Seq r s)
  | nullable r = Choice (Seq (deriv x r) s) (deriv x s)
  | otherwise =  Seq (deriv x r) s
deriv x (Star r) = Seq (deriv x r) (Star r)


matchDeriv :: String -> R -> Bool
matchDeriv xs r =
          go r xs
             where
               go r [] = nullable r
               go r (x:xs) = go (deriv x r) xs

Simplifications

Let r be some (regular expression). We say that s is a descendant if s can be obtained from r by a sequence of derivative application.

Here are some examples in Haskell syntax.

deriv 'a' (Star (L 'a'))
=> Seq Eps (Star (L 'a'))

deriv 'a' $ deriv 'a' (Star (L 'a'))
=> Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a')))

deriv 'a' $ deriv 'a' $ deriv 'a' (Star (L 'a'))
=> Choice (Seq Phi (Star (L 'a'))) (Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a'))))

So, we find that

Choice (Seq Phi (Star (L 'a'))) (Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a'))))

is a descendant of Star (L 'a').

The above example also shows that the size and number of descendants can grow infinitely.

To keep the size and number of descendants finite we simplify expressions by applying the following law: L(r) = L(r + r).

Here’s the Haskell implementation that simplifies descendants.

simp :: R -> R
simp r = let s = simp2 r
         in if s == r then r
            else simp s
     where
        simp2 (Seq r s) = Seq (simp2 r) (simp2 s)
        simp2 (Choice r s)
           | r == s    = simp2 r
           | otherwise = case s of
                          Choice s1 s2 -> if r == s1 then Choice (simp2 r) (simp2 s2)
                                          else Choice (simp2 r) (simp2 s)
                          _            -> Choice (simp2 r) (simp2 s)
        simp2 (Star r) = Star $ simp2 r
        simp2 r = r

    We keep applying simplifications until we have reached a fix point

    The “nested” case deals with expressions such as “Choice r (Choice …)”

After each derivative step, we simplify expressions.

derivSimp x r = simp $ deriv x r

We find that

derivSimp 'a' (Star (L 'a'))
=> Seq Eps (Star (L 'a'))

derivSimp 'a' $ derivSimp 'a' (Star (L 'a'))
=> Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a')))

derivSimp 'a' $ derivSimp 'a' $ derivSimp 'a' (Star (L 'a'))
=> Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a')))

derivSimp 'a' $ derivSimp 'a' $ derivSimp 'a' $ derivSimp 'a' (Star (L 'a'))
=> Choice (Seq Phi (Star (L 'a'))) (Seq Eps (Star (L 'a')))



slide 5/8
* help? contents? 

---


Partial derivatives

The derivative-based matcher effectively builds a DFA on the fly while consuming letters in the input word. Instead of DFA, we can build a NFA by employing partial derivatives.

The partial derivative operation pd yields a set of expressions and is defined as follows.

In the below, we write cup for set union.

pd(x,y) =   either {eps} if x == y or {} otherwise

pd(epsilon,y) = {}

pd(phi,y)     = {}

pd(r + s, y)  = pd(r,y) cup pd(s,y)

pd(r . s, y)  =  if n(r)
                 then  smartCs(pd(r,y),s)  cup  d(s,y)
                 else  smartCs(pd(r,y),s)

d(r*, y)     = smartCs(d(r,y),r*)

where smartCs is the smart construction

smartCs(rs,s) = { if r == eps then s
                  else r . s          | r in rs }

to normalize eps . r to r. This guarantees that partial derivatives are subexpressions of the original expression.

Let pd(r,x) = {r1,...,rn}. Then, we have that L(r1) cup ... cup L(rn) = x \ L(r).
Implementation in Haskell

pDeriv :: Char -> R -> [R]
pderiv _ Eps = []
pderiv _ Phi = []
pDeriv x (L y)
  | x == y    = [Eps]
  | otherwise = []
pDeriv x (Choice r s) = nub $ (pDeriv x r) ++ (pDeriv x s)
pDeriv x (Seq r s)
  | nullable r = nub $ [ Seq r' s | r' <- pDeriv x r ] ++ (pDeriv x s)
  | otherwise  = [ smartC r' s | r' <- pDeriv x r]
pDeriv x (Star r) = [ smartC r' (Star r) | r' <- pDeriv x r ]

-- smart constructor
-- `eps r` is normalized to `r` according to Antimirov
-- guarantees that partial derivatives are subexpressions
-- of the original expression
smartC Eps r = r
smartC r s = Seq r s


matchPDeriv :: String -> R -> Bool
matchPDeriv xs r =
          go [r] xs
             where
               go rs [] = any nullable rs
               go rs (x:xs) = go (nub $ concat [pDeriv x r | r <- rs]) xs



slide 6/8
* help? contents? 

---


Disambiguation and matching policies (POSIX vs Greedy)
Matching can be ambiguous

Consider the following well-known arithmetic example 1+2*3. We could either interpret the expression as 1+(2*3) or (1+2)*3. To avoid ambiguities, some disambiguation strategies are commonly applied. For example, “*” binds tigher than “+”.
Disambiguation policies for regular expressions

Similar ambiguity issues also arise in case of regular expression matching.

There are two common disambiguation strategies for regular expressions: Greedy and POSIX.

The following works formalize Greedy and POSIX matching:

    Greedy regular expression matching

    POSIX Regular Expression Parsing with Derivatives

Open problems:

    General framework to specify the Greey and POSIX order

    General derivative-based framework to specify Greedy and POSIX matching

    Efficient implementation in Rust



slide 7/8
* help? contents? 

---


POSIX parser

Here’s the Haskell implementation of the POSIX parser as described in POSIX Regular Expression Parsing with Derivatives.


-- Parse trees
-- [[r]] = u
data U where
  Nil :: U
  Empty :: U
  Letter :: Char -> U
  LeftU :: U -> U
  RightU :: U -> U
  Pair :: (U,U) -> U
  List :: [U] -> U
  deriving Show


mkEps :: R -> U
mkEps Phi            = Nil
mkEps Eps            = Empty
mkEps (L l)          = error "mkEps, letter impossible"
mkEps (Choice r1 r2)
  | nullable r1         = LeftU $ mkEps r1
  | nullable r2         = RightU $ mkEps r2
  | otherwise          = error "mkEps, choice non-empty"
mkEps (Seq r1 r2)   = Pair (mkEps r1, mkEps r2)
mkEps (Star r)      = List []


-- inj r r\l l yields
-- [[r\l] -> [[r]]
-- r\l derivatives of r wrt l
inj :: R -> R -> Char -> U -> U
inj (Star r) (Seq rd _) l (Pair (u, List us)) =  -- _ must be equal to r
    List $ (inj r rd l u) : us

inj (Seq r1 r2) (Choice (Seq rd1 _) _) l (LeftU u) = -- first _ = r2, second _ = r2\l
             let Pair (u', u'') = u
             in Pair (inj r1 rd1 l u', u'')
inj (Seq r1 r2) (Choice _ rd2) l (RightU u) =
             Pair (mkEps r1, inj r2 rd2 l u)
inj (Seq r1 r2) (Seq rd1 _) l (Pair (u',u'')) =   -- _ = r2
     Pair (inj r1 rd1 l u', u'')

inj (Choice r1 r2) (Choice rd1 rd2) l (LeftU u) =
      LeftU $ inj r1 rd1 l u
inj (Choice r1 r2) (Choice rd1 rd2) l (RightU u) =
      RightU $ inj r2 rd2 l u

inj (L l') Eps l Empty
  | l == l'   = Letter l
  | otherwise = error "impossible"
inj r1 r2 l u =
  error $ show r1 ++ "\n" ++ show r2 ++ "\n" ++ show l ++ "\n" ++ show u


parse r []
   | nullable r = mkEps r
   | otherwise = error "no match"
parse r (l:w) = inj r (deriv l r) l $ parse (deriv l r) w



slide 8/8
* help? contents? 
