#![allow(rustdoc::bare_urls)]
#![doc = include_str!("../README.md")]
//!
#![doc = include_str!("../COOKBOOK.md")]
//!
//! # Roadmap
//!
#![doc = include_str!("../ROADMAP.md")]
//!
//! For a history how unsynn evolved, check the [CHANGELOG].
// PLANNED: currently the Error type is a tad big. This could be either resolved by
// refactoring the Error type (only keep refine typename, remove 'at' and have the iter start
// at the error not after) or just Box<Error>. A conclusive optimization for this is postponed
// until we have benchmarks to decide whats the best approach. It may as well just stay as
// is...
#![allow(clippy::result_large_err)]

// When not using proc_macro2, we need the built-in proc_macro.
// This is only available when unsynn is used from a proc-macro crate.
#[cfg(not(feature = "proc_macro2"))]
extern crate proc_macro;

pub mod CHANGELOG {
    #![allow(non_snake_case)]
    #![allow(clippy::doc_markdown)]
    #![doc = include_str!("../CHANGELOG.md")]
}

// TokenIter
mod token_iter;
#[doc(inline)]
pub use token_iter::*;

/// The `Parser` trait that must be implemented by anything we want to parse. We are parsing
/// over a [`TokenIter`] ([`TokenStream`] iterator).
pub trait Parser
where
    Self: Sized,
{
    /// The actual parsing function that must be implemented. This mutates the `tokens`
    /// iterator directly. It should not be called from user code except for implementing
    /// parsers itself and then only when the rules below are followed.
    ///
    /// # Implementing Parsers
    ///
    /// The parsers for [`TokenStream`], [`TokenTree`], [`Group`], [`Ident`], [`Punct`],
    /// [`Literal`], [`Except`] and [`Nothing`] (and few more) are the fundamental parsers.
    /// Any other parser is composed from those.
    ///
    /// Calling another `T::parser()` implementation is only valid when this is a conjunctive
    /// operation and a failure is returned immediately by the `?` operator. This can be used
    /// as performance optimization. Any other call to a parser must be done within a transaction.
    /// Otherwise the iterator will be left in a consumed state which breaks further parsing.
    ///
    /// Transactions can be done by calling [`Parse::parse()`] or with the
    /// [`Transaction::transaction()`] method on the iterator.
    ///
    /// # Errors
    ///
    /// The `parser()` implementation must return an error when it cannot parse the
    /// input. This error must be a [`Error`]. User code will parse a grammar by calling
    /// [`Parse::parse_all()`], [`Parse::parse()`] or [`Parse::parse_with()`] which will call
    /// this method within a transaction and roll back on error.
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parser(tokens: &mut TokenIter) -> Result<Self>;
}

/// This trait provides the user facing API to parse grammatical entities. It is implemented
/// for anything that implements the [`Parser`] trait. The methods here encapsulating the
/// iterator that is used for parsing into a transaction. This iterator is always
/// `Clone`. Instead using a peekable iterator or implementing deeper peeking, parse clones
/// this iterator to make access transactional, when parsing succeeds then the transaction
/// becomes committed, otherwise it is rolled back.
///
/// This trait cannot be implemented by user code.
pub trait Parse: Parser {
    /// This is the user facing API to parse grammatical entities. Calls a `parser()` within a
    /// transaction. Commits changes on success and returns the parsed value.
    ///
    /// # Errors
    ///
    /// When the parser returns an error the transaction is rolled back and the error is
    /// returned.
    #[inline]
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parse(tokens: &mut TokenIter) -> Result<Self> {
        tokens.transaction(Self::parser)
    }

    /// Exhaustive parsing within a transaction. This is a convenience method that implies a
    /// `EndOfStream` at the end. Thus it will error if parsing is not exhaustive.
    ///
    /// # Errors
    ///
    /// When the parser returns an error or there are tokens left in the stream the
    /// transaction is rolled back and a error is returned.
    #[inline]
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parse_all(tokens: &mut TokenIter) -> Result<Self> {
        tokens
            .transaction(Cons::<Self, EndOfStream>::parser)
            .map(|result| result.first)
    }

    /// Parse a value in a transaction, pass it to a
    /// `FnOnce(Self, &mut TokenIter) -> Result<T>` closure which
    /// creates a new result or returns an Error.
    ///
    /// This method is a very powerful tool as it allows anything from simple validations to
    /// complete transformations into a new type. You may find this useful to implement
    /// parsers for complex types that need some runtime logic.
    ///
    /// The closures first argument is the parsed value and the second argument is the
    /// transactional iterator pointing after parsing `Self`. This can be used to create
    /// errors or parse further. In many cases it can be ignored with `_`.
    ///
    /// # Using with the `unsynn!` macro
    ///
    /// The [`unsynn!`] macro provides convenient syntax sugar for this method via the `parse_with`
    /// clause. See the [macro documentation](crate::unsynn#custom-parsing-with-parse_with) for details.
    ///
    /// ```rust
    /// # use unsynn::*;
    /// unsynn! {
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
    /// # Example
    ///
    /// ```rust
    /// # use unsynn::*;
    /// # use std::collections::BTreeSet;
    /// // A parser that parses a comma delimited list of anything but commas
    /// // and stores these lexical sorted.
    /// struct OrderedStrings {
    ///     strings: Vec<String>
    /// }
    ///
    /// impl Parser for OrderedStrings {
    ///     fn parser(tokens: &mut TokenIter) -> Result<Self> {
    ///         // Our input is CommaDelimitedVec<String>, we'll transform that into
    ///         // OrderedStrings.
    ///         Parse::parse_with(tokens, |this : CommaDelimitedVec<String>, _| -> Result<OrderedStrings> {
    ///             let mut strings: Vec<String> = this.into_iter()
    ///                 .map(|s| s.value)
    ///                 .collect();
    ///             strings.sort();
    ///             Ok(OrderedStrings { strings })
    ///         })
    ///     }
    /// }
    /// let mut input = "a, d, b, e, c,".to_token_iter();
    /// let ordered_strings: OrderedStrings = input.parse().unwrap();
    /// assert_eq!(ordered_strings.strings, vec!["a", "b", "c", "d", "e"]);
    /// ```
    ///
    /// # Errors
    ///
    /// When the parser or the closure returns an error, the transaction is rolled back and
    /// the error is returned.
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parse_with<T>(
        tokens: &mut TokenIter,
        f: impl FnOnce(Self, &mut TokenIter) -> Result<T>,
    ) -> Result<T> {
        tokens.transaction(|tokens| {
            let result = Self::parser(tokens)?;
            f(result, tokens)
        })
    }
}

/// Parse is implemented for anything that implements [`Parser`].
impl<T: Parser> Parse for T {}

/// unsynn defines its own [`ToTokens`] trait to be able to implement it for std container types.
/// This is similar to the `ToTokens` from the quote crate but adds some extra methods and is
/// implemented for more types. Moreover the `to_token_iter()` method is the main entry point
/// for crating an iterator that can be used for parsing.
///
/// # Using with the `unsynn!` macro
///
/// The [`unsynn!`] macro provides convenient syntax sugar for customizing token emission via the
/// `to_tokens` clause. See the [macro documentation](crate::unsynn#custom-token-emission-with-to_tokens)
/// for details.
///
/// ```rust
/// # use unsynn::*;
/// unsynn! {
///     struct BoolKeyword(bool);
///     to_tokens |s, tokens| {
///         let keyword = if s.0 { "true" } else { "false" };
///         Ident::new(keyword, Span::call_site()).to_tokens(tokens);
///     };
/// }
/// ```
pub trait ToTokens {
    /// Write `&self` to the given [`TokenStream`].
    ///
    /// This is the core method that needs to be implemented. All other methods in this trait
    /// have default implementations based on this method.
    ///
    /// # Using with the `unsynn!` macro
    ///
    /// The [`unsynn!`] macro's `to_tokens` clause provides syntax sugar for implementing this
    /// method. See [`unsynn!` documentation](crate::unsynn#custom-token-emission-with-to_tokens).
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn to_tokens(&self, tokens: &mut TokenStream);

    /// Convert `&self` into a [`TokenIter`] object.
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn to_token_iter(&self) -> TokenIter {
        TokenIter::new(self.to_token_stream())
    }

    /// Convert `self` into a [`TokenIter`] object.
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn into_token_iter(self) -> TokenIter
    where
        Self: Sized,
    {
        TokenIter::new(self.into_token_stream())
    }

    /// Convert `&self` into a [`TokenStream`] object.
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn to_token_stream(&self) -> TokenStream {
        let mut tokens = TokenStream::new();
        self.to_tokens(&mut tokens);
        tokens
    }

    /// Convert `self` into a [`TokenStream`] object.
    #[inline]
    #[mutants::skip]
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn into_token_stream(self) -> TokenStream
    where
        Self: Sized,
    {
        self.to_token_stream()
    }

    /// Convert `&self` into a [`String`] object.  This is mostly used in the test suite to
    /// compare the outputs.  When the input is a `&str` then this parses it and returns a
    /// normalized [`String`].
    #[inline]
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn tokens_to_string(&self) -> String {
        self.to_token_stream().to_string()
    }
}

// Full circle
impl ToTokens for TokenIter {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.clone());
    }
}

/// `ToTokens` for arrays and slices
///
/// # Example
///
/// ```rust
/// use unsynn::*;
/// let arr: [Ident; 3] = [
///     Ident::new("a", Span::call_site()),
///     Ident::new("b", Span::call_site()),
///     Ident::new("c", Span::call_site())
/// ];
/// let mut tokens = TokenStream::new();
/// arr.to_tokens(&mut tokens);
/// assert_eq!(tokens.to_string(), "a b c");
/// # let vec = vec![Ident::new("a", Span::call_site()), Ident::new("b", Span::call_site()), Ident::new("c", Span::call_site())];
/// # let mut tokens = TokenStream::new();
/// # vec[1..3].to_tokens(&mut tokens);
/// # assert_eq!(tokens.to_string(), "b c");
/// ```
impl<T: ToTokens> ToTokens for [T] {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for element in self {
            element.to_tokens(tokens);
        }
    }
}

/// implement `Display` using `ToTokens::tokens_to_string()` for all types that implement `ToTokens`
impl std::fmt::Display for dyn ToTokens {
    #[mutants::skip]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.tokens_to_string())
    }
}

/// Extension trait for [`TokenIter`] that calls [`Parse::parse()`].
#[allow(clippy::missing_errors_doc)]
pub trait IParse: private::Sealed {
    /// Parse a value from the iterator. This is a convenience method that calls
    /// [`Parse::parse()`].
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parse<T: Parse>(self) -> Result<T>;

    /// Parse a value from the iterator. This is a convenience method that calls
    /// [`Parse::parse_all()`].
    #[cfg_attr(feature = "trait_methods_track_caller", track_caller)]
    fn parse_all<T: Parse>(self) -> Result<T>;
}

impl private::Sealed for &mut TokenIter {}

/// Implements [`IParse`] for [`&mut TokenIter`]. This API is more convenient in cases where the
/// compiler can infer types because no turbofish notations are required.
///
/// # Example
///
/// ```rust
/// # use unsynn::*;
///
/// struct MyStruct {
///     number: LiteralInteger,
///     name:   Ident,
/// }
///
/// fn example() -> Result<MyStruct> {
///     let mut input = " 1234 name ".to_token_iter();
///     Ok(
///         MyStruct {
///             // types are inferred here
///             number: input.parse()?,
///             name: input.parse()?
///         }
///     )
/// }
/// ```
impl IParse for &mut TokenIter {
    #[inline]
    fn parse<T: Parse>(self) -> Result<T> {
        T::parse(self)
    }

    #[inline]
    fn parse_all<T: Parse>(self) -> Result<T> {
        T::parse_all(self)
    }
}

/// Helper trait to make [`TokenIter`] transactional
pub trait Transaction: Clone {
    /// Transaction on a [`TokenIter`], calls a `FnOnce(&mut TokenIter) -> Result<T>` within a
    /// transaction. When the closure succeeds, then the transaction is committed and its result
    /// is returned.
    ///
    /// # Errors
    ///
    /// When the closure returns an error, the transaction is rolled back and the error
    /// is returned.
    fn transaction<R>(&mut self, f: impl FnOnce(&mut Self) -> Result<R>) -> Result<R> {
        let mut ttokens = self.clone();
        #[allow(clippy::manual_inspect)] // not pre 1.81
        f(&mut ttokens).map(|result| {
            *self = ttokens;
            result
        })
    }
}

impl Transaction for TokenIter {}

// Result and error type
mod error;
pub use error::*;

// various declarative macros
mod macros;

// Parsers for the `proc_macro2` entities and other fundamental types
pub mod fundamental;
#[doc(inline)]
pub use fundamental::*;

// Groups by explicit bracket types
pub mod group;
#[doc(inline)]
pub use group::*;

// Punctuation, delimiters
pub mod punct;
#[doc(inline)]
pub use punct::*;

// operators
pub mod operator;
#[doc(inline)]
pub use operator::{names::*, *};

// Literals
pub mod literal;
#[doc(inline)]
pub use literal::*;

// Parse into certain rust types
pub mod rust_types;
#[doc(inline)]
/* is this a bug in the linter when the module only implements traits? */
//#[expect(unused_imports)] // don't want to bump msrv to 1.81 just for this
#[allow(unused_imports)]
pub use rust_types::*;

// Delimited sequences
pub mod delimited;
#[doc(inline)]
pub use delimited::*;

// containers and smart pointers
pub mod container;
#[doc(inline)]
pub use container::*;

// combinators
pub mod combinator;
#[doc(inline)]
pub use combinator::*;

// parse time transformers
pub mod transform;
#[doc(inline)]
pub use transform::*;

// dynamic transformers
pub mod dynamic;
#[doc(inline)]
pub use dynamic::*;

// expression parser building blocks
pub mod expressions;
#[doc(inline)]
pub use expressions::*;

// Parse predicates for compile-time parser control
pub mod predicates;
#[doc(inline)]
pub use predicates::*;

// helpers for the keyword macro
#[doc(hidden)]
pub mod keyword_group;
pub use keyword_group::*;

// debug utilities
pub mod debug;
#[doc(inline)]
pub use debug::*;

/// `unsynn` reexports the entities from `proc_macro2` it implements `Parse` and `ToTokens` for.
#[cfg(feature = "proc_macro2")]
pub use proc_macro2::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree,
};

/// `unsynn` reexports the entities from `proc_macro` it implements `Parse` and `ToTokens` for.
#[cfg(not(feature = "proc_macro2"))]
pub use proc_macro::{
    Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenStream, TokenTree,
};

mod private {
    pub trait Sealed {}
}

/// Helper macro that asserts that two entities implementing `ToTokens` result in the same
/// `TokenStream`. Used in tests to ensure that the output of parsing is as expected.  This
/// macro allows two forms:
///
///  * The first form takes two expressions, both expressions are converted into canonical
///    strings with `.tokens_to_string()` to be compared.
///  * The second form takes a string literal prefixed with `str` as second parameter. This
///    string literal is then taken literally for the comparison.
///
/// The later form is used for testing `Joint` punctuation and whitespace placement.
#[macro_export]
macro_rules! assert_tokens_eq {
    ($a:expr, $b:expr$(, $($arg:tt)*)?) => {
        assert_eq!($a.tokens_to_string(), $b.tokens_to_string() $(, $($arg)*)?);
    };
    ($a:expr, str $b:literal$(, $($arg:tt)*)?) => {
        assert_eq!($a.tokens_to_string(), $b $(, $($arg)*)?);
    };
}
