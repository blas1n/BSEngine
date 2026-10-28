use crate::{TokenIter, TokenStream, TokenTree};
use std::sync::Arc;

/// Result type for parsing.
pub type Result<T> = std::result::Result<T, Error>;

// To keep the Error passing simple and allocation free for the common cases we define these
// common cases plus adding the generic case as dyn boxed error.
/// Actual kind of an error.
#[derive(Clone)]
pub enum ErrorKind {
    /// A no error state that can be upgraded by later errors.
    NoError,
    /// Parser failed.
    UnexpectedToken,
    /// `RangedRepeats` with invalid bound.
    OutOfRange {
        /// The min/max bound that failed
        have: usize,
        /// The MIN/MAX defined in the type
        want: usize,
    },
    /// A repeating parser detected that the inner parser succeeded without consuming any tokens.
    InfiniteLoop {
        /// The type that succeeded without consuming tokens
        parser_type: &'static str,
    },
    /// Something else failed which can be formatted as `String`.
    Other {
        /// explanation what failed
        reason: String,
    },
    /// Any other error.
    Dynamic(Arc<dyn std::error::Error>),
}

/// Error type for parsing.
#[must_use]
#[derive(Clone)]
pub struct Error {
    /// Kind of the error.
    pub kind: ErrorKind,
    /// type name of what was expected
    expected: &'static str,
    /// refines type name for complex parsers
    refined: Option<&'static str>,
    at: Option<TokenTree>,
    /// Cloned iterator positioned at the error
    after: Option<TokenIter>,
    // TokenIter position where it happened
    // on disjunct parsers we use this to determine which error to keep
    pos: usize,
}

impl Error {
    /// Create a `ErrorKind::NoError` error.
    #[allow(clippy::missing_errors_doc)]
    pub const fn no_error() -> Self {
        Error {
            kind: ErrorKind::NoError,
            expected: "<NoError>",
            refined: None,
            at: None,
            after: None,
            pos: 0,
        }
    }

    /// Upgrade an error to one with greater pos value.
    #[allow(clippy::missing_errors_doc)]
    pub fn upgrade<T>(&mut self, r: Result<T>) -> Result<T> {
        if let Err(other) = &r {
            if matches!(self.kind, ErrorKind::NoError) || other.pos > self.pos {
                *self = other.clone();
            }
        }
        r
    }

    /// Set the position of the error.
    ///
    /// Sometimes the position of the error is not known at the time of creation. This allows
    /// to adjust it later.
    pub fn set_pos(&mut self, pos: impl TokenCount) {
        self.pos = pos.token_count();
    }

    /// Get the position of the error.
    #[must_use]
    pub const fn pos(&self) -> usize {
        self.pos
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::UnexpectedToken })` error at a token iter position.
    /// Takes the failed token (if available) and a reference to the `TokenIter` past the error.
    ///
    /// # Note on position tracking
    /// The `pos` field is set to `after.counter()`, which represents the number of tokens
    /// that were consumed up to and including the failed token (if any):
    /// - If `at` is `Some(token)`: The token was consumed by `next()` which incremented the counter,
    ///   so `pos` reflects the position AFTER consuming the failed token.
    /// - If `at` is `None`: The iterator was exhausted, so `pos` reflects how many tokens were
    ///   successfully consumed before running out.
    // This semantic is crucial for `Either<>` to correctly identify which alternative parsed
    // furthest into the token stream.
    #[allow(clippy::missing_errors_doc)]
    pub fn unexpected_token<T>(at: Option<TokenTree>, after: &TokenIter) -> Result<T> {
        // Clone the iterator and collect remaining tokens into a new TokenStream
        let pos = after.counter();
        Err(Error {
            kind: ErrorKind::UnexpectedToken,
            expected: std::any::type_name::<T>(),
            refined: None,
            at,
            after: Some(after.clone()),
            pos,
        })
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::UnexpectedToken })` error without a token iter.
    #[allow(clippy::missing_errors_doc)]
    pub fn unexpected_end<T>() -> Result<T> {
        Err(Error {
            kind: ErrorKind::UnexpectedToken,
            expected: std::any::type_name::<T>(),
            refined: None,
            at: None,
            after: None,
            pos: usize::MAX,
        })
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::OutOfRange })` error at a token iter position.
    /// Takes the failed token (if available) and a reference to the `TokenIter` past the error.
    #[allow(clippy::missing_errors_doc)]
    pub fn out_of_range<const LIM: usize, T>(
        have: usize,
        at: Option<TokenTree>,
        after: &TokenIter,
    ) -> Result<T> {
        let pos = after.counter();
        Err(Error {
            kind: ErrorKind::OutOfRange { have, want: LIM },
            expected: std::any::type_name::<T>(),
            refined: None,
            at,
            after: Some(after.clone()),
            pos,
        })
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::Other })` error.  Takes the failed
    /// token (if available), a reference to the `TokenIter` past the error and a `String`
    /// describing the error.
    #[allow(clippy::missing_errors_doc)]
    pub fn other<T>(at: Option<TokenTree>, after: &TokenIter, reason: String) -> Result<T> {
        let pos = after.counter();
        Err(Error {
            kind: ErrorKind::Other { reason },
            expected: std::any::type_name::<T>(),
            refined: None,
            at,
            after: Some(after.clone()),
            pos,
        })
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::InfiniteLoop })` error. Used when a
    /// repeating (conatianer) parser detects that the inner parser succeeded without
    /// consuming any tokens, which would lead to an infinite loop.
    #[allow(clippy::missing_errors_doc)]
    pub fn infinite_loop<T>(at: Option<TokenTree>, after: &TokenIter) -> Result<T> {
        let pos = after.counter();
        Err(Error {
            kind: ErrorKind::InfiniteLoop {
                parser_type: std::any::type_name::<T>(),
            },
            expected: std::any::type_name::<T>(),
            refined: None,
            at,
            after: Some(after.clone()),
            pos,
        })
    }

    /// Create a `Result<T>::Err(Error{ kind: ErrorKind::Dynamic })` error.  Takes the failed
    /// token (if available), a reference to the `TokenIter` past the error and a boxed error.
    #[allow(clippy::missing_errors_doc)]
    pub fn dynamic<T>(
        at: Option<TokenTree>,
        after: &TokenIter,
        error: impl std::error::Error + Send + Sync + 'static,
    ) -> Result<T> {
        let pos = after.counter();
        Err(Error {
            kind: ErrorKind::Dynamic(Arc::new(error)),
            expected: std::any::type_name::<T>(),
            refined: None,
            at,
            after: Some(after.clone()),
            pos,
        })
    }

    /// Returns the refined type name of the parser that failed.
    #[must_use]
    pub fn expected_type_name(&self) -> &'static str {
        self.refined.unwrap_or(self.expected)
    }

    /// Returns the original/fundamental type name of the parser that failed.
    #[must_use]
    pub const fn expected_original_type_name(&self) -> &'static str {
        self.expected
    }

    /// Returns a `Option<TokenTree>` where the error happend.
    #[must_use]
    pub fn failed_at(&self) -> Option<TokenTree> {
        self.at.clone()
    }

    /// Returns a iterator to the tokens after the error
    ///
    /// Creates a new `TokenIter` from the stored token stream position
    #[must_use]
    pub fn tokens_after(&self) -> TokenIter {
        self.after
            .clone()
            .unwrap_or_else(|| TokenIter::new(TokenStream::new()))
    }
}

impl std::error::Error for Error {}

impl std::fmt::Debug for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::NoError => {
                write!(f, "NoError")
            }
            ErrorKind::UnexpectedToken => {
                write!(
                    f,
                    "Unexpected token: expected {}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::OutOfRange { have, want } => {
                write!(
                    f,
                    "RangedRepeats out of bounds: expected {want}, requested {have} at {:?}",
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::InfiniteLoop { parser_type } => {
                write!(
                    f,
                    "Infinite loop detected: parser {} succeeded without consuming tokens at {:?}",
                    parser_type,
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::Other { reason } => {
                write!(
                    f,
                    "Parser failed: expected {}, because {reason}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::Dynamic(err) => {
                write!(
                    f,
                    "Parser failed: expected {}, because {err}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
        }
    }
}

impl std::fmt::Display for Error {
    #[cfg_attr(test, mutants::skip)]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match &self.kind {
            ErrorKind::NoError => {
                write!(f, "NoError")
            }
            ErrorKind::UnexpectedToken => {
                write!(
                    f,
                    "Unexpected token: expected {}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::OutOfRange { have, want } => {
                write!(
                    f,
                    "RangedRepeats out of bounds: expected {want}, requested {have} at {:?}",
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::InfiniteLoop { parser_type } => {
                write!(
                    f,
                    "Infinite loop detected: parser {} succeeded without consuming tokens at {:?}",
                    parser_type,
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::Other { reason } => {
                write!(
                    f,
                    "Parser failed: expected {}, because {reason}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
            ErrorKind::Dynamic(err) => {
                write!(
                    f,
                    "Parser failed: expected {}, because {err}, found {:?} at {:?}",
                    self.expected_type_name(),
                    OptionPP(&self.at),
                    OptionPP(&self.at.as_ref().map(|s| s.span().start()))
                )
            }
        }
    }
}

/// Helper Trait for refining error type names. Every parser type in unsynn eventually tries
/// to parse one of the fundamental types. When parsing fails then that fundamental type name
/// is recorded as expected type name of the error. Often this is not desired, a user wants to
/// know the type of parser that actually failed. Since we don't want to keep a stack/vec of
/// errors for simplicity and performance reasons we provide a way to register refined type
/// names in errors. Note that this refinement should only be applied to leaves in the
/// AST. Refining errors on composed types will lead to unexpected results.
pub trait RefineErr {
    /// Refines a errors type name to the type name of `T`.
    #[must_use]
    fn refine_err<T>(self) -> Self
    where
        Self: Sized;
}

impl<T> RefineErr for Result<T> {
    fn refine_err<U>(mut self) -> Self
    where
        Self: Sized,
    {
        if let Err(ref mut err) = self {
            err.refined = Some(std::any::type_name::<U>());
        }
        self
    }
}

/// Pretty printer for Options, either prints `None` or `T` without the enclosing Some.
struct OptionPP<'a, T>(&'a Option<T>);

impl<T: std::fmt::Debug> std::fmt::Debug for OptionPP<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{value:?}"),
            None => write!(f, "None"),
        }
    }
}

impl<T: std::fmt::Display> std::fmt::Display for OptionPP<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(value) => write!(f, "{value}"),
            None => write!(f, "None"),
        }
    }
}

#[test]
fn test_optionpp() {
    let none = format!("{}", OptionPP::<i32>(&None));
    assert_eq!(none, "None");
    let populated = format!("{}", OptionPP(&Some(42)));
    assert_eq!(populated, "42");
    let none = format!("{:?}", OptionPP::<i32>(&None));
    assert_eq!(none, "None");
    let populated = format!("{:?}", OptionPP(&Some(42)));
    assert_eq!(populated, "42");
}

/// We track the position of the error by counting tokens. This trait is implemented for
/// references to shadow counted `TokenIter`, and `usize`. The later allows to pass in a
/// position directly or use `usize::MAX` in case no position data is available (which will
/// make this error the be the final one when upgrading).
pub trait TokenCount {
    /// Get the position of the token iterator.
    fn token_count(self) -> usize;
}

// Allows passing a usize directly.
impl TokenCount for usize {
    #[inline]
    fn token_count(self) -> usize {
        self
    }
}

impl TokenCount for &TokenIter {
    #[inline]
    fn token_count(self) -> usize {
        self.counter()
    }
}

impl TokenCount for &mut TokenIter {
    #[inline]
    fn token_count(self) -> usize {
        self.counter()
    }
}

// implementing for &&mut allows us to pass a &mut TokenIter by reference when it is still needed
// later. Otherwise it would need to be reborrow '&mut *iter' which is less ergonomic.
impl TokenCount for &&mut TokenIter {
    #[inline]
    fn token_count(self) -> usize {
        self.counter()
    }
}
