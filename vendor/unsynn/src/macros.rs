//! This module contains the big macros and helper functions to define and parse custom types.
//!
//! **Note**: When the `proc_macro2` feature is disabled, format macros (`format_ident!`,
//! `format_literal!`, `format_literal_string!`) are not available.

#[cfg(doc)]
use crate::*;

/// This macro supports the definition of enums, tuple structs and normal structs and
/// generates [`Parser`] and [`ToTokens`] implementations for them. It will derive `Debug`.
/// Generics/Lifetimes are not supported on the primary type.  Note: eventually a derive macro
/// for `Parser` and `ToTokens` will become supported by a 'unsynn-derive' crate to give finer
/// control over the expansion. `#[derive(Copy, Clone)]` have to be manually defined. Keyword
/// and operator definitions can also be defined, they delegate to the `keyword!` and
/// `operator!` macro described below. All entities can be prefixed by `pub` to make them
/// public. Type aliases, function definitions, macros and use statements are passed through. This
/// makes thing easier readable when you define larger unsynn macro blocks.
///
/// The macro definition above is simplified for readability `struct`, `enum` and `type`
/// definitions can include most of the things normal rust definitions can do. This also
/// includes definitions of members of structs and enums:
///
/// * Any number of attributes (`#[...]`), including documentation comments. Note that the
///   unsynn macros have limited support for automatically generation documentation. This
///   auto-generated documentation is appended after the user supplied docs.
/// * structs, enums, types and members can exported with the usual `pub` declarations.
/// * struct, enum, and type definitions support generics and lifetime parameters. These can include
///   trait bounds (simple identifiers, qualified paths like `std::fmt::Debug`, and lifetime bounds
///   like `'static`) and defaults. Multiple bounds can be combined with `+`. Trait bounds can be
///   defined with where-clauses as well. Lifetime parameters (e.g., `<'a>`, `<'a, T>`) are fully
///   supported for structs. Enums and tuple structs support lifetime parameters except when combined
///   with inline `impl` blocks. HRTB (Higher-Ranked Trait Bounds) are not yet supported.
///   See the [COOKBOOK](https://docs.rs/unsynn/latest/unsynn/#generic-types-with-trait-bounds)
///   for examples.
///
/// Common for enum and struct variants is that entries are tried in order. Disjunctive for
/// enums and conjunctive in structures. This makes the order important, e.g. for enums, in
/// case some entries are subsets of others.
///
/// Enum variants without any data will never be parsed and will not generate any tokens. For
/// *parsing* a enum that is optional one can add a variant like `None(Nothing)` at the end
/// (at the end is important, because Nothing always matches).
///
/// The `unsynn!{}` macro is still a declarative macro and is somewhat limited in what it can
/// do. We extend it as needed and provide some convenient rust syntax extensions, but some
/// standard rust syntax is just hard to impossible to support.
///
///
/// ## Example
///
/// ```
/// # use unsynn::*;
/// // Define some types
/// unsynn!{
///     keyword MyKeyword = "keyword";
///     // all items can be declared pub/pub(..) etc
///     pub(crate) operator MyOperator = "+++";
///
///     enum MyEnum {
///         /// Entries can have attributes/doc comments
///         Ident(Ident),
/// #       TupleWithDocs(
/// #           /// fooo
/// #           Ident,
/// #           /// bar
/// #           Optional<Ident>
/// #       ),
///         Braced(BraceGroup),
///         Text(LiteralString),
///         Number(LiteralInteger),
/// #       TrailingComma(LiteralInteger, Ident,),
///         Struct{
///             keyword: MyKeyword,
///             id: Ident,
///         },
///         // finally if nothing of the above matched, this will match.
///         None(Nothing),
///         // won't be parsed/matched at all
///         Empty,
///     }
///
///     // With generics - qualified paths work, no need to import
///     struct MyStruct<T: std::fmt::Debug = i32> {
///         text: LiteralString,
///         number: T,
///     }
///
///     // Multiple bounds with lifetime bound
///     struct GenericContainer<T: Clone + std::fmt::Display + 'static> {
///         value: T,
///     }
///
///     struct MyTupleStruct(Ident, LiteralString);
///
///     // Lifetime parameters are also supported:
///     //struct WithLifetime<'a> { /* ... */ }
///     //struct MixedParams<'a, T> { /* ... */ }
///
///     // type definitions are pass-through.
///     pub type Alias = MyStruct<LiteralInteger>;
///
///     // functions are pass though too
///     fn testfn() -> bool { true }
/// }
///
/// // Create an iterator over the things we want to parse
/// let mut token_iter = r#"
///     // some enum variants
///     ident { within brace } "literal string" 1234 ()
///     // MyStruct fields
///     "literal string" 1234
///     // MyTupleStruct fields
///     ident "literal string"
///     // MyKeyword and MyOperator
///     keyword +++
/// "#.to_token_iter();
///
/// // Use the defined types
/// let MyEnum::Ident(myenum_ident) = MyEnum::parse(&mut token_iter).unwrap() else { panic!()};
/// # assert_tokens_eq!(myenum_ident, "ident");
/// let MyEnum::Braced(myenum_braced) = MyEnum::parse(&mut token_iter).unwrap() else { panic!()};
/// # assert_tokens_eq!(myenum_braced, "{within brace}");
/// let MyEnum::Text(myenum_text) = MyEnum::parse(&mut token_iter).unwrap() else { panic!()};
/// # assert_tokens_eq!(myenum_text, "\"literal string\"");
/// let MyEnum::Number(myenum_number) = MyEnum::parse(&mut token_iter).unwrap() else { panic!()};
/// # assert_tokens_eq!(myenum_number, "1234");
/// // the () will not be consumed by the MyEnum but match None(Nothing)
/// let myenum_nothing = MyEnum::parse(&mut token_iter).unwrap();
/// # assert_tokens_eq!(myenum_nothing, "");
/// // consume the ()
/// <ParenthesisGroup>::parse(&mut token_iter).unwrap();
///
/// let my_struct =  Alias::parse(&mut token_iter).unwrap();
/// let my_tuple_struct =  MyTupleStruct::parse(&mut token_iter).unwrap();
/// let my_keyword =  MyKeyword::parse(&mut token_iter).unwrap();
/// let my_operator =  MyOperator::parse(&mut token_iter).unwrap();
/// # assert!(testfn());
/// ```
///
///
/// # Traits within the `unsynn!` macro ***(EXPERIMENTAL)***
///
/// The `unsynn!` macro has limited but ergonomic support for defining and implementing
/// traits. The main purpose for this is to be able to define simple markers and accessors
/// this helps with compile-time validation of grammars.
///
/// Trait definitions are passed though. There are simplifications that they are allowed to
/// end with a `;` instead a `{}`. This signifies simple marker traits that have no
/// methods. The form `trait {Name; ...}` is supported to define a set of simple markers
/// in one go. This syntax not support traits with methods.
///
/// Trait implementations are either passed through or are a `impl {}` block directly
/// following an ADT definition. In the later case most boilerplate is left out and will be
/// auto generated by the macro.
///
/// ## Generic Types and Trait Bounds
///
/// Generic type parameters and lifetime parameters are fully supported:
/// - **Lifetime parameters**: `<'a>`, `<'a, 'b>`, or mixed `<'a, T>`
/// - **Simple trait names**: `T: Clone`
/// - **Qualified paths**: `T: std::fmt::Debug` (no need to import the trait)
/// - **Lifetime bounds**: `T: 'static` (can appear anywhere in the bound list)
/// - **Multiple bounds**: `T: Clone + std::fmt::Debug + 'static` (any combination)
/// - **Where clauses**: Support the same bound syntax as inline bounds
///
/// **Examples**:
/// ```ignore
/// struct WithLifetime<'a> { /* ... */ }
/// struct MultipleLifetimes<'a, 'b> { /* ... */ }
/// struct MixedParams<'a, T: Clone> { /* ... */ }
/// ```
///
/// **Limitations**:
/// - Generic type arguments in bounds (e.g., `T: Trait<U>`) are not supported
/// - HRTB (Higher-Ranked Trait Bounds like `for<'a>`) are not yet supported
/// - Lifetime parameters on tuple structs and enums with inline `impl` blocks are not supported
///   (use separate `impl` blocks outside the macro instead)
///
/// See the [COOKBOOK](https://docs.rs/unsynn/latest/unsynn/#generic-types-with-trait-bounds)
/// for detailed examples and patterns.
///
/// Trait support is very basic and experimental, it is unspecified what features are supported.
/// Try it out, we promise not to break working things (if possible). For trait definitions that
/// are not supported within the unsynn macro it is still possible and advised to define them
/// outside of the macro block.
///
///
/// ## Example
///
/// ```
/// # use unsynn::*;
/// unsynn!{
///     // Marker traits can be defined in a block
///     // with attributes and visibility at the whole block
///     #[doc = "This is a marker trait"] // applies to each item
///     pub trait {
///         TestMarker;
///         TestMarker2;
///     }
///     // or per item
///     trait {
///         pub MarkerPub;
///         MarkerPrivate;
///     }
///
///     // ending with a semicolon, supertraits are supported
///     trait TestMarker3: TestMarker + TestMarker2;
///     // or a brace
///     trait TestMarker4 {}
///
///     // Normal rust syntax for accessor methods
///     trait SimpleAccessor { fn get(&self) -> bool;}
///
///     // simplified trait impl block following the struct definition
///     pub struct SimpleStruct{
///         flag: bool
///     } impl {
///         #[doc = "impl attributes go here"]
///         TestMarker;
///         TestMarker2 {}
///         SimpleAccessor {fn get(&self) -> bool {self.flag}}
///     }
///
///     // Marker traits or traits that have blanket implementations can
///     // be implemented by a comma separated list
///     impl TestMarker for i32, u32, i64, u64;
///
///     // normal rust syntax, passthough trait impl
///     impl SimpleAccessor for bool {fn get(&self) -> bool {*self}}
/// }
///
/// // implementing traits outside of the macro works as usual
/// impl TestMarker4 for SimpleStruct {}
/// ```
///
///
/// # Extended Syntax Forms
///
/// The `unsynn!` macro extends standard Rust syntax with these conveniences:
///
/// ## Semicolon Terminators
///
/// Traits and tuple structs can end with `;` instead of `{}`:
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     trait MyMarker;  // Instead of: trait MyMarker {}
///     struct MyTuple(Ident);  // Semicolon instead of no punctuation
/// }
/// ```
///
/// ## Trait Blocks
///
/// Define multiple marker traits at once using `trait { Name1; Name2; }`:
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     trait {
///         MarkerA;
///         MarkerB;
///         MarkerC;
///     }
/// }
/// ```
///
/// ## Inline Impl Blocks
///
/// Apply traits directly after struct/enum definitions with `impl { Trait; }`:
/// ```rust
/// # use unsynn::*;
/// unsynn! {
/// #   trait { Marker; }
///     // Semicolon is optional before impl
///     struct MyStruct(Ident) impl { Marker; }
///     // Or with semicolon (both work)
///     struct OtherStruct(Ident); impl { Marker; }
/// }
/// ```
///
/// ## Multi-Type Impl
///
/// Implement traits for multiple types with `impl Trait for Type1, Type2, Type3;`:
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     trait MyTrait;
///     impl MyTrait for Ident, LiteralInteger, TokenTree;
/// }
/// ```
///
/// ## Custom Parsing with `parse_with`
///
/// Transform or validate parsed values using closure syntax. See [`Parse::parse_with()`] for the
/// underlying method this uses.
///
/// **Validation** (without `from`): Verify parsed value meets requirements
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     // Only accept positive integers
///     struct PositiveInt(LiteralInteger);
///     parse_with |this, tokens| {
///         if this.0.value() > 0 {
///             Ok(this)
///         } else {
///             Error::other(None, tokens, "must be positive".into())
///         }
///     };
/// }
/// ```
///
/// **Transformation** (with `from Type:`): Parse as one type, transform to another
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     // Parse integer as bool (0 = false, non-zero = true)
///     struct BoolInt(bool) from LiteralInteger:
///     parse_with |value, _tokens| {
///         Ok(Self(value.value() != 0))
///     };
/// }
/// ```
///
/// The closure receives:
/// - `value` - The parsed value (type after `from`, or `Self` without `from`)
/// - `tokens` - Reference to [`TokenIter`] for error reporting
///
/// Returns: `Result<Self>` where errors use [`Error`] type
///
/// ## Custom Token Emission with `to_tokens`
///
/// Customize how types are emitted back to tokens. See [`ToTokens::to_tokens()`] for the trait
/// method this implements.
///
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     // Emit booleans as custom keywords
///     struct BoolKeyword(bool);
///     to_tokens |s, tokens| {
///         if s.0 {
///             Ident::new("TRUE", Span::call_site()).to_tokens(tokens);
///         } else {
///             Ident::new("FALSE", Span::call_site()).to_tokens(tokens);
///         }
///     };
/// }
/// ```
///
/// The closure receives:
/// - `self` - Reference to the value being emitted
/// - `tokens` - Mutable reference to [`TokenStream`] to append to
///
/// ## Combining `parse_with` and `to_tokens`
///
/// Both clauses are independent and can be used together:
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     // Parse int as bool, emit bool as int
///     struct BoolInt(bool) from LiteralInteger:
///     parse_with |value, _tokens| { Ok(Self(value.value() != 0)) }
///     to_tokens |s, tokens| {
///         Literal::u64_unsuffixed(if s.0 {1} else {0}).to_tokens(tokens);
///     };
/// }
/// ```
///
/// **Clause Order**: For tuple structs: `struct Name(Fields) [from Type:] [parse_with ...] [to_tokens ...] [impl {...}];`
///
/// The `parse_with` and `to_tokens` clauses are independent and optional. The `from` clause requires `parse_with`. Impl blocks always come last.
///
/// See the [COOKBOOK](crate#custom-parsing-and-totokens-with-unsynn) for more examples and patterns.
///
///
/// ## Keywords and Operators
///
/// Define custom keywords and operators within the macro. See also the standalone [`keyword!`]
/// and [`operator!`] macros.
///
/// **Keywords**: Match specific identifier strings
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     keyword If = "if";
///     keyword While = "while";
///     keyword Function = "fn";
/// }
/// ```
///
/// **Operators**: Match specific punctuation sequences
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     operator Plus = "+";
///     operator Arrow = "->";
///     operator DoubleColon = "::";
/// }
/// ```
///
/// See [`operator::names`](crate::operator::names) for a comprehensive list of predefined operators.
///
/// # Parse Predicates
///
/// Parse predicates provide zero-cost compile-time control over parser behavior using type-level
/// constraints. The `unsynn!` macro supports two special forms for defining predicates:
///
/// - **`predicatetrait`**: Defines custom context traits that extend [`PredicateOp`], automatically
///   implementing them for universal predicates ([`Enable`], [`Disable`], [`TokensRemain`]) and
///   logical operators ([`AllOf`], [`AnyOf`], [`OneOf`], [`Not`]).
///
/// - **`predicateflag`**: Creates zero-sized newtype wrappers around base predicates that implement
///   custom traits, enabling type-safe context validation at compile time.
///
/// **Example**:
/// ```
/// # use unsynn::*;
/// unsynn! {
///     // Define context traits
///     predicatetrait ExpressionContext;
///     predicatetrait StatementContext;
///     
///     // Create context-specific predicates
///     predicateflag InExpression = Enable for ExpressionContext;
///     predicateflag InStatement = Disable for ExpressionContext;
///     
///     // Use predicates as type constraints
///     pub struct StructLiteral<P: ExpressionContext = InExpression> {
///         _guard: P,  // Zero-sized, no runtime cost
///         name: Ident,
///     }
/// }
/// ```
///
/// See the [COOKBOOK Parse Predicates section](https://docs.rs/unsynn/latest/unsynn/#parse-predicates)
/// and the [`predicates`] module for detailed examples and API reference.
#[cfg(doc)]
#[macro_export]
macro_rules! unsynn {
    (enum $name:ident { $($variant:ident),* }) => {};
    (struct $name:ident { $($member:ident: $parser:ty),* }) => {};
    (struct $name:ident ( $($parser:ty),*);) => {};
    (trait $name:ident;) => {};
    (trait $name:ident{}) => {};
    (impl { $($trait:ident $semicolon_or_block:tt)+ }) => {};
    (impl $trait:ident for $type:ty {$body:tt}) => {};
    (impl $trait:ident for $($type:ty),+;) => {};
    (fn $function:ident($params:tt) {$body:tt}) => {};
    (use $($path:path)+ $(as $alias:ident)?) => {};
    (keyword $name:ident = keyword_or_group;) => {};
    (keyword $name:ident != keyword_or_group;) => {};
    (operator $name:ident = "punct";) => {};
    (predicatetrait $name:ident $(: $super:ident $(+ $supers:ident)*)?;) => {};
    (predicateflag $name:ident = Enable $(for $($trait:ident),+)?;) => {};
    (predicateflag $name:ident = Disable $(for $($trait:ident),+)?;) => {};
    (predicateflag $name:ident = TokensRemain $(for $($trait:ident),+)?;) => {};
    (predicateflag $name:ident $(for $($trait:ident),+)?;) => {};
}

#[doc(hidden)]
#[cfg(not(doc))]
#[macro_export]
macro_rules! unsynn{
    // Lifetime parameter patterns for enums - MUST come before generic patterns
    (
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        {
            $($variants:tt)*
        }
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @enum_lifetime
            $(#[$attribute])* $pub enum $name
            <$lifetime $(, $($param),*)?>
            {
                $($variants)*
            }
        }
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        {
            $($variants:tt)*
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @enum_lifetime
            $(#[$attribute])* $pub enum $name
            <$lifetime $(, $($param),*)?>
            {
                $($variants)*
            }
        }
        $crate::unsynn!{$($cont)*}
    };

    // enums
    (
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($variants:tt)*
        }
        // PLANNED: impl$(<extragenerics: bounds>)* Trait$(<extra>)
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @enum
            $(#[$attribute])* $pub enum $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {
                $($variants)*
            }
        }
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($variants:tt)*
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @enum
            $(#[$attribute])* $pub enum $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {
                $($variants)*
            }
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        @enum
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($variants:tt)*
        }
    ) => {
        // The actual enum definition is written as given
        #[derive(Debug)]
        $(#[$attribute])* $pub enum $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            $($variants)*
        }

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                let mut err = $crate::Error::no_error();
                // try to parse each variant
                $crate::unsynn!{@enum_parse_variant(tokens, err) $($variants)*}
                // nothing matched, error out
                Err(err)
            }
        }

        impl$(<$($generic: $crate::ToTokens $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::ToTokens
        for $name$(< $($generic),* >)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                $crate::unsynn!{@enum_to_tokens(self, tokens) {$($variants)*}}
            }
        }
    };

    // @enum_lifetime helper for enums with lifetime parameters
    (
        @enum_lifetime
        $(#[$attribute:meta])* $pub:vis enum $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        {
            $($variants:tt)*
        }
    ) => {
        // The actual enum definition is written as given
        #[derive(Debug)]
        $(#[$attribute])* $pub enum $name<$lifetime $(, $($param),*)?> {
            $($variants)*
        }

        impl<$lifetime $(, $($param),*)?> $crate::Parser
        for $name<$lifetime $(, $($param),*)?>
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                let mut err = $crate::Error::no_error();
                // try to parse each variant
                $crate::unsynn!{@enum_parse_variant(tokens, err) $($variants)*}
                // nothing matched, error out
                Err(err)
            }
        }

        impl<$lifetime $(, $($param),*)?> $crate::ToTokens
        for $name<$lifetime $(, $($param),*)?>
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                $crate::unsynn!{@enum_to_tokens(self, tokens) {$($variants)*}}
            }
        }
    };

    // ERROR PATTERNS - Catch unsupported constructs and provide helpful messages

    // Unit struct - not yet supported
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident;
        $($cont:tt)*
    ) => {
        compile_error!(concat!(
            "Unit structs are not yet supported in unsynn! macro.\n",
            "\n",
            "Found: `", stringify!($pub struct $name;), "`\n",
            "\n",
            "Unit structs (like `struct Foo;`) are commonly used as marker types\n",
            "in unsynn (e.g., Enable, Disable, EndOfStream). Support for defining\n",
            "them inside unsynn! is planned but not yet implemented.\n",
            "\n",
            "Workarounds:\n",
            "  1. Define marker types outside unsynn! and implement Parser/ToTokens manually\n",
            "  2. Use empty tuple struct: `", stringify!($pub struct $name();), "`\n",
            "     (though this may also not be supported yet)\n",
            "\n",
            "See ROADMAP.md for planned features."
        ));
    };

    // Empty tuple struct - may want to make this an error too
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident();
        $($cont:tt)*
    ) => {
        compile_error!(concat!(
            "Empty tuple structs are not supported in unsynn! macro.\n",
            "\n",
            "Found: `", stringify!($pub struct $name();), "`\n",
            "\n",
            "Empty tuple structs would parse and emit no tokens, which is typically\n",
            "only useful for marker types. If you need a marker, define it outside\n",
            "unsynn! as a unit struct with manual Parser/ToTokens implementation.\n",
            "\n",
            "If you need a struct that parses tokens, use tuple struct with fields:\n",
            "  `", stringify!($pub struct $name(Type);), "`\n",
            "\n",
            "See ROADMAP.md for discussion on marker type support."
        ));
    };

    // normal structs - lifetime parameter patterns FIRST
    // Pattern 1: Lifetime + one type param with no bounds: <'a, T>
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime, $tparam:ident $(,)?>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct_lifetime_tparam
            $(#[$attribute])* $pub struct $name
            <$lifetime, $tparam>
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime, $tparam:ident $(,)?>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct_lifetime_tparam
            $(#[$attribute])* $pub struct $name
            <$lifetime, $tparam>
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        $crate::unsynn!{$($cont)*}
    };

    // Pattern 2: Lifetime only: <'a> or <'a, 'b>
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct_lifetime
            $(#[$attribute])* $pub struct $name
            <$lifetime $(, $($param),*)?>
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct_lifetime
            $(#[$attribute])* $pub struct $name
            <$lifetime $(, $($param),*)?>
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        $crate::unsynn!{$($cont)*}
    };

    // normal structs with generics
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        // PLANNED: impl$(<extragenerics: bounds>)* Trait$(<extra>)
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @struct
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {
                $($(#[$mattr])* $mpub $member: $parser),*
            }
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };

    (
        @struct
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            $(
                $(#[$mattr])* $mpub $member : $parser
            ),*
        }

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self{$($member: <$parser as $crate::Parse>::parse(tokens)?),*})
            }
        }

        impl$(<$($generic: $crate::ToTokens $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                $(self.$member.to_tokens(tokens);)*
            }
        }
    };

    // @struct_lifetime_tparam - struct with <'a, T>
    (
        @struct_lifetime_tparam
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime, $tparam:ident>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name<$lifetime, $tparam> {
            $(
                $(#[$mattr])* $mpub $member : $parser
            ),*
        }

        impl<$lifetime, $tparam: $crate::Parser> $crate::Parser
        for $name<$lifetime, $tparam>
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self{$($member: <$parser as $crate::Parse>::parse(tokens)?),*})
            }
        }

        impl<$lifetime, $tparam: $crate::ToTokens> $crate::ToTokens
        for $name<$lifetime, $tparam>
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                $(self.$member.to_tokens(tokens);)*
            }
        }
    };

    // @struct_lifetime - struct with just lifetimes
    (
        @struct_lifetime
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)?>
        {
            $($(#[$mattr:meta])* $mpub:vis $member:ident: $parser:ty),* $(,)?
        }
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name<$lifetime $(, $($param),*)?> {
            $(
                $(#[$mattr])* $mpub $member : $parser
            ),*
        }

        impl<$lifetime $(, $($param),*)?> $crate::Parser
        for $name<$lifetime $(, $($param),*)?>
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self{$($member: <$parser as $crate::Parse>::parse(tokens)?),*})
            }
        }

        impl<$lifetime $(, $($param),*)?> $crate::ToTokens
        for $name<$lifetime $(, $($param),*)?>
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                $(self.$member.to_tokens(tokens);)*
            }
        }
    };

    // tuple structs with parse_with and/or to_tokens - MUST COME BEFORE regular tuple struct patterns

    // Single-field with `from <type>: parse_with; to_tokens`
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $(#[$mattr:meta])* $mpub:vis $field:ty
        )
        from $parse_type:ty:
        parse_with |$pval:ident, $ptok:ident| $parse_body:block
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_parse_with_as_to_tokens
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            ($(#[$mattr])* $mpub $field)
            from $parse_type:
            parse_with |$pval, $ptok| $parse_body
            to_tokens |$tval, $ttok| $tokens_body
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };

    // Single-field with `from <type>: parse_with` (no to_tokens - uses default)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $(#[$mattr:meta])* $mpub:vis $field:ty
        )
        from $parse_type:ty:
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_parse_with_as
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            ($(#[$mattr])* $mpub $field)
            from $parse_type:
            parse_with |$pval, $ptok| $closure_body
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };

    // Tuple struct with `parse_with; to_tokens` (validation + custom tokens)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        parse_with |$pval:ident, $ptok:ident| $parse_body:block
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_parse_with_to_tokens
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            );
            parse_with |$pval, $ptok| $parse_body
            to_tokens |$tval, $ttok| $tokens_body
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };

    // Tuple struct with `parse_with` only (validation, default tokens)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_parse_with
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            );
            parse_with |$pval, $ptok| $closure_body
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };

    // Tuple struct with `to_tokens` only (default parsing, custom tokens)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_to_tokens
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            );
            to_tokens |$tval, $ttok| $tokens_body
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };

    // Error: `from` without `parse_with`
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($parser:ty),* $(,)?
        )
        from $parse_type:ty:
        $($cont:tt)*
    ) => {
        compile_error!("`from` requires `parse_with` clause. Use: `from Type: parse_with |value, tokens| { ... }`");
    };

    // TODO: review, is this restriction necessary?
    // Error: `from` with multiple fields
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $field1:ty,
            $field2:ty
            $(, $rest_fields:ty)* $(,)?
        )
        from $parse_type:ty:
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        $($cont:tt)*
    ) => {
        compile_error!("`from` only supported for single-field tuple structs. For multi-field structs, use `parse_with` without `from` for validation.");
    };

    // tuple structs - lifetime parameter patterns FIRST
    // w/ impl - WITH semicolon
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_lifetime
            $(#[$attribute])* $pub struct $name
            <$lifetime $(, $($param),*)?>
            (
                $($(#[$mattr])* $mpub $parser),*
            );
        }
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    // w/ impl - WITHOUT semicolon (optional)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        )
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_lifetime
            $(#[$attribute])* $pub struct $name
            <$lifetime $(, $($param),*)?>
            (
                $($(#[$mattr])* $mpub $parser),*
            );
        }
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)? $(,)?>
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct_lifetime
            $(#[$attribute])* $pub struct $name
            <$lifetime $(, $($param),*)?>
            (
                $($(#[$mattr])* $mpub $parser),*
            );
        }
        $crate::unsynn!{$($cont)*}
    };

    // tuple structs with generics
    // w/ traits - WITH semicolon
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        )
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        // PLANNED: impl$(<extragenerics: bounds>)* Trait$(<extra>)
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            )
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    // w/ traits - WITHOUT semicolon (optional semicolon)
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        )
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        // PLANNED: impl$(<extragenerics: bounds>)* Trait$(<extra>)
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            )
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    // w/o traits
    (
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        )
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @tuplestruct
            $(#[$attribute])* $pub struct $name
            $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
            (
                $($(#[$mattr])* $mpub $parser),*
            )
            $(where
                $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;
        }
        $crate::unsynn!{$($cont)*}
    };
    (
        @tuplestruct
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        )
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($($(#[$mattr])* $mpub $parser),*)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self($(<$parser as $crate::Parse>::parse(tokens)?),*))
            }
        }

        impl$(<$($generic: $crate::ToTokens $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                unsynn! {@tuple_for_each item in self : Self($($parser),*) {
                    item.to_tokens(tokens);
                }}
            }
        }
    };

    // @tuplestruct_parse_with_as - single-field tuple struct with transformation
    (
        @tuplestruct_parse_with_as
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        ($(#[$mattr:meta])* $mpub:vis $field:ty)
        from $parse_type:ty:
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($(#[$mattr])* $mpub $field)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                <$parse_type>::parse_with(tokens, |$pval, $ptok| $closure_body)
            }
        }

        impl$(<$($generic: $crate::ToTokens $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens)
            }
        }
    };

    // @tuplestruct_parse_with - tuple struct with validation (any number of fields)
    (
        @tuplestruct_parse_with
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($($(#[$mattr])* $mpub $parser),*)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                // Parse normally first
                let parsed = Self($(<$parser as $crate::Parse>::parse(tokens)?),*);
                // Then validate
                (|$pval: Self, $ptok: &mut $crate::TokenIter| $closure_body)(parsed, tokens)
            }
        }

        impl$(<$($generic: $crate::ToTokens $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                unsynn! {@tuple_for_each item in self : Self($($parser),*) {
                    item.to_tokens(tokens);
                }}
            }
        }
    };

    // @tuplestruct_parse_with_as_to_tokens - single-field with transformation + custom tokens
    (
        @tuplestruct_parse_with_as_to_tokens
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        ($(#[$mattr:meta])* $mpub:vis $field:ty)
        from $parse_type:ty:
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($(#[$mattr])* $mpub $field)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                <$parse_type>::parse_with(tokens, |$pval, $ptok| $closure_body)
            }
        }

        impl$(<$($generic),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                (|$tval: &Self, $ttok: &mut $crate::TokenStream| $tokens_body)(self, tokens)
            }
        }
    };

    // @tuplestruct_parse_with_to_tokens - validation + custom tokens (any number of fields)
    (
        @tuplestruct_parse_with_to_tokens
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        parse_with |$pval:ident, $ptok:ident| $closure_body:block
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($($(#[$mattr])* $mpub $parser),*)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                // Parse normally first
                let parsed = Self($(<$parser as $crate::Parse>::parse(tokens)?),*);
                // Then validate
                (|$pval: Self, $ptok: &mut $crate::TokenIter| $closure_body)(parsed, tokens)
            }
        }

        impl$(<$($generic),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                (|$tval: &Self, $ttok: &mut $crate::TokenStream| $tokens_body)(self, tokens)
            }
        }
    };

    // @tuplestruct_to_tokens - default parsing + custom tokens (any number of fields)
    (
        @tuplestruct_to_tokens
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        $(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)? $(= $default:ty)?),*$(,)?>)?
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
        to_tokens |$tval:ident, $ttok:ident| $tokens_body:block
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?;
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name
        $(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)? $(= $default)?),*>)?
        ($($(#[$mattr])* $mpub $parser),*)
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?;

        impl$(<$($generic: $crate::Parser $(+ $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $crate::Parser
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self($(<$parser as $crate::Parse>::parse(tokens)?),*))
            }
        }

        impl$(<$($generic),*>)? $crate::ToTokens
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                (|$tval: &Self, $ttok: &mut $crate::TokenStream| $tokens_body)(self, tokens)
            }
        }
    };

    // @tuplestruct_lifetime - tuple struct with lifetime parameters
    (
        @tuplestruct_lifetime
        $(#[$attribute:meta])* $pub:vis struct $name:ident
        <$lifetime:lifetime $(, $($param:tt),*)?>
        (
            $($(#[$mattr:meta])* $mpub:vis $parser:ty),* $(,)?
        );
    ) => {
        #[derive(Debug)]
        $(#[$attribute])* $pub struct $name<$lifetime $(, $($param),*)?>(
            $($(#[$mattr])* $mpub $parser),*
        );

        impl<$lifetime $(, $($param),*)?> $crate::Parser
        for $name<$lifetime $(, $($param),*)?>
        {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                Ok(Self($(<$parser as $crate::Parse>::parse(tokens)?),*))
            }
        }

        impl<$lifetime $(, $($param),*)?> $crate::ToTokens
        for $name<$lifetime $(, $($param),*)?>
        {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                unsynn! {@tuple_for_each item in self : Self($($parser),*) {
                    item.to_tokens(tokens);
                }}
            }
        }
    };

    // type passthough
    (
        $(#[$attribute:meta])* $pub:vis type $name:ident
        $(<$($generic:ident $(: $constraint:ident $(+ $constraints:ident)*)? $(= $default:ty)?),*$(,)?>)?
        = $orig:path;
        $($cont:tt)*
    ) => {
        $(#[$attribute])* $pub type $name$(<$($generic$(: $constraint $(+ $constraints)*)? $(= $default)?),*>)? = $orig;
        // next item
        $crate::unsynn!{$($cont)*}
    };

    // TODO: generics

    // marker trait blocks
    (
        $(#[$attributeall:meta])*
        $pub:vis trait {
            $($(#[$attribute:meta])* $name:ident$(: $super:ident $(+ $supers:ident)*)?);+$(;)?
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{@expand_marker_trait($(#[$attributeall])*)
            $({$(#[$attribute])* $pub $name$(: $super $(+ $supers)*)?})+
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attributeall:meta])*
        trait {
            $($(#[$attribute:meta])* $pub:vis $name:ident$(: $super:ident $(+ $supers:ident)*)?);+$(;)?
        }
        $($cont:tt)*
    ) => {
        $crate::unsynn!{@expand_marker_trait($(#[$attributeall])*)
            $({$(#[$attribute])* $pub $name$(: $super $(+ $supers)*)?})+
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        @expand_marker_trait($(#[$attributeall:meta])*)
        {$(#[$attribute:meta])* $pub:vis $name:ident$(: $super:ident $(+ $supers:ident)*)?}
        // next item
        $($cont:tt)*
    ) => {
        $(#[$attributeall])* $(#[$attribute])* $pub trait $name$(: $super $(+ $supers)*)? {}
        $crate::unsynn!{@expand_marker_trait($(#[$attributeall])*) $($cont)*}
    };
    (@expand_marker_trait($(#[$attributeall:meta])*)) => {};

    // marker trait definitions with ; at the end
    (
        $(#[$attribute:meta])* $pub:vis trait $name:ident$(: $super:ident $(+ $supers:ident)*)?;
        $($cont:tt)*
    ) => {
        $(#[$attribute])* $pub trait $name$(: $super $(+ $supers)*)? {}
        // next item
        $crate::unsynn!{$($cont)*}
    };
    // trait definition passthrough
    (
        $(#[$attribute:meta])* $pub:vis trait $name:ident$(: $super:ident $(+ $supers:ident)*)?
        {$($body:tt)*}
        $($cont:tt)*
    ) => {
        $(#[$attribute])* $pub trait $name$(: $super $(+ $supers)*)? {$($body)*}
        // next item
        $crate::unsynn!{$($cont)*}
    };

    // impl passthrough for inherent implementations (without 'for' keyword)
    (
        $(#[$attribute:meta])*
        impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*$(,)?>)?
        $type:ident$(<$($genparam:ident),*>)?
        {$($body:tt)*}
        $($cont:tt)*
    ) => {
        $(#[$attribute])*
        impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
        $type$(<$($genparam),*>)?
        {$($body)*}
        // next item
        $crate::unsynn!{$($cont)*}
    };

    // impl passthrough (trait impl with 'for')
    (
        $(#[$attribute:meta])*
        impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*$(,)?>)?
        $trait:ident$(<$($traitgenparam:ident),*$(,)?>)?
        for $type:ty {$($body:tt)*}
        $($cont:tt)*
    ) => {
        $(#[$attribute])*
        impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
        $trait$(<$($traitgenparam),*$(,)?>)?
        for $type {$($body)*}
        // next item
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])*
        impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*$(,)?>)?
        $trait:ident$(<$($traitgenparam:ident),*$(,)?>)?
        for $($type:ty),+;
        $($cont:tt)*
    ) => {
        $crate::unsynn!{
            @impl_markers(
                $(#[$attribute])*
                impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
                $trait$(<$($traitgenparam),*$(,)?>)?
            )
            $($type),+,
        }
        // next item
        $crate::unsynn!{$($cont)*}
    };

    // impl Marker for Many, Types, At, Once;
    (
        @impl_markers(
            $(#[$attribute:meta])*
            impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*$(,)?>)?
            $trait:ident$(<$($traitgenparam:ident),*$(,)?>)?
        )
        $type:ty,
        $($cont:tt)*
    ) => {
        $(#[$attribute])*
        impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
        $trait$(<$($traitgenparam),*$(,)?>)?
        for $type{}

        $crate::unsynn!{
            @impl_markers(
                $(#[$attribute])*
                impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
                $trait$(<$($traitgenparam),*$(,)?>)?
            )
            $($cont)*
        }
    };
    (@impl_markers($($_:tt)*)) => {};

    // fn passthrough - with optional generics
    (
        $(#[$attribute:meta])* $pub:vis fn $name:ident$(<$($generic:tt),*>)?($($param:tt)*) $(-> $ret:ty)? {$($body:tt)*}
        $($cont:tt)*
    ) => {
        $(#[$attribute])* $pub fn $name$(<$($generic),*>)?($($param)*) $(-> $ret)? {$($body)*}
        $crate::unsynn!{$($cont)*}
    };

    // use passthrough
    (use ::$($path:ident)::+$(::{$($inside:tt)+})?; $($cont:tt)*) => {
        use ::$($path)::+$(::{$($inside)+})?;
        $crate::unsynn!{$($cont)*}
    };
    (use $($path:ident)::+$(::{$($inside:tt)+})?; $($cont:tt)*) => {
        use $($path)::+$(::{$($inside)+})?;
        $crate::unsynn!{$($cont)*}
    };
    (use ::$($path:ident)::+ as $alias:ident; $($cont:tt)*) => {
        use $($path)::+ as $alias;
        $crate::unsynn!{$($cont)*}
    };
    (use ::$($path:ident)::+ as $alias:ident; $($cont:tt)*) => {
        use $($path)::+ as $alias;
        $crate::unsynn!{$($cont)*}
    };

    // macro_rules! passthrough
    (macro_rules! $name:ident {$($rules:tt)*} $($cont:tt)*) => {
        macro_rules! $name {$($rules)*}
        $crate::unsynn!{$($cont)*}
    };

    // macro passthrough
    ($macro:ident! {$($args:tt)*} $($cont:tt)*) => {
        $macro! {$($args)*}
        $crate::unsynn!{$($cont)*}
    };
    ($macro:ident! ($($args:tt)*); $($cont:tt)*) => {
        $macro! ($($args)*);
        $crate::unsynn!{$($cont)*}
    };
    ($macro:ident! [$($args:tt)*]; $($cont:tt)*) => {
        $macro! [$($args)*];
        $crate::unsynn!{$($cont)*}
    };

    // keyword delegation
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident = $str:literal;
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = $str}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident = $str:literal; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = $str}
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident != $str:literal;
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != $str}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident != $str:literal; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != $str}
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident = $group:path;
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = $group}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident = $group:path; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = $group}
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident != $group:path;
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != $group}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident != $group:path; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != $group}
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident = [$($keywords:tt),+ $(,)?];
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = [$($keywords),+]}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident = [$($keywords:tt),+ $(,)?]; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name = [$($keywords),+]}
        $crate::unsynn!{$($cont)*}
    };
    (
        $(#[$attribute:meta])* $pub:vis keyword $name:ident != [$($keywords:tt),+ $(,)?];
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != [$($keywords),+]}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis keyword $name:ident != [$($keywords:tt),+ $(,)?]; $($cont:tt)*) => {
        $crate::keyword!{$(#[$attribute])* $pub $name != [$($keywords),+]}
        $crate::unsynn!{$($cont)*}
    };

    // operator delegation
    (
        $(#[$attribute:meta])* $pub:vis operator $name:ident = $str:literal;
        impl {$($(#[$tattr:meta])* $trait:ident $bracesemi:tt)*}
        $($cont:tt)*
    ) => {
        $crate::operator!{$(#[$attribute])* $pub $name = $str}
        $crate::unsynn!{
            @impl
            for $name
            {$({$(#[$tattr])* $trait $bracesemi})*}
        }
        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis operator $name:ident = $str:literal; $($cont:tt)*) => {
        $crate::operator!{$(#[$attribute])* $pub $name = $str}
        $crate::unsynn!{$($cont)*}
    };

    // predicatetrait - creates a context trait and implements it for universal predicates
    ($(#[$attribute:meta])* $pub:vis predicatetrait $name:ident $(: $super:ident $(+ $supers:ident)*)? ; $($cont:tt)*) => {
        // Define the trait
        $(#[$attribute])* $pub trait $name$(: $super $(+ $supers)*)? : $crate::PredicateOp {}

        // Implement for universal predicates
        impl $name for $crate::Enable {}
        impl $name for $crate::Disable {}
        impl $name for $crate::TokensRemain {}

        // Implement for logic operators when all operands implement the trait
        impl<A: $name, B: $name, C: $name, D: $name> $name for $crate::AllOf<A, B, C, D> {}
        impl<A: $name, B: $name, C: $name, D: $name> $name for $crate::AnyOf<A, B, C, D> {}
        impl<A: $name, B: $name, C: $name, D: $name> $name for $crate::OneOf<A, B, C, D> {}
        impl<T: $name> $name for $crate::Not<T> {}

        $crate::unsynn!{$($cont)*}
    };

    // predicateflag - creates newtype wrappers with optional trait implementations
    ($(#[$attribute:meta])* $pub:vis predicateflag $name:ident = Enable $(for $($trait:ident),+ $(,)?)? ; $($cont:tt)*) => {
        $(#[$attribute])*
        #[derive(Debug, Clone, Default)]
        $pub struct $name($crate::Enable);

        impl $crate::Parser for $name {
            #[inline]
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                $crate::Enable::parse(tokens).map($name)
            }
        }

        impl $crate::ToTokens for $name {
            #[inline]
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl $crate::PredicateOp for $name {}

        $($(impl $trait for $name {})*)?

        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis predicateflag $name:ident = Disable $(for $($trait:ident),+ $(,)?)? ; $($cont:tt)*) => {
        $(#[$attribute])*
        #[derive(Debug, Clone)]
        $pub struct $name($crate::Disable);

        impl $crate::Parser for $name {
            #[inline]
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                $crate::Disable::parse(tokens).map($name)
            }
        }

        impl $crate::ToTokens for $name {
            #[inline]
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl $crate::PredicateOp for $name {}

        $($(impl $trait for $name {})*)?

        $crate::unsynn!{$($cont)*}
    };
    ($(#[$attribute:meta])* $pub:vis predicateflag $name:ident = TokensRemain $(for $($trait:ident),+ $(,)?)? ; $($cont:tt)*) => {
        $(#[$attribute])*
        #[derive(Debug, Clone, Default)]
        $pub struct $name($crate::TokensRemain);

        impl $crate::Parser for $name {
            #[inline]
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                $crate::TokensRemain::parse(tokens).map($name)
            }
        }

        impl $crate::ToTokens for $name {
            #[inline]
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl $crate::PredicateOp for $name {}

        $($(impl $trait for $name {})*)?

        $crate::unsynn!{$($cont)*}
    };

    // predicateflag without value defaults to Enable
    ($(#[$attribute:meta])* $pub:vis predicateflag $name:ident $(for $($trait:ident),+ $(,)?)? ; $($cont:tt)*) => {
        $(#[$attribute])*
        #[derive(Debug, Clone, Default)]
        $pub struct $name($crate::Enable);

        impl $crate::Parser for $name {
            #[inline]
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                $crate::Enable::parse(tokens).map($name)
            }
        }

        impl $crate::ToTokens for $name {
            #[inline]
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl $crate::PredicateOp for $name {}

        $($(impl $trait for $name {})*)?

        $crate::unsynn!{$($cont)*}
    };

    // terminate recursion
    () => {};

    // @impl muncher
    (
        @impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*>)?
        for $name:ident
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {{$(#[$tattr:meta])* $trait:ident ;} $($cont:tt)*}
    ) => {
        $(#[$tattr])*
        impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $trait
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {}
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            {$($cont)*}
        }
    };
    (
        @impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*>)?
        for $name:ident
        $(where
            $($where:ident: $wfirst:tt $(:: $wrest:ident)* $(+ $wmore:tt $(:: $wmrest:ident)*)*),*$(,)?)?
        {{$(#[$tattr:meta])* $trait:ident {$($body:tt)*}} $($cont:tt)*}
    ) => {
        $(#[$tattr])*
        impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)? $trait
        for $name$(<$($generic),*>)?
        $(where
            $($where: $wfirst $(:: $wrest)* $(+ $wmore $(:: $wmrest)*)*),*)?
        {$($body)*}
        $crate::unsynn!{
            @impl$(<$($generic$(: $first $(:: $rest)* $(+ $more $(:: $mrest)*)*)?),*>)?
            for $name
            {$($cont)*}
        }
    };
    (
        @impl$(<$($generic:ident$(: $first:tt $(:: $rest:ident)* $(+ $more:tt $(:: $mrest:ident)*)*)?),*>)?
        for $name:ident {}
    ) => {};

    // to_tokens for enum tuple variant
    (
        @enum_to_tokens($self:ident, $tokens:ident)
        {
            $(#[$_attrs:meta])* $variant:ident($($tuple:tt)*) $(,$($cont:tt)*)?
        }
    ) => {
        if matches!($self, Self::$variant(..)) {
            unsynn! {@tuple_for_each item in $self : Self::$variant($($tuple)*) {
                item.to_tokens($tokens);
            }}
            return
        }
        $crate::unsynn!{@enum_to_tokens($self, $tokens) {$($($cont)*)?}}
    };

    // to_tokens for enum struct variant
    (
        @enum_to_tokens($self:ident, $tokens:ident)
        {
            $(#[$_attrs:meta])* $variant:ident {
                $($(#[$_mattrs:meta])* $member:ident: $_type:ty),* $(,)?
            }
            $(,$($cont:tt)*)?
        }
    ) => {
            if matches!($self, Self::$variant{..}) {
                $(
                    let Self::$variant{$member: member, ..} = $self else {unreachable!()};
                    member.to_tokens($tokens);
                )*
                return
            }
            $crate::unsynn!{@enum_to_tokens($self, $tokens) {$($($cont)*)?}}
    };

    // to_tokens for empty variant does nothing
    (
        @enum_to_tokens($self:ident, $tokens:ident)
        {
            $(#[$_attrs:meta])* $variant:ident
            $(,$($cont:tt)*)?
        }
    ) => {
        if matches!($self, Self::$variant) {
            return
        }
        $crate::unsynn!{@enum_to_tokens($self, $tokens) {$($($cont)*)?}}
    };

    // end recursion
    (@enum_to_tokens($self:ident, $tokens:ident) {}) => {};

    // write for enum tuple variant
    (
        @enum_write($self:ident, $f:ident)
        {
            $(#[$_attrs:meta])* $variant:ident($($tuple:tt)*) $(,$($cont:tt)*)?
        }
    ) => {
        if matches!($self, Self::$variant(..)) {
            unsynn! {@tuple_for_each item in $self : Self::$variant($($tuple)*) {
                write!($f, "{} " , &item)?;
            }}
        }
        $crate::unsynn!{@enum_write($self, $f) {$($($cont)*)?}}
    };

    // to_tokens for enum struct variant
    (@enum_write($self:ident, $f:ident) {
        $(#[$_attrs:meta])* $variant:ident {
            $($(#[$_mattrs:meta])* $member:ident: $_type:ty),* $(,)?
        } $(,$($cont:tt)*)?} ) => {
            if matches!($self, Self::$variant{..}) {
                $(
                    let Self::$variant{$member: that, ..} = $self else {unreachable!()};
                    write!($f, "{} ", that)?;
                )*
            }
            $crate::unsynn!{@enum_write($self, $f) {$($($cont)*)?}}
    };

    // write for empty variant does nothing
    (
        @enum_write($self:ident, $f:ident)
        {
            $(#[$_attrs:meta])* $variant:ident $(,$($cont:tt)*)?
        }
    ) => {
        if matches!($self, Self::$variant) {
        }
        $crate::unsynn!{@enum_write($self, $f) {$($($cont)*)?}}
    };

    // end recursion
    (@enum_write($self:ident, $f:ident) {}) => {};

    // Tuple enum variant
    (
        @enum_parse_variant($tokens:ident, $err:ident)
        $(#[$_attrs:meta])* $variant:ident($($tuple:tt)*)
        $(, $($cont:tt)*)?
    ) => {
        if let Ok(parsed) = (|| -> $crate::Result<_> {
            $err.upgrade($crate::unsynn!{@enum_parse_tuple($tokens) $variant($($tuple)*)})
        })() {
            return Ok(parsed);
        }
        $crate::unsynn!{@enum_parse_variant($tokens, $err) $($($cont)*)?}
    };

    // Struct enum variant
    (
        @enum_parse_variant($tokens:ident, $err:ident)
        $(#[$_attrs:meta])*
        $variant:ident{$($members:tt)*}
        $(, $($cont:tt)*)?
    ) => {
        if let Ok(parsed) = (|| -> $crate::Result<_> {
            $err.upgrade($crate::unsynn!{@enum_parse_struct($tokens) $variant{$($members)*}})
        })() {
            return Ok(parsed);
        }
        $crate::unsynn!{@enum_parse_variant($tokens, $err) $($($cont)*)?}
    };

    // Empty enum variant
    (
        @enum_parse_variant($tokens:ident, $err:ident)
        $(#[$_attrs:meta])* $variant:ident
        $(, $($cont:tt)*)?
    ) => {
        /* NOP */
        $crate::unsynn!{@enum_parse_variant($tokens, $err) $($($cont)*)?}
    };

    // end recursion
    (@enum_parse_variant($tokens:ident, $err:ident)) => {};

    // Parse a tuple variant
    (
        @enum_parse_tuple($tokens:ident)
        $variant:ident($($(#[$_attrs:meta])* $parser:ty),* $(,)?)
    ) => {
        $crate::Transaction::transaction($tokens,
            |tokens| Ok(Self::$variant($(<$parser as $crate::Parse>::parse(tokens)?,)*))
        )
    };

    // Parse a struct variant
    (
        @enum_parse_struct($tokens:ident)
        $variant:ident{$($(#[$_attrs:meta])* $name:ident : $parser:ty),* $(,)?}
    ) => {
        $crate::Transaction::transaction($tokens,
            |tokens| Ok(Self::$variant{$($name : <$parser as $crate::Parse>::parse(tokens)?,)*})
        )
    };

    // iterate over $variant:($tuple) in $this and apply some $code for each $i
    (
        @tuple_for_each
        $i:ident in $this:ident :
        $($variant:ident)::*($($tuple:tt)*)
        {
            $($code:tt)*
        }
    ) => {
        {
            $crate::unsynn!{@tuple_for_each $i in $this : $($variant)::*[$($tuple)*] { $($code)* }}
        }
    };

    (
        @tuple_for_each
        $i:ident in $this:ident :
        $($variant:ident)::*[
            $(#[$_attrs:meta])* $_pub:vis $element:ty
            $(,$($rest:tt)*)?
        ]{
            $($code:tt)*
        }
    ) => {
        $crate::unsynn!{@tuple_for_each $i in $this : $($variant)::*[$($($rest)*)?] { $($code)* }}
        #[allow(irrefutable_let_patterns)]
        let $crate::unsynn!{@tuple_nth $i $($variant)::*[$($($rest)*)?]} = $this else {unreachable!()};
            $($code)*
        };
    (@tuple_for_each $i:ident in $_this:ident : $($variant:ident)::*[] { $($code:tt)* }) => {};

    // replaces each prefix item with a underscore, followed by $i and .. finally
    (@tuple_nth $i:ident $($variant:ident)::*[$($(#[$_attrs:meta])* $_pub:vis $_element:ty),* $(,)?]) => {
        $($variant)::*(
            $($crate::unsynn!(@_ $_element),)*
            $i,
            ..
        )
    };

    // replaces a single token with a underscore
    (@_ $_tt:tt) => {_};

    // Error handler: catch malformed input that doesn't match any pattern
    // This must come AFTER all valid patterns (including internal @ patterns) but BEFORE the final termination
    ($unexpected:tt $($rest:tt)*) => {
        compile_error!(concat!(
            "Unexpected token in unsynn! macro: '",
            stringify!($unexpected),
            "'\n\nExpected one of:\n",
            "  - struct definition: struct Name { field: Type }\n",
            "  - tuple struct: struct Name(Type);\n",
            "  - enum: enum Name { Variant }\n",
            "  - trait: trait Name;\n",
            "  - impl block: impl Type { ... } or impl Trait for Type { ... }\n",
            "  - function: fn name(params) { body }\n",
            "  - type alias: type Name = Path;\n",
            "  - use statement: use path;\n",
            "  - macro_rules! definition\n",
            "  - keyword/operator definition\n\n",
            "Check for:\n",
            "  - Missing semicolons\n",
            "  - Mismatched braces\n",
            "  - Invalid syntax in struct/enum definitions"
        ));
    };
}

/// Define types matching keywords.
///
///  * Any number of attributes (`#[...]`), including documentation comments. Keyword
///    documentation is automatically extended by a small auto generated doc comment listing
///    what a keyword definition will match.
/// * A optional `pub` declaration.
/// * `Name` is the name for the struct to be generated.
/// * `"identifier"` is the case sensitive keyword.
/// * `group` can be a non empty bracketed list of `"identifier"` or any an other keyword
///   definition.
/// * By using `=` the keyword must match the given definition while `!=` negates the output
///   and matches any identifier that is not in the definition.
///
/// `Name::parse()` will then only match the defined identifier.  It will implement `Debug`
/// and `Clone` for keywords. Additionally `AsRef<str>` is implemented for each Keyword
/// to access the identifier string from rust code.
///
/// The [`unsynn!`] macro supports defining keywords by using `keyword Name = "ident";`, the
/// `pub` specification has to come before `keyword` then. See the
/// [unsynn! keyword documentation](crate::unsynn#keywords-and-operators) for details.
///
/// In case a invalid keyword is defined (not an identifier) the compilation will panic. But
/// because the actual matching function is optimized and lazy evaluated this will only happen
/// on the first use of the invalid keyword definition.
///
/// Keywords implement `AsRef<str>`, `AsRef<Ident>` and `Keyword::as_str(&self) -> &str`. For
/// Keywords that are defined with a single literal string (`keyword!{ Foo = "foo"}`) the
/// `Default` trait is implemented. Thus they can be created and inserted statically.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// keyword!{
///     /// Optional documentation for `If`
///     pub If = "if";
///     pub Else = "else";
///     // keywords can be grouped from existing keywords
///     IfElse = [If, Else,];
///     // or contain identifiers in double quotes
///     IfElseThen = [IfElse, "then"];
///     // matching can be negated with `!=`
///     NotIfElseThen != [IfElse, "then"];
/// }
///
/// let mut tokens = "if".to_token_iter();
/// let if_kw = If::parse(&mut tokens).unwrap();
/// assert_eq!(if_kw.as_str(), "if");
/// # let mut tokens = "else if then something".to_token_iter();
/// # let else_kw = Else::parse(&mut tokens).unwrap();
/// # assert_eq!(else_kw.as_str(), "else");
/// # let ifelse_kw = IfElse::parse(&mut tokens).unwrap();
/// # assert_eq!(ifelse_kw.as_str(), "if");
/// # let ifelsethen_kw = IfElseThen::parse(&mut tokens).unwrap();
/// # assert_eq!(ifelsethen_kw.as_str(), "then");
/// # let notifelsethen_kw = NotIfElseThen::parse(&mut tokens).unwrap();
/// # assert_eq!(notifelsethen_kw.as_str(), "something");
/// ```
#[cfg(doc)]
#[macro_export]
macro_rules! keyword {
    ($name:ident = $str:literal; ...) => {};
    ($name:ident = $group:path; ...) => {};
    ($name:ident = [$($keywords:tt),+]; ...) => {};
    ($name:ident != $str:literal; ...) => {};
    ($name:ident != $group:path; ...) => {};
    ($name:ident != [$($keywords:tt),+]; ...) => {};
}

#[doc(hidden)]
#[cfg(not(doc))]
#[macro_export]
macro_rules! keyword{
    ($(#[$attribute:meta])* $pub:vis $name:ident = $str:literal $(;$($cont:tt)*)?) => {
        $crate::keyword!{
            @{} $(#[$attribute])* $pub $name [$str]
        }
        // implement `Default` for single token keywords
        $crate::keyword!{
            @default $name $str
        }
        $crate::keyword!{$($($cont)*)?}
    };
    ($(#[$attribute:meta])* $pub:vis $name:ident != $str:literal $(;$($cont:tt)*)?) => {
        $crate::keyword!{
            @{!} $(#[$attribute])* $pub $name [$str]
        }
        $crate::keyword!{$($($cont)*)?}
    };
    ($(#[$attribute:meta])* $pub:vis $name:ident = $group:path $(;$($cont:tt)*)?) => {
        $crate::keyword!{
            @{} $(#[$attribute])* $pub $name [$group]
        }
        $crate::keyword!{$($($cont)*)?}
    };
    ($(#[$attribute:meta])* $pub:vis $name:ident != $group:path $(;$($cont:tt)*)?) => {
        $crate::keyword!{
            @{!} $(#[$attribute])* $pub $name [$group]
        }
        $crate::keyword!{$($($cont)*)?}
    };
    (
        $(#[$attribute:meta])*
        $pub:vis $name:ident = [$($keywords:tt),+ $(,)?]
        $(;$($cont:tt)*)?
    ) => {
        $crate::keyword!{
            @{} $(#[$attribute])* $pub $name [$($keywords),+]
        }
        $crate::keyword!{$($($cont)*)?}
    };
    (
        $(#[$attribute:meta])*
        $pub:vis $name:ident != [$($keywords:tt),+ $(,)?]
        $(;$($cont:tt)*)?
    ) => {
        $crate::keyword!{
            @{!} $(#[$attribute])* $pub $name [$($keywords),+]
        }
        $crate::keyword!{$($($cont)*)?}
    };
    (
        @{$($not:tt)?} $(#[$attribute:meta])*
        $pub:vis $name:ident [$($keywords:tt),+]
        $(;$($cont:tt)*)?
    ) => {
        $(#[$attribute])*
        // TODO $crate::docgen!{keyword {$($not)?} {$keywords}},
        #[doc = concat!(
             $crate::docgen!{@keyword_header $($not)?},
             $($crate::docgen!{@keyword_doc $keywords}),+
        )]
        #[derive(Debug, Clone, PartialEq, Eq)]
        $pub struct $name($crate::CachedIdent);

        impl $crate::Parser for $name {
            fn parser(tokens: &mut $crate::TokenIter) -> $crate::Result<Self> {
                use $crate::{Parse, RefineErr};
                let at = tokens.clone().next();
                $crate::CachedIdent::parse_with(tokens, |ident, tokens| {
                    if $($not)? Self::matches(ident.as_str()) {
                        Ok($name(ident))
                    } else {
                        $crate::Error::other::<$name>(
                            at,
                            tokens,
                            format!(
                                "keyword for {:?} expected, got {:?} at {:?}",
                                stringify!($name),
                                ident.as_str(),
                                ident.span().start()
                            ),
                        )
                    }
                }).refine_err::<Self>()
            }
        }

        impl $crate::ToTokens for $name {
            fn to_tokens(&self, tokens: &mut $crate::TokenStream) {
                self.0.to_tokens(tokens);
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.0.as_str()
            }
        }

        impl AsRef<$crate::Ident> for $name {
            fn as_ref(&self) -> &$crate::Ident {
                &*self.0
            }
        }

        impl $name {
            /// get the underlying `&str` from a keyword
            #[allow(dead_code)]
            pub fn as_str(&self) -> &str {
                self.0.as_str()
            }

            #[doc(hidden)]
            #[inline]
            pub const fn keywords() -> &'static $crate::KeywordGroup {
                static KEYWORDS: $crate::KeywordGroup = $crate::keyword! {@group $($keywords),+};
                &KEYWORDS
            }

            fn matches(this: &str) -> bool {
                static MATCHFN: std::sync::LazyLock<Box<dyn Fn(&str) -> bool + Send + Sync>> =
                    std::sync::LazyLock::new(|| $crate::create_matchfn($name::keywords()));
                MATCHFN(this)
            }
        }

        $crate::keyword!{$($($cont)*)?}
    };
    () => {};

    (@default $name:ident $str:literal) => {
        impl Default for $name {
            fn default() -> Self {
                use $crate::{ToTokens, Parse};
                Self($crate::CachedIdent::parse(&mut $str.to_token_iter()).unwrap())
            }
        }
    };

    // keyword group creation
    (@group $($entry:tt),+) => {
        $crate::KeywordGroup::List(
            &[$(&$crate::keyword!{@entry $entry}),+]
        )
    };
    (@entry $kw:literal) => {
        $crate::KeywordGroup::Keyword($kw)
    };
    (@entry $sub:path) => {
        *<$sub>::keywords()
    };
}

/// Define types matching operators (punctuation sequences).
///
/// `operator!{ pub Op = "punct"; ...}`
///
/// * A optional `pub` defines the operators public, default is private
/// * `Op` is the name for the struct to be generated
/// * `"punct"` is up to 4 ASCII punctuation characters
///
/// `Op::parse()` will match the defined operator. It will implement `Debug` and `Clone`
/// for operators.
///
/// The [`unsynn!`] macro supports defining operators by using `operator Op = "chars";`, the
/// `pub` specification has to come before `operator` then. See the
/// [unsynn! operator documentation](crate::unsynn#keywords-and-operators) for details.
///
/// For predefined operators, see [`operator::names`](crate::operator::names) which includes
/// common operators like `Plus`, `Minus`, `Star`, `Arrow`, etc.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// operator!{
///     /// Optional documentation for `<~~`
///     WLArrow = "<~~";
///     WRArrow = "~~>";
/// }
///
/// let mut tokens = "<~~~~> ~~><~~".to_token_iter();
/// let wl = WLArrow::parse(&mut tokens).unwrap();
/// assert_tokens_eq!(wl, str "<~~");
/// let wr = WRArrow::parse(&mut tokens).unwrap();
/// assert_tokens_eq!(wr, str "~~>");
/// # let wr = WRArrow::parse(&mut tokens).unwrap();
/// # assert_tokens_eq!(wr, "~~>");
/// # let wl = WLArrow::parse(&mut tokens).unwrap();
/// # assert_tokens_eq!(wl, "<~~");
/// ```
#[macro_export]
macro_rules! operator{
    // match a list of operator! defs
    ($($(#[$attribute:meta])* $pub:vis $name:ident = $op:literal);*$(;)?) => {
        $(
            $crate::operator!(@operator $(#[$attribute])* $pub $name = $op);
        )*
    };

    // match a single operator! defs with len 1-4
    (@operator $(#[$attribute:meta])* $pub:vis $name:ident = $op:literal) => {
        $(#[$attribute])*
        // TODO $crate::docgen!{operator $op},
        #[doc = $crate::docgen!{@operator_doc $op}]
        $pub type $name = $crate::Operator<
        {
                assert!(
                    $op.len() >= 1 && $op.len() <= 4,
                    "Operators must be 1-4 ASCII punctuation characters"
                );
                let c0 = $crate::operator!(@char_at 0 $op);
                assert!(c0.is_ascii_punctuation(), "Operator must be ASCII punctuation");
                c0
        },
        {
                let c1 = $crate::operator!(@char_at 1 $op);
                assert!(c1 == '\0' || c1.is_ascii_punctuation(), "Operator must be ASCII punctuation");
                c1
        },
        {
                let c2 = $crate::operator!(@char_at 2 $op);
                assert!(c2 == '\0' || c2.is_ascii_punctuation(), "Operator must be ASCII punctuation");
                c2
        },
        {
                let c3 = $crate::operator!(@char_at 3 $op);
                assert!(c3 == '\0' || c3.is_ascii_punctuation(), "Operator must be ASCII punctuation");
                c3
        },
        >;
    };

    // get a single ascii character from a literal string
    (@char_at $at:literal $op:literal) => {
       const {
           concat!($op, "\0\0\0").as_bytes()[$at] as char
       }
    }
}

// The documentation generator
#[cfg(not(feature = "docgen"))]
#[doc(hidden)]
#[macro_export]
macro_rules! docgen {
    // just dispose anything
    ($($_:tt)*) => {
        ""
    };
}

#[cfg(feature = "docgen")]
#[doc(hidden)]
#[macro_export]
macro_rules! docgen {
    (@keyword_header) => {
        "Matches: "
    };
    (@keyword_header !) => {
        "Matches any `Ident` but: "
    };
    (@keyword_doc $kw:literal) => {
        concat!("`", $kw, "`, ")
    };
    (@keyword_doc $sub:path) => {
        concat!("[`", stringify!($sub), "`], ")
    };
    (@operator_doc $op:literal) => {
        concat!("`", $op, "`")
    };
}

/// unsynn provides its own `quote!{}` macro that translates tokens into a `TokenStream` while
/// interpolating variables prefixed with a `Pound` sign (#). This is similar to what the quote macro from
/// the quote crate does but not as powerful. There is no `#(...)` repetition (yet).
///
/// Instead we provide `#{...}` blocks which must return a `IntoIterator` whose items
/// implement `ToTokens`. When blocks returns a single value just wrap this in `Some()`
/// because `Option` implements the necessary `IntoIterator`.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let ast = <Cons<ConstInteger<1>, Plus, ConstInteger<2>>>::default();
/// let quoted = quote! { let a = #ast;};
/// assert_tokens_eq!(quoted, "let a = 1+2;");
///
/// // or using #{...} blocks
/// let quoted = quote! {
///     let a = #{Some(<Cons<ConstInteger<1>, Plus, ConstInteger<2>>>::default())};
/// };
/// assert_tokens_eq!(quoted, "let a = 1+2;");
/// ```
#[macro_export]
macro_rules! quote {
    ($($tokens:tt)*) => {
        {
            let mut tokenstream = $crate::TokenStream::new();
            $crate::quote_intern!{tokenstream $($tokens)*};
            tokenstream
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! quote_intern {
    ($tokenstream:ident #$var:ident $($rest:tt)*) => {
        $var.to_tokens(&mut $tokenstream);
        $crate::quote_intern!{$tokenstream $($rest)*}
    };
    ($tokenstream:ident #{$($code:tt)*} $($rest:tt)*) => {
        {$($code)*}.into_iter().for_each(|i| i.to_tokens(&mut $tokenstream));
        $crate::quote_intern!{$tokenstream $($rest)*}
    };
    // hash followed by parenthesis is reserved
    ($tokenstream:ident #($($reserved:tt)*) $($rest:tt)*) => {
        compile_error!("#(...) reserved for future")
    };
    ($tokenstream:ident ( $($nested:tt)* ) $($rest:tt)*) => {
        $tokenstream.extend(
            Some(
                $crate::TokenTree::Group(
                    $crate::Group::new($crate::Delimiter::Parenthesis, $crate::quote!{$($nested)*})
                )
            ).into_iter()
        );
        $crate::quote_intern!{$tokenstream $($rest)*}
    };
    ($tokenstream:ident { $($nested:tt)* } $($rest:tt)*) => {
        $tokenstream.extend(
            Some(
                $crate::TokenTree::Group(
                    $crate::Group::new($crate::Delimiter::Brace, $crate::quote!{$($nested)*})
                )
            ).into_iter()
        );
        $crate::quote_intern!{$tokenstream $($rest)*}
    };
    ($tokenstream:ident [ $($nested:tt)* ] $($rest:tt)*) => {
        $tokenstream.extend(
            Some(
                $crate::TokenTree::Group(
                    $crate::Group::new($crate::Delimiter::Bracket, $crate::quote!{$($nested)*})
                )
            ).into_iter()
        );
        $crate::quote_intern!{$tokenstream $($rest)*}
    };

    ($tokenstream:ident $token:tt $($rest:tt)*) => {
        let t: $crate::TokenStream = std::str::FromStr::from_str(stringify!($token)).unwrap();
        $tokenstream.extend(Some(t).into_iter());
        $crate::quote_intern!{$tokenstream $($rest)*}
    };

    ($tokenstream:ident) => {};
}
