use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::token::Token;
use syn::{LitBool, Token};

/// An optional boolean flag, returned by [`ParseFlag::parse_flag`].
pub(crate) type Flag = Option<LitBool>;

/// Extension trait for smartly parsing a boolean flag from a [`ParseStream`].
pub(crate) trait ParseFlag {
    /// Parse the next token(s) in the stream as a [`LitBool`].
    ///
    /// Accepts either:
    /// - An instance of `T`
    /// - An instance of `T` followed by an equals sign and a boolean literal
    ///
    /// For example, both `foo` and `foo = true` will evaluate to `true`.
    /// Whereas, `foo = false` will evaluate to `false`.
    fn parse_flag<T: Parse + Spanned>(&self) -> syn::Result<Flag>;
}

impl<'a> ParseFlag for ParseStream<'a> {
    fn parse_flag<T: Parse + Spanned>(&self) -> syn::Result<Flag> {
        let keyword = self.parse::<T>()?;
        if self.peek(Token![=]) {
            self.parse::<Token![=]>()?;
            Ok(Some(self.parse::<LitBool>()?))
        } else {
            Ok(Some(LitBool::new(true, keyword.span())))
        }
    }
}

/// Converts an optional boolean flag into a `bool`.
pub(crate) fn flag_to_bool(flag: &Flag) -> bool {
    flag.as_ref().map(LitBool::value).unwrap_or_default()
}

/// Attempts to merge two optional boolean flags.
pub(crate) fn merge_flags<T: Token>(base: Flag, other: Flag) -> syn::Result<Flag> {
    match (base, other) {
        (Some(a), Some(b)) if a.value == b.value => Ok(Some(a)),
        (Some(a), None) => Ok(Some(a)),
        (None, Some(b)) => Ok(Some(b)),
        (None, None) => Ok(None),
        (Some(_), Some(b)) => Err(syn::Error::new_spanned(
            b,
            format_args!("conflicting `{}` arguments", T::display()),
        )),
    }
}
