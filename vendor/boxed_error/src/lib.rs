// Copyright 2018-2024 the Deno authors. MIT license.

#![deny(clippy::print_stderr)]
#![deny(clippy::print_stdout)]

use proc_macro::TokenStream;
use quote::quote;
use syn::parse_macro_input;
use syn::spanned::Spanned;
use syn::Data;
use syn::DeriveInput;
use syn::Error;
use syn::Ident;
use syn::Type;

#[proc_macro_derive(Boxed)]
pub fn derive_boxed(item: TokenStream) -> TokenStream {
  let input = parse_macro_input!(item as DeriveInput);
  let error_name = &input.ident;
  let field_type = match &input.data {
    Data::Struct(data_struct) => {
      // Check if the struct has exactly one field
      if data_struct.fields.len() != 1 {
        let error = Error::new(
          error_name.span(),
          "Struct must have exactly one field of type `Box<T>`",
        );
        return TokenStream::from(error.to_compile_error());
      }

      // Extract the type of the single field
      let field = data_struct.fields.iter().next().unwrap();
      &field.ty
    }
    _ => {
      let error = Error::new(
        error_name.span(),
        "Boxed can only be derived on structs with a single field",
      );
      return TokenStream::from(error.to_compile_error());
    }
  };
  let inner_type = match field_type {
    Type::Path(type_path) => {
      if type_path.path.segments.len() == 1
        && type_path.path.segments[0].ident == "Box"
      {
        // Extract the inner type from `Box<T>`
        match &type_path.path.segments[0].arguments {
          syn::PathArguments::AngleBracketed(args) => {
            if args.args.len() == 1 {
              if let syn::GenericArgument::Type(inner) = &args.args[0] {
                inner
              } else {
                let error = Error::new(
                  field_type.span(),
                  "Expected a single generic argument in `Box<T>`",
                );
                return TokenStream::from(error.to_compile_error());
              }
            } else {
              let error = Error::new(
                field_type.span(),
                "Expected a single generic argument in `Box<T>`",
              );
              return TokenStream::from(error.to_compile_error());
            }
          }
          _ => {
            let error = Error::new(
              field_type.span(),
              "Expected angle-bracketed arguments in `Box<T>`",
            );
            return TokenStream::from(error.to_compile_error());
          }
        }
      } else {
        let error =
          Error::new(field_type.span(), "Field must be of type `Box<T>`");
        return TokenStream::from(error.to_compile_error());
      }
    }
    _ => {
      let error =
        Error::new(field_type.span(), "Field must be of type `Box<T>`");
      return TokenStream::from(error.to_compile_error());
    }
  };
  let inner_name = match inner_type {
    Type::Path(type_path) => &type_path.path.segments[0].ident,
    _ => {
      let error =
        Error::new(inner_type.span(), "Expected an identifier in `Box<T>`");
      return TokenStream::from(error.to_compile_error());
    }
  };
  let inner_name_str = inner_name.to_string();
  let expected_suffix = if inner_name_str.ends_with("Kind") {
    "kind"
  } else if inner_name_str.ends_with("Data") {
    "data"
  } else {
    "inner"
  };

  let as_name =
    Ident::new(&format!("as_{}", expected_suffix), error_name.span());
  let into_name =
    Ident::new(&format!("into_{}", expected_suffix), error_name.span());

  // generate the code for the wrapper struct and implementations
  let expanded = quote! {
    impl #inner_name {
      pub fn into_box(self) -> #error_name {
        #error_name(Box::new(self))
      }
    }

    impl std::fmt::Display for #error_name {
      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
      }
    }

    impl std::error::Error for #error_name {
      fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
      }
    }

    impl std::ops::Deref for #error_name {
      type Target = #inner_name;

      fn deref(&self) -> &Self::Target {
        &self.0
      }
    }

    impl #error_name {
      pub fn #as_name(&self) -> &#inner_name {
        &self.0
      }

      pub fn #into_name(self) -> #inner_name {
        *self.0
      }
    }

    // implement conversion from other errors into the boxed error
    impl<E> From<E> for #error_name
    where
      #inner_name: From<E>,
    {
      fn from(err: E) -> Self {
        #error_name(Box::new(#inner_name::from(err)))
      }
    }
  };

  TokenStream::from(expanded)
}
