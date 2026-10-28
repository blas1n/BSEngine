use crate::flags::{flag_to_bool, merge_flags, Flag, ParseFlag};
use syn::parse::{Parse, ParseStream};
use syn::Token;

#[derive(Default)]
pub struct FieldArgs {
    skip: Flag,
    skip_type: Flag,
}

impl FieldArgs {
    /// Controls whether the annotated field should be skipped.
    pub fn skip(&self) -> bool {
        flag_to_bool(&self.skip)
    }

    /// Controls whether checking the annotated field's type should be skipped.
    pub fn skip_type(&self) -> bool {
        flag_to_bool(&self.skip_type)
    }

    /// Merges two sets of [field arguments].
    ///
    /// [field arguments]: Self
    pub fn merge(self, other: Self) -> syn::Result<Self> {
        let skip = merge_flags::<kw::skip>(self.skip, other.skip)?;
        let skip_type = merge_flags::<kw::skip_type>(self.skip_type, other.skip_type)?;

        Ok(Self { skip, skip_type })
    }
}

impl Parse for FieldArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut this = Self {
            skip: None,
            skip_type: None,
        };

        loop {
            let lookahead = input.lookahead1();
            if lookahead.peek(kw::skip) {
                this.skip = input.parse_flag::<kw::skip>()?;
            } else if lookahead.peek(kw::skip_type) {
                this.skip_type = input.parse_flag::<kw::skip_type>()?;
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
    syn::custom_keyword!(skip_type);
}
