use crate::flags::{flag_to_bool, Flag, ParseFlag};
use syn::parse::{Parse, ParseStream};
use syn::{Token, TypePath};

/// The arguments for the `assert_type_match` attribute macro.
pub(crate) struct Args {
    foreign_ty: TypePath,
    test_only: Flag,
    skip_name: Flag,
    skip_types: Flag,
    from: Flag,
}

impl Args {
    /// Get the foreign type that the annotated struct or enum should match.
    pub fn foreign_ty(&self) -> &TypePath {
        &self.foreign_ty
    }

    /// Controls whether to output the annotated struct or enum in the generated code.
    pub fn test_only(&self) -> bool {
        flag_to_bool(&self.test_only)
    }

    /// Controls whether a `From` implementation should be generated.
    pub fn from(&self) -> bool {
        flag_to_bool(&self.from)
    }

    /// Controls whether checking the name of the annotated struct or enum should be skipped.
    pub fn skip_name(&self) -> bool {
        flag_to_bool(&self.skip_name)
    }

    /// Controls whether checking field types should be skipped.
    pub fn skip_types(&self) -> bool {
        flag_to_bool(&self.skip_types)
    }
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let foreign_ty = input.parse().map_err(|err| {
            syn::Error::new(err.span(), "expected the type path to a foreign type (e.g. `#[assert_type_match(path::to::ForeignType)]`)")
        })?;

        let mut this = Self {
            foreign_ty,
            test_only: None,
            skip_name: None,
            skip_types: None,
            from: None,
        };

        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;

            if input.is_empty() {
                break;
            }

            let lookahead = input.lookahead1();
            if lookahead.peek(kw::test_only) {
                this.test_only = input.parse_flag::<kw::test_only>()?;
            } else if lookahead.peek(kw::from) {
                this.from = input.parse_flag::<kw::from>()?;
            } else if lookahead.peek(kw::skip_name) {
                this.skip_name = input.parse_flag::<kw::skip_name>()?;
            } else if lookahead.peek(kw::skip_types) {
                this.skip_types = input.parse_flag::<kw::skip_types>()?;
            } else {
                return Err(lookahead.error());
            }
        }

        Ok(this)
    }
}

mod kw {
    syn::custom_keyword!(test_only);
    syn::custom_keyword!(skip_name);
    syn::custom_keyword!(skip_types);
    syn::custom_keyword!(from);
}
