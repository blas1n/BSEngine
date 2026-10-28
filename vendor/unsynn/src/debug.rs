//! Debug utilities for token stream inspection

use crate::{Parser, Result, TokenIter};

// When debug_grammar feature is enabled (or generating docs), implement the full debug functionality
#[cfg(any(doc, feature = "debug_grammar"))]
mod enabled {
    use super::*;
    use crate::IntoTokenIter;
    #[cfg(not(feature = "proc_macro2"))]
    use proc_macro::{Delimiter, TokenTree};
    #[cfg(feature = "proc_macro2")]
    use proc_macro2::{Delimiter, TokenTree};

    /// A debug parser that prints the typename of `T` and the next `N` tokens to stderr.
    ///
    /// This parser clones the token iterator to peek ahead without consuming tokens,
    /// making it useful for debugging parsers without affecting the actual parsing.
    ///
    /// Output format:
    /// - Line 1: `Type: typename`
    /// - Line 2: `Source: literal token representation`
    ///
    /// If there are more tokens than `N`, the output ends with ` …`.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to parse (typename will be printed)
    /// * `N` - The number of tokens to print (default: 5)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// # use unsynn::*;
    /// let mut tokens = "fn foo ( bar : i32 ) { }".to_token_iter();
    ///
    /// // This will print to stderr:
    /// // Type: proc_macro2::Ident
    /// // Source: fn foo (bar : i32) …
    /// let result: StderrLog<Ident, 3> = tokens.parse().unwrap();
    ///
    /// // The actual Ident was also parsed
    /// assert_eq!(result.value.to_string(), "fn");
    /// ```
    pub struct StderrLog<T, const N: usize = 5> {
        /// The parsed value
        pub value: T,
    }

    // we have only debug side-effects here
    #[mutants::skip]
    impl<T: Parser, const N: usize> Parser for StderrLog<T, N> {
        fn parser(tokens: &mut TokenIter) -> Result<Self> {
            // Print the typename
            let typename = std::any::type_name::<T>();
            eprintln!("Type: {}", typename);

            // Clone the iterator to peek without consuming
            let mut peek_iter = tokens.clone();

            // Collect and print the next N tokens
            let mut output = String::new();
            let mut count = 0;
            let mut has_more = false;

            collect_tokens(&mut peek_iter, N, &mut output, &mut count, &mut has_more);

            // Print the second line with tokens
            eprint!("Source: {}", output);
            if has_more {
                eprintln!(" …");
            } else {
                eprintln!();
            }

            // Now actually parse T
            let value = T::parser(tokens)?;

            Ok(StderrLog { value })
        }
    }

    /// Helper function to collect tokens into a string representation
    #[mutants::skip]
    fn collect_tokens(
        tokens: &mut TokenIter,
        mut remaining: usize,
        output: &mut String,
        count: &mut usize,
        has_more: &mut bool,
    ) {
        while remaining > 0 {
            if let Some(token) = tokens.next() {
                *count += 1;
                remaining -= 1;

                match &token {
                    TokenTree::Group(group) => {
                        let (open, close) = match group.delimiter() {
                            Delimiter::Parenthesis => ("(", ")"),
                            Delimiter::Brace => ("{", "}"),
                            Delimiter::Bracket => ("[", "]"),
                            Delimiter::None => ("", ""),
                        };

                        output.push_str(open);

                        // Recurse into the group
                        let group_stream = group.stream();
                        let mut group_iter = group_stream.into_iter().into_token_iter();
                        collect_tokens(&mut group_iter, remaining, output, count, has_more);

                        output.push_str(close);
                    }
                    TokenTree::Ident(ident) => {
                        if !output.is_empty()
                            && !output.ends_with(|c: char| matches!(c, '(' | '[' | '{' | ' '))
                        {
                            output.push(' ');
                        }
                        output.push_str(&ident.to_string());
                    }
                    TokenTree::Punct(punct) => {
                        let ch = punct.as_char();
                        // Add space before certain punctuation for readability
                        if !output.is_empty()
                            && !output.ends_with(|c: char| matches!(c, '(' | '[' | '{' | ' '))
                            && matches!(
                                ch,
                                ':' | '='
                                    | '<'
                                    | '>'
                                    | '!'
                                    | '&'
                                    | '|'
                                    | '+'
                                    | '-'
                                    | '*'
                                    | '/'
                                    | '%'
                            )
                        {
                            output.push(' ');
                        }
                        output.push(ch);
                    }
                    TokenTree::Literal(literal) => {
                        if !output.is_empty()
                            && !output.ends_with(|c: char| matches!(c, '(' | '[' | '{' | ' '))
                        {
                            output.push(' ');
                        }
                        output.push_str(&literal.to_string());
                    }
                }
            } else {
                return;
            }
        }

        // Check if there are more tokens
        if tokens.clone().next().is_some() {
            *has_more = true;
        }
    }
}

// When debug_grammar feature is disabled, provide a no-op implementation
#[cfg(not(any(doc, feature = "debug_grammar")))]
mod disabled {
    use super::{Parser, Result, TokenIter};
    use std::marker::PhantomData;

    /// A no-op debug parser that does nothing when the `debug_grammar` feature is disabled.
    ///
    /// This type becomes a zero-sized type that doesn't parse anything and has no runtime cost.
    /// It exists only to maintain API compatibility when debug output is disabled.
    pub struct StderrLog<T, const N: usize = 5>(PhantomData<T>);

    #[mutants::skip]
    impl<T, const N: usize> Parser for StderrLog<T, N> {
        #[inline]
        fn parser(_tokens: &mut TokenIter) -> Result<Self> {
            // Complete no-op: don't parse T, don't consume tokens
            Ok(StderrLog(PhantomData))
        }
    }

    #[mutants::skip]
    impl<T, const N: usize> crate::ToTokens for StderrLog<T, N> {
        #[inline]
        fn to_tokens(&self, _tokens: &mut crate::TokenStream) {
            // No-op: emit nothing
        }
    }
}

// Re-export the appropriate implementation
#[cfg(any(doc, feature = "debug_grammar"))]
pub use enabled::*;

#[cfg(not(any(doc, feature = "debug_grammar")))]
pub use disabled::*;
