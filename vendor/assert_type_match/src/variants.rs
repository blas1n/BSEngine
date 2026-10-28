use crate::flags::{flag_to_bool, merge_flags, Flag, ParseFlag};
use syn::parse::{Parse, ParseStream};
use syn::Token;

#[derive(Default)]
pub(crate) struct VariantArgs {
    skip: Flag,
}

impl VariantArgs {
    /// Controls whether the annotated variant should be skipped.
    pub fn skip(&self) -> bool {
        flag_to_bool(&self.skip)
    }

    /// Merges two sets of [variant arguments].
    ///
    /// [variant arguments]: Self
    pub fn merge(self, other: Self) -> syn::Result<Self> {
        let skip = merge_flags::<kw::skip>(self.skip, other.skip)?;

        Ok(Self { skip })
    }
}

impl Parse for VariantArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut this = Self { skip: None };

        loop {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::skip) {
                this.skip = input.parse_flag::<kw::skip>()?;
            } else {
                return Err(lookahead.error());
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;

                if input.is_empty() {
                    break;
                }
            } else {
                break;
            }
        }

        Ok(this)
    }
}

mod kw {
    syn::custom_keyword!(skip);
}
