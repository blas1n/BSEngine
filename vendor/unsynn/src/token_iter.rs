//! Iterator type for parsing token streams.

#[cfg(feature = "proc_macro2")]
use proc_macro2::{TokenStream, TokenTree};

#[cfg(not(feature = "proc_macro2"))]
use proc_macro::{TokenStream, TokenTree};

use crate::EndOfStream;
use crate::Parser;

/// The base iterator
#[derive(Clone)]
#[cfg_attr(feature = "proc_macro2", derive(Debug))]
struct TokenIterBase {
    /// The actual token iterator
    iter: <TokenStream as IntoIterator>::IntoIter,
    /// Counter for this level
    counter: usize,

    #[cfg(feature = "extra_asserts")]
    /// Tracks if a nested iterator has been committed at the current position
    /// Set to true on commit, cleared when parent advances via `.next()`
    committed: bool,
}

impl TokenIterBase {
    fn new(
        iter: impl IntoIterator<IntoIter = <TokenStream as IntoIterator>::IntoIter>,
        start_at: usize,
    ) -> Self {
        Self {
            iter: iter.into_iter(),
            counter: start_at,
            #[cfg(feature = "extra_asserts")]
            committed: false,
        }
    }
}

/// Iterator type for parsing token streams.
///
/// # Examples
///
/// ```rust
/// # use unsynn::*;
/// // Simple parsing
/// let tokens = "foo bar".to_token_iter();
/// ```
#[derive(Clone)]
#[cfg_attr(feature = "proc_macro2", derive(Debug))]
pub struct TokenIter {
    base: TokenIterBase,
}

impl TokenIter {
    /// Create a new owned `TokenIter`
    pub fn new(
        stream: impl IntoIterator<IntoIter = <TokenStream as IntoIterator>::IntoIter>,
    ) -> Self {
        TokenIter {
            base: TokenIterBase::new(stream, 0),
        }
    }
}

impl TokenIter {
    /// Get the current counter value
    #[must_use]
    pub const fn counter(&self) -> usize {
        self.base.counter
    }

    /// Adjust the counter by an offset
    pub(crate) fn add(&mut self, add: usize) {
        self.base.counter += add;
    }

    /// Scoped parsing of a nested group.
    ///
    /// Creates a new iterator with the stream to the inner of the group,
    /// parses type `T`, and ensures all tokens are consumed.
    pub(crate) fn parse_group<T>(
        &mut self,
        stream: impl IntoIterator<IntoIter = <TokenStream as IntoIterator>::IntoIter>,
    ) -> crate::Result<T>
    where
        T: crate::Parser,
    {
        #[cfg(feature = "extra_asserts")]
        assert!(!self.base.committed);
        let mut tokens = TokenIter {
            base: TokenIterBase::new(stream, self.counter()),
        };
        let parsed = T::parser(&mut tokens)?;
        // Must parse exhaustively, otherwise we would miscount tokens
        EndOfStream::parser(&mut tokens)?;
        self.base.counter = tokens.counter();
        #[cfg(feature = "extra_asserts")]
        {
            self.base.committed = true;
        };
        Ok(parsed)
    }
}

// Iterator implementation
impl Iterator for TokenIter {
    type Item = TokenTree;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.base.iter.next()?;
        self.base.counter += 1;
        #[cfg(feature = "extra_asserts")]
        {
            self.base.committed = false;
        };
        Some(item)
    }
}

/// Extension trait to convert iterators into [`TokenIter`].
pub trait IntoTokenIter: Sized {
    /// Convert this iterator into a [`TokenIter`].
    fn into_token_iter(self) -> TokenIter;
}

impl IntoTokenIter for <TokenStream as IntoIterator>::IntoIter {
    fn into_token_iter(self) -> TokenIter {
        TokenIter {
            base: TokenIterBase::new(self, 0),
        }
    }
}

/// Extension trait to convert `TokenStreams` into [`TokenIter`].
pub trait ToTokenIter: Sized {
    /// Convert this into a [`TokenIter`].
    fn to_token_iter(self) -> TokenIter;
}

impl ToTokenIter for TokenStream {
    fn to_token_iter(self) -> TokenIter {
        TokenIter::new(self)
    }
}
