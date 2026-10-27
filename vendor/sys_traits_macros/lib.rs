use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::ItemTrait;

#[proc_macro_attribute]
pub fn auto_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
  let input = parse_macro_input!(item as ItemTrait);
  let trait_ident = &input.ident;
  let supertraits = &input.supertraits;

  let expanded = quote! {
      #input

      impl<T> #trait_ident for T where T: #supertraits {}
  };

  TokenStream::from(expanded)
}
