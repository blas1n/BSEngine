//! This module contains the transforming parsers. This are the parsers that add, remove,
//! replace or reorder Tokens while parsing.
//!
//! **Note**: When the `proc_macro2` feature is disabled, `IntoIdent<T>` is not available
//! because it requires parsing from strings (`&str.into_token_iter()`) which is only
//! available with `proc_macro2`.

#[allow(clippy::wildcard_imports)]
use crate::*;

use std::marker::PhantomData;
use std::ops::Deref;

/// Succeeds when the next token matches `T`. The token will be removed from the stream but not stored.
/// Consequently the `ToTokens` implementations will panic with a message that it can not be emitted.
/// This can only be used when a token should be present but not stored and never emitted.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "ident ()".to_token_iter();
///
/// let _ = Discard::<Ident>::parse(&mut token_iter).unwrap();
/// assert!(ParenthesisGroup::parse(&mut token_iter).is_ok());
/// ```
#[derive(Clone)]
pub struct Discard<T>(PhantomData<T>);

impl<T: Parse> Parser for Discard<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match T::parser(tokens).refine_err::<Self>() {
            Ok(_) => Ok(Self(PhantomData)),
            Err(e) => Err(e),
        }
    }
}

impl<T> ToTokens for Discard<T> {
    #[inline]
    #[mutants::skip]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        unimplemented!("Can not emit tokens for Discard<T>")
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for Discard<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Discard<{}>", std::any::type_name::<T>()))
            .finish()
    }
}

/// Skips over expected tokens. Will parse and consume the tokens but not store them.
/// Consequently the `ToTokens` implementations will not output any tokens.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "ident,".to_token_iter();
///
/// let _ = Skip::<Cons<Ident, Comma>>::parse_all(&mut token_iter).unwrap();
/// ```
#[derive(Clone)]
pub struct Skip<T>(PhantomData<T>);

impl<T: Parse> Parser for Skip<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        T::parser(tokens).refine_err::<Self>()?;
        Ok(Self(PhantomData))
    }
}

impl<T> ToTokens for Skip<T> {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for Skip<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Skip<{}>", std::any::type_name::<T>()))
            .finish()
    }
}

/// Injects tokens without parsing anything.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo bar".to_token_iter();
///
/// let parsed = <Cons<Ident, Insert<Plus>, Ident>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "foo + bar");
/// ```
pub struct Insert<T>(pub T);

impl<T: Default> Parser for Insert<T> {
    #[inline]
    fn parser(_tokens: &mut TokenIter) -> Result<Self> {
        Ok(Self(T::default()))
    }
}

impl<T: ToTokens> ToTokens for Insert<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<T> Deref for Insert<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for Insert<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!("Insert<{}>", std::any::type_name::<T>()))
            .field(&self.0)
            .finish()
    }
}

/// Tries to parse a `T` or inserts a `D` when that fails.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo".to_token_iter();
///
/// let parsed = <OrDefault<u32, Question>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "?");
/// ```
pub type OrDefault<T, D = T> = Either<T, Insert<D>>;

/// Parse-skip a `T` and inserts a `U: Default` in place. This is a zero sized type.
pub type Replace<T, U> = Cons<Skip<T>, Insert<U>>;

/// Parse a `T` and replace it with its default value. This is a zero sized type.  It can be
/// used for no allocation replacement elements in a `Vec` since it has a optimization for
/// zero-sized-types where it wont allocate any memory but just act as counter then.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "..........".to_token_iter();
///
/// let parsed = <Vec<AsDefault<Dot>>>::parser(&mut token_iter).unwrap();
/// assert_eq!(parsed.len(), 10);
/// // Vec's ZST optimization at work:
/// assert_eq!(parsed.capacity(), usize::MAX);
/// assert_tokens_eq!(parsed, ". . . . . . . . . .");
/// ```
pub type AsDefault<T> = Replace<T, T>;

/// Swaps the order of two entities.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo bar".to_token_iter();
///
/// let parsed = <Swap<Ident, Ident>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "bar foo");
/// ```
#[derive(Debug)]
pub struct Swap<A, B>(pub B, pub A);

impl<A: Parse, B: Parse> Parser for Swap<A, B> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let a: A = tokens.parse()?;
        let b: B = tokens.parse()?;
        Ok(Self(b, a))
    }
}

impl<A: ToTokens, B: ToTokens> ToTokens for Swap<A, B> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
        self.1.to_tokens(tokens);
    }
}

/// Parses `T` and creates a `LiteralString` from it. When `T` implements `Default`, such as
/// single string (non group) keywords, operators and `Const*` literals. It can be used to
/// create `IntoLiteralString` on the fly.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo 123".to_token_iter();
///
/// let parsed = <IntoLiteralString<Cons<Ident, LiteralInteger>>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, r#" "foo 123" "#);
///
/// keyword!{Foo = "foo"}
/// let default = <IntoLiteralString<Cons<Foo, ConstInteger<1234>>>>::default();
/// assert_tokens_eq!(default, r#" "foo 1234" "#);
/// ```
#[derive(Debug)]
pub struct IntoLiteralString<T>(LiteralString, PhantomData<T>);

impl<T: ToTokens> IntoLiteralString<T> {
    /// Creates a `IntoLiteralString` from an AST.
    ///
    /// ```
    /// # use unsynn::*;
    /// let mut token_iter = "foo 123".to_token_iter();
    ///
    /// let parsed = <Cons<Ident, LiteralInteger>>::parser(&mut token_iter).unwrap();
    /// let as_string = IntoLiteralString::from(&parsed);
    ///
    /// assert_eq!(as_string.as_str(), "foo 123");
    /// ```
    pub fn from(from: &T) -> Self {
        Self(
            LiteralString::from_str(from.tokens_to_string()),
            PhantomData,
        )
    }
}

impl<T> IntoLiteralString<T> {
    /// Returns the underlying `&str`without its surrounding quotes.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Destructures `IntoLiteralString<T>` to get the inner `LiteralString`.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> LiteralString {
        self.0
    }
}

impl<T: Parse + ToTokens> Parser for IntoLiteralString<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok(Self(
            LiteralString::from_str(tokens.parse::<T>()?.tokens_to_string()),
            PhantomData,
        ))
    }
}

impl<T> ToTokens for IntoLiteralString<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<T: Default + ToTokens> Default for IntoLiteralString<T> {
    fn default() -> Self {
        Self(
            LiteralString::from_str(T::default().tokens_to_string()),
            PhantomData,
        )
    }
}

/// Parses `T` and concats all its elements to a single identifier by removing all characters
/// that are not valid in identifiers.  When `T` implements `Default`, such as single string
/// (non group) keywords, operators and `Const*` literals. Then it can be used to create
/// `IntoIdentifier` on the fly. Note that construction may still fail when one tries to
/// create a invalid identifier such as one starting with digits for example.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo 123".to_token_iter();
///
/// let parsed = <IntoIdent<Cons<Ident, LiteralInteger>>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "foo123");
///
/// keyword!{Foo = "foo"}
/// let default = <IntoIdent<Cons<Foo, ConstInteger<1234>>>>::default();
/// assert_tokens_eq!(default, "foo1234");
/// ```
#[cfg(feature = "proc_macro2")]
#[derive(Debug)]
pub struct IntoIdent<T>(CachedIdent, PhantomData<T>);

#[cfg(feature = "proc_macro2")]
impl<T: ToTokens> IntoIdent<T> {
    /// Creates a `IntoIdent` from an AST.
    ///
    /// # Errors
    ///
    /// This function returns an error when the provided data cannot be parsed as an
    /// identifier.
    ///
    /// ```
    /// # use unsynn::*;
    /// let mut token_iter = r#" foo "123" "#.to_token_iter();
    ///
    /// let parsed = <Cons<Ident, LiteralString>>::parser(&mut token_iter).unwrap();
    /// let ident = IntoIdent::from(&parsed).unwrap();
    ///
    /// assert_eq!(ident.as_str(), "foo123");
    /// ```
    pub fn from(from: &T) -> Result<Self> {
        let mut string = from.tokens_to_string();
        string.retain(|c| c.is_alphanumeric() || c == '_');
        Ok(Self(CachedIdent::from_string(string)?, PhantomData))
    }
}

#[cfg(feature = "proc_macro2")]
impl<T> IntoIdent<T> {
    /// Returns the underlying `&str`without its surrounding quotes.
    #[inline]
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Destructures `IntoIdent<T>` to get the inner `CachedIdent`.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> CachedIdent {
        self.0
    }
}

/// Converts `IntoIdent<T>` into `Ident` by consuming the `IntoIdent<T>`.
#[cfg(feature = "proc_macro2")]
impl<T> From<IntoIdent<T>> for Ident {
    fn from(this: IntoIdent<T>) -> Self {
        this.0.into_inner()
    }
}

#[cfg(feature = "proc_macro2")]
impl<T: Parse + ToTokens> Parser for IntoIdent<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut string = tokens.parse::<T>().refine_err::<Self>()?.tokens_to_string();
        string.retain(|c| c.is_alphanumeric() || c == '_');
        Ok(Self(CachedIdent::from_string(string)?, PhantomData))
    }
}

#[cfg(feature = "proc_macro2")]
impl<T: ToTokens> ToTokens for IntoIdent<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

/// Creates a default constructed `IntoIdent<T>` from `T`
///
/// # Panics
///
/// When the concatenation of `T` does not form a valid `Ident`.
#[cfg(feature = "proc_macro2")]
impl<T: Default + ToTokens> Default for IntoIdent<T> {
    fn default() -> Self {
        let mut string = T::default().tokens_to_string();
        string.retain(|c| c.is_alphanumeric() || c == '_');
        Self(
            CachedIdent::from_string(string).expect("invalid default constructed IntoIdent"),
            PhantomData,
        )
    }
}

/// Parses `T` and keeps it as opaque `TokenStream`. This is useful when one wants to parse a
/// sequence of tokens and keep it as opaque unit or re-parse it later as something else.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo 123".to_token_iter();
///
/// let parsed = <IntoTokenStream<Cons<Ident, LiteralInteger>>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "foo 123");
/// # assert_tokens_eq!(parsed.into_inner(), "foo 123")
/// ```
#[derive(Debug)]
pub struct IntoTokenStream<T>(TokenStream, PhantomData<T>);

impl<T: ToTokens> IntoTokenStream<T> {
    /// Creates a `IntoTokenStream` from an AST.
    pub fn from(from: &T) -> Self {
        Self(from.to_token_stream(), PhantomData)
    }
}

impl<T> IntoTokenStream<T> {
    /// Destructures `IntoTokenStream<T>` to get the inner `TokenStream`.
    #[inline]
    #[must_use]
    pub fn into_inner(self) -> TokenStream {
        self.0
    }
}

impl<T> Deref for IntoTokenStream<T> {
    type Target = TokenStream;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Parse + ToTokens> Parser for IntoTokenStream<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let tokenstream = tokens
            .parse::<T>()
            .refine_err::<Self>()?
            .into_token_stream();
        Ok(Self(tokenstream, PhantomData))
    }
}

impl<T: ToTokens> ToTokens for IntoTokenStream<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

/// Creates a default constructed `IntoTokenStream<T>` from `T`
impl<T: Default + ToTokens> Default for IntoTokenStream<T> {
    fn default() -> Self {
        Self(T::default().into_token_stream(), PhantomData)
    }
}

/// Parses a `TokenStream` until, but excluding `T`. The presence of `T` is mandatory.
///
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo bar ; baz".to_token_iter();
///
/// let parsed = <TokenStreamUntil<Semicolon>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "foo bar");
/// # <TokenStreamUntil<Plus>>::parser(&mut token_iter).unwrap_err();
/// ```
pub type TokenStreamUntil<T> = IntoTokenStream<Cons<Vec<Cons<Except<T>, TokenTree>>, Expect<T>>>;
