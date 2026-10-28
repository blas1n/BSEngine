unsynn (from german 'unsinn' for nonsense) is a minimalist rust parser library. It achieves
this by leaving out the actual grammar implementations which are implemented in distinct
crates. Still it comes with batteries included, there are parsers, combinators and
transformers to solve most parsing tasks.

In exchange it offers simple composeable Parsers and declarative Parser construction. Grammars
will be implemented in their own crates (see unsynn-rust).

It is primarily intended use is when one wants to create proc macros for rust that define their
own grammar or need only sparse rust parsers.

Other uses can be building parsers for gramars outside a rust/proc-macro context. Unsynn can
parse any `&str` data (The tokenizer step relies on `proc_macro2`).


# Examples

## Creating and Parsing Custom Types

The [`unsynn!{}`](crate::unsynn!) macro generates the [`Parser`] and [`ToTokens`] implementations for your types.

Notice that unsynn implements [`Parser`] and [`ToTokens`] for many standard rust types. Like
we use `u32` in this example.

```rust
# use unsynn::*;
let mut token_iter = "foo ( 1, 2, 3 )".to_token_iter();

unsynn!{
    struct IdentThenParenthesisedNumbers {
        ident: Ident,
        numbers: ParenthesisGroupContaining::<CommaDelimitedVec<u32>>,
    }
}

// iter.parse() is from the IParse trait
let ast: IdentThenParenthesisedNumbers = token_iter.parse().unwrap();

assert_tokens_eq!(ast, "foo(1,2,3)");
```

In case the automatiically generated [`Parser`] and [`ToTokens`] implementations are not
sufficient the macro supports custom parsing and token emission through [`parse_with`](crate::unsynn#custom-parsing-with-parse_with) and 
[`to_tokens`](crate::unsynn#custom-token-emission-with-to_tokens) clauses:

- **[`from Type:`](crate::unsynn#custom-parsing-with-parse_with)**: Parse from a different type before transformation (requires [`parse_with`](crate::unsynn#custom-parsing-with-parse_with))
- **[`parse_with`](crate::unsynn#custom-parsing-with-parse_with)**: Transform or validate values during parsing (used alone for validation, with `from` for transformation)
- **[`to_tokens`](crate::unsynn#custom-token-emission-with-to_tokens)**: Customize how types are emitted back to tokens (independent)

The [`parse_with`](crate::unsynn#custom-parsing-with-parse_with) and [`to_tokens`](crate::unsynn#custom-token-emission-with-to_tokens) clauses are independent and optional. The [`from`](crate::unsynn#custom-parsing-with-parse_with) clause must be used together with [`parse_with`](crate::unsynn#custom-parsing-with-parse_with). When more control is needed, the implementations can also be written manually.


### Custom Parsing and `ToTokens`

Example of custom parsing and token emission - parse emoji as bools, emit as emoji:

```rust
# use unsynn::*;
unsynn! {
    struct ThumbVote(bool) from LiteralCharacter:
    parse_with |value, _tokens| {
        Ok(Self(value.value() == '👍'))
    }
    to_tokens |s, tokens| {
        Literal::character(if s.0 {'👍'} else {'👎'}).to_tokens(tokens);
    };
}
```

See the [COOKBOOK](crate#cookbook) for more details on [`parse_with`](crate::unsynn#custom-parsing-with-parse_with) 
and [`to_tokens`](crate::unsynn#custom-token-emission-with-to_tokens) clauses.


## Using Composition

Composition can be used without defining new datatypes. This is useful for simple parsers or
when one wants to parse things on the fly which are desconstructed immediately. See the 
[`combinator`](crate::combinator) module for more composition types.

```rust
# use unsynn::*;
// We parse this below
let mut token_iter = "foo ( 1, 2, 3 )".to_token_iter();

// Type::parse() is from the Parse trait
let ast =
    Cons::<Ident, ParenthesisGroupContaining::<CommaDelimitedVec<u32>>>
        ::parse(&mut token_iter).unwrap();

assert_tokens_eq!(ast, "foo ( 1, 2, 3 )");
```

## Custom Operators and Keywords

Keywords and operators can be defined within the [`unsynn!{}`](crate::unsynn!) macro:

```rust
# use unsynn::*;
unsynn! {
    keyword Calc = "CALC";
    operator Add = "+";
    operator Substract = "-";
    operator Multiply = "*";
    operator Divide = "/";
}

// Build expression parser with proper precedence
type Expression = Cons<Calc, AdditiveExpr, Semicolon>;
type AdditiveExpr = LeftAssocExpr<MultiplicativeExpr, Either<Add, Substract>>;
type MultiplicativeExpr = LeftAssocExpr<LiteralInteger, Either<Multiply, Divide>>;

let ast = "CALC 2*3+4*5 ;".to_token_iter()
    .parse::<Expression>().expect("syntax error");
```

Keywords and operators can also be defined using standalone [`keyword!{}`](crate::keyword!) and [`operator!{}`](crate::operator!) macros.
See the [operator names reference](crate::operator::names) for predefined operators.

For more details on building expression parsers with proper precedence and associativity,
see the [`expressions`](crate::expressions) module documentation.


# Feature Flags

* `proc_macro2`:  
  Controls whether unsynn uses the `proc_macro2` crate or the built-in `proc_macro` crate for
  token handling. This is **enabled by default**. When enabled, unsynn can parse from strings
  (via `&str::to_token_iter()`), convert tokens to strings (via [`tokens_to_string()`](crate::ToTokens::tokens_to_string())), and be
  used in any context (tests, examples, etc.). When disabled, unsynn uses only the built-in
  `proc_macro` crate and can **only be used from proc-macro crates** (with `proc-macro = true`
  in Cargo.toml). This creates leaner proc macros without the `proc_macro2` dependency.

  **APIs disabled without `proc_macro2`:**
  - String parsing: [`ToTokens`] for &str/String (parsing strings into tokens)
  - Format macros: [`format_ident!()`](macro.format_ident), [`format_literal!()`](macro.format_literal), [`format_literal_string!()`](macro.format_literal_string)
  - Transform types: [`IntoIdent<T>`](struct.IntoIdent) (requires string parsing for validation)
  - Test helper: [`assert_tokens_eq!()`](macro.assert_tokens_eq) (requires string parsing)
  - String-based constructors: [`Cached::new()`](struct.Cached::new), [`Cached::from_string()`](struct.Cached::from_string) (require string parsing)

  **APIs that remain available:**
  - Token to string conversion: [`tokens_to_string()`](crate::ToTokens::tokens_to_string()), [`to_token_iter()`](crate::ToTokens::to_token_iter()), [`into_token_iter()`](crate::ToTokens::into_token_iter())
  - All parsing functionality (works with [`TokenStream`] from proc macro input)
  - All [`ToTokens`] implementations (except for &str/String)
  - Transform types: [`IntoLiteralString<T>`](crate::IntoLiteralString) (uses `Literal::string()` constructor)
  - Type: [`Cached<T>`](crate::Cached) (but not the string-based constructors)

* `hash_keywords`:  
  This enables hash tables for larger keyword groups.  This is **enabled by default** since it
  guarantees fast lookup in all use-cases and the extra dependency it introduces is very
  small. Nevertheless this feature can be disabled when keyword grouping is not or rarely used
  to remove the dependency on `rust_hash`. Keyword lookups then fall back to a binary search
  implementation. Note that the implementation already optimizes the cases where only one or
  only a few keywords are in a group.

* `criterion`:  
  Enables the `criterion` benchmarking framework for performance benchmarks. This is **disabled
  by default** to keep the dependency tree light. Use `cargo bench --features criterion` to run
  the criterion benchmarks. Without this feature, only non-criterion benchmarks will run.

* `docgen`:  
  The [`unsynn!{}`](crate::unsynn!), [`keyword!{}`](crate::keyword!) and [`operator!{}`](crate::operator!) macros will automatically generate
  some additional docs. This is **enabled by default**.

* `nonparsable`:  
  This enables the implementation of [`Parser`] and [`ToTokens`] for the [`NonParseable`](crate::NonParseable)
  type. When not set, any use of it will result in a compile error. One may disable this for
  release builds to prevent any `NonParsable` left used in the code, thus checking for
  completeness (`NonParseable` is used for marking unimplemented types) and avoiding potential
  panics at runtime. This is **enabled by default**, consider to disable it in release builds.

* `debug_grammar`:  
  Enables the [`StderrLog<T, N>`](crate::debug::StderrLog) debug type that prints type information
  and token sequences to stderr during parsing. This is useful for debugging complex grammars
  and understanding parser behavior. When disabled (the default), `StderrLog` becomes is
  zero-cost/no-op. This is **disabled by default**. Enable it during development with
  `cargo test --features debug_grammar` or `cargo build --features debug_grammar`. See the
  [COOKBOOK](crate#grammar-debugging) for usage examples.

* `trait_methods_track_caller`:  
  Adds `#[track_caller]` to [`Parse`], [`Parser`], [`IParse`] and [`ToTokens`] trait methods. The idea
  here is to make unsynn more transparent in case of a panic and point closer to the users
  code that caused the problem. This has a neglible performance impact and is a experimental
  feature. When it has some bad side effects, please report it. This is **enabled by
  default**.

* `extra_asserts`:  
  Enables expensive runtime sanity checks for unsynn internals. Enabled while developing
  unsynn. This adds diagnostics to datastructures and makes unsynn slower and bigger. Should
  be disabled when unsynn is used by another crate. Currently enabled by default which may
  (and eventually will) be disabled for stable releases.

* `extra_tests`:  
  Enable expensive tests that check semantics that should taken 'for granted', will make the
  testsuite slower. Even without these tests enabled we aim for full (cargo-mutants) test
  coverage with `extra_asserts` enabled. This is **disabled by default**. Many of these tests
  are kept from development to assert correct semantics but are covered elsewhere.
