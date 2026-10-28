//! Expression parser building blocks for creating operator precedence parsers.
//!
//! This module provides generic types for building expression parsers with proper operator
//! precedence and associativity. These are the fundamental building blocks that can be
//! composed to create complete expression grammars.
//!
//! # Expression Parser Types
//!
//! - [`InfixExpr`] (or [`NonAssocExpr`]) - Non-associative binary operators (e.g., comparison operators)
//! - [`PrefixExpr`] - Unary prefix operators (e.g., `-`, `!`, `*`, `&`)
//! - [`PostfixExpr`] - Unary postfix operators (e.g., `?`, `!`)
//! - [`LeftAssocExpr`] - Left-associative binary operators (e.g., `+`, `-`, `*`, `/`)
//! - [`RightAssocExpr`] - Right-associative binary operators (e.g., `=`, `+=`)
//!
//! For complete examples of building expression parsers with proper precedence hierarchies,
//! see the [Expression Parsing](../COOKBOOK.md#expression-parsing) section in the cookbook.
//!
//! Left/Right associativity is not handled by the parser, we parse flat sequences. We only
//! distinguish it here at parsing level for handing the AST structure to the later consumer.
//!
//!
//! # No Nesting Required
//!
//! All expression types use a vector internally to store multiple operators or operands.
//! This means **you don't need to nest expression types** - the vec handles multiplicity:
//!
//! - ✅ `PrefixExpr<Op, Operand>` - Handles 0..MAX operators via `DelimitedVec` (++x)
//! - ✅ `PostfixExpr<Operand, Op>` - Handles 0..MAX operators via `DelimitedVec` (x--)
//! - ✅ `LeftAssocExpr<Operand, Op>` and `RightAssocExpr<Operand, Op>` - Handle 1..MAX operands via `DelimitedVec` (1+2+3)
//! - ❌ `PrefixExpr<Op, PrefixExpr<Op, Operand>>` - Redundant! Don't nest these types.
//!
//! Since we use vecs internally, `Box` is only needed for recursive expression grammars
//! (e.g., parenthesized expressions), not for handling multiple operators.
//!
//! # Handling Parenthesized Expressions
//!
//! Parenthesized expressions are **not** part of these building blocks. They need special
//! handling in your grammar because they allow overriding operator precedence.
//!
//! The user defines how to integrate parentheses by including them in their primary expression type.
//!
//! # Examples
//!
//! ```rust
//! # use unsynn::*;
//!
//! unsynn! {
//!     // For simple binary operations (non-associative):
//!     type SimpleBinary = NonAssocExpr<LiteralInteger, Plus>;
//!
//!     // For left-associative chains (unlimited by default):
//!     type Addition = LeftAssocExpr<LiteralInteger, Plus>;
//! }
//!
//! // Parse a simple addition
//! let mut tokens = "1 + 2".to_token_iter();
//! let expr: Addition = tokens.parse().unwrap();
//! assert_eq!(expr.len(), 2); // 2 operands
//! ```

use crate::{
    DelimitedVec, Nothing, Parse, Parser, Result, ToTokens, TokenIter, TokenStream,
    TrailingDelimiter,
};
use std::ops::{Deref, DerefMut};

/// Generic infix operator expression.
///
/// Parses: `Operand (Op Operand)?`  (minimum 1 operand, maximum 2 operands by default)
///
/// This is implemented as a `DelimitedVec` with:
/// - `MIN = 1`: At least one operand must be present (the left operand)
/// - `MAX = 2`: At most two operands (left and optional right), can be overridden
/// - `TrailingDelimiter::Forbidden`: No trailing operator allowed
///
/// # Type Parameters
///
/// - `Operand`: The type of expressions that can appear on either side of the operator
/// - `Operator`: The operator type (typically an enum of different operators at this precedence level)
/// - `MAX`: Maximum number of operands (defaults to 2 for binary, can be set to limit chaining)
pub type InfixExpr<Operand, Operator, const MAX: usize = 2> =
    DelimitedVec<Operand, Operator, TrailingDelimiter::Forbidden, 1, MAX>;

/// Type alias for non-associative binary operators.
///
/// Non-associative operators cannot be chained without parentheses. For example,
/// comparison operators in Rust: `a == b == c` is a syntax error, must be
/// `(a == b) == c` or `a == (b == c)`.
///
/// This is a convenience alias for [`InfixExpr`] with default MAX=2, intended for
/// documenting non-associative operators in expression grammars.
///
/// # Examples
///
/// ```rust
/// # use unsynn::*;
/// # use unsynn::operator::names::Equal;
///
/// // Comparison operators are typically non-associative
/// type ComparisonExpr = NonAssocExpr<LiteralInteger, Equal>;
///
/// let mut tokens = "1 == 2".to_token_iter();
/// let expr: ComparisonExpr = tokens.parse().unwrap();
/// assert_eq!(expr.len(), 2);
/// ```
pub type NonAssocExpr<Operand, Operator> = InfixExpr<Operand, Operator>;

/// Prefix unary operator expression.
///
/// Parses: `Op* Operand`  (0 to MAX operators, then operand)
///
/// Allows zero or more prefix operators before an operand. Multiple prefix operators
/// are applied right-to-left (innermost first). For example, `--x` is parsed as
/// `-(-x)` (two negations).
///
/// # Type Parameters
///
/// - `Operator`: The prefix operator type (typically an enum of different unary operators)
/// - `Operand`: The type of expression that the operators apply to
/// - `MAX`: Maximum number of prefix operators (defaults to `usize::MAX` for unlimited)
///
/// # Examples
///
/// ```rust
/// # use unsynn::*;
/// # use unsynn::operator::names::Minus;
///
/// unsynn! {
///     type Number = LiteralInteger;
/// }
///
/// // Parse "- - 5"
/// let mut tokens = "--5".to_token_iter();
/// let expr: PrefixExpr<Minus, Number> = tokens.parse().unwrap();
/// assert_eq!(expr.operators.len(), 2); // 2 Minus operators
///
/// // Limit prefix depth
/// type LimitedPrefix = PrefixExpr<Minus, Number, 3>;  // Max 3 prefix operators
/// ```
#[derive(Clone, Debug)]
pub struct PrefixExpr<Operator, Operand, const MAX: usize = { usize::MAX }> {
    /// The prefix operators (0 to MAX, applied right-to-left)
    pub operators: DelimitedVec<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>,
    /// The operand that the operators apply to
    pub operand: Operand,
}

impl<Operator, Operand, const MAX: usize> PrefixExpr<Operator, Operand, MAX> {
    /// Create a new `PrefixExpr` with the given operators and operand.
    pub const fn new(
        operators: DelimitedVec<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>,
        operand: Operand,
    ) -> Self {
        Self { operators, operand }
    }
}

/// Postfix unary operator expression.
///
/// Parses: `Operand Op*`  (operand, then 0 to MAX operators)
///
/// Allows zero or more postfix operators after an operand. Multiple postfix operators
/// are applied left-to-right (leftmost first). For example, `x??` is parsed as
/// `(x?)?` (try operators chain left-to-right).
///
/// # Type Parameters
///
/// - `Operand`: The type of expression that the operators apply to
/// - `Operator`: The postfix operator type (typically an enum of different postfix operations)
/// - `MAX`: Maximum number of postfix operators (defaults to `usize::MAX` for unlimited)
///
/// # Examples
///
/// ```rust
/// # use unsynn::*;
/// # use unsynn::operator::names::Bang;
///
/// unsynn! {
///     type Number = LiteralInteger;
/// }
///
/// // Parse "5 ! !"
/// let mut tokens = "5 ! !".to_token_iter();
/// let expr: PostfixExpr<Number, Bang> = tokens.parse().unwrap();
/// assert_eq!(expr.operators.len(), 2); // 2 Bang operators
///
/// // Limit postfix depth
/// type LimitedPostfix = PostfixExpr<Number, Bang, 3>;  // Max 3 postfix operators
/// ```
#[derive(Clone, Debug)]
pub struct PostfixExpr<Operand, Operator, const MAX: usize = { usize::MAX }> {
    /// The operand that the operators apply to
    pub operand: Operand,
    /// The postfix operators (0 to MAX, applied left-to-right)
    pub operators: DelimitedVec<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>,
}

impl<Operand, Operator, const MAX: usize> PostfixExpr<Operand, Operator, MAX> {
    /// Create a new `PostfixExpr` with the given operand and operators.
    pub const fn new(
        operand: Operand,
        operators: DelimitedVec<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>,
    ) -> Self {
        Self { operand, operators }
    }
}

impl<Operator, Operand, const MAX: usize> Parser for PrefixExpr<Operator, Operand, MAX>
where
    Operator: Parse + ToTokens,
    Operand: Parse + ToTokens,
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let operators =
            DelimitedVec::<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>::parser(
                tokens,
            )?;
        let operand = Operand::parser(tokens)?;
        Ok(Self { operators, operand })
    }
}

impl<Operand, Operator, const MAX: usize> Parser for PostfixExpr<Operand, Operator, MAX>
where
    Operand: Parse + ToTokens,
    Operator: Parse + ToTokens,
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let operand = Operand::parser(tokens)?;
        let operators =
            DelimitedVec::<Operator, Nothing, TrailingDelimiter::Mandatory, 0, MAX>::parser(
                tokens,
            )?;
        Ok(Self { operand, operators })
    }
}

impl<Operator, Operand, const MAX: usize> ToTokens for PrefixExpr<Operator, Operand, MAX>
where
    Operator: ToTokens,
    Operand: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.operators.to_tokens(tokens);
        self.operand.to_tokens(tokens);
    }
}

impl<Operand, Operator, const MAX: usize> ToTokens for PostfixExpr<Operand, Operator, MAX>
where
    Operand: ToTokens,
    Operator: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.operand.to_tokens(tokens);
        self.operators.to_tokens(tokens);
    }
}

/// Left-associative infix operator expression.
///
/// Parses: `Operand (Op Operand)*`  (minimum 1 operand, unlimited by default)
///
/// Left-associative operators group left-to-right when chained. For example,
/// `a + b + c` should be interpreted as `((a + b) + c)`.
///
/// This is a newtype wrapper around `InfixExpr` that allows unlimited chaining by default.
/// During parsing, associativity doesn't matter - we parse a flat sequence. The
/// associativity only affects evaluation order, which is handled elsewhere.
///
/// # Type Parameters
///
/// - `Operand`: The type of expressions that can appear as operands
/// - `Operator`: The operator type (typically an enum of different operators at this precedence level)
/// - `MAX`: Maximum number of operands (defaults to `usize::MAX` for unlimited chaining)
///
/// # Examples
///
/// ```
/// # use unsynn::*;
/// # use unsynn::operator::names::Plus;
///
/// unsynn! {
///     type Number = LiteralInteger;
/// }
///
/// // Parse "1 + 2 + 3 + 4"
/// let mut tokens = "1 + 2 + 3 + 4".to_token_iter();
/// let expr: LeftAssocExpr<Number, Plus> = tokens.parse().unwrap();
/// assert_eq!(expr.len(), 4); // 4 operands
///
/// // With depth limit
/// type LimitedChain = LeftAssocExpr<Number, Plus, 10>;  // Max 10 operands
/// ```
#[derive(Clone, Debug)]
pub struct LeftAssocExpr<Operand, Operator, const MAX: usize = { usize::MAX }>(
    pub InfixExpr<Operand, Operator, MAX>,
);

impl<Operand, Operator, const MAX: usize> Deref for LeftAssocExpr<Operand, Operator, MAX> {
    type Target = InfixExpr<Operand, Operator, MAX>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Operand, Operator, const MAX: usize> DerefMut for LeftAssocExpr<Operand, Operator, MAX> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Operand, Operator, const MAX: usize> Parser for LeftAssocExpr<Operand, Operator, MAX>
where
    Operand: Parse + ToTokens,
    Operator: Parse + ToTokens,
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        InfixExpr::<Operand, Operator, MAX>::parser(tokens).map(Self)
    }
}

impl<Operand, Operator, const MAX: usize> ToTokens for LeftAssocExpr<Operand, Operator, MAX>
where
    Operand: ToTokens,
    Operator: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}

/// Right-associative infix operator expression.
///
/// Parses: `Operand (Op Operand)*`  (minimum 1 operand, unlimited by default)
///
/// Right-associative operators group right-to-left when chained. For example,
/// `a = b = c` should be interpreted as `a = (b = c)`.
///
/// # Type Parameters
///
/// - `Operand`: The type of expressions that can appear as operands
/// - `Operator`: The operator type (typically an enum of different operators at this precedence level)
/// - `MAX`: Maximum number of operands (defaults to `usize::MAX` for unlimited chaining)
///
/// # Examples
///
/// ```
/// # use unsynn::*;
///
/// unsynn! {
///     type Number = LiteralInteger;
///     
///     operator Caret = "^";
/// }
///
/// // Parse "2 ^ 3 ^ 4"
/// let mut tokens = "2 ^ 3 ^ 4".to_token_iter();
/// let expr: RightAssocExpr<Number, Caret> = tokens.parse().unwrap();
/// assert_eq!(expr.len(), 3); // 3 operands
///
/// // With depth limit
/// type LimitedChain = RightAssocExpr<Number, Caret, 10>;  // Max 10 operands
/// ```
#[derive(Clone, Debug)]
pub struct RightAssocExpr<Operand, Operator, const MAX: usize = { usize::MAX }>(
    pub InfixExpr<Operand, Operator, MAX>,
);

impl<Operand, Operator, const MAX: usize> Deref for RightAssocExpr<Operand, Operator, MAX> {
    type Target = InfixExpr<Operand, Operator, MAX>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Operand, Operator, const MAX: usize> DerefMut for RightAssocExpr<Operand, Operator, MAX> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<Operand, Operator, const MAX: usize> Parser for RightAssocExpr<Operand, Operator, MAX>
where
    Operand: Parse + ToTokens,
    Operator: Parse + ToTokens,
{
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        InfixExpr::<Operand, Operator, MAX>::parser(tokens).map(Self)
    }
}

impl<Operand, Operator, const MAX: usize> ToTokens for RightAssocExpr<Operand, Operator, MAX>
where
    Operand: ToTokens,
    Operator: ToTokens,
{
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.0.to_tokens(tokens);
    }
}
