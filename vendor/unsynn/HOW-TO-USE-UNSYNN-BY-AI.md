# How to Use Unsynn - AI Agent Reference

**Target Audience:** AI coding assistants\
**Purpose:** Comprehensive, compressed reference for implementing parsers with unsynn\
**Key Principle:** Generated code must be both functional AND maintainable by humans

---

**⚠️ CRITICAL REMINDER FOR AI ASSISTANTS:**
- **ALWAYS re-read this document** when starting work on unsynn parsers
- **CHECK THIS FILE FIRST** before making assumptions about unsynn behavior
- This is the **authoritative reference** - maintained alongside the codebase
- If you encounter uncertainty, **re-read the relevant section** rather than guessing
- Document location: `/home/ct/src/unsynn/HOW-TO-USE-UNSYNN-BY-AI.md`

---

**Document Status:** This is the authoritative reference for unsynn, maintained alongside the codebase. It reflects the current implementation and is updated when significant features change or are added.

---

**Official Documentation:** For complete API reference and examples, see the [unsynn documentation](https://docs.rs/unsynn/latest/unsynn/).\
**Note:** If working with unreleased/pre-release/beta versions, some features documented here may not yet be available in the published docs.

---

**CRITICAL: Do NOT confuse unsynn with syn/quote crates:**
- unsynn has its own `ToTokens` trait (different from quote's `ToTokens`)
- unsynn does NOT use the `quote!` macro (completely different from quote crate)
- **NEVER add `syn` or `quote` as dependencies** to projects using unsynn
- unsynn is a complete standalone parsing solution

---

## Document Structure

**🎯 NAVIGATION GUIDE FOR AI ASSISTANTS:**

This document is organized for quick lookup. When implementing parsers:

1. **Starting out?** → Read sections 1-2 (Core Concepts, Syntax Patterns)
2. **Combining parsers?** → Section 3 (Combinator Types)
3. **Building expressions?** → Sections 4, 6.1-6.3 (Operators, Expression Parsing)
4. **Debugging issues?** → Section 9 (Performance and Pitfalls)
5. **Quick lookup?** → Section 10 (Quick Reference)

**⚠️ When uncertain:** Re-read the relevant section rather than making assumptions!

---

1. **Core Concepts** - Fundamental types and traits
2. **Syntax Patterns** - The `unsynn!` macro and type composition
3. **Operators Reference** - Predefined operators and critical patterns
4. **Literal Parsers** - Special behavior for String, Character, Integer literals
5. **Advanced Features** - Context, expressions, transactions, lookahead, utilities
6. **Testing** - Required test patterns
7. **Conventions** - Naming, visibility, documentation rules
8. **Performance** - Pitfalls, optimization, stability guarantees
9. **Quick Reference** - Lookup tables for all patterns

---

## 1. CORE CONCEPTS

### 1.1 Fundamental Architecture

unsynn is a **recursive descent PEG parser** for Rust `TokenStream`:

```
TokenStream → TokenIter → Parser trait → Parsed AST → ToTokens → TokenStream
```

**Key traits:**
- `Parser` - Implement to parse tokens (low-level, non-transactional)
- `Parse` - User API (transactional, auto-implemented for all `Parser` types)
- `ToTokens` - Serialize AST back to tokens
- `IParse` - Convenience for `TokenIter.parse::<T>()`

### 1.2 TokenIter - The Parsing Iterator

`TokenIter` wraps a `TokenStream` and provides:
- Token-by-token iteration
- Position tracking for errors
- Transaction support (clone-based backtracking)

**Creating TokenIter:**
```rust
use unsynn::*;

// From string (requires proc_macro2 feature)
let mut tokens = "foo, bar".to_token_iter();

// From TokenStream
let mut tokens = input.to_token_iter();
```

**Critical:** `TokenIter` is `Clone` - transactions clone the entire iterator state.

### 1.3 Parser vs Parse

**`Parser::parser()`:**
- Low-level, **non-transactional**
- Fast (30%+ faster than `Parse::parse()`)
- **Leaves iterator in consumed state on error**
- Use only for conjunctive (sequential) parsing
- Safe when chained with `?` operator

**`Parse::parse()`:**
- High-level, **transactional**
- Auto-implemented for all `Parser` types
- Rolls back iterator on error
- Use for disjunctive (alternative) parsing
- Default choice when unsure

**Rule:** In user code, always use `parse()` for safety. Only use `parser()` when implementing custom conjunctive parsers where you chain multiple `parser()` calls with `?` and want the performance benefit (e.g., inside `impl Parser`).

### 1.4 Error Handling

unsynn returns **first error**, no automatic recovery:

```rust
use unsynn::*;

let mut tokens = "invalid".to_token_iter();
match LiteralInteger::parse(&mut tokens) {
    Ok(int) => println!("Parsed: {}", int.value()),
    Err(e) => println!("Error: {:?}", e), // Contains: token, expected type, position
}
```

**For disjunctions** (`Either`, enums): Error that made **most progress** is returned.

### 1.5 Error Creation and Refinement

**Error creation methods:**
```rust
use unsynn::*;

// Unexpected token
Error::unexpected_token(Some(token), &tokens)?;

// Unexpected end of input
Error::unexpected_end()?;

// Out of range (for DelimitedVec bounds)
Error::out_of_range::<MAX>(expected, actual, &tokens)?;

// Custom error
Error::other(Some(token), &tokens, "validation failed".into())?;

// Dynamic error (runtime typename)
Error::dynamic(typename, Some(token), &tokens)?;
```

**Error refinement:**
```rust
use unsynn::*;

// Refine error with better type name
let result = tokens.parse::<ComplexType>()
    .refine_err::<BetterName>();
```

**Error information:**
```rust
let err: Error = /* ... */;

// Get expected type name
let typename: &str = err.expected_type_name();

// Get token that failed
let token: Option<TokenTree> = err.failed_at();

// Get iterator position after error
let iter: TokenIter = err.tokens_after();
```

### 1.6 ToTokens - Serialization

`ToTokens` turns parsed types back into tokens:

```rust
use unsynn::*;

let mut tokens = "foo, bar".to_token_iter();
let parsed: Cons<Ident, Comma, Ident> = tokens.parse().unwrap();

// Back to string (requires proc_macro2)
assert_eq!(parsed.tokens_to_string(), "foo , bar");

// Or use Display
println!("{}", parsed); // Uses tokens_to_string()
```

**Critical:** Always preserve original tokens when implementing parsers - never reconstruct from semantic values.

#### tokens_to_string() Spacing Behavior

**Not pretty-printing:** `tokens_to_string()` produces **unambiguous, non-pretty-printed** output. Spaces are **not part of the syntax** but are inserted after `Punct::Alone` to disambiguate operators:

```rust
use unsynn::*;

// Parse "a-=-b" where -= is parsed as MinusEq (single compound operator)
let mut tokens = "a-=-b".to_token_iter();
let expr: Cons<Ident, MinusEq, Minus, Ident> = tokens.parse().unwrap();

// tokens_to_string() adds space after MinusEq (Alone) to show operator boundaries
assert_eq!(expr.tokens_to_string(), "a -= - b");
// Without space: "a-=-b" doesn't show which chars form which operators (-=, -, or --, =, -?)
```

**Why spaces are inserted:**
- `Punct::Joint` - No space (part of compound operator like `+` in `+=`)
- `Punct::Alone` - Space added after to prevent ambiguous operator sequences
- This is a **string representation issue only** - not a parsing or tokenization concern

**Critical:** All operator types (predefined from `operator::names::*` and those defined with `operator!` or inside `unsynn!`) **always emit with `Spacing::Alone`**. When tokenizing `"+++"`, Rust produces `Joint-Joint-Alone`, but parsing `Cons<Plus, Plus, Plus>` produces three separate operators, each emitted as `Alone-Alone-Alone`. This is why `tokens_to_string()` adds spaces between them.

**In test suites:**
```rust
// Tokenizing "a-=-b" preserves Joint spacing: "a -=- b"
// But parsing Cons<Ident, MinusEq, Minus, Ident> emits all operators as Alone: "a -= - b"

// assert_tokens_eq! roundtrips BOTH sides through tokens_to_string()
assert_tokens_eq!(parsed, "a-=-b");
// Internally: parsed.tokens_to_string() == "a-=-b".to_token_iter().tokens_to_string()
// Left side (parsed operators):  "a -= - b"  (all Alone)
// Right side (tokenized string): "a -=- b"   (Joint preserved)
// THESE DON'T MATCH! This test would fail.

// Correct approach - use string that matches operator emission:
assert_tokens_eq!(parsed, "a -= - b");  // Both sides produce "a -= - b"

// When using assert_eq! with tokens_to_string(), manually match spacing:
assert_eq!(expr.tokens_to_string(), "a -= - b");  // Operators are Alone
```

**Key insight:** The spacing is **deterministic and unambiguous** - it ensures operator sequences can be round-tripped correctly, but it's not about aesthetics or pretty-printing.

### 1.7 Fundamental Token Types

**Direct token parsers** (from proc_macro/proc_macro2):

```rust
use unsynn::*;

// TokenTree - Matches any single token
let tt: TokenTree = tokens.parse().unwrap();

// Ident - Identifier
let id: Ident = tokens.parse().unwrap();

// Literal - Any literal token
let lit: Literal = tokens.parse().unwrap();

// Punct - Single punctuation character
let p: Punct = tokens.parse().unwrap();

// Group - Delimited group ((), [], {})
let g: Group = tokens.parse().unwrap();
```

**Punctuation with spacing:**
```rust
use unsynn::*;

// PunctAny<C> - Punctuation with any spacing
type AnyPlus = PunctAny<'+'>;

// PunctJoint<C> - Punctuation with Joint spacing (for compound operators)
type JointPlus = PunctJoint<'+'>;

// PunctAlone<C> - Punctuation with Alone spacing
type AlonePlus = PunctAlone<'+'>;
```

**Group types:**
```rust
// Specific delimiter groups
ParenthesisGroup    // ( ... )
BracketGroup        // [ ... ]
BraceGroup          // { ... }
```

**CachedLiteral - Efficient literal with string access:**
```rust
let lit: CachedLiteral = tokens.parse().unwrap();
let s = lit.as_str();  // O(1) cached access, no allocation
```

### 1.8 Parse Predicates - Compile-Time Context

Parse predicates provide **zero-cost** compile-time parser control without runtime overhead.

**Base predicates:**
```rust
use unsynn::predicates::*;

Enable         // Always succeeds
Disable        // Always fails  
TokensRemain   // Succeeds only if tokens remain
```

**Logical operators:**
```rust
Not<P>              // NOT
AllOf<A, B>         // AND (2-4 operands)
AnyOf<A, B>         // OR (2-4 operands)
OneOf<A, B>         // XOR (2-4 operands)
```

**Custom context predicates:**
```rust
use unsynn::*;

unsynn! {
    // Define context trait
    trait ExpressionContext: PredicateOp;
    
    // Create predicate implementing the trait
    predicateflag InExpression = Enable for ExpressionContext;
    predicateflag NotInExpression = Disable for ExpressionContext;
    
    // Use predicate to constrain parser
    struct StructLiteral<P: ExpressionContext> {
        _guard: P,
        name: Ident,
    }
}

// Only callable with ExpressionContext predicates
type AllowedStruct = StructLiteral<InExpression>;
```

**All predicates are zero-sized** - no runtime cost. See Section 6.6 for advanced usage.

---

## 2. SYNTAX PATTERNS

**🔄 REMINDER:** If you haven't read Section 1 (Core Concepts) recently, review it first. Understanding `Parser` vs `Parse`, `TokenIter`, and error handling is essential.

### 2.1 The `unsynn!` Macro

**Primary tool** for defining grammars. Generates `Parser` and `ToTokens` implementations:

```rust
use unsynn::*;

unsynn! {
    // Simple struct (sequential parsing)
    pub struct Assignment {
        pub name: Ident,
        pub eq: Assign,
        pub value: LiteralInteger,
    }
    
    // Enum (alternative parsing)
    pub enum Literal {
        Int(LiteralInteger),
        Str(LiteralString),
        Char(LiteralCharacter),
    }
    
    // Type alias (for documentation/reuse)
    pub type Parameters = CommaDelimitedVec<Parameter>;
}
```

**Critical:** Order matters in enums - specific types before general types.

### 2.1.1 Pass-Through Items

The `unsynn!` macro passes through certain items without processing them. These work naturally:

**Type aliases** - Supported for simple paths and generic definitions:
```rust
unsynn! {
    // ✅ Simple path-based aliases work
    pub type NodeId = LiteralInteger;
    pub type HashMap = std::collections::HashMap;
    
    // ✅ Generic type alias definitions work
    pub type Result<T> = std::result::Result<T, Error>;
    pub type OptVec<T> = Option<Vec<T>>;
    pub type ParamList<T> = CommaDelimitedVec<T>;
    
    // ✅ Complex type aliases work
    pub type EitherLit = Either<LiteralInteger, LiteralString>;
    pub type Parameters = CommaDelimitedVec<Parameter>;
}
```

**Note:** Type aliases pass through the macro unchanged - they work exactly as they do in regular Rust. Both simple paths and complex generic type expressions are fully supported.

**Use statements** - Import external types:
```rust
unsynn! {
    use crate::ast::ExpressionNode;
    use std::collections::HashMap;
    
    struct MyStruct {
        expr: ExpressionNode,
    }
}
```

**Functions** - Including generic functions:
```rust
unsynn! {
    fn helper(x: Ident) -> bool {
        x.to_string() == "foo"
    }
    
    fn identity<T>(x: T) -> T {
        x
    }
}
```

**Macro invocations** - Attributes and macro_rules!:
```rust
unsynn! {
    // Derive macros
    #[derive(Clone, PartialEq)]
    struct MyStruct {
        field: Ident,
    }
    
    // macro_rules! definitions
    macro_rules! helper {
        () => { 42 };
    }
}
```

**Traits** - Both marker and full definitions:
```rust
unsynn! {
    // Marker trait
    trait MyMarker;
    
    // Full trait definition
    trait MyTrait {
        fn method(&self) -> bool;
    }
}
```

**Impl blocks** - Both trait and inherent:
```rust
unsynn! {
    // Inherent impl (simple type names only)
    impl MyStruct {
        fn new() -> Self { MyStruct { field: Default::default() } }
    }
    
    // Generic inherent impl
    impl<T> MyStruct<T> {
        fn create(value: T) -> Self { MyStruct { value } }
    }
    
    // Trait impl
    impl MyTrait for MyStruct {
        fn method(&self) -> bool { true }
    }
}
```

**What does NOT pass through:**
- Unit structs like `struct Foo;` (not yet supported - see [ROADMAP.md](ROADMAP.md) §"Unit struct/marker type support")
- Inherent impl for path-based types (e.g., `impl foo::Bar { ... }` - only works for simple idents)
- Const and static items (not yet tested/documented)

### 2.2 Struct Patterns

**Named struct (preferred):**
```rust
unsynn! {
    pub struct NamedStruct {
        pub field1: Type1,
        pub field2: Type2,
    }
}
```

**Tuple struct:**
```rust
unsynn! {
    pub struct TupleStruct(Type1, Type2, Type3);
}
```

**Note:** Unit structs (`struct Foo;`) are not yet supported. See [ROADMAP.md](ROADMAP.md) §"Unit struct/marker type support" for the current status and planned implementation.

**Keyword fields - avoid name clashes:**
```rust
unsynn! {
    pub struct ReturnExpr {
        pub return_kw: KwReturn,  // Suffix avoids clash with Rust keyword 'return'
        pub value: Option<Box<Expr>>,
    }
}
```

**Critical:** Field names that match Rust keywords (e.g., `return`, `fn`, `let`, `match`) will cause compilation errors. Use suffixes like `_kw`, `_token`, or descriptive names like `return_keyword`. The `_kw` suffix is a common convention but not mandatory.

### 2.3 Enum Patterns

**Critical:** Order matters - try specific types before general:

```rust
unsynn! {
    // CORRECT: Specific first
    pub enum Expression {
        Binary(BinaryExpr),      // Try specific types first
        Unary(UnaryExpr),
        Literal(LiteralInteger),
        Ident(Ident),           // More general
        Grouped(ParenGroup),
    }
    
    // WRONG: General first (Ident/TokenTree match too much)
    pub enum Expression {
        Ident(Ident),           // Would match before more specific types
        Literal(LiteralInteger),
        Binary(BinaryExpr),
    }
}
```

### 2.4 Operator Definition

**Prefer defining operators inside `unsynn!` blocks:**

```rust
unsynn! {
    operator Plus = "+";
    operator FatArrow = "=>";
    operator PathSep = "::";
    operator QuadDot = "....";  // Up to 4 punctuation chars
}
```

**Alternative:** Use standalone `operator!` macro only when needed outside `unsynn!`.

**Use predefined operators when possible** (see Section 4).

### 2.5 Generics and Lifetimes

Full support for generics with trait bounds:

```rust
unsynn! {
    // Simple generic
    pub struct Container<T> {
        pub value: T,
    }
    
    // With trait bounds
    pub struct Wrapper<T: Clone + std::fmt::Debug> {
        pub item: T,
    }
    
    // Lifetime parameters
    pub struct Reference<'a, T> {
        pub data: T,
        pub _lifetime: std::marker::PhantomData<&'a ()>,
    }
    
    // Multiple bounds with +
    pub struct Complex<T: Clone + Send + 'static> {
        pub value: T,
    }
}
```

### 2.6 Custom Parsing with `parse_with`

For validation or transformation:

**Validation (no `from`):**
```rust
unsynn! {
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

**Transformation (requires `from Type`):**
```rust
unsynn! {
    struct BoolInt(bool) from LiteralInteger;
    parse_with |value, _tokens| {
        Ok(Self(value.value() != 0))
    };
}
```

### 2.7 Custom ToTokens with `to_tokens`

Override token emission:

```rust
unsynn! {
    struct Normalized(String);
    to_tokens |val, output| {
        // Emit normalized version
        Ident::new(&val.0, Span::call_site()).to_tokens(output);
    };
}
```

### 2.8 HiddenState - Non-Syntactic Data

Store metadata that doesn't affect parsing:

```rust
unsynn! {
    pub struct Analyzed {
        pub expr: Expression,
        pub metadata: HiddenState<AnalysisResult>,  // Not parsed
    }
}

// Populate via parse_with
```

**Critical:** `HiddenState<T>` never parses, always defaults. Use `parse_with` to populate.

---

## 3. COMBINATOR TYPES

**🔄 REMINDER:** Before using combinators, verify you understand:
- Section 2.3: Enum ordering rules (specific before general)
- Section 1.3: When to use `Parse::parse()` vs `Parser::parser()`
- This section describes how to compose parsers - re-read if uncertain about any combinator

### 3.1 Sequential Composition - `Cons`

Parse types in sequence:

```rust
use unsynn::*;

// Cons<A, B, C> parses A, then B, then C
type Assignment = Cons<Ident, Assign, LiteralInteger>;

let mut tokens = "x = 42".to_token_iter();
let parsed: Assignment = tokens.parse().unwrap();
assert_eq!(parsed.first.to_string(), "x");
assert_eq!(parsed.third.value(), 42);
```

**Access fields:** `first`, `second`, `third`, `fourth` (up to 4 types).

**For more than 4 types:** Nest `Cons` types:
```rust
// 6 sequential types - nest Cons
type Complex = Cons<A, B, Cons<C, D, E, F>>;
```

### 3.2 Alternative Composition - `Either`

Parse first matching alternative:

```rust
use unsynn::*;

// Either<A, B, C, D> tries A, then B, then C, then D (up to 4 types)
type Value = Either<LiteralInteger, LiteralString, Ident>;

let mut tokens = "42".to_token_iter();
let parsed: Value = tokens.parse().unwrap();
match parsed {
    Either::First(int) => println!("Integer: {}", int.value()),
    Either::Second(s) => println!("String: {}", s.value()),
    Either::Third(id) => println!("Ident: {}", id),
}
```

**For more than 4 alternatives:** Nest `Either` types:
```rust
// 6 alternatives - nest Either
type ManyChoices = Either<A, B, C, Either<D, E, F>>;
```

**Critical:** Order matters - specific before general (see Section 2.3).

### 3.3 Optional - `Option<T>`

Parse optional content:

```rust
use unsynn::*;

unsynn! {
    pub struct OptionalSuffix {
        pub base: Ident,
        pub suffix: Option<Semicolon>,
    }
}
```

**Warning:** `Option<T>` can match empty input if `T` has fallback. Use `NonEmptyOption<T>` to prevent.

### 3.4 Repetition - `Vec<T>`

Parse repeated content (0 or more):

```rust
use unsynn::*;

unsynn! {
    pub struct Items {
        pub items: Vec<Item>,
    }
}
```

**Critical:** Never use `Vec<T>` if `T` has a fallback variant that always succeeds (infinite loop). Use `LazyVec` or `LazyVecUntil` instead.

### 3.5 LazyVec - Repetition with Terminator (Consumes Terminator)

Parse repeated content until terminator appears, **consuming the terminator**:

```rust
use unsynn::*;

// LazyVec<T, Stop> checks for Stop BEFORE parsing each T, then consumes Stop
// Critical when T could match Stop (e.g., TokenTree matches everything)
unsynn! {
    pub struct Sequence {
        pub items: LazyVec<TokenTree, Semicolon>,
        // Semicolon is consumed and stored in items.terminator
    }
}

// items.terminator contains the parsed Semicolon
// items.value contains Vec<TokenTree>
```

**Use case:** When `T` could match the terminator. Examples:
- `LazyVec<TokenTree, Semicolon>` - TokenTree matches any token, including semicolons
- Any case where the item parser is general enough to consume the terminator

**Note:** If you don't want to consume the terminator, use `LazyVecUntil` instead (see 3.6).

### 3.6 LazyVecUntil - Lookahead Terminator

Parse until terminator (don't consume it):

```rust
use unsynn::*;

// LazyVecUntil<T, Stop> stops BEFORE Stop, doesn't consume it
unsynn! {
    pub struct Sequence {
        pub items: LazyVecUntil<Item, Semicolon>,
        // Semicolon remains in stream
    }
}
```

### 3.7 DelimitedVec - The Foundation

**Most powerful repetition type** - all others build on this:

```rust
DelimitedVec<
    T,              // Item type
    D,              // Delimiter type (Nothing for no delimiter)
    P,              // TrailingDelimiter policy
    MIN,            // Minimum count
    MAX             // Maximum count
>
```

**TrailingDelimiter policies:**
- `TrailingDelimiter::Forbidden` - No trailing delimiter allowed
- `TrailingDelimiter::Optional` - Trailing delimiter optional
- `TrailingDelimiter::Mandatory` - Trailing delimiter required

**Common aliases:**
```rust
// 0+ items, optional trailing delimiter
type Any<T, D = Nothing> = DelimitedVec<T, D, TrailingDelimiter::Optional, 0, {usize::MAX}>;

// 1+ items, optional trailing delimiter
type Many<T, D = Nothing> = DelimitedVec<T, D, TrailingDelimiter::Optional, 1, {usize::MAX}>;

// 0-1 items
type Optional<T, D = Nothing> = DelimitedVec<T, D, TrailingDelimiter::Optional, 0, 1>;

// Exactly N items
type Exactly<T, N> = DelimitedVec<T, Nothing, TrailingDelimiter::Mandatory, N, N>;

// Common delimiters
type CommaDelimitedVec<T> = DelimitedVec<T, Comma, TrailingDelimiter::Optional, 0, {usize::MAX}>;
type SemicolonDelimitedVec<T> = DelimitedVec<T, Semicolon, TrailingDelimiter::Optional, 0, {usize::MAX}>;
type PathSepDelimitedVec<T> = DelimitedVec<T, PathSep, TrailingDelimiter::Forbidden, 1, {usize::MAX}>;
```

### 3.8 Groups - Parentheses, Brackets, Braces

Parse delimited content:

```rust
use unsynn::*;

unsynn! {
    pub struct Function {
        pub name: Ident,
        pub params: ParenthesisGroupContaining<Parameters>,
        pub body: BraceGroupContaining<Block>,
    }
    
    pub struct Array {
        pub elements: BracketGroupContaining<CommaDelimitedVec<Expr>>,
    }
}
```

**Available:**
- `ParenthesisGroupContaining<T>` - `(T)`
- `BracketGroupContaining<T>` - `[T]`
- `BraceGroupContaining<T>` - `{T}`

**Access:** `.value` field contains the parsed content.

### 3.9 Box and Rc - Recursion

Break recursive type cycles:

```rust
use unsynn::*;

unsynn! {
    pub enum Expr {
        Literal(LiteralInteger),
        Binary(BinaryExpr),
        Grouped(ParenthesisGroupContaining<Box<Expr>>),  // ← Box breaks recursion
    }
    
    pub struct BinaryExpr {
        pub left: Box<Expr>,
        pub op: Operator,
        pub right: Box<Expr>,
    }
}
```

**Use `Box`** for recursion and size optimization.\
**Use `Rc`** for shared references (rare).

### 3.10 NonParseable - Development Placeholder

Use `NonParseable` as a placeholder for unimplemented parsers during development:

```rust
use unsynn::*;

unsynn! {
    pub enum Statement {
        Let(LetStatement),
        Return(ReturnStatement),
        Expression(ExprStatement),
        // TODO: Implement these later
        For(NonParseable),
        While(NonParseable),
    }
}
```

**With `nonparseable` feature:**
- `Parser::parser()` panics with clear message
- `ToTokens::to_tokens()` panics with clear message
- Helps identify unimplemented parts at runtime

**Without `nonparseable` feature:**
- `Parser` and `ToTokens` not implemented
- Compilation error if used
- Use for release builds

**Critical:** Remove all `NonParseable` before production use.

### 3.11 DynNode - Runtime Type Replacement

Parse a type, then replace it at runtime with anything implementing `ToTokens`:

```rust
use unsynn::*;

let mut tokens = "foo".to_token_iter();
let parsed = <DynNode<Ident>>::parser(&mut tokens).unwrap();
assert_tokens_eq!(parsed, "foo");

// Replace with different type
parsed.replace_all_with(LiteralInteger::new(123));
assert_tokens_eq!(parsed, "123");

// Local replacement (only this clone)
let mut other = parsed.clone();
other.replace_here_with(Bang::default());
assert_tokens_eq!(other, "!");
assert_tokens_eq!(parsed, "123");  // Original unchanged
```

**Backed by `Rc`** - replacements can be global or local.

**Use cases:**
- Macro expansion transformations
- Variable substitution
- Template instantiation
- Dynamic AST rewriting

**Rarely needed** for standard parsing - only for advanced transformations.

### 3.12 NonEmptyOption - Optional with Token Check

Parse optional content only if tokens are available:

```rust
use unsynn::*;

// Only parses if tokens remain
type TrailingExpr = NonEmptyOption<Expression>;
```

**Difference from `Option<T>`:**
- `Option<T>` can match empty input if `T` accepts empty (e.g., `Vec`, `Option`)
- `NonEmptyOption<T>` checks token availability first
- Prevents matching types that accept zero input

**Use when:**
- ✅ Optional content at end of input
- ✅ Preventing empty matches from fallback types
- ❌ Simple optional fields (use `Option<T>`)

---

## 4. OPERATORS REFERENCE

**🔄 REMINDER:** Section 4.2 covers the CRITICAL `Except` pattern for operator precedence. If building expression parsers with operators like `+` vs `+=`, you MUST read 4.2 and 6.2 before proceeding.

### 4.1 Predefined Operators

**Critical:** Always use predefined operators from `unsynn::operator::names::*`:

```rust
use unsynn::operator::names::*;

// Arithmetic
Plus        // +
Minus       // -
Star        // *
Slash       // /
Percent     // %

// Compound assignment
PlusEq      // +=
MinusEq     // -=
StarEq      // *=
SlashEq     // /=
PercentEq   // %=

// Bitwise
And         // &
Or          // |
Caret       // ^
Shl         // <<
Shr         // >>

// Compound bitwise
AndEq       // &=
OrEq        // |=
CaretEq     // ^=
ShlEq       // <<=
ShrEq       // >>=

// Logical
AndAnd      // &&
OrOr        // ||
Bang        // !

// Comparison
Equal       // ==
NotEqual    // !=
Lt          // <
Gt          // >
Le          // <=
Ge          // >=

// Assignment
Assign      // =

// Ranges
Dot         // .
DotDot      // ..
DotDotEq    // ..=
Ellipsis    // ...

// Delimiters
Comma       // ,
Semicolon   // ;
Colon       // :
PathSep     // ::

// Arrows
RArrow      // ->
LArrow      // <-
FatArrow    // =>

// Other
At          // @
Pound       // #
Dollar      // $
Question    // ?
Tilde       // ~
Backslash   // \

// Special
LifetimeTick // ' with Spacing::Joint
```

### 4.2 CRITICAL: Except Pattern for Compound Operators

**When needed:** Building operator precedence hierarchies where **shorter operators have HIGHER precedence** than longer operators sharing their prefix.

**Problem:** Tokenizer creates `+=` as two tokens: `+(Joint)` + `=(Alone)`. When `+` (additive) has higher precedence than `+=` (assignment), the additive parser runs first and matches just `+`, leaving `=` unparsed.

**Solution:** Use `Cons<Except<LongerOp>, ShorterOp>` when the shorter operator has higher precedence:

```rust
use unsynn::*;
use unsynn::operator::names::*;

unsynn! {
    // Additive operators (HIGHER precedence than assignment)
    pub enum AdditiveOp {
        // CRITICAL: + has higher precedence than +=
        // Without Except, + would match first token of +=
        Add(Cons<Except<PlusEq>, Plus>),
        Sub(Cons<Except<MinusEq>, Minus>),
    }
    
    // Multiplicative operators (HIGHER precedence than assignment)
    pub enum MultiplicativeOp {
        Mul(Cons<Except<StarEq>, Star>),
        Div(Cons<Except<SlashEq>, Slash>),
        Mod(Cons<Except<PercentEq>, Percent>),
    }
    
    // Bitwise operators (HIGHER precedence than logical/assignment)
    pub enum BitwiseOp {
        // & has higher precedence than && and &=
        And(Cons<Except<Either<AndAnd, AndEq>>, And>),
        // | has higher precedence than || and |=
        Or(Cons<Except<Either<OrOr, OrEq>>, Or>),
        // ^ has higher precedence than ^=
        Xor(Cons<Except<CaretEq>, Caret>),
    }
    
    // Shift operators (HIGHER precedence than comparison in some grammars)
    pub enum ShiftOp {
        Shl(Cons<Except<ShlEq>, Shl>),
        Shr(Cons<Except<ShrEq>, Shr>),
    }
    
    // Comparison operators (may conflict with shifts)
    pub enum ComparisonOp {
        // If < has higher precedence than <<
        Lt(Cons<Except<Either<Shl, ShlEq>>, Lt>),
        Gt(Cons<Except<Either<Shr, ShrEq>>, Gt>),
        Le(Le),  // No conflict
        Ge(Ge),  // No conflict
    }
    
    // Assignment operators (LOWEST precedence - no Except needed)
    pub enum AssignmentOp {
        Assign(Assign),      // = has lowest precedence, parsed last
        PlusEq(PlusEq),      // += parsed at assignment level
        MinusEq(MinusEq),    // No Except needed - assignment is lowest
        // ... other compound assignments
    }
}
```

**Rule:** Use `Cons<Except<LongerOp>, ShorterOp>` **only when** the shorter operator has **higher precedence** than the longer one in your grammar. If the longer operator (e.g., `+=`) has lower or equal precedence and is parsed at a different level, no `Except` is needed.

### 4.3 Keywords and Keyword Groups

**Prefer defining keywords inside `unsynn!` blocks:**

```rust
use unsynn::*;

unsynn! {
    // Single keywords
    keyword Fn = "fn";
    keyword Let = "let";
    keyword Pub = "pub";
    
    // Keyword groups (matches any keyword in the list)
    keyword Visibility = [Pub, "priv", "crate"];
    
    // From string literals
    keyword IfElseThen = ["if", "else", "then"];
    
    // Negated - matches idents that are NOT in the list
    keyword NotKeyword != ["fn", "let", "const"];
    
    // Use in grammars
    pub struct Function {
        pub vis: Option<Visibility>,
        pub fn_kw: Fn,
        pub name: Ident,
    }
}
```

**Alternative:** Use standalone `keyword!` macro when needed outside `unsynn!`:
```rust
// Only use when you need keywords outside unsynn! blocks
keyword! {
    pub Return = "return";
    pub Break = "break";
}
```

**Performance:**
- Linear search for ≤4 keywords
- Hash table for >4 keywords (with `hash_keywords` feature)
- Binary search as fallback (without `hash_keywords`)

**Note:** Keywords are matched as `Ident` tokens with specific values, not punctuation.

---

## 5. LITERAL PARSERS - SPECIAL BEHAVIOR

### 5.1 Why CachedLiteral

`Literal` from proc_macro/proc_macro2 doesn't provide efficient string access:

```rust
// Standard Literal requires allocation
let s = lit.to_string();  // Allocates new String

// CachedLiteral provides O(1) access
let s = lit.as_str();  // No allocation
```

**Rule:** Always use `CachedLiteral` for custom literal validation.

### 5.2 Validated Literals Pattern

Use `parse_with()` for custom literal types:

```rust
use unsynn::*;

pub struct IntegerLiteral {
    original: CachedLiteral,
}

impl IntegerLiteral {
    fn is_integer(s: &str) -> bool {
        // Exclude float notation
        if s.contains('.') || s.contains('e') || s.contains('E') {
            return false;
        }
        // Exclude string/char literals
        if s.starts_with('"') || s.starts_with('\'') {
            return false;
        }
        true
    }
}

impl Parser for IntegerLiteral {
    fn parser(input: &mut TokenIter) -> Result<Self> {
        CachedLiteral::parse_with(input, |lit, pos| {
            if Self::is_integer(lit.as_str()) {
                Ok(IntegerLiteral { original: lit })
            } else {
                Error::unexpected_token(Some(lit.into()), pos)
            }
        })
    }
}

impl ToTokens for IntegerLiteral {
    fn to_tokens(&self, output: &mut TokenStream) {
        // Always preserve original token
        self.original.to_tokens(output);
    }
}
```

**Pattern applies to:**
- Integer literals (exclude floats)
- Byte literals (validate byte range)
- Raw strings (validate format)
- Character literals (validate char contents)

**Critical:** Always preserve original token in `ToTokens` - never reconstruct from values.

### 5.3 Built-in Literal Types

unsynn provides:
- `LiteralInteger` - Integer literals only
- `LiteralCharacter` - Character literals
- `LiteralString` - String literals
- `CachedLiteral` - Any literal with cached string

**All validated literal types** follow the pattern in 5.2.

---

## 6. ADVANCED FEATURES

**⚠️ IMPORTANT CHECKPOINT:** Advanced features require solid understanding of basics:
- Have you read Section 1 (Core Concepts)?
- Have you read Section 2 (Syntax Patterns)?
- Have you read Section 3 (Combinator Types)?

If not, **stop and read those sections first**. Advanced features build on fundamentals.

**🔄 RE-READ THIS DOCUMENT** if you're implementing:
- Expression parsers with operator precedence → Section 6.1-6.2
- Lookahead patterns → Section 6.4
- Context-sensitive parsing → Section 6.6

### 6.1 Expression Parsing Building Blocks

**Critical:** Use expression types from `unsynn::expressions::*` for operator precedence.

**Available types:**
```rust
use unsynn::expressions::*;

// Non-associative binary (a OP b, no chaining)
InfixExpr<Operand, Operator, MAX=2>
NonAssocExpr<Operand, Operator>  // Alias for InfixExpr

// Left-associative ((a + b) + c)
LeftAssocExpr<Operand, Operator, MAX=usize::MAX>

// Right-associative (a = (b = c))
RightAssocExpr<Operand, Operator, MAX=usize::MAX>

// Prefix unary (Op... Operand)
PrefixExpr<Operator, Operand, MAX=usize::MAX>

// Postfix unary (Operand Op...)
PostfixExpr<Operand, Operator, MAX=usize::MAX>
```

**All use DelimitedVec internally - NO NESTING REQUIRED:**

```rust
// ✅ CORRECT: Single type handles multiple operators
type MyPrefix = PrefixExpr<Minus, Ident>;  // Handles ---x

// ❌ WRONG: Don't nest - it's redundant!
type MyPrefix = PrefixExpr<Minus, PrefixExpr<Minus, Ident>>;
```

### 6.2 Building Precedence Hierarchies

**Build from highest to lowest precedence:**

```rust
use unsynn::*;
use unsynn::expressions::*;
use unsynn::operator::names::*;

unsynn! {
    // Define operators at each level
    operator Pow = "^";
    
    pub enum MultiplicativeOp {
        Mul(Cons<Except<StarEq>, Star>),
        Div(Cons<Except<SlashEq>, Slash>),
    }
    
    pub enum AdditiveOp {
        Add(Cons<Except<PlusEq>, Plus>),
        Sub(Cons<Except<MinusEq>, Minus>),
    }
    
    // Primary expressions (includes parentheses for recursion)
    pub enum PrimaryExpr {
        Literal(LiteralInteger),
        Ident(Ident),
        Grouped(ParenthesisGroupContaining<Box<Expr>>),  // Box breaks recursion
    }
    
    // Build precedence tower (highest to lowest binding)
    pub type PowerExpr = RightAssocExpr<PrimaryExpr, Pow>;
    pub type MultExpr = LeftAssocExpr<PowerExpr, MultiplicativeOp>;
    pub type AddExpr = LeftAssocExpr<MultExpr, AdditiveOp>;
    
    // Top-level expression type
    pub type Expr = AddExpr;
}

// Correctly parses: 2 + 3 * 4^5 as 2 + (3 * (4^5))
```

**Key insight:** Each level uses next HIGHER precedence level as operand type.

### 6.3 Parentheses in Expressions

**Expression types don't include parentheses** - add them in primary expression:

```rust
enum PrimaryExpr {
    Literal(LiteralInteger),
    Grouped(ParenthesisGroupContaining<Box<Expr>>),  // ← Box here for recursion
    // ... other primary forms
}
```

**Box is required** to break the recursive type cycle.

### 6.4 Lookahead - Expect and Except

**`Except<T>` - Negative lookahead:**
```rust
use unsynn::*;

// Succeeds if T would NOT match
Cons<Except<PlusEq>, Plus>  // Match + only if += doesn't match
```

**`Expect<T>` - Positive lookahead:**
```rust
use unsynn::*;

// Succeeds if T would match (doesn't consume)
Cons<Ident, Expect<Assign>>  // Match ident, verify = follows (don't consume =)
```

**Both are zero-sized** - no runtime cost.

### 6.5 Transactions

**Manual transaction for alternatives:**

```rust
use unsynn::*;

impl Parser for MyEnum {
    fn parser(input: &mut TokenIter) -> Result<Self> {
        // Transaction for compound alternatives
        if let Ok(variant) = input.transaction(|t| {
            Ok(MyEnum::Complex(
                Field1::parser(t)?,
                Field2::parser(t)?,
            ))
        }) {
            return Ok(variant);
        }
        
        // Parse::parse() includes transaction
        if let Ok(simple) = input.parse() {
            return Ok(MyEnum::Simple(simple));
        }
        
        // Final alternative - no transaction needed
        Ok(MyEnum::Last(LastType::parser(input)?))
    }
}
```

**Rule:**
- Use transaction for compound alternatives (multiple `parser()` calls)
- `Parse::parse()` already includes transaction
- Final alternative doesn't need transaction

### 6.6 Parse Predicates - Compile-Time Context

**Zero-cost compile-time parser control** using predicate types.

**Base predicates:**
```rust
use unsynn::predicates::*;

Enable         // Always succeeds (no tokens consumed)
Disable        // Always fails
TokensRemain   // Succeeds only if tokens remain in stream
```

**Logical operators (2-4 operands):**
```rust
Not<P>              // Logical NOT
AllOf<A, B>         // Logical AND (all must succeed)
AnyOf<A, B>         // Logical OR (at least one must succeed)
OneOf<A, B>         // Logical XOR (exactly one must succeed)

// Examples
let mut tokens = "".to_token_iter();
assert!(AllOf::<Enable, Enable>::parse(&mut tokens).is_ok());
assert!(AnyOf::<Enable, Disable>::parse(&mut tokens).is_ok());
assert!(OneOf::<Enable, Disable>::parse(&mut tokens).is_ok());
assert!(Not::<Disable>::parse(&mut tokens).is_ok());
```

**Custom context predicates:**

`predicatetrait` defines custom marker traits for context validation. It automatically:
1. Creates the trait extending `PredicateOp` (always flat - no hierarchy)
2. Implements it for all base predicates (Enable, Disable, TokensRemain)
3. Implements it for all logical operators (AllOf, AnyOf, OneOf, Not) when their operands also implement the trait

**Critical:** Predicate traits are **flat** - they do not support supertrait hierarchies. Use logical operators (AllOf, AnyOf, etc.) to compose multiple context requirements instead.

```rust
use unsynn::*;

unsynn! {
    // Define custom context traits - all are flat, extending only PredicateOp
    predicatetrait ExpressionContext;
    predicatetrait StatementContext;
    
    // Create predicates implementing the trait
    predicateflag InExpression = Enable for ExpressionContext;
    predicateflag NotInExpression = Disable for ExpressionContext;
    
    // Parser that requires specific context
    struct StructLiteral<P: ExpressionContext> {
        _guard: P,           // Zero-sized, no runtime cost
        name: Ident,
        fields: BraceGroupContaining<Fields>,
    }
}

// Type aliases for different contexts
type AllowedStructLiteral = StructLiteral<InExpression>;
type ForbiddenStructLiteral = StructLiteral<NotInExpression>;  // Won't parse

// Use in grammar
unsynn! {
    pub enum Expression {
        StructLit(StructLiteral<InExpression>),  // Only allowed in expression context
        // ... other variants
    }
    
    pub enum Statement {
        // StructLiteral<NotInExpression> would fail to parse here
        // ... statement variants
    }
}
```

**Composing multiple context requirements:**
```rust
unsynn! {
    predicatetrait ExprContext;
    predicatetrait TypeContext;
    
    predicateflag InExpr = Enable for ExprContext;
    predicateflag InType = Enable for TypeContext;
    
    // ✅ CORRECT: Use AllOf to require both contexts
    struct Generic<P: AllOf<ExprContext, TypeContext>> {
        _guard: P,
        content: Ident,
    }
    
    // ❌ WRONG: Don't try to make TypeContext extend ExprContext
    // predicatetrait TypeContext: ExprContext;  // Not supported!
}
```

**Type identity checking with `PredicateCmp<A, B, Same = Enable, Different = Disable>`:**

`PredicateCmp` enables distinguishing between different predicate flags at compile time, even
when they implement the same trait. It dispatches to `Same` when `A == B`, and `Different` when `A != B`.

```rust
use unsynn::*;

unsynn! {
    predicatetrait Context;
    predicateflag InExpr = Enable for Context;
    predicateflag InStmt = Enable for Context;
    
    // Only accepts InExpr, rejects InStmt (even though both implement Context)
    struct StructLiteral<C: Context> {
        guard: PredicateCmp<C, InExpr>,  // Type identity check
        // ... fields
    }
    
    // Accepts InExpr but NOT InStmt using logical operators
    struct TernaryExpr<C: Context> {
        guard: AllOf<PredicateCmp<C, InExpr>, Not<PredicateCmp<C, InStmt>>>,
        // ... fields
    }
}

// ✅ StructLiteral::<InExpr> - accepted (InExpr == InExpr)
// ❌ StructLiteral::<InStmt> - rejected (InStmt != InExpr)
```

**Requirements:**
- Type parameters must implement `PredicateOp` (which includes `'static` bound)
- Zero runtime cost (optimized to compile-time constant by LLVM)
- Combine with `Not`, `AllOf`, `AnyOf` for complex conditions

**When to use predicates:**
- ✅ Grammar rules that forbid constructs in certain contexts
- ✅ Compile-time validation of parser combinations
- ✅ Distinguishing between different predicate flags at compile time (`PredicateCmp`)
- ✅ Zero-cost abstraction (no runtime overhead)
- ❌ Runtime state tracking (use regular struct fields instead - predicates are compile-time only)
- ❌ Runtime validation (use `parse_with()` for runtime checks instead)
- ❌ Runtime repetition counts (use `RangedRepeats` trait for dynamic min/max instead)
- ❌ Simple lookahead (use `Except<T>`, `Expect<T>`, `LazyVec`, `LazyVecUntil`, `NonEmptyOption` instead)

---

## 6.7 Additional Utilities and Macros

### 6.7.1 Token Generation Macros

**`quote!` macro** - unsynn's own quote macro:
```rust
use unsynn::*;

// Basic usage - interpolate with #
let ast = <Cons<ConstInteger<1>, Plus, ConstInteger<2>>>::default();
let tokens = quote! { let a = #ast; };
assert_tokens_eq!(tokens, "let a = 1+2;");

// #{...} blocks - must return IntoIterator whose items implement ToTokens
// For single values, wrap in Some() because Option implements IntoIterator
let tokens = quote! {
    let a = #{Some(<Cons<ConstInteger<1>, Plus, ConstInteger<2>>>::default())};
};
assert_tokens_eq!(tokens, "let a = 1+2;");
```

**Note:** unsynn's quote! uses `#{...}` blocks, not quote crate's `#(...)` repetition syntax. There is no `#(...)` repetition support yet. DO NOT add quote crate as dependency.

**Format macros:**
```rust
use unsynn::*;

// Generate Ident from format string
let name = "value";
let id = format_ident!("get_{}", name);  // Produces: get_value

// Generate CachedIdent (cached string access)
let cached = format_cached_ident!("item_{}", 42);  // Produces: item_42

// Generate Literal from format (no quotes added)
let num = format_literal!("{}", 42);  // Produces: 42

// Generate LiteralString (quotes added automatically)
let s = format_literal_string!("hello {}", "world");  // Produces: "hello world"
```

### 6.7.2 Transform Types

**Stream manipulation:**
```rust
use unsynn::*;

// Skip<T> - parse but don't store (ToTokens is no-op)
Skip<Comma>  // Consumes comma without storing, emits nothing

// Discard<T> - parse and remove from stream (ToTokens panics!)
Discard<Comment>  // Removes matching token, cannot be serialized back

// Insert<T> - inject tokens without parsing
Insert<Semicolon>  // Injects semicolon into output

// Swap<A, B> - swap order of two entities
Swap<Type, Ident>  // Reverses parse order in output
```

**Critical difference:** `Skip<T>` has a no-op `ToTokens` (emits nothing), while `Discard<T>` **panics** on `ToTokens` because the token was removed from the stream and cannot be reconstructed.

**Type conversions:**
```rust
use unsynn::*;

// IntoIdent<T> - concat all elements into single identifier
IntoIdent<Cons<Ident, Ident>>  // Combines idents

// IntoLiteralString<T> - create literal string from tokens
IntoLiteralString<TokenStream>

// IntoTokenStream<T> - keep as opaque TokenStream
IntoTokenStream<Expression>  // Stores unparsed for later
```

**Additional transform types:**
```rust
use unsynn::*;

// AsDefault<T> - Parse T, replace with T::default() (zero-sized)
// Useful for counting occurrences without allocating storage
let parsed: Vec<AsDefault<Dot>> = tokens.parse().unwrap();  // Count dots

// OrDefault<T, D = T> - Parse T or insert D::default() on failure
type OptionalOp = OrDefault<Plus, Question>;  // Parse +, or insert ? if missing

// Replace<T, U> - Parse and skip T, insert U::default() instead
type SwapComma = Replace<Comma, Semicolon>;  // Parse comma, emit semicolon

// TokenStreamUntil<T> - Parse TokenStream until T (T must be present, not consumed)
type UntilSemi = TokenStreamUntil<Semicolon>;  // Captures tokens before semicolon
```

### 6.7.3 Const Literals

**ConstInteger<V>** - Constant integer that must match value V:
```rust
use unsynn::*;

// Only matches literal "42"
type FortyTwo = ConstInteger<42>;

// Has Default - produces "42" when default constructed
let forty_two = FortyTwo::default();
```

**ConstCharacter<V>** - Constant character that must match value V:
```rust
use unsynn::*;

// Only matches literal 'x'
type CharX = ConstCharacter<'x'>;
```

### 6.7.4 Special Parsers

**EndOfStream** - Matches only when no tokens remain:
```rust
use unsynn::*;

unsynn! {
    struct Complete {
        pub content: Expression,
        pub eof: EndOfStream,  // Ensures entire stream consumed
    }
}
```

**NonEmptyTokenStream** - TokenStream that requires tokens:
```rust
use unsynn::*;

// Unlike TokenStream, fails if empty
type RequiredContent = NonEmptyTokenStream;
```

**Invalid** - Always fails (useful as generic default):
```rust
use unsynn::*;

// Used internally when Either has fewer than 4 alternatives
Either<A, B, Invalid, Invalid>
```

**Cached<T>** - Caches string representation:
```rust
use unsynn::*;

// Cached versions for O(1) string access:
CachedIdent        // Ident with cached string
CachedLiteral      // Literal with cached string
CachedPunct        // Punct with cached char
CachedTokenTree    // TokenTree with cached string
CachedGroup        // Group with cached delimiters
```

**NoneGroup** - Matches group with no delimiter (for proc_macro2 compatibility).

### 6.7.5 Grammar Debugging

**StderrLog<T, N = 5>** - Debug wrapper that prints parse info to stderr:

```rust
use unsynn::*;

// Requires feature: debug_grammar (disabled by default)
// Cargo.toml: unsynn = { version = "...", features = ["debug_grammar"] }

// Wrap any parser to see what's being parsed
let mut tokens = "fn foo(bar: i32) -> Result<(), Error>".to_token_iter();

// With debug_grammar enabled, prints:
// Type: proc_macro2::Ident
// Source: fn foo(bar : i32) -> Result …
let ident: StderrLog<Ident, 10> = tokens.parse().unwrap();

// Note: When feature disabled, StderrLog is PhantomData - doesn't parse T or consume tokens
```

**Key characteristics:**
- **True zero-cost when disabled**: doesn't parse `T`, doesn't consume tokens, completely erased
- When enabled: shows typename and literal token representation
- `N` parameter controls token count (default: 5)
- Recurses into groups automatically
- Output truncated with ` …` if more tokens exist

**Usage pattern:**
```rust
// Add wrappers at key points during development
type DebugExpr = StderrLog<Expression, 20>;

// Leave them in production code - they're erased when feature is disabled
// No need to remove after debugging

// Enable only during debugging:
// cargo test --features debug_grammar
// cargo build --features debug_grammar
```

**Best for:** Tracking parser flow, identifying where parsing fails, understanding token sequences at specific grammar points.

**Important:** When `debug_grammar` is disabled, `StderrLog<T>` becomes `PhantomData<T>` and is a complete no-op in the parser chain - it doesn't call `T::parser()` and doesn't consume any tokens. This is a true zero-cost abstraction.

---

## 7. TESTING - RECOMMENDED PATTERNS

### 7.1 Testing Parsers

**Testing strategies:**

All parsers should be tested with both valid and invalid inputs. Use `assert_tokens_eq!` for comparing tokens:

```rust
use unsynn::*;

#[test]
fn test_my_type() {
    let mut tokens = "foo, bar".to_token_iter();
    let parsed: MyType = tokens.parse().unwrap();
    // Use assert_tokens_eq! for token comparison (handles roundtripping)
    assert_tokens_eq!(parsed.0, "foo");
    assert_tokens_eq!(parsed.2, "bar");
}

#[test]
fn test_invalid_input() {
    let mut tokens = "foo bar".to_token_iter();
    assert!(MyType::parse(&mut tokens).is_err());
}
```

**Critical:** Always test:
- Valid inputs (positive tests)
- Invalid inputs (negative tests)
- Edge cases (empty, maximum values, etc.)

### 7.2 assert_tokens_eq! Macro

**Compare token output:**

The `assert_tokens_eq!` macro is **required** for testing token equality because it handles the roundtripping through `tokens_to_string()` correctly:

```rust
use unsynn::*;

let ast = MyType::parse(&mut input)?;
assert_tokens_eq!(ast, "expected output");

// Whitespace doesn't matter - both sides roundtrip through tokens_to_string()
assert_tokens_eq!(ast, " 1  +  2 ");

// Comments are stripped
assert_tokens_eq!(ast, "1 + 2 // comment");
```

**Requires `proc_macro2` feature** (parses expected string into tokens).

**Critical:** Do NOT use raw string comparison (e.g., `assert_eq!(ast.to_string(), "foo")`) - this is unreliable and ambiguous because:
- String representation doesn't preserve token boundaries
- Spacing is arbitrary and inconsistent
- Operator tokenization (Joint vs Alone) is lost
- Comments and whitespace handling differs

Always use `assert_tokens_eq!` for token comparisons.

**Critical behavior:** `assert_tokens_eq!(a, b)` internally does:
```rust
a.tokens_to_string() == b.tokens_to_string()
```

**Both sides get roundtripped**, but they may not produce identical strings because:
- Tokenizing `"a-=-b"` preserves `Joint` spacing from compound operators
- Parsed operators always emit with `Alone` spacing (see Section 1.6)

```rust
// These will NOT match because spacing differs:
assert_tokens_eq!(expr, "a-=-b");      // ✗ Tokenized: "a -=- b" (Joint preserved)
                                        //   vs Parsed: "a -= - b" (all Alone)

// ✓ Correct - match the Alone spacing that parsed operators emit:
assert_tokens_eq!(expr, "a -= - b");
```

**When using `tokens_to_string()` directly** (not in `assert_tokens_eq!`), you must manually match the disambiguation spacing:
```rust
// ✓ Correct - matches disambiguation spacing that parsed operators emit
assert_eq!(expr.tokens_to_string(), "a -= - b");

// ✗ Wrong - doesn't match operator emission spacing
assert_eq!(expr.tokens_to_string(), "a-=-b");  // Will fail!
```

---

## 8. CONVENTIONS - HUMAN MAINTAINABILITY

### 8.1 Naming Rules

**Keywords:**
```rust
// Keyword types: Kw prefix
KwFn, KwConst, KwStatic, KwLet, KwMut

// Keyword fields: avoid clashing with Rust keywords
pub struct ReturnExpr {
    pub return_kw: KwReturn,     // ✅ GOOD - suffix avoids clash
    pub return_token: KwReturn,  // ✅ GOOD - descriptive suffix
    pub ret: KwReturn,           // ✅ GOOD - abbreviation avoids clash
    pub return: KwReturn,        // ❌ WRONG - clashes with Rust keyword
    pub _return: KwReturn,       // ❌ WRONG - leading underscore means "unused"
}
```

**Operators:**
```rust
// Descriptive names (use predefined when possible)
Plus, Minus, Star, Slash, FatArrow, PathSep
```

**Types:**
```rust
// Follow Rust Reference naming when possible
TypePath, TypeNoBounds, ImplTraitType
```

**Groups:**
```rust
// Suffix with Group or GroupContaining
ParenthesisGroupContaining<T>
BraceGroup
```

## 9. PERFORMANCE AND PITFALLS

**⚠️ CRITICAL SECTION - READ BEFORE DEBUGGING:**

If you encounter:
- Infinite loops → Check 9.1 "Vec with Fallback"
- Wrong parsing results → Check 9.1 "Order in Either"  
- Performance issues → Check 9.2 "Optimization"
- Type recursion errors → Check 9.1 "Box for Recursion"

**🔄 Re-read this section** when troubleshooting unexpected behavior.

### 9.1 Common Pitfalls

**Order in Either:**
```rust
// ❌ WRONG - General types first
Either<TokenTree, Ident, Literal>  // TokenTree matches everything

// ✅ CORRECT - Specific types first
Either<Ident, Literal, TokenTree>
```

**Vec with Fallback:**
```rust
// ❌ WRONG - Infinite loop if Statement has fallback
pub struct Block {
    statements: Vec<Statement>,
}

// ✅ CORRECT - LazyVec checks terminator first
pub struct Block {
    statements: LazyVec<Statement, BraceClose>,
}
```

**Box for Recursion:**
```rust
// Box required to break recursive type cycles
pub enum Expr {
    Binary(Box<BinaryExpr>),  // Box breaks infinite size recursion
    Grouped(ParenthesisGroupContaining<Box<Expr>>),
}
```

### 9.2 Performance Rules

**Parser complexity:**
- Worst-case: Exponential (due to backtracking)
- Mitigation: Structure disjunctions to fail early

**Optimization:**
```rust
// ❌ SLOW: Long shared prefix in alternatives
Either<Cons<Long, A>, Cons<Long, B>>

// ✅ FAST: Factor out common prefix
Cons<Long, Either<A, B>>
```

**Parser vs Parse:**
- `parser()`: 30%+ faster, non-transactional
- `parse()`: Transactional, safe
- Use `parser()` only for conjunctive parsing

---

## 10. QUICK REFERENCE

### 10.1 Combinator Lookup

| Pattern | Usage | Example |
|---------|-------|---------|
| `Cons<A, B>` | Sequential | `Cons<Ident, Comma, Type>` |
| `Either<A, B>` | Alternative | `Either<KwConst, KwMut>` |
| `Option<T>` | Optional | `Option<Lifetime>` |
| `NonEmptyOption<T>` | Optional (non-empty) | `NonEmptyOption<Expr>` |
| `Vec<T>` | Repeated (if T fails) | `Vec<Attribute>` |
| `LazyVec<T, S>` | Repeated (check term) | `LazyVec<Stmt, BraceClose>` |
| `LazyVecUntil<T, S>` | Repeated (lookahead) | `LazyVecUntil<Item, Semicolon>` |
| `Box<T>` | Recursive/Large | `Box<TypeNoBounds>` |
| `Rc<T>` | Shared recursion | `Rc<Expression>` |
| `NonParseable` | Unimplemented placeholder | Development only |
| `DynNode<T>` | Runtime replacement | Advanced transformations |
| `HiddenState<T>` | Non-syntactic data | Metadata storage |
| `Nothing` | Empty match | Optional delimiter |
| `Skip<T>` | Parse but don't store | `Skip<Comma>` |
| `Discard<T>` | Remove from stream | `Discard<Comment>` |
| `Insert<T>` | Inject tokens | `Insert<Semicolon>` |
| `Swap<A, B>` | Reverse order | `Swap<Type, Ident>` |
| `IntoIdent<T>` | Concat to ident | `IntoIdent<Path>` |
| `IntoLiteralString<T>` | Create literal string | `IntoLiteralString<Tokens>` |
| `IntoTokenStream<T>` | Opaque stream | `IntoTokenStream<Expr>` |
| `EndOfStream` | Match stream end | Ensure complete parse |
| `NonEmptyTokenStream` | TokenStream with tokens | Require non-empty |
| `Invalid` | Always fails | Generic default |
| `ConstInteger<V>` | Match specific int | `ConstInteger<42>` |
| `ConstCharacter<V>` | Match specific char | `ConstCharacter<'x'>` |

### 10.2 DelimitedVec Aliases

| Type | Definition | Use Case |
|------|------------|----------|
| `Any<T, D>` | `0..∞` | Optional repeated |
| `Many<T, D>` | `1..∞` | Required repeated |
| `Optional<T, D>` | `0..1` | Single optional |
| `Exactly<T, N>` | `N..N` | Fixed count |
| `CommaDelimitedVec<T>` | `0..∞, ','` | Comma lists |
| `SemicolonDelimitedVec<T>` | `0..∞, ';'` | Semicolon lists |
| `PathSepDelimitedVec<T>` | `1..∞, '::'` | Path segments |

### 10.3 Expression Types

| Type | Associativity | Example |
|------|---------------|---------|
| `InfixExpr<O, Op, MAX>` | Non-assoc | `a OP b` |
| `NonAssocExpr<O, Op>` | Non-assoc | `a == b` |
| `LeftAssocExpr<O, Op>` | Left | `(a + b) + c` |
| `RightAssocExpr<O, Op>` | Right | `a = (b = c)` |
| `PrefixExpr<Op, O>` | Right | `-(-x)` |
| `PostfixExpr<O, Op>` | Left | `(x?)!` |

### 10.4 Lookahead

| Type | Behavior | Example |
|------|----------|---------|
| `Except<T>` | Fail if T matches | `Cons<Except<PlusEq>, Plus>` |
| `Expect<T>` | Succeed if T matches | `Cons<Ident, Expect<Assign>>` |

### 10.5 Predicates

| Type | Behavior |
|------|----------|
| `Enable` | Always succeeds |
| `Disable` | Always fails |
| `TokensRemain` | Succeeds if tokens remain |
| `AllOf<A, B>` | Logical AND (2-4 operands) |
| `AnyOf<A, B>` | Logical OR (2-4 operands) |
| `OneOf<A, B>` | Logical XOR (2-4 operands) |
| `Not<P>` | Logical NOT |

### 10.6 Macros

| Macro | Purpose | Example |
|-------|---------|---------|
| `unsynn!` | Define parsers | `unsynn! { struct Foo { ... } }` |
| `operator!` | Define operators | `operator! { Plus = "+"; }` |
| `keyword!` | Define keywords | `keyword! { Fn = "fn"; }` |
| `quote!` | Create TokenStream | `quote! { fn foo() {} }` |
| `format_ident!` | Format Ident | `format_ident!("get_{}", name)` |
| `format_cached_ident!` | Format CachedIdent | `format_cached_ident!("item_{}", n)` |
| `format_literal!` | Format Literal | `format_literal!("{}", 42)` |
| `format_literal_string!` | Format LiteralString | `format_literal_string!("hello")` |
| `assert_tokens_eq!` | Test token equality | `assert_tokens_eq!(a, b)` |

### 10.7 Cached Types (O(1) String Access)

| Type | Base Type | Purpose |
|------|-----------|---------|
| `CachedIdent` | `Ident` | Cached identifier string |
| `CachedLiteral` | `Literal` | Cached literal string |
| `CachedLiteralInteger` | `LiteralInteger` | Cached integer string |
| `CachedLiteralString` | `LiteralString` | Cached string literal |
| `CachedPunct` | `Punct` | Cached punct char |
| `CachedTokenTree` | `TokenTree` | Cached token string |
| `CachedGroup` | `Group` | Cached group delimiters |

**Use Cached types** for custom validation to avoid repeated string allocations (see Section 5.1).

---

## ESSENTIAL RULES SUMMARY

**📌 BOOKMARK THIS SECTION** - Quick reference for the most critical rules. When in doubt, re-read the full section referenced.

**⚠️ BEFORE IMPLEMENTING, VERIFY YOU UNDERSTAND:**
- Rule 1: Why manual `Parser` implementations are like 'unsafe' code
- Rule 3: When to use `Cons<Except<LongerOp>, ShorterOp>` (operator precedence)
- Rule 6: Why `Vec<T>` can cause infinite loops with fallback types
- Rule 7: Why order matters in `Either` and enums

**🔄 IF UNSURE ABOUT ANY RULE:** Re-read `/home/ct/src/unsynn/HOW-TO-USE-UNSYNN-BY-AI.md` - don't guess!

---

1. **ALWAYS use `unsynn!` macro over manual implementations** - The `unsynn!` macro is the safe, correct way to define parsers. Manual `Parser` and `ToTokens` implementations are the equivalent of 'unsafe' code in grammars - they can violate invariants (mixing `parse()` vs `parser()`, incorrect token preservation, broken transactions). Only implement these traits manually when absolutely necessary (custom validation, transformation) and use extreme care.

2. **Always use predefined operators** from `unsynn::operator::names::*`
3. **Use `Cons<Except<LongerOp>, ShorterOp>`** when shorter operator has **higher precedence** than longer operator sharing its prefix (critical in precedence hierarchies)
4. **Never nest expression types** - they handle multiplicity internally
5. **Always use `Box`** for recursive expressions in primary expression enum
6. **Never use `Vec<T>`** if T has always-succeeding fallback - use `LazyVec`
7. **Order matters in `Either` and enums** - specific types before general
8. **Always test parsers** - both positive and negative cases
9. **Use `CachedLiteral`** for custom literal validation
10. **Avoid Rust keyword clashes in field names** - use suffixes (`_kw`, `_token`) or descriptive names when field types are keywords
11. **Preserve original tokens in `ToTokens`** by default - only transform intentionally (reconstruction loses span information for error reporting)
12. **Generated code must be maintainable** - use named structs, clear names, documentation

**Critical distinction:** The `unsynn!` macro handles all parsing complexity correctly. Manual implementations should be treated with the same caution as `unsafe` code - only when necessary, with thorough testing and documentation.

---

## USAGE WORKFLOW

**🔄 FINAL REMINDER - Read this document when:**
- Starting new unsynn parser work
- Encountering unexpected parsing behavior
- Implementing operator precedence or expressions
- Debugging infinite loops or type errors
- Unsure about any unsynn pattern or behavior

---

1. **Define grammar** using `unsynn!` macro
2. **Add tests** for each type
3. **Run tests** to verify parsing works
4. **Document** with Rust Reference links
5. **Commit** working increments

**Iterate:** Build grammars incrementally, testing each addition.

---

*This document covers unsynn v0.2.0+. For changelog and version-specific details, see the unsynn documentation.*
