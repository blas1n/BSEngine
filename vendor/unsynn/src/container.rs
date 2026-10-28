//! This module provides parsers for types that contain possibly multiple values. This
//! includes stdlib types like [`Option`], [`Vec`], [`Box`], [`Rc`], [`RefCell`] and types
//! for delimited and repeated values with numbered repeats.

use crate::{
    Colon, Comma, Cons, Delimited, Dot, EndOfStream, Error, Except, Expect, Nothing, Parse, Parser,
    PathSep, RefineErr, Result, Semicolon, ToTokens, TokenIter, TokenStream,
};

use std::{cell::RefCell, marker::PhantomData, ops::Deref, rc::Rc};

// PLANNED: investigate which collections can impl FromIterator
// PLANNED: Which unsynn types should impl Deref?

/// Zero or One of T.
impl<T: Parse> Parser for Option<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        match T::parse(tokens) {
            Ok(value) => Ok(Some(value)),
            Err(_) => Ok(None),
        }
    }
}

impl<T: ToTokens> ToTokens for Option<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(t) = self.as_ref() {
            t.to_tokens(tokens);
        }
    }
}

/// Any number of T
impl<T: Parse> Parser for Vec<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut output = Vec::new();
        let mut track_pos = tokens.counter();

        while let Ok(value) = T::parse(tokens) {
            if tokens.counter() == track_pos {
                return Error::infinite_loop::<Vec<T>>(tokens.clone().next(), tokens);
            }
            track_pos = tokens.counter();
            output.push(value);
        }
        Ok(output)
    }
}

impl<T: ToTokens> ToTokens for Vec<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        for value in self {
            value.to_tokens(tokens);
        }
    }
}

/// `NonEmptyOption<T>` prevents `Option` from matching when `T` can succeed with empty
/// input. It ensures `None` is returned when no tokens remain, regardless of whether `T`
/// could succeed on an empty stream. This is crucial when parsing optional trailing content
/// that should only match if tokens are actually available to consume.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "ident".to_token_iter();
///
/// let parsed = NonEmptyOption::<Punct>::parser(&mut token_iter).unwrap();
/// assert_tokens_eq!(parsed, "");
/// # assert_tokens_eq!(NonEmptyOption::<Punct>::parser(&mut ".".to_token_iter()).unwrap(), ".");
/// ```
///
/// # Contrast with `Option<T>`
///
/// ```
/// # use unsynn::*;
/// // Vec<T> can succeed with zero items
/// let mut token_iter = "".to_token_iter();
///
/// // Option<T> returns Some(empty_vec) even at EndOfStream
/// let parsed = Option::<Vec<Ident>>::parser(&mut token_iter).unwrap();
/// assert!(parsed.is_some());
///
/// // NonEmptyOption<T> returns None at EndOfStream
/// let mut token_iter = "".to_token_iter();
/// let parsed = NonEmptyOption::<Vec<Ident>>::parser(&mut token_iter).unwrap();
/// assert!(parsed.is_none());
/// ```
#[derive(Clone)]
pub struct NonEmptyOption<T>(pub Option<T>);

impl<T: Parse> Parser for NonEmptyOption<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok(Self(<Option<Cons<Except<EndOfStream>, T>>>::parse_with(
            tokens,
            |t, _| Ok(t.map(|t| t.second)),
        )?))
    }
}

impl<T: ToTokens> ToTokens for NonEmptyOption<T> {
    #[inline]
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug> std::fmt::Debug for NonEmptyOption<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("NonEmptyOption<{}>", std::any::type_name::<T>()))
            .finish()
    }
}

impl<T> std::ops::Deref for NonEmptyOption<T> {
    type Target = Option<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A trait for parsing a repeating `T` with a minimum and maximum limit.
/// Sometimes the number of elements to be parsed is determined at runtime eg. a number of
/// header items needs a matching number of values.
///
/// # Example
///
/// Parse at table with a number of headers followed by values.
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "
///     foo:       bar:
///     foo_value  bar_value
/// ".to_token_iter();
///
/// let headers = Vec::<Cons<Ident,Colon>>::parse(&mut token_iter).unwrap();
/// let values = Vec::<Ident>::parse_exactly(&mut token_iter, headers.len()).unwrap();
/// ```
#[allow(clippy::missing_errors_doc)]
pub trait RangedRepeats: Sized {
    /// Parse at least `min` and up to `max` (inclusive) elements.
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self>;

    /// Parse any number of elements.
    fn parse_any(tokens: &mut TokenIter) -> Result<Self> {
        Self::parse_repeats(tokens, 0, usize::MAX)
    }

    /// Parse at least one element.
    fn parse_many(tokens: &mut TokenIter) -> Result<Self> {
        Self::parse_repeats(tokens, 1, usize::MAX)
    }

    /// Parse zero or one element.
    fn parse_optional(tokens: &mut TokenIter) -> Result<Self> {
        Self::parse_repeats(tokens, 0, 1)
    }

    /// Parse exactly `n` elements.
    fn parse_exactly(tokens: &mut TokenIter, n: usize) -> Result<Self> {
        Self::parse_repeats(tokens, n, n)
    }

    /// Parse at most `n` elements.
    fn parse_at_most(tokens: &mut TokenIter, n: usize) -> Result<Self> {
        Self::parse_repeats(tokens, 0, n)
    }

    /// Parse at least `n` elements.
    fn parse_at_least(tokens: &mut TokenIter, n: usize) -> Result<Self> {
        Self::parse_repeats(tokens, n, usize::MAX)
    }
}

impl<T: Parse> RangedRepeats for Vec<T> {
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        let mut output = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            let pos_before = tokens.counter();
            at = tokens.clone().next();
            if let Ok(value) = T::parse(tokens) {
                if tokens.counter() == pos_before {
                    return Error::infinite_loop::<Vec<T>>(at, tokens);
                }
                output.push(value);
            } else {
                break;
            }
        }

        if output.len() >= min {
            Ok(output)
        } else {
            Error::other(
                at,
                tokens,
                format!("less than {} elements, got {}", min, output.len()),
            )
        }
    }
}

/// Box a parseable entity. In a enum it may happen that most variants are rather small while
/// few variants are large. In this case it may be beneficial to box the large variants to
/// keep the enum lean. `Box` or `Rc` are required for parsing recursive grammars.
impl<T: Parse> Parser for Box<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok(Box::new(T::parser(tokens).refine_err::<Self>()?))
    }
}

impl<T: ToTokens> ToTokens for Box<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.as_ref().to_tokens(tokens);
    }
}

/// Rc a parseable entity. Just because we can. Sometimes when a value is shared between
/// multiple entities it may be beneficial to use Rc. `Box` or `Rc` are required for parsing recursive grammars.
impl<T: Parse> Parser for Rc<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok(Rc::new(T::parser(tokens).refine_err::<Self>()?))
    }
}

impl<T: ToTokens> ToTokens for Rc<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.as_ref().to_tokens(tokens);
    }
}

/// Put any parseable entity in a `RefCell`. In case one wants to mutate the a parse tree on the
/// fly.
impl<T: Parse> Parser for RefCell<T> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Ok(RefCell::new(T::parser(tokens).refine_err::<Self>()?))
    }
}

impl<T: ToTokens> ToTokens for RefCell<T> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.borrow().to_tokens(tokens);
    }
}

/// A `Vec<T>` that is filled up to the first appearance of an terminating `S`.  This `S` may
/// be a subset of `T`, thus parsing become lazy.  This is the same as
/// `Cons<Vec<Cons<Except<S>,T>>,S>` but more convenient and efficient.
///
/// # Example
///
/// Parse anything until a `;`.
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo bar ; baz ;".to_token_iter();
///
/// type Example = LazyVec<TokenTree, Semicolon>;
///
/// let _example = Example::parse(&mut token_iter).unwrap();
/// let _example = Example::parse(&mut token_iter).unwrap();
/// ```
#[derive(Clone)]
pub struct LazyVec<T, S> {
    /// The vector of repeating `T`
    pub vec: Vec<T>,
    /// The terminating `S`
    pub terminator: S,
}

impl<T: Parse, S: Parse> Parser for LazyVec<T, S> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut vec = Vec::new();

        loop {
            if let Ok(terminator) = S::parse(tokens) {
                return Ok(Self { vec, terminator });
            }

            let pos_before_t = tokens.counter();
            let value = T::parse(tokens)?;

            if tokens.counter() == pos_before_t {
                return Error::infinite_loop::<LazyVec<T, S>>(tokens.clone().next(), tokens);
            }

            vec.push(value);
        }
    }
}

impl<T: ToTokens, S: ToTokens> ToTokens for LazyVec<T, S> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.vec.iter().for_each(|value| value.to_tokens(tokens));
        self.terminator.to_tokens(tokens);
    }
}

impl<T: Parse, S: Parse> RangedRepeats for LazyVec<T, S> {
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        let mut vec = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            at = tokens.clone().next();
            if let Ok(terminator) = S::parse(tokens) {
                return if vec.len() >= min {
                    Ok(Self { vec, terminator })
                } else {
                    Error::other(
                        at,
                        tokens,
                        format!("less than {} elements, got {}", min, vec.len()),
                    )
                };
            }

            let pos_before_t = tokens.counter();
            let value = T::parse(tokens)?;
            if tokens.counter() == pos_before_t {
                return Error::infinite_loop::<LazyVec<T, S>>(at, tokens);
            }

            vec.push(value);
        }
        Error::other(at, tokens, format!("more than {max} elements"))
    }
}

impl<T, S> IntoIterator for LazyVec<T, S> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug, S: std::fmt::Debug> std::fmt::Debug for LazyVec<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!(
            "LazyVec<{}, {}>",
            std::any::type_name::<T>(),
            std::any::type_name::<S>()
        ))
        .field("vec", &self.vec)
        .field("terminator", &self.terminator)
        .finish()
    }
}

/// A `Vec<T>` that is filled up to the first appearance of an terminating `S`.  This `S` may
/// be a subset of `T`, thus parsing become lazy.  Unlike `LazyVec` this variant does not consume
/// the final terminator. This is the same as `Vec<Cons<Except<S>,T>>>` but more convenient.
///
/// # Example
///
/// Parse anything until a `;`.
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "foo bar ; baz ;".to_token_iter();
///
/// type Example = LazyVecUntil<TokenTree, Semicolon>;
///
/// let _example = Example::parse(&mut token_iter).unwrap();
/// let _example = Semicolon::parse(&mut token_iter).unwrap();
/// let _example = Example::parse(&mut token_iter).unwrap();
/// let _example = Semicolon::parse(&mut token_iter).unwrap();
/// ```
#[derive(Clone)]
pub struct LazyVecUntil<T, S> {
    /// The vector of repeating `T`
    pub vec: Vec<T>,
    phantom: PhantomData<S>,
}

impl<T: Parse, S: Parse> Parser for LazyVecUntil<T, S> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let mut vec = Vec::new();

        loop {
            if <Expect<S>>::parse(tokens).is_ok() {
                return Ok(Self {
                    vec,
                    phantom: PhantomData,
                });
            }

            let pos_before_t = tokens.counter();
            let value = T::parse(tokens)?;
            if tokens.counter() == pos_before_t {
                return Error::infinite_loop::<LazyVecUntil<T, S>>(tokens.clone().next(), tokens);
            }

            vec.push(value);
        }
    }
}

impl<T: ToTokens, S: ToTokens> ToTokens for LazyVecUntil<T, S> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.vec.iter().for_each(|value| value.to_tokens(tokens));
    }
}

impl<T: Parse, S: Parse> RangedRepeats for LazyVecUntil<T, S> {
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        let mut vec = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            at = tokens.clone().next();
            if <Expect<S>>::parse(tokens).is_ok() {
                return if vec.len() >= min {
                    Ok(Self {
                        vec,
                        phantom: PhantomData,
                    })
                } else {
                    Error::other(
                        at,
                        tokens,
                        format!("less than {} elements, got {}", min, vec.len()),
                    )
                };
            }

            let pos_before_t = tokens.counter();
            let value = T::parse(tokens)?;
            if tokens.counter() == pos_before_t {
                return Error::infinite_loop::<LazyVecUntil<T, S>>(at, tokens);
            }

            vec.push(value);
        }
        Error::other(at, tokens, format!("more than {max} elements"))
    }
}

impl<T, S> IntoIterator for LazyVecUntil<T, S> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec.into_iter()
    }
}

#[mutants::skip]
impl<T: std::fmt::Debug, S: std::fmt::Debug> std::fmt::Debug for LazyVecUntil<T, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!(
            "LazyVec<{}, {}>",
            std::any::type_name::<T>(),
            std::any::type_name::<S>()
        ))
        .field("vec", &self.vec)
        .finish()
    }
}

/// Policy for the delimiter of the last element in a sequence.  Note that delimiters are
/// after some element, for cases where you have leading delimiters you need to define
/// grammars that start with `Delimiter` or `Option<Delimiter>`.
#[allow(non_snake_case)]
pub mod TrailingDelimiter {
    /// The delimiter at the end must be present `a; b; c;`.
    #[derive(Copy, Clone, Debug)]
    pub struct Mandatory;

    /// The delimiter at the end is optional `a, b` or `a, b,`.
    #[derive(Copy, Clone, Debug)]
    pub struct Optional;

    /// There must be no delimiter at the end `a::b`.
    #[derive(Copy, Clone, Debug)]
    pub struct Forbidden;
}

/// Since the delimiter in [`Delimited<T,D>`] is optional a [`Vec<Delimited<T,D>>`] would
/// parse consecutive values even without delimiters. [`DelimitedVec<T, D, MIN, MAX, P>`] will
/// stop parsing by MIN/MAX number of elements and depending on the policy defined by `P`
/// which can be one of [`TrailingDelimiter`].
#[derive(Clone)]
pub struct DelimitedVec<
    T,
    D,
    P = TrailingDelimiter::Optional,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
>(Vec<Delimited<T, D>>, PhantomData<P>);

impl<T: Parse, D: Parse, const MIN: usize, const MAX: usize> Parser
    for DelimitedVec<T, D, TrailingDelimiter::Optional, MIN, MAX>
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        let mut output = Vec::new();
        let mut at = tokens.clone().next();
        while output.len() < MAX {
            let pos_before = tokens.counter();
            if let Ok(delimited) = Delimited::<T, D>::parse(tokens) {
                if tokens.counter() == pos_before {
                    return Error::infinite_loop::<
                        DelimitedVec<T, D, TrailingDelimiter::Optional, MIN, MAX>,
                    >(at, tokens);
                }
                at = tokens.clone().next();
                let done = delimited.delimiter.is_none();
                output.push(delimited);
                if done {
                    break;
                }
            } else {
                break;
            }
        }

        #[allow(unused_comparisons)]
        if output.len() >= MIN {
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                at,
                tokens,
                format!(
                    "Expected at least {MIN} elements in DelimitedVec<{}, {}, {MIN}, {MAX}, {:?}> but got only {}",
                    std::any::type_name::<T>(),
                    std::any::type_name::<D>(),
                    TrailingDelimiter::Optional,
                    output.len(),
                ),
            )
        }
    }
}

impl<T: Parse + ToTokens, D: Parse, const MIN: usize, const MAX: usize> Parser
    for DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        let mut output = Vec::new();
        let at = tokens.clone().next();
        #[allow(unused_comparisons)]
        while output.len() < MAX {
            let pos_before = tokens.counter();
            match Delimited::<T, D>::parse_with(tokens, |d, tokens| {
                if d.delimiter.is_some() {
                    Ok(d)
                } else {
                    Error::unexpected_token(d.value.to_token_iter().next(), tokens)
                }
            }) {
                Ok(delimited) => {
                    if tokens.counter() == pos_before {
                        return Error::infinite_loop::<
                            DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>,
                        >(at, tokens);
                    }
                    output.push(delimited);
                }
                Err(_) => break,
            }
        }

        #[allow(unused_comparisons)]
        if output.len() >= MIN {
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                at,
                tokens,
                format!(
                    "Expected at least {MIN} elements in DelimitedVec<{}, {}, {MIN}, {MAX}, {:?}> but got only {}",
                    std::any::type_name::<T>(),
                    std::any::type_name::<D>(),
                    TrailingDelimiter::Mandatory,
                    output.len(),
                ),
            )
        }
    }
}

impl<T: Parse + ToTokens, D: Parse, const MIN: usize, const MAX: usize> Parser
    for DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        let mut output = Vec::new();
        let at = tokens.clone().next();

        while output.len() < MAX {
            let pos_before = tokens.counter();
            if let Ok(delimited) = Delimited::<T, D>::parse(tokens) {
                if tokens.counter() == pos_before {
                    return Error::infinite_loop::<
                        DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>,
                    >(at, tokens);
                }
                let done = delimited.delimiter.is_none();
                output.push(delimited);
                if done {
                    break;
                }
            } else {
                break;
            }
        }

        #[allow(unused_comparisons)]
        if output.len() >= MIN {
            if let Some(last) = output.last() {
                if last.delimiter.is_some() {
                    return Error::unexpected_token::<Self>(tokens.clone().next(), tokens);
                }
            }
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                        at,
                        tokens,
                        format!(
                            "Expected at least {MIN} elements in DelimitedVec<{}, {}, {MIN}, {MAX}, {:?}> but got only {}",
                            std::any::type_name::<T>(),
                            std::any::type_name::<D>(),
                            TrailingDelimiter::Forbidden,
                            output.len(),
                        ),
                    )
        }
    }
}

/// Converts a [`DelimitedVec<T, D>`] into a [`Vec<T>`].
/// This loses all delimiters, which may have been stateful (eg. `Either`).
impl<T, D, P, const MIN: usize, const MAX: usize> From<DelimitedVec<T, D, P, MIN, MAX>> for Vec<T> {
    fn from(delimited_vec: DelimitedVec<T, D, P, MIN, MAX>) -> Self {
        const { assert!(MIN <= MAX) };
        delimited_vec
            .0
            .into_iter()
            .map(|delimited| delimited.value)
            .collect()
    }
}

impl<T: ToTokens, D: ToTokens, P, const MIN: usize, const MAX: usize> ToTokens
    for DelimitedVec<T, D, P, MIN, MAX>
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.iter().for_each(|value| value.to_tokens(tokens));
    }
}

impl<T: Parse, D: Parse, const MIN: usize, const MAX: usize> RangedRepeats
    for DelimitedVec<T, D, TrailingDelimiter::Optional, MIN, MAX>
{
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        if min < MIN {
            Error::out_of_range::<MIN, _>(min, tokens.clone().next(), tokens)?;
        }
        if max > MAX {
            Error::out_of_range::<MAX, _>(max, tokens.clone().next(), tokens)?;
        }
        let mut output = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            at = tokens.clone().next();
            if let Ok(delimited) = Delimited::<T, D>::parse(tokens) {
                let done = delimited.delimiter.is_none();
                output.push(delimited);
                if done {
                    break;
                }
            } else {
                break;
            }
        }

        if output.len() >= min {
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                at,
                tokens,
                format!("less than {} elements, got {}", min, output.len()),
            )
        }
    }
}

impl<T: Parse, D: Parse, const MIN: usize, const MAX: usize> RangedRepeats
    for DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>
{
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        if min < MIN {
            Error::out_of_range::<MIN, _>(min, tokens.clone().next(), tokens)?;
        }
        if max > MAX {
            Error::out_of_range::<MAX, _>(max, tokens.clone().next(), tokens)?;
        }
        let mut output = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            at = tokens.clone().next();
            if let Ok(delimited) = Delimited::<T, D>::parse(tokens) {
                if delimited.delimiter.is_none() {
                    return Error::other(at, tokens, "missing mandatory delimiter".to_string());
                }
                output.push(delimited);
            } else {
                break;
            }
        }

        if output.len() >= min {
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                at,
                tokens,
                format!("less than {} elements, got {}", min, output.len()),
            )
        }
    }
}

impl<T: Parse + ToTokens, D: Parse, const MIN: usize, const MAX: usize> RangedRepeats
    for DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>
{
    fn parse_repeats(tokens: &mut TokenIter, min: usize, max: usize) -> Result<Self> {
        const { assert!(MIN <= MAX) };
        if min < MIN {
            Error::out_of_range::<MIN, _>(min, tokens.clone().next(), tokens)?;
        }
        if max > MAX {
            Error::out_of_range::<MAX, _>(max, tokens.clone().next(), tokens)?;
        }
        let mut output = Vec::with_capacity(min);
        let mut at = tokens.clone().next();
        for _ in 0..max {
            at = tokens.clone().next();
            if let Ok(delimited) = Delimited::<T, D>::parse(tokens) {
                let done = delimited.delimiter.is_none();
                output.push(delimited);
                if done {
                    break;
                }
            } else {
                break;
            }
        }

        if output.len() >= min {
            if let Some(last) = output.last() {
                if last.delimiter.is_some() {
                    Error::unexpected_token::<_>(last.value.to_token_iter().next(), tokens)?;
                }
            }
            Ok(Self(output, PhantomData))
        } else {
            Error::other(
                at,
                tokens,
                format!("less than {} elements, got {}", min, output.len()),
            )
        }
    }
}

impl<T, D, const MIN: usize, P, const MAX: usize> IntoIterator for DelimitedVec<T, D, P, MIN, MAX> {
    type Item = Delimited<T, D>;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<T, D, P, const MIN: usize, const MAX: usize> Deref for DelimitedVec<T, D, P, MIN, MAX> {
    type Target = Vec<Delimited<T, D>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Creates a `DelimitedVec` from a iterator over T and default constructed delimiters with mandatory
/// trailing delimiter.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "a b c d".into_token_iter();
/// let chars: Vec<Ident> = token_iter.parse().unwrap();
/// let comma_delimited: CommaDelimitedVec<Ident, TrailingDelimiter::Mandatory>
///                      = chars.into_iter().collect();
/// assert_tokens_eq!(comma_delimited, "a, b, c, d,");
/// ```
impl<T, D: Default, const MIN: usize, const MAX: usize> FromIterator<T>
    for DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut v: Vec<Delimited<T, D>> = Vec::with_capacity(iter.size_hint().0);
        for value in iter {
            v.push(Delimited {
                value,
                delimiter: Some(D::default()),
            });
        }

        Self(v, PhantomData)
    }
}

/// A `DelimitedVec` with mandatory trailing delimiter can be zero-cost converted to one with
/// optional delimiter.
impl<T, D, const MIN: usize, const MAX: usize>
    From<DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>>
    for DelimitedVec<T, D, TrailingDelimiter::Optional, MIN, MAX>
{
    fn from(input: DelimitedVec<T, D, TrailingDelimiter::Mandatory, MIN, MAX>) -> Self {
        Self(input.0, PhantomData)
    }
}

/// Creates a `DelimitedVec` from a iterator over T and default constructed delimiters without
/// trailing delimiter.
///
/// # Example
///
/// ```
/// # use unsynn::*;
/// let mut token_iter = "a b c d".into_token_iter();
/// let chars: Vec<Ident> = token_iter.parse().unwrap();
/// let comma_delimited: CommaDelimitedVec<Ident, TrailingDelimiter::Forbidden>
///                      = chars.into_iter().collect();
/// assert_tokens_eq!(comma_delimited, "a, b, c, d");
/// ```
impl<T, D: Default, const MIN: usize, const MAX: usize> FromIterator<T>
    for DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>
{
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let mut v: Vec<Delimited<T, D>> = Vec::with_capacity(iter.size_hint().0);
        for value in iter {
            if let Some(last) = v.last_mut() {
                last.delimiter = Some(D::default());
            }
            v.push(Delimited {
                value,
                delimiter: None,
            });
        }

        Self(v, PhantomData)
    }
}

/// A `DelimitedVec` with forbidden trailing delimiter can be zero-cost converted to one with
/// optional delimiter.
impl<T, D, const MIN: usize, const MAX: usize>
    From<DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>>
    for DelimitedVec<T, D, TrailingDelimiter::Optional, MIN, MAX>
{
    fn from(input: DelimitedVec<T, D, TrailingDelimiter::Forbidden, MIN, MAX>) -> Self {
        Self(input.0, PhantomData)
    }
}

#[mutants::skip]
impl<
        T: std::fmt::Debug,
        D: std::fmt::Debug,
        P: std::fmt::Debug,
        const MIN: usize,
        const MAX: usize,
    > std::fmt::Debug for DelimitedVec<T, D, P, MIN, MAX>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple(&format!(
            "DelimitedVec<{}, {}, {}, {MIN}, {MAX}>",
            std::any::type_name::<T>(),
            std::any::type_name::<D>(),
            std::any::type_name::<P>()
        ))
        .field(&self.0)
        .finish()
    }
}

/// `DelimitedVec` of `T` delimited by `,` with `P` as policy for the last delimiter.
pub type CommaDelimitedVec<
    T,
    P = TrailingDelimiter::Optional,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
> = DelimitedVec<T, Comma, P, MIN, MAX>;

/// `DelimitedVec` of `T` delimited by `;` with `P` as policy for the last delimiter.
pub type SemicolonDelimitedVec<
    T,
    P = TrailingDelimiter::Mandatory,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
> = DelimitedVec<T, Semicolon, P, MIN, MAX>;

/// `DelimitedVec` of `T` delimited by `.` with `P` as policy for the last delimiter.
pub type DotDelimitedVec<
    T,
    P = TrailingDelimiter::Optional,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
> = DelimitedVec<T, Dot, P, MIN, MAX>;

/// `DelimitedVec` of `T` delimited by `:` with `P` as policy for the last delimiter.
pub type ColonDelimitedVec<
    T,
    P = TrailingDelimiter::Optional,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
> = DelimitedVec<T, Colon, P, MIN, MAX>;

/// `DelimitedVec` of `T` delimited by `::` with `P` as policy for the last delimiter.
pub type PathSepDelimitedVec<
    T,
    P = TrailingDelimiter::Forbidden,
    const MIN: usize = 0,
    const MAX: usize = { usize::MAX },
> = DelimitedVec<T, PathSep, P, MIN, MAX>;

/// `DelimitedVec<T,D>` with a minimum and maximum (inclusive) number of elements at first
/// without defaults.  Parsing will succeed when at least the minimum number of elements is
/// reached and stop at the maximum number.  The delimiter `D` defaults to [`Nothing`] to
/// parse sequences which don't have delimiters.
pub type Repeats<
    const MIN: usize,
    const MAX: usize,
    T,
    D = Nothing,
    P = TrailingDelimiter::Optional,
> = DelimitedVec<T, D, P, MIN, MAX>;

/// Any number of T delimited by D or [`Nothing`]
pub type Any<T, D = Nothing, P = TrailingDelimiter::Optional> = Repeats<0, { usize::MAX }, T, D, P>;

/// One or more of T delimited by D or [`Nothing`]
pub type Many<T, D = Nothing, P = TrailingDelimiter::Optional> =
    Repeats<1, { usize::MAX }, T, D, P>;

/// Zero or one of T delimited by D or [`Nothing`]
pub type Optional<T, D = Nothing, P = TrailingDelimiter::Optional> = Repeats<0, 1, T, D, P>;

/// Exactly N of T delimited by D or [`Nothing`]
pub type Exactly<const N: usize, T, D = Nothing, P = TrailingDelimiter::Optional> =
    Repeats<N, N, T, D, P>;

/// At most N of T delimited by D or [`Nothing`]
pub type AtMost<const N: usize, T, D = Nothing, P = TrailingDelimiter::Optional> =
    Repeats<0, N, T, D, P>;

/// At least N of T delimited by D or [`Nothing`]
pub type AtLeast<const N: usize, T, D = Nothing, P = TrailingDelimiter::Optional> =
    Repeats<N, { usize::MAX }, T, D, P>;

// PLANNED: needs https://github.com/rust-lang/rust/issues/96097 impl<const N: usize, T: Parse> Parser for [T;N] {
