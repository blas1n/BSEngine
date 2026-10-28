//! Parse predicates for compile-time parser control.
//!
//! This module provides zero-cost abstractions for controlling parser behavior at compile time.
//! All predicate types are zero-sized and have no runtime overhead.
//!
//! # Base Values
//!
//! - [`Enable`] - Always succeeds without consuming tokens
//! - [`Disable`] - Always fails without consuming tokens
//! - [`TokensRemain`] - Succeeds only when tokens remain in the stream
//!
//! # Logical Operators
//!
//! - [`AllOf`] - Logical AND (all must succeed, 2-4 operands)
//! - [`AnyOf`] - Logical OR (at least one must succeed, 2-4 operands)
//! - [`OneOf`] - Logical XOR (exactly one must succeed, 2-4 operands)
//! - [`Not`] - Logical NOT (succeeds if inner fails)
//!
//! # Example
//!
//! The `predicateflag` macro creates newtype wrappers that can implement custom traits for
//! compile-time context validation:
//!
//! ```rust
//! use unsynn::*;
//! use unsynn::predicates::*;
//!
//!
//! unsynn! {
//!    // Define a custom context predicate marker-trait
//!     trait ExpressionContext: PredicateOp;
//!
//!     // Creates newtype implementing ExpressionContext
//!     predicateflag InExpression = Enable for ExpressionContext;
//!
//!     // Only accepts ExpressionContext predicates
//!     struct StructLiteral<P: ExpressionContext> {
//!         _guard: P,
//!         name: Ident,
//!     }
//! }
//! ```
//!
//! See the **[Parse Predicates]** chapter in the Cookbook for detailed usage examples.
//!
//! [Parse Predicates]: https://docs.rs/unsynn/latest/unsynn/#parse-predicates

use crate::{
    Cons, Either, Error, Except, Expect, Parse, Parser, Result, ToTokens, TokenIter, TokenStream,
    TokenTree,
};
use std::any::TypeId;
use std::marker::PhantomData;

/// Marker trait for compile-time parser predicates.
///
/// All predicate types are zero-sized with no runtime cost.
/// The `'static` bound is required for type identity checking with [`PredicateCmp`].
pub trait PredicateOp: Parser + ToTokens + Clone + 'static {}

// =============================================================================
// BASE VALUES
// =============================================================================

/// Always succeeds without consuming tokens.
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "foo bar".to_token_iter();
/// let result = Enable::parse(&mut tokens);
/// assert!(result.is_ok());
/// assert_eq!(tokens.clone().count(), 2); // No tokens consumed
/// ```
#[derive(Debug, Clone, Default)]
pub struct Enable;

impl Parser for Enable {
    #[inline]
    #[mutants::skip] // Mutant Ok(Default::default()) is equivalent since Enable: Default
    fn parser(_tokens: &mut TokenIter) -> Result<Self> {
        Ok(Enable)
    }
}

impl ToTokens for Enable {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}

impl PredicateOp for Enable {}

/// Always fails without consuming tokens.
///
/// Note: Does not implement [`Default`].
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "foo bar".to_token_iter();
/// let result = Disable::parse(&mut tokens);
/// assert!(result.is_err());
/// ```
#[derive(Debug, Clone)]
pub struct Disable;

impl Parser for Disable {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Error::unexpected_token(None, tokens)
    }
}

impl ToTokens for Disable {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}

impl PredicateOp for Disable {}

/// Succeeds only when tokens remain in the stream.
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
/// use unsynn::predicates::*;
///
/// let mut tokens = "foo".to_token_iter();
/// assert!(TokensRemain::parse(&mut tokens).is_ok());
///
/// let mut empty = "".to_token_iter();
/// assert!(TokensRemain::parse(&mut empty).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct TokensRemain;

impl Parser for TokensRemain {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Expect::<TokenTree>::parse(tokens)?;
        Ok(TokensRemain)
    }
}

impl ToTokens for TokensRemain {
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {}
}

impl Default for TokensRemain {
    #[inline]
    fn default() -> Self {
        TokensRemain
    }
}

impl PredicateOp for TokensRemain {}

// =============================================================================
// LOGICAL NOT
// =============================================================================

/// Logical NOT: succeeds if inner predicate fails.
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "".to_token_iter();
/// assert!(Not::<Disable>::parse(&mut tokens).is_ok());
/// assert!(Not::<Enable>::parse(&mut tokens).is_err());
/// ```
#[derive(Debug, Clone)]
pub struct Not<T: PredicateOp>(Except<T>);

impl<T: PredicateOp> Parser for Not<T> {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Except::<T>::parse(tokens).map(Not)
    }
}

impl<T: PredicateOp> ToTokens for Not<T> {
    #[inline]
    #[mutants::skip] // Inner type produces no tokens, so mutation to () is equivalent
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<T: PredicateOp> Default for Not<T>
where
    Except<T>: Default,
{
    #[inline]
    fn default() -> Self {
        Not(Except::default())
    }
}

impl<T: PredicateOp> PredicateOp for Not<T> {}

// =============================================================================
// LOGICAL AND (2-4 operands)
// =============================================================================

/// Logical AND: 2-4 predicates must all succeed.
///
/// C and D default to [`Enable`] (always succeeds).
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "".to_token_iter();
/// // Two predicates
/// assert!(AllOf::<Enable, Enable>::parse(&mut tokens).is_ok());
/// assert!(AllOf::<Enable, Disable>::parse(&mut tokens).is_err());
/// // Four predicates
/// assert!(AllOf::<Enable, Enable, Enable, Enable>::parse(&mut tokens).is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct AllOf<
    A: PredicateOp,
    B: PredicateOp,
    C: PredicateOp + 'static = Enable,
    D: PredicateOp + 'static = Enable,
>(Cons<A, B, C, D>);

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Parser for AllOf<A, B, C, D> {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Cons::<A, B, C, D>::parse(tokens).map(AllOf)
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> ToTokens
    for AllOf<A, B, C, D>
{
    #[inline]
    #[mutants::skip] // Inner type produces no tokens, so mutation to () is equivalent
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Default for AllOf<A, B, C, D>
where
    Cons<A, B, C, D>: Default,
{
    #[inline]
    fn default() -> Self {
        AllOf(Cons::default())
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> PredicateOp
    for AllOf<A, B, C, D>
{
}

// =============================================================================
// LOGICAL OR (2-4 operands)
// =============================================================================

/// Logical OR: 2-4 predicates, at least one must succeed.
///
/// C and D default to [`Disable`] (always fails).
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "".to_token_iter();
/// // Two predicates
/// assert!(AnyOf::<Enable, Disable>::parse(&mut tokens).is_ok());
/// assert!(AnyOf::<Disable, Disable>::parse(&mut tokens).is_err());
/// // Four predicates
/// assert!(AnyOf::<Disable, Disable, Enable, Disable>::parse(&mut tokens).is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct AnyOf<
    A: PredicateOp,
    B: PredicateOp,
    C: PredicateOp + 'static = Disable,
    D: PredicateOp + 'static = Disable,
>(Either<A, B, C, D>);

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Parser for AnyOf<A, B, C, D> {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        Either::<A, B, C, D>::parse(tokens).map(AnyOf)
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> ToTokens
    for AnyOf<A, B, C, D>
{
    #[inline]
    #[mutants::skip] // Inner type produces no tokens, so mutation to () is equivalent
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Default for AnyOf<A, B, C, D>
where
    Either<A, B, C, D>: Default,
{
    #[inline]
    fn default() -> Self {
        AnyOf(Either::default())
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> PredicateOp
    for AnyOf<A, B, C, D>
{
}

// =============================================================================
// LOGICAL XOR (2-4 operands)
// =============================================================================

/// Logical XOR: exactly one of 2-4 predicates must succeed.
///
/// C and D default to [`Disable`].
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
///
/// let mut tokens = "".to_token_iter();
/// // Two predicates
/// assert!(OneOf::<Enable, Disable>::parse(&mut tokens).is_ok());
/// assert!(OneOf::<Enable, Enable>::parse(&mut tokens).is_err());
/// // Four predicates
/// assert!(OneOf::<Enable, Disable, Disable, Disable>::parse(&mut tokens).is_ok());
/// assert!(OneOf::<Enable, Enable, Disable, Disable>::parse(&mut tokens).is_err());
/// ```
#[allow(clippy::type_complexity)]
#[derive(Debug, Clone)]
pub struct OneOf<
    A: PredicateOp + 'static,
    B: PredicateOp + 'static,
    C: PredicateOp + 'static = Disable,
    D: PredicateOp + 'static = Disable,
>(
    AnyOf<
        AllOf<A, Not<B>, Not<C>, Not<D>>,
        AllOf<Not<A>, B, Not<C>, Not<D>>,
        AllOf<Not<A>, Not<B>, C, Not<D>>,
        AllOf<Not<A>, Not<B>, Not<C>, D>,
    >,
);

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Parser for OneOf<A, B, C, D> {
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        AnyOf::<
            AllOf<A, Not<B>, Not<C>, Not<D>>,
            AllOf<Not<A>, B, Not<C>, Not<D>>,
            AllOf<Not<A>, Not<B>, C, Not<D>>,
            AllOf<Not<A>, Not<B>, Not<C>, D>,
        >::parse(tokens)
        .map(OneOf)
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> ToTokens
    for OneOf<A, B, C, D>
{
    #[inline]
    #[mutants::skip] // Inner type produces no tokens, so mutation to () is equivalent
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> Default for OneOf<A, B, C, D>
where
    AnyOf<
        AllOf<A, Not<B>, Not<C>, Not<D>>,
        AllOf<Not<A>, B, Not<C>, Not<D>>,
        AllOf<Not<A>, Not<B>, C, Not<D>>,
        AllOf<Not<A>, Not<B>, Not<C>, D>,
    >: Default,
{
    #[inline]
    fn default() -> Self {
        OneOf(AnyOf::default())
    }
}

impl<A: PredicateOp, B: PredicateOp, C: PredicateOp, D: PredicateOp> PredicateOp
    for OneOf<A, B, C, D>
{
}

// =============================================================================
// TYPE IDENTITY CHECKING
// =============================================================================

/// Predicate that compares type `A` with type `B` at runtime.
///
/// Acts like `Same` when `A == B` (=[`Enable`]), and `Different`when `A != B` (=[`Disable`]).
/// This enables distinguishing between different predicate flags at compile time through
/// runtime type checks (which are usually optimized away by the compiler).
///
/// # Examples
///
/// ```rust
/// use unsynn::*;
/// //use unsynn::predicates::*;
///
/// unsynn! {
///     predicatetrait Context;
///     predicateflag InExpr for Context;
///     predicateflag InStmt for Context;
///
///     // Only accepts InExpr, rejects InStmt
///     struct OnlyInExpr<C: Context> {
///         _guard: PredicateCmp<C, InExpr>,
///         content: Ident,
///     }
///
///     // Accepts InExpr but NOT InStmt
///     struct ExprButNotStmt<C: Context> {
///         _guard: AllOf<PredicateCmp<C, InExpr>, Not<PredicateCmp<C, InStmt>>>,
///         content: Ident,
///     }
/// }
/// ```
#[derive(Debug)]
pub struct PredicateCmp<
    A: PredicateOp,
    B: PredicateOp,
    Same: PredicateOp = Enable,
    Different: PredicateOp = Disable,
>(PhantomData<(A, B, Same, Different)>);

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> PredicateOp
    for PredicateCmp<A, B, Same, Different>
{
}

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp>
    PredicateCmp<A, B, Same, Different>
{
    /// Create a new `PredicateCmp` instance.
    ///
    /// This is primarily useful for testing or when you need to manually
    /// construct the predicate.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        PredicateCmp(PhantomData)
    }
}

// Manual Clone impl to avoid requiring A: Clone, B: Clone bounds on type parameters.
// We implement Clone explicitly even though PredicateCmp is Copy because the type
// parameters A and B don't need to be Clone - they're only used in PhantomData.
#[mutants::skip]
#[allow(clippy::expl_impl_clone_on_copy)]
impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> Clone
    for PredicateCmp<A, B, Same, Different>
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> Copy
    for PredicateCmp<A, B, Same, Different>
{
}

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> Parser
    for PredicateCmp<A, B, Same, Different>
{
    #[inline]
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        // Check type equality at compile/runtime
        if TypeId::of::<A>() == TypeId::of::<B>() {
            // Types match
            Same::parse(tokens)?;
        } else {
            // Types don't match
            Different::parse(tokens)?;
        }
        Ok(Self(PhantomData))
    }
}

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> ToTokens
    for PredicateCmp<A, B, Same, Different>
{
    #[inline]
    fn to_tokens(&self, _tokens: &mut TokenStream) {
        // Zero-sized, emits nothing
    }
}

impl<A: PredicateOp, B: PredicateOp, Same: PredicateOp, Different: PredicateOp> Default
    for PredicateCmp<A, B, Same, Different>
{
    #[inline]
    fn default() -> Self {
        // Check at const time if possible, otherwise runtime
        if TypeId::of::<A>() == TypeId::of::<B>() {
            PredicateCmp(PhantomData)
        } else {
            // This will panic if called with mismatched types
            // But it's safe because Default should only be called when it's valid
            panic!("PredicateCmp::default() called with mismatched types")
        }
    }
}
