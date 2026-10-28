//! This module contains the fundamental parsers. These are the basic tokens from
//! [`proc_macro2`](https://docs.rs/proc-macro2/latest/proc_macro2/)/[`proc_macro`](https://doc.rust-lang.org/proc_macro/index.html)
//! and a few other ones defined by unsynn. These are the terminal entities when parsing tokens.
//! Being able to parse [`TokenTree`] and [`TokenStream`] allows one to parse opaque entities where
//! internal details are left out. The [`Cached`] type is used to cache the string representation
//! of the parsed entity. The [`Nothing`] type is used to match without consuming any tokens.
//! The [`Except`] type is used to match when the next token does not match the given type.
//! The [`EndOfStream`] type is used to match the end of the stream when no tokens are left.
//! The [`HiddenState`] type is used to hold additional information that is not part of the parsed syntax.
//!
//! **Note**: When the `proc_macro2` feature is disabled, format macros (`format_ident!`,
//! `format_literal!`) are unavailable, but `Cached<T>` remains fully functional using
//! `.to_string()` from `proc_macro` types.

#[cfg(feature = "proc_macro2")]
pub use proc_macro2::{Group, Ident, Literal, Punct, TokenStream, TokenTree};

#[cfg(not(feature = "proc_macro2"))]
pub use proc_macro::{Group, Ident, Literal, Punct, TokenStream, TokenTree};

#[allow(clippy::wildcard_imports)]
use crate::*;

use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// Helper function to count only the tokens INSIDE groups (not the groups themselves).
/// This is used to adjust the shadow counter after `extend()` which only counts outer-level tokens.
fn count_nested_tokens(stream: &TokenStream) -> usize {
    stream
        .clone()
        .into_iter()
        .map(|tt| match tt {
            // For a group, count all tokens inside it recursively
            TokenTree::Group(g) => count_tokens_recursive(g.stream()),
            _ => 0, // Non-group tokens are already counted by extend()
        })
        .sum()
}

/// Helper function to recursively count all tokens in a `TokenStream`, including nested groups.
/// A Group token counts as 1, plus all tokens inside it (recursively).
pub(crate) fn count_tokens_recursive(stream: TokenStream) -> usize {
    stream
        .into_iter()
        .map(|tt| match tt {
            TokenTree::Group(g) => 1 + count_tokens_recursive(g.stream()),
            _ => 1,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_recursive_basic() {
        let stream: TokenStream = "a b c".parse().unwrap();
        assert_eq!(count_tokens_recursive(stream), 3);
    }

    #[test]
    fn test_count_tokens_recursive_with_group() {
        let stream: TokenStream = "a { b c } d".parse().unwrap();
        // a(1) + Group(1) + b(1) + c(1) + d(1) = 5
        assert_eq!(count_tokens_recursive(stream), 5);
    }

    #[test]
    fn test_count_tokens_recursive_nested_groups() {
        // "a { b { c } d } e"
        // Outer: a, Group, e = 3
        // First group: b, Group, d = 3
        // Second group: c = 1
        // Total: 3 + 3 + 1 = 7
        let stream: TokenStream = "a { b { c } d } e".parse().unwrap();
        assert_eq!(count_tokens_recursive(stream), 7);
    }

    #[test]
    fn test_count_tokens_recursive_empty() {
        let stream: TokenStream = "".parse().unwrap();
        assert_eq!(count_tokens_recursive(stream), 0);
    }

    #[test]
    fn test_count_tokens_recursive_empty_group() {
        // "a { } b"
        // Outer: a, Group, b = 3
        // Inside group: 0
        // Total: 3
        let stream: TokenStream = "a { } b".parse().unwrap();
        assert_eq!(count_tokens_recursive(stream), 3);
    }

    #[test]
    fn test_count_tokens_recursive_multiple_groups() {
        // "{ a } { b } { c }"
        // Outer: Group, Group, Group = 3
        // Groups: a, b, c = 3
        // Total: 6
        let stream: TokenStream = "{ a } { b } { c }".parse().unwrap();
        assert_eq!(count_tokens_recursive(stream), 6);
    }

    #[test]
    fn test_count_nested_tokens() {
        let stream: TokenStream = "a { b c } d".parse().unwrap();
        // Only count tokens INSIDE groups: b(1) + c(1) = 2
        assert_eq!(count_nested_tokens(&stream), 2);
    }

    #[test]
    fn test_count_nested_tokens_nested() {
        let stream: TokenStream = "a { b { c } d } e".parse().unwrap();
        // Inside first group: b(1) + Group(1) + c(1) + d(1) = 4
        assert_eq!(count_nested_tokens(&stream), 4);
    }

    #[test]
    fn test_count_nested_tokens_empty() {
        let stream: TokenStream = "a b c".parse().unwrap();
        // No groups, so no nested tokens
        assert_eq!(count_nested_tokens(&stream), 0);
    }

    #[test]
    fn test_count_nested_tokens_empty_group() {
        let stream: TokenStream = "a { } b".parse().unwrap();
        // Empty group contains 0 tokens
        assert_eq!(count_nested_tokens(&stream), 0);
    }

    #[test]
    fn test_count_nested_tokens_multiple_groups() {
        // "{ a } { b } { c }"
        // Inside groups: a, b, c = 3
        let stream: TokenStream = "{ a } { b } { c }".parse().unwrap();
        assert_eq!(count_nested_tokens(&stream), 3);
    }
}

/// Parses a [`TokenStream`] from the input tokens. This is the primary entity to parse when
/// dealing with opaque entities where internal details are left out.
/// Note that this matches a empty stream (see [`EndOfStream`]) as well.
impl Parser for TokenStream {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut output = TokenStream::new();
        output.extend(&mut *tokens);

        // Count tokens INSIDE groups and adjust the shadow counter
        // `extend()` already counted outer-level tokens (a, Group, b)
        // but didn't count tokens inside the Groups
        let nested_count = count_nested_tokens(&output);
        tokens.add(nested_count);

        Ok(output)
    }
}

impl ToTokens for TokenStream {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.clone());
    }
}

/// Since parsing a [`TokenStream`] succeeds even when no tokens are left, this type is used to
/// parse a [`TokenStream`] that is not empty.
pub struct NonEmptyTokenStream(pub TokenStream);

impl TryFrom<TokenStream> for NonEmptyTokenStream {
    type Error = Error;

    fn try_from(value: TokenStream) -> Result<Self> {
        if value.is_empty() {
            Error::unexpected_end()
        } else {
            Ok(Self(value))
        }
    }
}

impl Parser for NonEmptyTokenStream {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        tokens.parse::<Expect<TokenTree>>().refine_err::<Self>()?;
        // A TokenStream will always match, so we can safely unwrap here.
        #[allow(clippy::unwrap_used)]
        Ok(Self(TokenStream::parser(tokens).unwrap()))
    }
}

impl ToTokens for NonEmptyTokenStream {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(self.0.clone());
    }
}

#[test]
#[cfg(feature = "proc_macro2")]
fn test_non_empty_token_stream() {
    let mut token_iter = "ident".to_token_iter();
    let _ = NonEmptyTokenStream::parser(&mut token_iter).unwrap();
}

#[test]
#[cfg(feature = "proc_macro2")]
fn test_empty_token_stream() {
    let mut token_iter = "".to_token_iter();
    assert!(NonEmptyTokenStream::parser(&mut token_iter).is_err());
}

impl Parser for TokenTree {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            Some(token) => Ok(token),
            None => Error::unexpected_end(),
        }
    }
}

impl ToTokens for TokenTree {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(std::iter::once(self.clone()));
    }
}

impl Parser for Group {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            Some(TokenTree::Group(group)) => {
                // Count tokens inside the group and advance the token counter
                let nested_count = count_tokens_recursive(group.stream());
                tokens.add(nested_count);
                Ok(group)
            }
            at => Error::unexpected_token(at, tokens),
        }
    }
}

impl ToTokens for Group {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(std::iter::once(TokenTree::Group(self.clone())));
    }
}

impl Parser for Ident {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            Some(TokenTree::Ident(ident)) => Ok(ident),
            at => Error::unexpected_token(at, tokens),
        }
    }
}

impl ToTokens for Ident {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(std::iter::once(TokenTree::Ident(self.clone())));
    }
}

impl Parser for Punct {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            Some(TokenTree::Punct(punct)) => Ok(punct),
            at => Error::unexpected_token(at, tokens),
        }
    }
}

impl ToTokens for Punct {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(std::iter::once(TokenTree::Punct(self.clone())));
    }
}

impl Parser for Literal {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            Some(TokenTree::Literal(literal)) => Ok(literal),
            at => Error::unexpected_token(at, tokens),
        }
    }
}

impl ToTokens for Literal {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        tokens.extend(std::iter::once(TokenTree::Literal(self.clone())));
    }
}

/// Getting the underlying string expensive as it always allocates a new [`String`].
/// This type caches the string representation of a given entity. Note that this is
/// only reliable for fundamental entities that represent a single token. Spacing between
/// composed tokens is not stable and should be considered informal only.
///
/// # Example
///
/// ```
/// use unsynn::*;
/// let mut token_iter = "ident 1234".to_token_iter();
///
/// let cached_ident = Cached::<Ident>::parse(&mut token_iter).unwrap();
/// assert!(cached_ident == "ident");
/// ```
#[derive(Clone)]
pub struct Cached<T> {
    value: T,
    string: String,
}

impl<T: Parse + ToTokens> Parser for Cached<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let value = T::parser(tokens).refine_err::<Self>()?;
        let string = value.tokens_to_string();
        Ok(Self { value, string })
    }
}

impl<T: Parse + ToTokens> ToTokens for Cached<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.value.to_tokens(tokens);
    }
}

impl<T: Parse + ToTokens> Cached<T> {
    /// Sets the value and updates the string representation.
    pub fn set(&mut self, value: T) {
        self.value = value;
        self.string = self.value.tokens_to_string();
    }
}

impl<T: Parse> Cached<T> {
    /// Deconstructs self and returns the inner value.
    pub fn into_inner(self) -> T {
        self.value
    }

    /// Deconstructs self and returns the contained `String` representation.
    pub fn into_string(self) -> String {
        self.string
    }

    /// Gets the cached string representation
    #[allow(clippy::missing_const_for_fn)] // bug in clippy
    pub fn as_str(&self) -> &str {
        &self.string
    }
}

#[cfg(feature = "proc_macro2")]
impl<T: Parse> Cached<T> {
    /// Creates a new `Cached<T>` from a `&str`.
    ///
    /// # Panics
    ///
    /// Panics when `s` can't be parsed.
    ///
    /// # Example
    ///
    /// ```
    /// use unsynn::*;
    /// let cached_ident = Cached::<Ident>::new("ident");
    /// assert!(cached_ident == "ident");
    /// ```
    #[must_use]
    pub fn new(s: &str) -> Self {
        let value = s.into_token_iter().parse().expect("Valid token");
        Self {
            value,
            string: s.to_string(),
        }
    }

    /// Creates a new `Cached<T>` from a owned `String`.
    ///
    /// # Errors
    ///
    /// Returns `Err` when `s` can't be parsed.
    ///
    /// # Example
    ///
    /// ```
    /// use unsynn::*;
    /// let cached_ident = Cached::<Ident>::from_string("ident".into()).unwrap();
    /// assert!(cached_ident == "ident");
    /// ```
    pub fn from_string(s: String) -> Result<Self> {
        let value = s.to_token_iter().parse()?;
        Ok(Self { value, string: s })
    }
}

impl<T: Parse> Deref for Cached<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Parse> PartialEq<&str> for Cached<T> {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl<T: Parse> PartialEq for Cached<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl<T: Parse> Eq for Cached<T> {}

impl<T: Parse> std::hash::Hash for Cached<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl<T: Parse> AsRef<T> for Cached<T> {
    fn as_ref(&self) -> &T {
        &self.value
    }
}

impl<T: Parse> AsRef<str> for Cached<T> {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

#[mutants::skip]
impl<T: Parse + std::fmt::Debug> std::fmt::Debug for Cached<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Cached<{}>", std::any::type_name::<T>()))
            .field("value", &self.value)
            .field("string", &self.string)
            .finish()
    }
}

/// Convert a `Cached<T: Into<TokenTree>>` object into a `TokenTree`.
impl<T: Into<TokenTree>> From<Cached<T>> for TokenTree {
    fn from(cached: Cached<T>) -> Self {
        cached.value.into()
    }
}

#[cfg(feature = "proc_macro2")]
impl<T: Parse> TryFrom<String> for Cached<T> {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        let mut token_iter = value.to_token_iter();
        let t = T::parser(&mut token_iter).refine_err::<Self>()?;
        Ok(Self {
            value: t,
            string: value,
        })
    }
}

#[cfg(feature = "proc_macro2")]
impl<T: Parse> TryFrom<&str> for Cached<T> {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::try_from(value.to_string())
    }
}

#[test]
#[cfg(feature = "proc_macro2")]
fn test_cached_into_tt() {
    let mut token_iter = "ident".to_token_iter();
    let ident = Cached::<Ident>::parser(&mut token_iter).unwrap();
    let _: TokenTree = ident.into();
}

macro_rules! gen_cached_types {
    ($($cached:ident = $basic:ident);* $(;)?) => {
        $(
        #[doc = concat!("[`", stringify!($basic), "`] with cached string representation.")]
        pub type $cached = Cached<$basic>;

        #[doc = concat!("Convert `", stringify!($cached), " into a `", stringify!($basic), "`.")]
        impl From<$cached> for $basic {
            fn from(cached: $cached) -> Self {
                cached.value
            }
        }
        )*
    }
}

gen_cached_types! {
    CachedGroup = Group;
    CachedIdent = Ident;
    CachedPunct = Punct;
    CachedLiteral = Literal;
    CachedLiteralString = LiteralString;
    CachedLiteralInteger = LiteralInteger;
}

// cant use the macro, TokenTree conversion is generic over T defined above
/// [`TokenTree`] (any token) with cached string representation.
pub type CachedTokenTree = Cached<TokenTree>;

/// Generates a `Ident` from a format specification.
///
/// # Panics
///
/// Panics when the formatted string is not a valid identifier.
///
/// # Example
///
/// ```
/// use unsynn::*;
/// let ident = format_ident!("my_{}", "identifier");
/// assert_tokens_eq!(ident, "my_identifier");
/// ```
#[cfg(feature = "proc_macro2")]
#[macro_export]
macro_rules! format_ident {
    ($($args:tt)*) => {
        <$crate::Ident as $crate::Parse>::parse(&mut format!($($args)*).into_token_iter()).expect("Not a valid identifier")
    };
}

/// Generates a `CachedIdent` from a format specification.
///
/// # Panics
///
/// Panics when the formatted string is not a valid identifier.
///
/// # Example
///
/// ```
/// use unsynn::*;
/// let cached_ident = format_cached_ident!("my_{}", "identifier");
/// assert_tokens_eq!(cached_ident, "my_identifier");
/// ```
#[cfg(feature = "proc_macro2")]
#[macro_export]
macro_rules! format_cached_ident {
    ($($args:tt)*) => {
        $crate::CachedIdent::from_string(format!($($args)*)).expect("Not a valid identifier")
    };
}

/// Generates a `LiteralString` from a format specification. Quote characters around the
/// string are automatically added.
///
/// # Panics
///
/// Panics when the formatted string is not a valid literal string.
///
/// # Example
///
/// ```
/// use unsynn::*;
/// let literal_string = format_literal_string!("my_{}", "literal_string");
/// assert_tokens_eq!(literal_string, r#" "my_literal_string" "#);
/// ```
#[cfg(feature = "proc_macro2")]
#[macro_export]
macro_rules! format_literal_string {
    ($fmt:literal $(, $($args:tt)*)?) => {
        <$crate::LiteralString as $crate::Parse>::parse(&mut format!(concat!("\"",$fmt,"\"") $(, $($args)*)?)
            .into_token_iter())
        .expect("Not a valid string literal")
    };
}

/// Generates a `Literal` from a format specification. Unlike [`format_literal_string!`], this does not
/// add quotes and can be used to create any kind of literal, such as integers or floats.
///
/// # Panics
///
/// Panics when the formatted string is not a valid literal.
///
/// # Example
///
/// ```
/// use unsynn::*;
/// let literal = format_literal!("123{}", ".456");
/// assert_tokens_eq!(literal, str "123.456");
/// ```
#[cfg(feature = "proc_macro2")]
#[macro_export]
macro_rules! format_literal{
    ($($args:tt)*) => {
        <$crate::Literal as $crate::Parse>::parse(&mut format!($($args)*)
            .into_token_iter())
        .expect("Not a valid literal")
    };
}

/// A unit that always matches without consuming any tokens.  This is required when one wants
/// to parse a [`Repeats`] without a delimiter.  Note that using [`Nothing`] as primary entity
/// in a [`Vec`], [`LazyVec`], [`DelimitedVec`] or [`Repeats`] will result in an infinite
/// loop.
#[derive(Debug, Clone, Default)]
pub struct Nothing;

impl Parser for Nothing {
    #[inline]
    #[mutants::skip]
    fn parser(_tokens: &mut TokenIter) -> Result<Self> {
        Ok(Self)
    }
}

impl ToTokens for Nothing {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

/// A unit that always fails to match. This is useful as default for generics.
/// See how [`Either<A, B, C, D>`] uses this for unused alternatives.
///
/// # Panics
///
/// `Invalid` tokens can not be emitted and will panic when calling [`ToTokens::to_tokens()`].
#[derive(Debug, Clone)]
pub struct Invalid;

impl Parser for Invalid {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Error::unexpected_token(None, tokens)
    }
}

impl ToTokens for Invalid {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        unimplemented!("`Invalid` can not be converted to tokens")
    }
}

/// A unit that can not be parsed. This is useful as diagnostic placeholder for parsers that
/// are (yet) unimplemented. The `nonparseable` feature flag controls if `Parser` and `ToTokens`
/// will be implemented for it. This is useful in release builds that should not have any
/// `NonParseable` left behind.
///
///
/// # Panics
///
/// Only when the `nonparseable` feature flag is set:
///
/// * `NonParseable` will panic when calling [`Parser::parser()`].
/// * `NonParseable` tokens can not be emitted and will panic when calling [`ToTokens::to_tokens()`].
///
/// Otherwise `Parser` and `ToTokens` are not implemented and will result in a compile time error.
///
/// # Example
///
/// ```should_panic
/// # use unsynn::*;
/// let mut tokens = "something".to_token_iter();
/// let nonparseable: NonParseable = tokens.parse().unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct NonParseable;

#[cfg(feature = "nonparseable")]
impl Parser for NonParseable {
    #[inline]
    fn parser(_tokens: &mut TokenIter) -> Result<Self> {
        unimplemented!("`NonParseable` can not be parsed")
    }
}

#[cfg(feature = "nonparseable")]
impl ToTokens for NonParseable {
    #[mutants::skip]
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        unimplemented!("`NonParseable` can not be converted to tokens")
    }
}

/// Succeeds when the next token does not match `T`. **Will not consume any tokens.** Usually
/// this has to be followed with a conjunctive match such as `Cons<Except<T>, U>` or followed
/// by another entry in a struct or tuple.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "ident".to_token_iter();
///
/// let _ = Except::<Punct>::parser(&mut token_iter).unwrap();
/// ```
#[derive(Clone)]
pub struct Except<T>(PhantomData<T>);

impl<T: Parse> Parser for Except<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut ptokens = tokens.clone();
        match T::parser(&mut ptokens) {
            Ok(_) => Error::unexpected_token(tokens.clone().next(), tokens),
            Err(_) => Ok(Self(PhantomData)),
        }
    }
}

impl<T> ToTokens for Except<T> {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for Except<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Except<{}>", std::any::type_name::<T>()))
            .finish()
    }
}

/// Succeeds when the next token would match `T`. **Will not consume any tokens.**
/// This is similar to peeking.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "ident".to_token_iter();
///
/// let _ = Expect::<Ident>::parser(&mut token_iter).unwrap();
/// ```
#[derive(Clone)]
pub struct Expect<T>(PhantomData<T>);

impl<T: Parse> Parser for Expect<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut ptokens = tokens.clone();
        match T::parser(&mut ptokens) {
            Ok(_) => Ok(Self(PhantomData)),
            Err(e) => Err(e),
        }
    }
}

impl<T> ToTokens for Expect<T> {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for Expect<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Expect<{}>", std::any::type_name::<T>()))
            .finish()
    }
}

/// Matches the end of the stream when no tokens are left.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "".to_token_iter();
///
/// let _end_ = EndOfStream::parser(&mut token_iter).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct EndOfStream;

impl Parser for EndOfStream {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match tokens.next() {
            None => Ok(Self),
            at => Error::unexpected_token(at, tokens),
        }
    }
}

impl ToTokens for EndOfStream {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

/// Sometimes one want to compose types or create structures for unsynn that have members that
/// are not part of the parsed syntax but add some additional information. This struct can be
/// used to hold such members while still using the [`Parser`] and [`ToTokens`] trait
/// implementations automatically generated by the [`unsynn!{}`] macro or composition syntax.
/// [`HiddenState`] will not consume any tokens when parsing and will not emit any tokens when
/// generating a [`TokenStream`]. On parsing it is initialized with a default value. It has
/// [`Deref`] and [`DerefMut`] implemented to access the inner value.
#[derive(Clone)]
pub struct HiddenState<T: Default>(pub T);

impl<T: Default> Deref for HiddenState<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Default> DerefMut for HiddenState<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<T: Default> Parser for HiddenState<T> {
    #[inline]
    #[mutants::skip]
    fn parser(_ctokens: &mut TokenIter) -> Result<Self> {
        Ok(Self(T::default()))
    }
}

impl<T: Default> ToTokens for HiddenState<T> {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

impl<T: Default> Default for HiddenState<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[mutants::skip]
impl<T: Default + std::fmt::Debug> std::fmt::Debug for HiddenState<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("HiddenState<{}>", std::any::type_name::<T>()))
            .field(&self.0)
            .finish()
    }
}
