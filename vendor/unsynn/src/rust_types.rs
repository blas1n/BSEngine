//! Parsers for rusts types.
//!
//! **Note**: When the `proc_macro2` feature is disabled, `ToTokens` implementations for
//! `&str` and `String` are not available.

use crate::{
    Error, Ident, LiteralCharacter, LiteralInteger, Parse, Parser, RefineErr, Result, Span,
    ToTokens, TokenIter, TokenStream, TokenTree,
};

// Parser and ToTokens for unsigned integer types
macro_rules! impl_unsigned_integer {
    ($($ty:ty),*) => {
        $(
            #[doc = stringify!(Parse $ty may have a positive sign but no suffix)]
            impl Parser for $ty {
                fn parser(tokens: &mut TokenIter) -> Result<Self> {
                    let at = tokens.clone().next();
                    let lit = crate::Cons::<Option<crate::Plus>, LiteralInteger>::parser(tokens).refine_err::<Self>()?;
                    <$ty>::try_from(lit.second.value())
                        .or_else(|e| Error::dynamic::<Self>(at, tokens, e))
                }
            }

            #[doc = stringify!(Emit a literal $ty without sign and suffix)]
            impl ToTokens for $ty {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    #[allow(clippy::cast_lossless)]
                    LiteralInteger::new(*self as u128).to_tokens(tokens);
                }
            }

            #[doc = stringify!(Emit a literal $ty without sign and suffix)]
            impl ToTokens for &$ty {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    #[allow(clippy::cast_lossless)]
                    LiteralInteger::new(**self as u128).to_tokens(tokens);
                }
            }
        )*
    };
}

impl_unsigned_integer! {u8, u16, u32, u64, u128, usize}

// Parser and ToTokens for signed integer types
macro_rules! impl_signed_integer {
    ($($ty:ty),*) => {
        $(
            #[doc = stringify!(Parse $ty may have a positive or negative sign but no suffix)]
            impl Parser for $ty {
                fn parser(tokens: &mut TokenIter) -> Result<Self> {
                    let at = tokens.clone().next();
                    let lit = crate::Cons::<Option<crate::Either<crate::Plus, crate::Minus>>, LiteralInteger>::parser(tokens).refine_err::<Self>()?;
                    let value = <$ty>::try_from(lit.second.value())
                        .or_else(|e| Error::dynamic::<Self>(at, tokens, e))?;
                    match lit.first {
                        Some(crate::Either::Second(_)) => Ok(-value),
                        _ => Ok(value),
                    }
                }
            }

            #[doc = stringify!(Emit a literal $ty with negative sign and without suffix)]
            impl ToTokens for $ty {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    if *self < 0 {
                        crate::Minus::new().to_tokens(tokens);
                    }
                    LiteralInteger::new(self.abs().try_into().unwrap()).to_tokens(tokens);
                }
            }

            #[doc = stringify!(Emit a literal $ty with negative sign and without suffix)]
            impl ToTokens for &$ty {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    if **self < 0 {
                        crate::Minus::new().to_tokens(tokens);
                    }
                    LiteralInteger::new(self.abs().try_into().unwrap()).to_tokens(tokens);
                }
            }
        )*
    };
}

impl_signed_integer! {i8, i16, i32, i64, i128, isize}

// Parser and ToTokens for char
impl Parser for char {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let lit = LiteralCharacter::parser(tokens).refine_err::<Self>()?;
        Ok(lit.value())
    }
}

impl ToTokens for char {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        LiteralCharacter::new(*self).to_tokens(tokens);
    }
}

// Helper function for parsing bool identifiers without proc_macro2.
// This is tested in the proc-macro workspace (tests-proc-macro/) where proc_macro2
// is disabled. cargo-mutants only tests the main workspace with default features
// (proc_macro2 enabled), where this code is not compiled. Therefore, we skip mutation testing.
#[cfg(not(feature = "proc_macro2"))]
#[mutants::skip]
fn parse_bool_ident(ident: &Ident, at: Option<TokenTree>, tokens: &mut TokenIter) -> Result<bool> {
    let ident_str = ident.to_string();
    if ident_str == "true" {
        Ok(true)
    } else if ident_str == "false" {
        Ok(false)
    } else {
        Error::unexpected_token(at, tokens)
    }
}

// Parser and ToTokens for bool
/// Parse a boolean value from the input stream.
/// Only `true` and `false` are valid boolean values.
impl Parser for bool {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let at = tokens.clone().next();
        Ident::parse_with(tokens, |ident, tokens| {
            // proc_macro2::Ident implements PartialEq<&str>, so we can compare directly without allocation
            // proc_macro::Ident does not, so we need to_string() which allocates
            #[cfg(feature = "proc_macro2")]
            {
                if ident == "true" {
                    Ok(true)
                } else if ident == "false" {
                    Ok(false)
                } else {
                    Error::unexpected_token(at, tokens)
                }
            }
            #[cfg(not(feature = "proc_macro2"))]
            {
                parse_bool_ident(&ident, at, tokens)
            }
        })
        .refine_err::<Self>()
    }
}

impl ToTokens for bool {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        Ident::new(if *self { "true" } else { "false" }, Span::call_site()).to_tokens(tokens);
    }
}

/// Parse a `String` from the input stream.  Parsing into a string is special as it parses any
/// kind of `TokenTree` and converts it `.to_string()`. Thus it looses its relationship to the
/// type of the underlying token/syntactic entity. This is only useful when one wants to parse
/// string like parameters in a macro that are not emitted later. This limits the use of this
/// parser significantly.
impl Parser for String {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        TokenTree::parse_with(tokens, |token, _| Ok(token.to_string())).refine_err::<Self>()
    }
}

/// Tokenizes a `&str`. Panics if the input string does not tokenize.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut tokens = "foo -> {1,2,3}".to_token_stream();
///
/// assert_tokens_eq!(
///     tokens,
///     "foo -> { 1 , 2 , 3 }"
/// );
/// ```
#[cfg(feature = "proc_macro2")]
impl ToTokens for &str {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use std::str::FromStr;
        let ts = TokenStream::from_str(self).expect("Failed to tokenize input string.");
        tokens.extend(ts);
    }
}

#[cfg(feature = "proc_macro2")]
impl ToTokens for str {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        use std::str::FromStr;
        let ts = TokenStream::from_str(self).expect("Failed to tokenize input string.");
        tokens.extend(ts);
    }
}

#[cfg(feature = "proc_macro2")]
impl ToTokens for &String {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.as_str().to_tokens(tokens);
    }
}

/// `PhantomData` behaves like `Nothing` it doesn't parse anything and doesnt emit tokens.
impl<T> Parser for std::marker::PhantomData<T> {
    #[inline]
    #[mutants::skip]
    fn parser(_tokens: &mut TokenIter) -> Result<Self> {
        Ok(Self)
    }
}

impl<T> ToTokens for std::marker::PhantomData<T> {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        /*NOP*/
    }
}

// Parser and ToTokens implementations for tuples

/// Parse a 2-element tuple by parsing each element in sequence.
///
/// This provides a more idiomatic alternative to [`Cons`](crate::Cons) for simple cases.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut tokens = "42 true".to_token_iter();
/// let (num, flag): (u8, bool) = Parse::parse(&mut tokens).unwrap();
/// assert_eq!(num, 42);
/// assert_eq!(flag, true);
/// ```
impl<A: Parse, B: Parse> Parser for (A, B) {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok((A::parser(tokens)?, B::parser(tokens)?))
    }
}

/// Emit tokens for a 2-element tuple by emitting each element in sequence.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut tokens = TokenStream::new();
/// (42u8, true).to_tokens(&mut tokens);
/// assert_eq!(tokens.to_string(), "42 true");
/// ```
impl<A: ToTokens, B: ToTokens> ToTokens for (A, B) {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
        self.1.to_tokens(tokens);
    }
}

/// Parse a 3-element tuple by parsing each element in sequence.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut tokens = "foo + 42".to_token_iter();
/// let (id, op, num): (Ident, Punct, u8) = Parse::parse(&mut tokens).unwrap();
/// assert_eq!(id.to_string(), "foo");
/// assert_eq!(op.to_string(), "+");
/// assert_eq!(num, 42);
/// ```
impl<A: Parse, B: Parse, C: Parse> Parser for (A, B, C) {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok((A::parser(tokens)?, B::parser(tokens)?, C::parser(tokens)?))
    }
}

/// Emit tokens for a 3-element tuple by emitting each element in sequence.
impl<A: ToTokens, B: ToTokens, C: ToTokens> ToTokens for (A, B, C) {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
        self.1.to_tokens(tokens);
        self.2.to_tokens(tokens);
    }
}

/// Parse a 4-element tuple by parsing each element in sequence.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut tokens = "5 true 'a' 255".to_token_iter();
/// let tuple: (u8, bool, char, u8) = Parse::parse(&mut tokens).unwrap();
/// assert_eq!(tuple, (5, true, 'a', 255));
/// ```
impl<A: Parse, B: Parse, C: Parse, D: Parse> Parser for (A, B, C, D) {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok((
            A::parser(tokens)?,
            B::parser(tokens)?,
            C::parser(tokens)?,
            D::parser(tokens)?,
        ))
    }
}

/// Emit tokens for a 4-element tuple by emitting each element in sequence.
impl<A: ToTokens, B: ToTokens, C: ToTokens, D: ToTokens> ToTokens for (A, B, C, D) {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
        self.1.to_tokens(tokens);
        self.2.to_tokens(tokens);
        self.3.to_tokens(tokens);
    }
}
