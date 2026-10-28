//! Groups are a way to group tokens together. They are used to represent the contents between
//! `()`, `{}`, `[]` or no delimiters at all.  This module provides parser implementations for
//! opaque group types with defined delimiters and the [`GroupContaining`] types that parses the
//! surrounding delimiters and content of a group type.

#![allow(clippy::module_name_repetitions)]

#[cfg(feature = "proc_macro2")]
pub use proc_macro2::Delimiter;

#[cfg(not(feature = "proc_macro2"))]
pub use proc_macro::Delimiter;

use crate::{
    private, Error, Group, Parse, Parser, Result, ToTokens, TokenIter, TokenStream, TokenTree,
};

// Import the helper function from fundamental module
use crate::fundamental::count_tokens_recursive;

macro_rules! make_group {
    ($($name:ident: $delimiter:ident);* $(;)?) => {
        $(
            #[doc = stringify!(A opaque group of tokens within a $delimiter)]
            #[derive(Debug, Clone)]
            pub struct $name(pub Group);

            impl From<$name> for Group {
                #[inline]
                fn from(group: $name) -> Self {
                    group.0
                }
            }

            impl Parser for $name {
                fn parser(tokens: &mut TokenIter) -> Result<Self> {
                    match tokens.next() {
                        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::$delimiter => {
                            // Count tokens inside the group and advance the shadow counter
                            let nested_count = count_tokens_recursive(group.stream());
                            tokens.add(nested_count);
                            Ok(Self(group))
                        }
                        at => Error::unexpected_token(at, tokens),
                    }
                }
            }

            impl ToTokens for $name {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    self.0.to_tokens(tokens);
                }
            }

            impl private::Sealed for $name {}

            impl GroupDelimiter for $name {
                fn delimiter(&self) -> Delimiter {
                    Delimiter::$delimiter
                }
            }

            impl From<$name> for TokenTree {
                fn from(group: $name) -> Self {
                    group.0.into()
                }
            }
        )*
    };
}

make_group! {
    ParenthesisGroup: Parenthesis;
    BraceGroup: Brace;
    BracketGroup: Bracket;
    NoneGroup: None;
}

#[test]
fn test_bracegroup_into_tt() {
    let mut token_iter = "{a b c}".to_token_iter();
    let group = BraceGroup::parse(&mut token_iter).unwrap();
    let _: TokenTree = group.into();
}

/// Access to the surrounding `Delimiter` of a `GroupContaining` and its variants.
pub trait GroupDelimiter: private::Sealed {
    /// The surrounding `Delimiter` of the group.
    fn delimiter(&self) -> Delimiter;
}

/// Any kind of Group `G` with parseable content `C`.  The content `C` must parse exhaustive,
/// an [`EndOfStream`](crate::EndOfStream) is automatically implied.
#[derive(Clone)]
pub struct GroupContaining<C> {
    /// The delimiters around the group.
    pub delimiter: Delimiter,
    /// The content of the group.
    pub content: C,
}

impl<C> GroupContaining<C> {
    /// Create a new `GroupContaining` instance.
    ///
    /// # Example
    ///
    /// ```
    /// # use unsynn::*;
    ///
    /// let group = GroupContaining::new(
    ///     Delimiter::Parenthesis,
    ///     Literal::i32_unsuffixed(123),
    /// );
    /// # assert_tokens_eq!(group, "(123)");
    /// ```
    pub const fn new(delimiter: Delimiter, content: C) -> Self {
        Self { delimiter, content }
    }
}

impl<C: Parse> Parser for GroupContaining<C> {
    fn parser(tokens: &mut TokenIter) -> Result<Self> {
        let group = Group::parser(tokens)?;
        let delimiter = group.delimiter();

        let content = tokens.parse_group::<C>(group.stream())?;

        Ok(Self { delimiter, content })
    }
}

impl<C: ToTokens> ToTokens for GroupContaining<C> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        Group::new(self.delimiter, self.content.to_token_stream()).to_tokens(tokens);
    }
}

#[mutants::skip]
impl<C: std::fmt::Debug> std::fmt::Debug for GroupContaining<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct(&format!("GroupContaining<{}>", std::any::type_name::<C>()))
            .field("delimiter", &self.delimiter)
            .field("content", &self.content)
            .finish()
    }
}

impl<C> private::Sealed for GroupContaining<C> {}

impl<C> GroupDelimiter for GroupContaining<C> {
    fn delimiter(&self) -> Delimiter {
        self.delimiter
    }
}

impl<C: ToTokens> From<GroupContaining<C>> for TokenTree {
    fn from(group: GroupContaining<C>) -> Self {
        Group::new(group.delimiter(), group.content.to_token_stream()).into()
    }
}

#[test]
fn test_groupcontaining_into_tt() {
    let mut token_iter = "{a b c}".to_token_iter();
    let group = GroupContaining::<TokenStream>::parse(&mut token_iter).unwrap();
    let _: TokenTree = group.into();
}

macro_rules! make_group_containing {
    ($($name:ident: $delimiter:ident);* $(;)?) => {
        $(
            #[doc = stringify!(Parseable content within a $delimiter)]
            #[derive(Clone)]
            pub struct $name<C>{
                /// The inner content of the group.
                pub content: C
            }

            impl<C> $name<C> {
                #[doc = stringify!(create a new $name instance)]
                pub const fn new(content: C) -> Self {
                    Self{content}
                }
            }

            impl<C: Parse> Parser for $name<C> {
                fn parser(tokens: &mut TokenIter) -> Result<Self> {
                    match tokens.next() {
                        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::$delimiter => {
                            let content = tokens.parse_group::<C>(group.stream())?;
                            Ok(Self{content})
                        }
                        at => Error::unexpected_token(at, tokens),
                    }
                }
            }

            impl<C: ToTokens> ToTokens for $name<C> {
                fn to_tokens(&self, tokens: &mut TokenStream) {
                    Group::new(Delimiter::$delimiter, self.content.to_token_stream()).to_tokens(tokens);
                }
            }

            impl<C: std::fmt::Debug> std::fmt::Debug
                for $name<C>
            {
                fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                    f.debug_tuple(&format!(
                        stringify!($name<{}>),
                        std::any::type_name::<C>()
                    ))
                     .field(&self.content)
                     .finish()
                }
            }

            impl<C> private::Sealed for $name<C> {}

            impl<C> GroupDelimiter for $name<C> {
                fn delimiter(&self) -> Delimiter {
                    Delimiter::$delimiter
                }
            }

            impl<C: ToTokens> From<$name<C>> for TokenTree {
                fn from(group: $name<C>) -> Self {
                    Group::new(Delimiter::$delimiter, group.content.to_token_stream()).into()
                }
            }
        )*
    };
}

make_group_containing! {
    ParenthesisGroupContaining: Parenthesis;
    BraceGroupContaining: Brace;
    BracketGroupContaining: Bracket;
    NoneGroupContaining: None;
}

#[test]
fn test_bracegroupcontaining_into_tt() {
    let mut token_iter = "{a b c}".to_token_iter();
    let group = BraceGroupContaining::<TokenStream>::parse(&mut token_iter).unwrap();
    let _: TokenTree = group.into();
}
