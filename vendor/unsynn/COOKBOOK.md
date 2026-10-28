# Cookbook

While `unsynn` is pretty lean most code should have documentation and examples. Still some
things need an explanantion how to be efficiently used. Which will be explained here.


## Parsing

Parsing is done over a [`TokenIter`] which iterates over the token stream.  `TokenIter` can be
created with `to_token_iter()` or `into_token_iter()`, which are implemented for `TokenStream`
and string types (string impls require the `proc_macro2` feature).

The main trait for parsing a [`TokenIter`] is the [`Parse`] trait. This traits methods are all
default implemented and can be used as is. [`Parse`] is implemented for all types that
implement [`Parser`] and [`ToTokens`]. [`Parser`] is the trait that has to be implemented for
each type that should be parsed.

The [`IParse`] trait is implemented for [`TokenIter`], this calls `Parse::parse()` in a
convenient way when types can be inferred.


### [`Parse::parse_with()`] transformations

The `Parse::parse_with()` method is used for parsing in more complex situations. In the
simplest case it can be used to validate the values of a parsed type. More complex usage will
fill in `HiddenState` and other not parsed members or construct completely new types from
parsed entities. See also the [`transform`](crate::transform) module for transformation helpers.


## ToTokens

The [`ToTokens`] trait complements [`Parser`] by turning parsed entities back into a
`TokenStream`. Unlike the trait from the quote crate, we define `ToTokens` for many more types 
and provide additional methods.

Notably it provides methods to create the entry points for parsing (when the `proc_macro2`
feature is enabled: `to_token_iter()` and `into_token_iter()`).

When textual representation of a parsed entity is required then `tokens_to_string()` can be
used (requires `proc_macro2` feature). The standard `Display` trait is implemented on top of
that, as such every type that has [`ToTokens`] implemented can be printed as text.


## Composition and Type Aliases

For moderately complex types it is possible to use composition with [`Cons`], [`Either`] and
other [container types](crate::container) instead defining new enums or structures.

It is recommended to alias such composed types to give them useful names. This can be used for
creating grammars on the fly without any boilerplate code.


## The [`unsynn!{}`] Macro

The recommended way to describe grammars is to use the [`unsynn!{}`] macro. This allows to define
grammars by defining enums and structures. The macro will generate the necessary implementations
for [`Parser`] and [`ToTokens`] in a safe/optimized way. It is possible to add
[`HiddenState<T>`](crate::HiddenState) members to add non syntactic entries to there custom structs.

The macro supports generic types and static lifetime parameters with trait bounds, including
simple trait names (`Clone`), qualified paths (`std::fmt::Debug`), lifetime bounds
(`'static`), and multiple bounds combined with `+` (`T: Clone + std::fmt::Debug +
'static`). For complete syntax details, see the [`unsynn!`](crate::unsynn!) macro
documentation.

Custom parsing and token emission can be implemented with through `parse_with` and `to_tokens`
clauses.

### Custom Parsing with `parse_with`

Use `parse_with` alone for validation, or with `from Type` for transformation:

**Transformation** (requires `from Type`):
```rust
# use unsynn::*;
unsynn! {
    // Transform: parse integer as bool (from Type required)
    struct BoolInt(bool) from LiteralInteger:
    parse_with |value, _tokens| {
        Ok(Self(value.value() != 0))
    };
}
```

**Validation** (without `from`):
```rust
# use unsynn::*;
unsynn! {
    // Validate: ensure positive integers only
    struct PositiveInt(LiteralInteger);
    parse_with |this, tokens| {
        if this.0.value() > 0 {
            Ok(this)
        } else {
            Error::other(None, tokens, "must be positive".into())
        }
    };
}
```

### Custom Token Emission with `to_tokens`

Use `to_tokens` to customize how types are emitted back to tokens:

```rust
# use unsynn::*;
unsynn! {
    // Custom output: emit bools as YES/NO
    struct YesNo(bool);
    to_tokens |s, tokens| {
        if s.0 {
            Literal::string("YES").to_tokens(tokens);
        } else {
            Literal::string("NO").to_tokens(tokens);
        }
    };
}
```

### Combining `parse_with` and `to_tokens`

Both clauses are independent and can be used together:

```rust
# use unsynn::*;
unsynn! {
    // Transformation + custom output
    struct BoolInt(bool) from LiteralInteger:
    parse_with |value, _tokens| { Ok(Self(value.value() != 0)) }
    to_tokens |s, tokens| {
        Literal::u64_unsuffixed(if s.0 {1} else {0}).to_tokens(tokens);
    };
}

unsynn! {
    // Validation + custom output
    struct PositiveInt(LiteralInteger);
    parse_with |this, tokens| {
        if this.0.value() > 0 { Ok(this) }
        else { Error::other(None, tokens, "must be positive".into()) }
    }
    to_tokens |s, tokens| {
        Punct::new('+', Spacing::Alone).to_tokens(tokens);
        s.0.to_tokens(tokens);
    };
}
```


## Implementing Parsers

### Transactions

The [`Parse`] Trait parses items within a transaction. This is with the
[`Transaction::transaction()`] method. Internally this clones the iterator, calls the
[`Parser::parser()`] method and copies the cloned iterator back to the original on success.
This means that if a parser fails, the input is reset to the state before the parser was
called. For efficiency reasons the [`Parser::parser()`] methods are not transactional,
when they fail they leave the input in a consumed state.

**NOTE:**: For conjunctive parsers (sequential parsing without alternatives/backtracking),
calling `Parser::parser()` directly instead of `Parse::parse()` eliminates transaction overhead. 
This is also safe for final elements in a disjunction, but **dangerous** for non-final
alternatives that need rollback on failure. Benchmarks over 30% speedup depending on
complexity. When in doubt calling the [`Parse::parse()`] method is alweays on the
safe side, but comes with some performance penalty.

When one wants to manually parse *alternatives* within a [`Parser`] (like in a enum) these
**must** be manually called within a transaction. This is only necessary when the parsed entity
is compound and not the final alternative.

```rust
# use unsynn::*;
enum MyEnum {
    // Complex variant
    Tuple(i32, i32),
    // Simple variant
    Simple(i32),
    // Another simple variant
    Another(Ident),
}

impl Parser for MyEnum {
    fn parser(input: &mut TokenIter) -> Result<Self> {
        // Use [`Transaction::transaction()`] to parse the tuple variant
        if let Ok(tuple) = input.transaction(
            |trans_input| -> Result<MyEnum> {
                Ok(MyEnum::Tuple(
                    i32::parser(trans_input)?,
                    i32::parser(trans_input)?,
                ))
            }
        ) {
            Ok(tuple)
        } else
        // Try to parse the simple variant
        // can use the `Parse::parse()` or `IParse::parse()` method directly since a
        // single entity will be put in a transaction by those.
        if let Ok(i) = input.parse() {
            Ok(MyEnum::Simple(i))
        } else {
            // Try to parse the last variant
            // this can use the `Parser::parser()` method since this is the final alternative
            Ok(MyEnum::Another(Ident::parser(input)?))
        }
    }
}
```


### Different ways to design Parsers

There are different approaches how one can implement parsers. Each has its own advantages and
disadvantages. unsynn is agnostic, one can freely mix whatever makes most sense in a
particular case.

unsynn uses a rather simple, first come - first served approach when parsing. Parsers may be a
subset or share a common prefix with other parsers. This needs some attention.

In the case where parsers are subsets to other parsers and one puts them into a disjunction in
a `unsynn! { enum ..}` or in a `Either` combinator the more specific case must come first,
otherwise it will never match.

```rust
# use unsynn::*;
// Ident must come first since TokenTree matches any token.
type Example = Either<Ident, TokenTree>;
```

For the other case where parsers sharing longer prefixes (this should rarely happen in
practice) it may benefit performance to break these into a type with the shared prefix that
dispatches on the distinct parts.


#### Exact AST Representation

One approach is to define a structures that reflects the AST of the grammar exactly.  This is
what the [`unsynn!{}`] macro and composition does. The program later works with the parsed
structure directly. The advantage is that [`Parser`] and [`ToTokens`] are simple and come for
free and that the source structure of the AST stays available.

```rust
# use unsynn::*;

unsynn!{
    // define a list of Ident = "LiteralString",.. assignments
    struct Assignment {
        id: Ident,
        _equal: Assign,
        value: LiteralString,
    }

    struct AssignmentList {
        list: DelimitedVec<Assignment, Comma>
    }
}
```

When the implementation generated by the [`unsynn!{}`] macro is not sufficient one can
implement [`Parser`] and [`ToTokens`] for custom structs and enums manually.


#### High level representation

Another approach is to represent the data more in the way further processing requires. This
simplifies working with the data but one **has** to implement the [`Parser`] and [`ToTokens`]
traits manually. Often the [`Parse::parse_with()`] method will become useful in such
cases. The [`transform`](crate::transform) module provides parsers that can transform the input already when parsing.

```rust
# use unsynn::*;
# use std::collections::HashMap;
// We could go with `unsynn!{struct Assignment{...}}` as above here. But lets use composition
// as example here. This stays internal so its complexity isnt exposed.
type Assignment = Cons<Ident, Assign, LiteralString>;

// Here we'll parse the list of assignments into a structure that represents the
// data in a way thats easier to use from a rust program
#[derive(Default)]
struct AssignmentList {
    // each 'Ident = LiteralString'
    list: Vec<(Ident, String)>,
    // We want to have a fast lookup to the entries
    lookup: HashMap<Ident, usize>,
}

impl Parser for AssignmentList {
    fn parser(input: &mut TokenIter) -> Result<Self> {
        let mut assignment_list = AssignmentList::default();

        // We construct the `AssignmentList` by parsing the content, appending and processing it.
        while let Ok(assignment) = Delimited::<Assignment, Comma>::parse(input) {
            assignment_list.list.push((
                assignment.value.first.clone(),
                // Create a String without the enclosing double quotes
                assignment.value.third.as_str().to_string()
            ));
            // add it to the lookup
            assignment_list.lookup.insert(
                assignment.value.first.clone(),
                assignment_list.list.len()-1
            );
            // No Comma, no more assignments
            if assignment.delimiter.is_none() {
                break;
            }
        }
        Ok(assignment_list)
    }
}

impl ToTokens for AssignmentList {
    fn to_tokens(&self, output: &mut TokenStream) {
        for a in &self.list {
            a.0.to_tokens(output);
            Assign::new().to_tokens(output);
            LiteralString::from_str(&a.1).to_tokens(output);
            Comma::new().to_tokens(output);
        }
    }
}
# fn test_assignment_list() {
#     let mut input = r#"a = "b", c = "d""#.to_token_iter();
#     let parsed = AssignmentList::parse(&mut input).unwrap();
#     assert_eq!(parsed.list.len(), 2);
#     assert_eq!(parsed.list[1].1, "d");
# }
```


## Generic Types with Trait Bounds

The [`unsynn!{}`] macro supports comprehensive generic type syntax with lifetime parameters 
and trait bounds. You can use:

- **Lifetime parameters**: `<'a>`, `<'a, 'b>`, or mixed `<'a, T>`
- **Simple trait names**: `T: Clone`
- **Qualified paths**: `T: std::fmt::Debug` (no imports needed)
- **Lifetime bounds**: `T: 'static`
- **Multiple bounds**: `T: Clone + std::fmt::Debug + 'static`
- **Where clauses**: `where T: Clone + 'static`

### Examples

```rust
# use unsynn::*;
unsynn! {
    // Simple trait bounds
    struct SimpleGeneric<T: Clone>(T);
    
    // Qualified paths (no import needed!)
    struct WithDebug<T: std::fmt::Debug>(T);
    
    // Multiple bounds with lifetime bounds
    struct Complex<T: Clone + std::fmt::Display + 'static>(T);
    
    // Where clauses work too
    struct WhereClause<T>(T) where T: Clone + std::fmt::Debug;
}
```

Lifetime parameters are also supported:

```text
unsynn! {
    // Single lifetime parameter
    struct WithLifetime<'a> {
        name: LiteralString,
        _marker: std::marker::PhantomData<&'a ()>,
    }
    
    // Mixed lifetime and type parameters
    struct MixedParams<'a, T: Clone> {
        data: T,
        _marker: std::marker::PhantomData<&'a ()>,
    }
}
```

**Limitations**: 
- Generic type arguments in bounds (e.g., `T: Trait<U>`) are not supported
- HRTB (Higher-Ranked Trait Bounds like `for<'a>`) are not yet supported
- Lifetime parameters on tuple structs and enums with inline `impl` blocks are not supported
  (use separate `impl` blocks outside the macro instead)

For implementation details, see the [`unsynn!`](crate::unsynn!) macro documentation.


## Parse Predicates

Parse predicates provide compile-time control over parser behavior using zero-sized types
(ZSTs). They enable context-sensitive parsing by acting as compile-time guards on generic
types. The `predicateflag` keyword creates newtype wrappers that can implement custom traits
for type-safe context validation.

See the [`predicates`](crate::predicates) module for the full API reference and the
[`unsynn!` macro Parse Predicates section](crate::unsynn#parse-predicates) for syntax details.

### Type-Safe Context Predicates

The `predicatetrait` macro creates custom context marker traits for compile-time type safety.
It automatically generates trait definitions and implementations for universal predicates
(`Enable`, `Disable`, `TokensRemain`) and logical operators, eliminating boilerplate while
maintaining type safety.

```rust
# use unsynn::*;
# #[allow(unused)]
# fn test_predicates() {
unsynn! {
    // Define context traits - automatically implements for universal predicates
    predicatetrait ExpressionContext;
    predicatetrait StatementContext;
    
    // Create context-specific predicates
    predicateflag InExpression for ExpressionContext;
    predicateflag InStatement for StatementContext;
    predicateflag InBothContexts for ExpressionContext, StatementContext;

    keyword KwIf = "if";
    keyword KwWhile = "while";
    
    // Type parameters use custom context traits
    pub struct IfExpr<P: ExpressionContext = InExpression> {
        _guard: P,
        kw_if: KwIf,
        condition: ParenthesisGroup,
        body: BraceGroup,
    }
    
    pub struct WhileStmt<P: StatementContext = InStatement> {
        _guard: P,
        kw_while: KwWhile,
        condition: ParenthesisGroup,
        body: BraceGroup,
    }
    
    // Conditional field using logical operators
    pub struct ConditionalField<P: ExpressionContext = InExpression> {
        _guard: P,
        name: Ident,
    }
}

// Context-specific predicates work as expected
let mut tokens = "if (x > 0) { return true; }".to_token_iter();
let result = IfExpr::<InExpression>::parse(&mut tokens);
assert!(result.is_ok());

// Universal predicates (Enable/Disable/TokensRemain) work in ANY context
let mut tokens = "if (true) { }".to_token_iter();
let result = IfExpr::<Enable>::parse(&mut tokens);  // Enable implements ExpressionContext
assert!(result.is_ok());

# let mut tokens = "while (true) { }".to_token_iter();
# let result = WhileStmt::<TokensRemain>::parse(&mut tokens);  // TokensRemain implements StatementContext
# assert!(result.is_ok());

// Logic operators automatically implement context traits when operands do
let mut tokens = "if (x) { y }".to_token_iter();
let result = IfExpr::<AllOf<InExpression, InBothContexts>>::parse(&mut tokens);
assert!(result.is_ok());  // AllOf implements ExpressionContext because both operands do

# let mut tokens = "while (x) { y }".to_token_iter();
# let result = WhileStmt::<AnyOf<InStatement, InBothContexts>>::parse(&mut tokens);
# assert!(result.is_ok());  // AnyOf implements StatementContext because both operands do

// Not inverts the logic while preserving trait implementations
let mut tokens = "if (x) { y }".to_token_iter();
let result = IfExpr::<Not<Disable>>::parse(&mut tokens);
assert!(result.is_ok());  // Not<Disable> implements ExpressionContext and succeeds

// OneOf: exactly one predicate must succeed
type OnlyInExpression = OneOf<InExpression, Disable>;
let mut tokens = "value".to_token_iter();
let result = ConditionalField::<OnlyInExpression>::parse(&mut tokens);
assert!(result.is_ok());  // Succeeds: exactly one (InExpression) succeeds
# }
#
# // Type safety: This won't compile - InStatement doesn't implement ExpressionContext
# // let result = IfExpr::<InStatement>::parse(&mut tokens);  // Compile error!
```

**Base predicates:** [`Enable`](crate::Enable) (always succeeds), [`Disable`](crate::Disable)
(always fails), [`TokensRemain`](crate::predicates::TokensRemain) (succeeds only when tokens
remain).

**Logical operators:** [`AllOf`](crate::AllOf) (AND), [`AnyOf`](crate::AnyOf) (OR),
[`OneOf`](crate::OneOf) (XOR), [`Not`](crate::predicates::Not) (negation) - each accepts 2-4
operands.

Each `predicateflag` creates a newtype tuple struct that automatically implements `Parser`,
`ToTokens`, `Clone`, `Debug`, `PredicateOp`, and `Default` (when the base is `Enable` or
`TokensRemain`), plus any custom traits specified after `impl`.

### Type Identity Checking with `PredicateCmp`

The [`PredicateCmp<A, B, Same = Enable, Different =
Disable>`](crate::predicates::PredicateCmp) predicate enables distinguishing between different
predicate flags at compile time. It dispatches to `Same` types `A` and `B` are identical, and
like `Differnt` when they differ.

This is useful when you need to accept a specific predicate flag while rejecting others that
implement the same trait:

```rust
# use unsynn::*;
# use unsynn::predicates::*;
unsynn! {
    predicatetrait Context;
    predicateflag InExpr = Enable for Context;
    predicateflag InStmt = Enable for Context;
    predicateflag InType = Enable for Context;

    // Only accepts InExpr specifically
    struct StructLiteral<C: Context> {
        guard: PredicateCmp<C, InExpr>,
        name: Ident,
        fields: BraceGroup,
    }

    // Accepts InExpr but NOT InStmt
    struct TernaryExpr<C: Context> {
        guard: AllOf<PredicateCmp<C, InExpr>, Not<PredicateCmp<C, InStmt>>>,
        condition: Ident,
        if_true: Ident,
        if_false: Ident,
    }

    // Accepts either InExpr or InType, but not InStmt
    struct GenericPath<C: Context> {
        guard: AnyOf<PredicateCmp<C, InExpr>, PredicateCmp<C, InType>>,
        segments: PathSepDelimitedVec<Ident>,
    }
}

// ✅ StructLiteral accepts InExpr
let mut tokens = "Point { x: 1, y: 2 }".to_token_iter();
assert!(StructLiteral::<InExpr>::parse(&mut tokens).is_ok());

// ❌ StructLiteral rejects InStmt (even though InStmt implements Context)
let mut tokens = "Point { x: 1, y: 2 }".to_token_iter();
assert!(StructLiteral::<InStmt>::parse(&mut tokens).is_err());

// ✅ TernaryExpr accepts InExpr and explicitly rejects InStmt
let mut tokens = "cond val1 val2".to_token_iter();
assert!(TernaryExpr::<InExpr>::parse(&mut tokens).is_ok());

// ❌ TernaryExpr rejects InStmt due to Not<PredicateCmp<C, InStmt>>
let mut tokens = "cond val1 val2".to_token_iter();
assert!(TernaryExpr::<InStmt>::parse(&mut tokens).is_err());

// ✅ GenericPath accepts InExpr
let mut tokens = "std::vec::Vec".to_token_iter();
assert!(GenericPath::<InExpr>::parse(&mut tokens).is_ok());

// ✅ GenericPath accepts InType
let mut tokens = "std::vec::Vec".to_token_iter();
assert!(GenericPath::<InType>::parse(&mut tokens).is_ok());

// ❌ GenericPath rejects InStmt
let mut tokens = "std::vec::Vec".to_token_iter();
assert!(GenericPath::<InStmt>::parse(&mut tokens).is_err());
```

**Requirements:**
- Type parameters must implement [`PredicateOp`](crate::predicates::PredicateOp) (which
  includes a `'static` bound)
- Zero runtime overhead (usually optimized to compile-time constant)

**Combining with logical operators:**

```rust
# use unsynn::*;
# use unsynn::predicates::*;
# unsynn! {
#     predicatetrait Ctx;
#     predicateflag A = Enable for Ctx;
#     predicateflag B = Enable for Ctx;
#     predicateflag C = Enable for Ctx;
// Accept A or B, but not C
type AcceptAOrBNotC<T> = AllOf<
    AnyOf<PredicateCmp<T, A>, PredicateCmp<T, B>>,
    Not<PredicateCmp<T, C>>
>;

// Accept exactly A (not B, not C)
type OnlyA<T> = AllOf<
    PredicateCmp<T, A>,
    Not<AnyOf<PredicateCmp<T, B>, PredicateCmp<T, C>>>
>;
# }
```

See [`PredicateCmp`](crate::predicates::PredicateCmp) for more details.


## Expression Parsing

Unsynn provides [expression building blocks](crate::expressions) for creating operator precedence parsers. These
types handle the common patterns found in expression grammars:

- **[`expressions::PrefixExpr`]** - Unary prefix operators (e.g., `-x`, `!flag`, `*ptr`)
- **[`expressions::PostfixExpr`]** - Unary postfix operators (e.g., `x?`, `x!`)
- **[`expressions::InfixExpr`]** (or [`expressions::NonAssocExpr`]) - Non-associative binary operators (e.g., `a == b`)
- **[`expressions::LeftAssocExpr`]** - Left-associative operators (e.g., `a + b + c`)
- **[`expressions::RightAssocExpr`]** - Right-associative operators (e.g., `a = b = c`)

### Basic Usage

```rust
# use unsynn::*;
# use unsynn::operator::names::{Plus, Star, Minus};
// Define precedence levels (highest to lowest binding)
type MultiplicativeExpr = LeftAssocExpr<LiteralInteger, Star>;
type AdditiveExpr = LeftAssocExpr<MultiplicativeExpr, Plus>;

// Parse with proper precedence: "1 + 2 * 3" → 1 + (2 * 3)
let mut tokens = "1 + 2 * 3".to_token_iter();
let expr: AdditiveExpr = tokens.parse().unwrap();
assert_eq!(expr.len(), 2); // Two operands at addition level: 1 and (2*3)
```

### No Nesting Required

All expression types use `DelimitedVec` internally, so you don't need to nest them:

```rust
# use unsynn::*;
# use unsynn::operator::names::Minus;
// ✅ Correct: Single PrefixExpr handles multiple operators (--x)
type MyPrefixExpr = PrefixExpr<Minus, Ident>;

// ❌ Wrong: Don't nest - it's redundant!
// type MyPrefixExpr = PrefixExpr<Minus, PrefixExpr<Minus, Ident>>;
```

### Design Patterns for Recursive Expressions

When building expression parsers, you often need to handle **recursive structures** like
parenthesized expressions `(expr)` or grouped expressions that can contain the same expression
type they're part of. This creates a circular type dependency that Rust's type system doesn't
allow directly.

These patterns solve the **"infinitely sized type"** problem by breaking the recursion cycle.
Without indirection, a type like `PrimaryExpr::Grouped(Expr)` would require `Expr` to contain
`PrimaryExpr`, which contains `Expr`, which contains `PrimaryExpr`... creating an infinite
chain.

**Option 1: Box for Indirection (Recommended)**
- Simplest approach
- Use `Box<Expression>` in grouped/parenthesized variants
- Works naturally with Rust's type system
- Small allocation overhead (usually negligible)
- Example: `Grouped(ParenthesisGroupContaining<Box<Expr>>)`

**Option 2: Generic Parameter (Advanced)**
- Make primary expression generic over root type: `PrimaryExpr<E>`
- Pass the expression type to itself: `type Expression = AssignmentExpr<Expression>;`
- More type-safe but more complex signatures
- Allows different recursion strategies per grammar

**Option 3: Traditional Hierarchy**
- Explicit wrapper types at each precedence level
- Matches traditional parser generator patterns
- More boilerplate but very clear structure
- Each level wraps the next explicitly

For more details, see the [`expressions`] module documentation.

### Complete Example with Parentheses

This example shows a complete expression parser with proper precedence hierarchy and 
parenthesized expressions. Parentheses are **not** part of the expression building blocks - 
you handle them in your primary expression type using `Box` for recursion.

```rust
# use unsynn::*;

unsynn!{
    keyword  Calc = "CALC";
    operator Pow = "^";
    operator Factorial = "!";
    operator Mul = "*";
    operator Div = "/";
    operator Add = "+";
    operator Sub = "-";

    // Define primary expressions (THIS is where parentheses go)
    enum PrimaryExpr {
        Literal(LiteralInteger),
        Grouped(ParenthesisGroupContaining<Box<Expr>>),  // Box breaks recursion
    }

    // Build precedence hierarchy (highest to lowest binding)
    // Power level: Mix infix (^) and postfix (!) at same precedence
    // Parse postfix first, then allow x^y between postfix expressions
    type MyPostfixExpr = PostfixExpr<PrimaryExpr, Factorial>;
    type PowerExpr = RightAssocExpr<MyPostfixExpr, Pow>;

    // Multiplication and Division (left-associative)
    struct MultiplicativeOp(Either<Mul, Div>);
    type MultiplicativeExpr = LeftAssocExpr<PowerExpr, MultiplicativeOp>;

    // Addition and Subtraction (left-associative)
    struct AdditiveOp(Either<Add, Sub>);
    type AdditiveExpr = LeftAssocExpr<MultiplicativeExpr, AdditiveOp>;

    // Step 3: Define root expression type
    type Expr = AdditiveExpr;

    // Top-level statement
    struct Expression(Calc, Expr, Semicolon);
}

// Parentheses override precedence
let mut tokens = "CALC (2 + 3) * 4 ;".to_token_iter();
let ast: Expression = tokens.parse().unwrap();
assert_tokens_eq!(ast, "CALC (2 + 3) * 4 ;");

// Standard precedence: 10 - 5 + 2 * 3 = (10 - 5) + (2 * 3)
let mut tokens = "CALC 10 - 5 + 2 * 3 ;".to_token_iter();
let ast: Expression = tokens.parse().unwrap();
assert_tokens_eq!(ast, "CALC 10 - 5 + 2 * 3 ;");

// Power is right-associative: 2^3^2 = 2^(3^2) = 2^9 = 512
let mut tokens = "CALC 2 ^ 3 ^ 2 ;".to_token_iter();
let ast: Expression = tokens.parse().unwrap();
assert_tokens_eq!(ast, "CALC 2 ^ 3 ^ 2 ;");

// Mixing infix and postfix at same level: 2^3! = 2^(3!) = 2^6 = 64
// Postfix binds to its left operand first, then ^ operates on the result
let mut tokens = "CALC 2 ^ 3 ! ;".to_token_iter();
let ast: Expression = tokens.parse().unwrap();
assert_tokens_eq!(ast, "CALC 2 ^ 3 ! ;");

// The parsed structure shows 3! is grouped before ^ operates on it
let debug_str = format!("{:?}", ast);
assert!(debug_str.contains("PostfixExpr { operand: Literal(LiteralInteger { literal: Literal { lit: 3"));
assert!(debug_str.contains("operators: DelimitedVec"));
assert!(debug_str.contains("Operator<'!'>"));
```


## Lazy Parsing and Lookahead

Grammars frequently need to parse tokens into lists or lookahead to match only when some
conditions are met without consuming tokens. See also the [`container`](crate::container) 
module for container implementations and the [`fundamental`](crate::fundamental) module 
for lookahead types.

### Greedy Container Types

Unsynn provides several containers that parse elements greedily (as many as possible):

- **`Vec<T>`** -
  Parses `T` repeatedly until it fails. Always succeeds (empty vec if no matches).
- **[`LazyVec<T, S>`](crate::LazyVec)** -
  Parses `T` until terminator `S`, consuming and storing `S` in `.terminator` field.
- **[`LazyVecUntil<T, S>`](crate::LazyVecUntil)** -
  Parses `T` until seeing terminator `S` (via lookahead), without consuming `S`.
- **[`DelimitedVec<T, D, P, MIN, MAX>`](crate::DelimitedVec)** -
  Parses delimited lists with configurable trailing delimiter policy and length constraints.

All these containers implement the **[`RangedRepeats`](crate::RangedRepeats)** trait, which allows constraining the
number of parsed elements.

#### Length constrained Type Aliases

For convenience, several type aliases provide common repeat patterns:

- **[`Any<T>`](crate::Any)** - Parse 0 or more
- **[`Many<T>`](crate::Many)** - Parse 1 or more
- **[`Exactly<T, N>`](crate::Exactly)** - Parse exactly N
- **[`AtLeast<T, N>`](crate::AtLeast)** - Parse N or more
- **[`AtMost<T, N>`](crate::AtMost)** - Parse N or less

#### Examples

```rust,ignore
# use unsynn::*;

// Parse any number of attributes
type Attributes = Vec<Attribute>;

// Parse comma-separated parameters (trailing comma optional)
type Params = DelimitedVec<Param, Comma, TrailingDelimiter::Optional>;

// Parse 1 or more statements
type Statements = Many<Statement>;
```

### Lookahead and Composition

Lookahead types check what's ahead without consuming tokens:

- **[`Expect<T>`](crate::Expect)** - Positive lookahead: succeeds if `T` matches (doesn't consume)
- **[`Except<T>`](crate::Except)** - Negative lookahead: succeeds if `T` does NOT match (doesn't consume)

Both return zero-sized types and never advance the token position.

#### Composing with `Cons`

Lookahead is most powerful when combined with [`Cons`](crate::Cons) for sequential parsing:

**Negative lookahead** (ensure something is NOT next):  
```rust,ignore
# use unsynn::*;
// Parse ButThis only if we're NOT at NotThis
type NegativeExample = Cons<Except<NotThis>, ButThis>;
```

**Positive lookahead** (check something follows):  
```rust,ignore
# use unsynn::*;
// Parse This, then verify ThenThis follows (without consuming it)
type PositiveExample = Cons<This, Expect<ThenThis>>;
```

**`parse_with()` transitions**:  
Another common pattern is to use lookahead together with `parse_with()` transitions.  This is
especially useful in custom `Parser` implementations. unsynn itself make frequent use of this.

```rust
# use unsynn::*;
// Parse an identifier and verify it's followed by '='
let mut tokens = "foo = 42".to_token_iter();
let ident: Ident = <Cons<Ident, Expect<Assign>>>::parse_with(
    &mut tokens,
    |t, _| Ok(t.first)
).unwrap();

assert_eq!(ident.to_string(), "foo");
assert_tokens_eq!(tokens, "= 42");
```


#### Using `unsynn!` - conjunctive structs

When defining structs in `unsynn!{}`, fields are conjunctive, so you can write lookahead patterns directly:

**Negative lookahead:**  
```rust,ignore
# use unsynn::*;
unsynn! {
    struct NegativeLookahead {
        // Ensure we're not at NotThis before parsing This
        _check: Except<NotThis>,
        pub this: This,
    }
}
```

**Positive lookahead:**  
```rust,ignore
# use unsynn::*;
unsynn! {
    struct PositiveLookahead {
        pub first: First,
        // Verify Second follows (without consuming it)
        _check: Expect<Second>,
    }
}
```

#### [`NonEmptyOption<T>`](crate::NonEmptyOption) - Optional Non-Empty Content

A common pattern is parsing optional content that should NOT match on empty input:

```rust,ignore
# use unsynn::*;
// Parse expression only if tokens remain
type TrailingExpr = NonEmptyOption<Expression>;
```

[`NonEmptyOption<T>`](crate::NonEmptyOption) is like `Option<T>` but only succeeds when tokens are available to
parse. It prevents matching types that accept empty input (like `Vec`, `Option`, etc.).

### Quick Reference

| Pattern | Type | Use Case |
|---------|------|----------|
| Parse until failure | `Vec<T>` | Unknown terminator, parse as much as possible |
| Parse until terminator | `LazyVec<T, S>` | Known terminator, consume it |
| Parse before terminator | `LazyVecUntil<T, S>` | Known terminator, don't consume it |
| Delimited list | `DelimitedVec<T, D, P, MIN, MAX>` | Comma-separated, sized constraints |
| Optional non-empty | `NonEmptyOption<T>` | Parse T only if tokens remain |
| Positive lookahead | `Expect<T>` | Check what's next without consuming |
| Negative lookahead | `Except<T>` | Ensure what's next is NOT T |
| Conditional parse | `Either<Cons<Expect<A>, X>, Y>` | Parse X if A ahead, else Y |

**`RangedRepeats` type aliases:** `Any<T>`, `Many<T>`, `AtLeast<T,N>`, `AtMost<T,N>`, `Exactly<T,N>`

### See Also

- [Container Types](crate::container) - Implementation of lazy containers
- [Combinator Types](crate::combinator) - `Cons`, `Either`, and other combinators
- [Fundamental Types](crate::fundamental) - `Expect`, `Except`, `EndOfStream`


## Errors

Unsynn parsers return the first error encountered. It does not try error recovery on its own
or being smart about what may have caused an error (typos, missing semicolons etc).  The only
exception to this is when parsing disjunct entities ([`Either`](crate::Either) or other enums) where errors are
expected to happen on the first branches.  When any branch succeeds the error is dropped and
parsing goes on, when all branches fail then that error which made the most progress is
returned. Progress is tracked with the `ShadowCountedIter`.  This is implemented for enums
created with the `unsynn!` macro as well for the `Either::parser()` method.  This covers all
normal cases.

When one needs to implement disjunct parsers manually this has to be taken into account.
This is then done by creating an [`Error`](crate::Error) with [`ErrorKind::NoError`](crate::ErrorKind::NoError) by
`let mut err = Error::no_error()` within the `Parser::parse` implementation. Then any parser
that is called subsequently tries to `err.upgrade(Item::parser(..))` which handles storing the
error which made the most progress. Eventually a `Ok(...)` or the upgraded `Err(err)` is
returned. For details look at the source of [`Either::parser`](crate::Either).

Errors carry the failed token, the type name that was expected (possibly refined) and a
iterator past the location where the error happened. This can be used for further inspection.

Some parser types in unsynn are ZST's this means they don't carry the token they parsed and
consequently the have no `Span` thus the location of an error will be unavailable for them.
If that poses to be a problem this might be revised in future unsynn versions.

In other cases where spans are wrong this is considered a happy accident, please fill a Bug or
send a PR fixing this issue.


### Error Recovery

Trying to recover from syntax errors is usually not required/advised for compiled
languages. When there is a syntax error, report it to the user and let them fix the source
code.

Recoverable parsing would need knowledge of the grammar (or even context) being parsed. This
can not be supported by unsynn itself as it does not define any grammars. When one needs
recoverable parsers then this has to implemented into the grammar definition. Future versions
of unsynn may provide some tools to assist with this. The actual approach is still in
discussion.


## Writing Tests

When writing tests one often has to compare a parsed entity against some expected source code.
The `tokens_to_string()` method provides a reliable way to get a canonical (not pretty printed) 
string representation. This is used by the [`assert_tokens_eq!`](crate::assert_tokens_eq!) macro (requires `proc_macro2` 
feature for the string parsing):

```
# use unsynn::*;
// default construct some AST
let expression = <Cons<ConstInteger<1>, Plus, ConstInteger<2>>>::default();
// whitespace doesn't matter here, comments are stripped
assert_tokens_eq!(expression, " 1  +  2 // comments are stripped");
```

**Note**: `tokens_to_string()` works with both `proc_macro` and `proc_macro2`, but 
[`assert_tokens_eq!`](crate::assert_tokens_eq!) requires the `proc_macro2` feature because it needs to parse the 
expected string into tokens for comparison.


## Grammar Debugging

When developing complex parsers, it can be helpful to see what tokens are being parsed at 
specific points in your grammar. The [`debug`](crate::debug) module provides [`StderrLog<T, N>`](crate::debug::StderrLog) 
for this purpose. `T` is a informal type, the name will be printed, `N` is a integer limit
for how many tokens are printed (default: 5). Debug output is only generated when the
`debug_grammar` feature is enabled, otherwise this types parser is no-op.


### Usage

Wrap any parser type with [`StderrLog`](crate::StderrLog) to print debug information to stderr:

```rust,ignore
use unsynn::*;

// Enable with: cargo build --features debug_grammar

let mut tokens = "fn foo(bar: i32) -> Result<(), Error> { }".to_token_iter();

// With debug_grammar enabled, this prints to stderr:
// Type: proc_macro2::Ident
// Source: fn foo(bar : i32) -> Result …
// 
// Without debug_grammar, StderrLog is a complete no-op (doesn't parse or consume tokens)
let _ident: StderrLog<Ident, 10> = tokens.parse().unwrap();
```

### Debug Output Recursion

`StderrLog` automatically recurses into groups (parentheses, brackets, braces) and prints 
tokens from within them, respecting the `N` limit at each level.

### Best Practices

- Add `StderrLog` wrappers at key points in your grammar during development
- Use different `N` values to see more or less context
- **Leave them in place** - without the feature they're completely erased (zero-cost)
- Enable the feature only during debugging: `cargo test --features debug_grammar`

See the [`debug`](crate::debug) module documentation for more details.


## Generating and Transforming Code

After parsing one usually wants to emit modified or generated code. This is done with the
[`ToTokens`](crate::ToTokens) trait. For convenience we also provide a [`quote!{}`](crate::quote!) macro which allows one to
template code in place. Unlike its big cousin (the quote crate) ours is rather simple and has
less features (no `#(...)` repeats, but we have `#{...}` blocks). Eventually this may be
extended.


## Implementation/Performance Notes

Unsynn is (as of now) implemented as recursive descent PEG with backtracking.  This has
worst-case exponential complexity, there is a plan to fix that in future releases. Currently
to avoid these cases it is recommended to formulate disjunctive parsers so that they fail early
and don't share long prefixes.

For example things like
`Either<Cons<LongValidCode, OneThing>, Cons<LongValidCode, OtherThing>>` should be
rewritten as `Cons<LongValidCode, Either<OneThing, OtherThing>>`.


## Stability Guarantees

Unsynn is in development, nevertheless we give some stability promises. These will only be
broken when we discover technical reasons that make it infeasible to keep them up. Eventually
things in the unstable category below will move up to stable.


### Stable

 * Operator name definitions in [`operator::names`](crate::operator::names)  
   These follow common (rust) naming, if not this will be fixed, otherwise you can rely on
   these to be stable.
 * Functionality  
   Existing types and parsers are there to stay. A few things especially when they are freshly
   added may be shaken out and refined/extended but existing functionality should be
   preserved. In some cases changes may require minor adjustments on code using it.
   The unsynn! macro itself feels pretty good now. Few new features may be
   added and minor things fixed (internals, trait bounds etc.). Overall syntax probably stays


### Unstable

 * modules  
   The module organization is still in flux and may be refactored at any time. This shouldn't
   matter because unsynn reexports everything at the crate root. The only exception here is
   the [`operator::names`](crate::operator::names) which we try to keep stable.
 * internal representation  
   Don't rely on the internal representations there are some plans and ideas to change
   these. Some types that are currently ZST may become stateful, collections may become
   wrapped as `Rc<Vec<T>>` etc.
 * traits  
   Currently there are the main traits `Parse`, `Parser`, `IParse` and `ToTokens`. In future these may
   become refactored and split into smaller traits and some more may be added. This will then
   require some changes for the user. The existing functionality will be preserved nevertheless.
 * trait bounds  
   Trait bound are still in flux. Expect that types eventually should be at least
   `Clone + Hash + PartialEq`, some types will require `Default`. Possibly some helper trait needs to
   be implemented too. Details will be worked out over time.
 * TokenIter  
   Will be (again) completely rewritten in 0.4 for performance. No idea how well we can keep
   backward compatibility.
 * Parser Predicates  
   They work so far I am not totally happy with them. Decision if they stay as they are is postponed.
 * Debug module/`StderrLog`
   It's a hack will certainly see some makeover, but maybe the `StderrLog` just stays as is.
 * Lifetimes  
   Where and when we can support lifetimes in bounds (reverted back to 'static)
   needs to be shaken out. Probably it stays with 'static


## AI Agent Reference Guide

For AI coding assistants working with unsynn, see 'HOW-TO-USE-UNSYNN-BY-AI.md' included in the distribution - 
a comprehensive, compressed reference designed for AI agents. This standalone guide can be placed directly 
into projects using unsynn.

**Note for human readers:** The AI guide is optimized for machine parsing and may be dense. This cookbook 
and the [official documentation](https://docs.rs/unsynn/latest/unsynn/) are better suited for human learning.
