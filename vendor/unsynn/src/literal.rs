//! This module provides a set of literal types that can be used to parse and tokenize
//! literals.  The literals are parsed from the token stream and can be used to represent the
//! parsed value. unsynn defines only simplified literals, such as integers, characters and
//! strings. The literals here are not full rust syntax, which will be defined in the
//! `unsynn-rust` crate. There are `Literal*` for `Integer, Character, String` to parse simple
//! literals and `ConstInteger<V>` and `ConstCharacter<V>` who must match an exact character.
//! The later two also implement `Default`, thus they can be used to create constant tokens.
//! There is no `ConstString`; constant literal strings can be constructed with
//! `IntoLiteralString<T>`.

#![allow(clippy::module_name_repetitions)]

#[cfg(doc)]
use crate::*;

use crate::{
    Error, Literal, Parse, Parser, RefineErr, Result, ToTokens, TokenIter, TokenStream, TokenTree,
};

/// A simple unsigned 128 bit integer. This is the most simple form to parse integers. Note
/// that only decimal integers without any other characters, signs or suffixes are supported,
/// this is *not* full rust syntax.
#[derive(Debug, Clone)]
pub struct LiteralInteger {
    /// Literal representing an integer
    literal: Literal,
    /// Value of the integer
    value: u128,
}

impl LiteralInteger {
    /// Create a new `LiteralInteger` from a `u128` value.
    #[must_use]
    pub fn new(value: u128) -> Self {
        let literal = Literal::u128_unsuffixed(value);
        Self { literal, value }
    }

    /// Get the value.
    #[must_use]
    pub const fn value(&self) -> u128 {
        self.value
    }

    /// Set to a new the value.
    pub fn set(&mut self, value: u128) {
        *self = Self {
            literal: Literal::u128_unsuffixed(value),
            value,
        };
    }

    /// Deconstructs `self` and gets the `Literal`
    #[must_use]
    pub fn into_inner(self) -> Literal {
        self.literal
    }
}

impl Parser for LiteralInteger {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        let literal = Literal::parser(tokens).refine_err::<Self>()?;
        let value: u128 = match literal.to_string().parse() {
            Ok(v) => v,
            Err(e) => return Error::dynamic::<Self>(at, tokens, e),
        };
        Ok(Self { literal, value })
    }
}

impl ToTokens for LiteralInteger {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.literal.to_tokens(tokens);
    }
}

impl PartialEq<u128> for LiteralInteger {
    fn eq(&self, other: &u128) -> bool {
        &self.value == other
    }
}

impl From<LiteralInteger> for TokenTree {
    fn from(lit: LiteralInteger) -> Self {
        TokenTree::Literal(lit.into_inner())
    }
}

#[test]
fn test_literalinteger_into_tt() {
    let lit = LiteralInteger::new(42);
    let _: TokenTree = lit.into();
}

/// A constant `u128` integer of value `V`. Must match V and also has `Default` implemented to create
/// a `LiteralInteger` with value `V`.
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo".to_token_iter();
///
/// let parsed = <OrDefault<u32, ConstInteger<1234>>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "1234");
/// ```
#[derive(Debug, Clone)]
pub struct ConstInteger<const V: u128>(LiteralInteger);

impl<const V: u128> ConstInteger<V> {
    /// Get the value.
    #[must_use]
    pub const fn value(&self) -> u128 {
        self.0.value
    }

    /// Deconstructs `self` and gets the `LiteralInteger`
    #[must_use]
    pub fn into_inner(self) -> LiteralInteger {
        self.0
    }
}

impl<const V: u128> Parser for ConstInteger<V> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        Parse::parse_with(tokens, |this: LiteralInteger, e| {
            if this.value == V {
                Ok(Self(this))
            } else {
                Error::unexpected_token(at, e)
            }
        })
        .refine_err::<Self>()
    }
}

impl<const V: u128> ToTokens for ConstInteger<V> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<const V: u128> Default for ConstInteger<V> {
    fn default() -> Self {
        Self(LiteralInteger::new(V))
    }
}

/// A single quoted character literal (`'x'`).
#[derive(Debug, Clone)]
pub struct LiteralCharacter {
    /// Literal representing a single quoted character
    literal: Literal,
    /// The character value
    value: char,
}

impl LiteralCharacter {
    /// Create a new `LiteralCharacter` from a `char` value.
    #[must_use]
    pub fn new(value: char) -> Self {
        let literal = Literal::character(value);
        Self { literal, value }
    }

    /// Get the value.
    #[must_use]
    pub const fn value(&self) -> char {
        self.value
    }

    /// Set to a new value.
    pub fn set(&mut self, value: char) {
        *self = Self {
            literal: Literal::character(value),
            value,
        };
    }

    /// Deconstructs `self` and gets the `Literal`
    #[must_use]
    pub fn into_inner(self) -> Literal {
        self.literal
    }
}

impl Parser for LiteralCharacter {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        let literal = Literal::parser(tokens).refine_err::<Self>()?;
        let string = literal.to_string();
        let mut chars = string.chars();
        // We only need to to check for first single quote, since the lexer already checked
        // for proper literals
        if let (Some('\''), Some(value)) = (chars.next(), chars.next()) {
            Ok(Self { literal, value })
        } else {
            Error::unexpected_token(at, tokens)
        }
    }
}

impl ToTokens for LiteralCharacter {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.literal.to_tokens(tokens);
    }
}

impl PartialEq<char> for LiteralCharacter {
    fn eq(&self, other: &char) -> bool {
        &self.value == other
    }
}

impl From<LiteralCharacter> for TokenTree {
    fn from(lit: LiteralCharacter) -> Self {
        TokenTree::Literal(lit.into_inner())
    }
}

#[test]
fn test_literalcharacter_into_tt() {
    let lit = LiteralCharacter::new('c');
    let _: TokenTree = lit.into();
}

/// A constant `char` of value `V`. Must match V and also has `Default` implemented to create
/// a `LiteralCharacter` with value `V`.
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "'f'".to_token_iter();
///
/// let parsed = <OrDefault<u32, ConstCharacter<'f'>>>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "'f'");
/// ```
#[derive(Debug, Clone)]
pub struct ConstCharacter<const V: char>(LiteralCharacter);

impl<const V: char> ConstCharacter<V> {
    /// Get the value.
    #[must_use]
    pub const fn value(&self) -> char {
        self.0.value
    }

    /// Deconstructs `self` and gets the `Literal`
    #[must_use]
    pub fn into_inner(self) -> LiteralCharacter {
        self.0
    }
}

impl<const V: char> Parser for ConstCharacter<V> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        Parse::parse_with(tokens, |this: LiteralCharacter, e| {
            if this.value == V {
                Ok(Self(this))
            } else {
                Error::unexpected_token(at, e)
            }
        })
        .refine_err::<Self>()
    }
}

impl<const V: char> ToTokens for ConstCharacter<V> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<const V: char> Default for ConstCharacter<V> {
    fn default() -> Self {
        Self(LiteralCharacter::new(V))
    }
}

/// A double quoted string literal (`"hello"`). The quotes are included in the value.  Note
/// that this is a simplified string literal, and only double quoted strings are supported,
/// this is *not* full rust syntax, eg. byte and C string literals are not supported.
#[derive(Debug, Clone)]
pub struct LiteralString {
    /// Literal representing a double quoted string
    literal: Literal,
    /// The string value
    value: String,
}

impl LiteralString {
    /// Create a new `LiteralString` from a `String` value. The supplied `String` must start
    /// and end with a double quote.
    ///
    /// # Panics
    ///
    /// Panics if the string does not start and end with a double quote.
    #[must_use]
    pub fn new(value: String) -> Self {
        assert!(value.starts_with('"') && value.ends_with('"'));
        let literal = Literal::string(&value);
        Self { literal, value }
    }

    /// Create a new `LiteralString` from any `AsRef<str>` slice. Adds double quotes around
    /// the supplied string.
    #[must_use]
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(string: impl AsRef<str>) -> Self {
        let string = string.as_ref();
        let value = format!(r#""{string}""#);
        let literal = Literal::string(string);
        Self { literal, value }
    }

    /// Get the `&str` including the surrounding quotes.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // bug in clippy
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Get the `&str` with the surrounding quotes removed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value[1..self.value.len() - 1]
    }

    /// Set the value to a new `String`.
    pub fn set(&mut self, value: String) {
        *self = Self {
            literal: Literal::string(&value),
            value,
        };
    }

    /// Deconstructs `self` and gets the `Literal`
    #[must_use]
    pub fn into_inner(self) -> Literal {
        self.literal
    }
}

impl Parser for LiteralString {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        let literal = Literal::parser(tokens).refine_err::<Self>()?;
        let string = literal.to_string();
        // The lexer did its job here as well
        if &string[0..1] == "\"" {
            Ok(Self {
                literal,
                value: string,
            })
        } else {
            Error::unexpected_token(at, tokens)
        }
    }
}

impl ToTokens for LiteralString {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.literal.to_tokens(tokens);
    }
}

/// Compares without the surrounding quotes.
impl PartialEq<&str> for LiteralString {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl From<LiteralString> for TokenTree {
    fn from(lit: LiteralString) -> Self {
        TokenTree::Literal(lit.into_inner())
    }
}

#[test]
fn test_literalstring_into_tt() {
    let lit = LiteralString::from_str("foobar");
    let _: TokenTree = lit.into();
}
