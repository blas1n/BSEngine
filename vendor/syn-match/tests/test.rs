use quote::ToTokens;
use syn_match::path_match;

#[test]
fn test_simple_path() {
  let path: syn::Path = syn::parse_quote!(String);
  let result = path_match!(&path, String => "matched", _ => "no match");
  assert_eq!(result, "matched");
}

#[test]
fn test_simple_path_short() {
  let path: syn::Path = syn::parse_quote!(str);
  let result = path_match!(&path,
    std?::str?::str => "we got full",
    _ => "no",
  );
  assert_eq!(result, "we got full");
}

#[test]
fn test_fully_qualified() {
  let path: syn::Path = syn::parse_quote!(::std::string::String);
  let result =
    path_match!(&path, ::std::string::String => "matched", _ => "no match");
  assert_eq!(result, "matched");
}

#[test]
fn test_optional_segments() {
  let path1: syn::Path = syn::parse_quote!(std::str::String);
  let path2: syn::Path = syn::parse_quote!(str::String);
  let path3: syn::Path = syn::parse_quote!(String);

  let r1 = path_match!(&path1, std?::str?::String => "matched", _ => "no");
  let r2 = path_match!(&path2, std?::str?::String => "matched", _ => "no");
  let r3 = path_match!(&path3, std?::str?::String => "matched", _ => "no");

  assert_eq!(r1, "matched");
  assert_eq!(r2, "matched");
  assert_eq!(r3, "matched");
}

#[test]
fn test_wildcard_binding() {
  let path: syn::Path = syn::parse_quote!(std::str);
  let result = path_match!(
    &path,
    std::str::String => "String".to_string(),
    std::str::$rest* => {
      rest.iter().map(|seg| seg.ident.to_string()).collect::<String>()
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "");
}

#[test]
fn test_binding() {
  let path: syn::Path = syn::parse_quote!(std::str::String);
  let result = path_match!(
    &path,
    std::str::$foo => foo.ident.to_string(),
    _ => "no match".to_string(),
  );

  assert_eq!(result, "String");
}

#[test]
fn test_multiple_bindings() {
  let path: syn::Path = syn::parse_quote!(std::collections::HashMap);
  let result = path_match!(
    &path,
    std::$module::$ty => format!("{}::{}", module.ident, ty.ident),
    _ => "no match".to_string(),
  );
  assert_eq!(result, "collections::HashMap");
}

#[test]
fn test_wildcard_fallback() {
  let path: syn::Path = syn::parse_quote!(Something::Random);
  let result = path_match!(
    &path,
    String => "string",
    _ => "wildcard",
  );
  assert_eq!(result, "wildcard");
}

#[test]
fn test_multi_patterns() {
  let path: syn::Path = syn::parse_quote!(String);
  let path2: syn::Path = syn::parse_quote!(HashMap);

  let result = path_match!(
    &path,
    String | HashMap => "correct",
    _ => "wildcard",
  );
  let result2 = path_match!(
    &path2,
    String | HashMap => "correct",
    _ => "wildcard",
  );
  assert_eq!(result, "correct");
  assert_eq!(result2, "correct");
}

#[test]
fn test_multi_binding_at_start() {
  let path: syn::Path = syn::parse_quote!(std::collections::HashMap);

  let result = path_match!(
    &path,
    $prefix*::HashMap => {
      prefix.iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "std::collections");
}

#[test]
fn test_multi_binding_at_start_single_segment() {
  let path: syn::Path = syn::parse_quote!(std::String);

  let result = path_match!(
    &path,
    $prefix*::String => {
      prefix.iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "std");
}

#[test]
fn test_multi_binding_at_start_multiple_after() {
  let path: syn::Path = syn::parse_quote!(std::collections::hash::HashMap);

  let result = path_match!(
    &path,
    $prefix*::hash::HashMap => {
      prefix.iter()
        .map(|seg| seg.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "std::collections");
}

#[test]
fn test_generic_binding() {
  let path: syn::Path = syn::parse_quote!(Option<String>);

  let result = path_match!(
    &path,
    Option<$inner> => {
      if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = inner {
        type_path.path.segments.last().unwrap().ident.to_string()
      } else {
        "not a path".to_string()
      }
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "String");
}

#[test]
fn test_generic_binding_path() {
  let path: syn::Path = syn::parse_quote!(Option<std::str::String>);

  let result = path_match!(
    &path,
    Option<std::str::$inner> => {
      inner.ident.to_string()
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "String");
}

#[test]
fn test_generic_binding_path_multiple_segments() {
  let path: syn::Path =
    syn::parse_quote!(Result<std::collections::HashMap, io::Error>);

  let result = path_match!(
    &path,
    Result<std::$module::$ty, io::Error> => {
      format!("{}::{}", module.ident, ty.ident)
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "collections::HashMap");
}

#[test]
fn test_path_coercion_binding() {
  let path: syn::Path = syn::parse_quote!(Vec<String>);

  let result = path_match!(
    &path,
    Vec<::$ty> => {
      ty.segments.last().unwrap().ident.to_string()
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "String");
}

#[test]
fn test_path_coercion_no_match() {
  let path: syn::Path = syn::parse_quote!(Vec<123>);

  let result = path_match!(
    &path,
    Vec<::$ty> => {
      ty.segments.last().unwrap().ident.to_string()
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "no match");
}

#[test]
fn test_path_coercion_complex_path() {
  let path: syn::Path = syn::parse_quote!(Vec<std::collections::HashMap>);

  let result = path_match!(
    &path,
    Vec<::$ty> => {
      format!("{} segments", ty.segments.len())
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "3 segments");
}

#[test]
fn test_path_coercion_vs_regular_binding() {
  let path_with_string: syn::Path = syn::parse_quote!(Vec<String>);
  let path_with_number: syn::Path = syn::parse_quote!(Vec<[u8]>);

  let result1 = path_match!(
    &path_with_string,
    Vec<$ty> => {
      match ty {
        syn::GenericArgument::Type(syn::Type::Path(p)) => {
          format!("path: {}", p.path.segments.last().unwrap().ident)
        }
        syn::GenericArgument::Type(_) => "non-path type".to_string(),
        _ => "other".to_string(),
      }
    },
    _ => "no match".to_string(),
  );

  let result2 = path_match!(
    &path_with_number,
    Vec<$ty> => {
      match ty {
        syn::GenericArgument::Type(syn::Type::Path(p)) => {
          format!("path: {}", p.path.segments.last().unwrap().ident)
        }
        syn::GenericArgument::Type(_) => "non-path type".to_string(),
        _ => "other".to_string(),
      }
    },
    _ => "no match".to_string(),
  );

  let result3 = path_match!(
    &path_with_string,
    Vec<::$ty> => {
      format!("path: {}", ty.segments.last().unwrap().ident)
    },
    _ => "no match".to_string(),
  );

  let result4 = path_match!(
    &path_with_number,
    Vec<::$ty> => {
      format!("path: {}", ty.segments.last().unwrap().ident)
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result1, "path: String");
  assert_eq!(result2, "non-path type");
  assert_eq!(result3, "path: String");
  assert_eq!(result4, "no match");
}

#[test]
fn test_path_coercion_with_path_prefix() {
  let path: syn::Path = syn::parse_quote!(std::vec::Vec<String>);

  let result = path_match!(
    &path,
    std?::vec?::Vec<::$ty> => {
      format!("Found path type: {}", ty.segments.last().unwrap().ident)
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "Found path type: String");

  let path2: syn::Path = syn::parse_quote!(std::vec::Vec<[u8]>);

  let result2 = path_match!(
    &path2,
    std?::vec?::Vec<::$ty> => {
      format!("Found path type: {}", ty.segments.last().unwrap().ident)
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result2, "no match");
}

#[test]
fn test_generic_binding_nested() {
  let path: syn::Path =
    syn::parse_quote!(Option<Outer<Other<More<Yet<String>>>>>);

  let result = path_match!(
    &path,
    Option<Outer<Other<More<Yet<$inner>>>>> => {
      if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = inner {
        type_path.path.segments.last().unwrap().ident.to_string()
      } else {
        "not a path".to_string()
      }
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "String");
}

#[test]
fn test_result_generic_bindings() {
  let path: syn::Path = syn::parse_quote!(Result<String, Error>);

  let result = path_match!(
    &path,
    Result<$ok, $err> => {
      format!("Result<{}, {}>",
        if let syn::GenericArgument::Type(syn::Type::Path(p)) = ok { p.path.segments.last().unwrap().ident.to_string() } else { "?".to_string() },
        if let syn::GenericArgument::Type(syn::Type::Path(p)) = err { p.path.segments.last().unwrap().ident.to_string() } else { "?".to_string() }
      )
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "Result<String, Error>");
}

#[test]
fn test_result_generic_bindings_mutliple_branch_with_wildcard() {
  let path: syn::Path = syn::parse_quote!(Result<String, Error>);

  let result = path_match!(
    &path,
    $_package*::Result<$ok> | $_package*::Result<$ok, _> => {
      format!("Result<{}, ?>",
        if let syn::GenericArgument::Type(syn::Type::Path(p)) = ok { p.path.segments.last().unwrap().ident.to_string() } else { "?".to_string() },
      )
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "Result<String, ?>");
}

#[test]
fn test_associated_type() {
  let path: syn::Path = syn::parse_quote!(Future<Output = String>);

  let result = path_match!(
    &path,
    Future<Output = $output> => {
      if let syn::Type::Path(type_path) = output {
        format!("Future output: {}", type_path.path.segments.last().unwrap().ident.to_string())
      } else {
        "not a path".to_string()
      }
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "Future output: String");
}

#[test]
fn test_associated_type_mismatch() {
  let path: syn::Path = syn::parse_quote!(Future<Input = String>);

  let result = path_match!(
    &path,
    Future<Output = $output> => {
      if let syn::Type::Path(type_path) = output {
        format!("Future output: {}", type_path.path.segments.last().unwrap().ident.to_string())
      } else {
        "not a path".to_string()
      }
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "no match");
}

#[test]
fn test_associated_type_wildcard() {
  let path: syn::Path = syn::parse_quote!(Future<Output = String>);

  let result = path_match!(
    &path,
    Future<Output = _> => "matched with wildcard",
    _ => "no match",
  );

  assert_eq!(result, "matched with wildcard");
}

#[test]
fn test_associated_type_specific_type() {
  let path: syn::Path = syn::parse_quote!(Future<Output = String>);

  let result = path_match!(
    &path,
    Future<Output = String> => "matched specific type",
    Future<Output = i32> => "matched i32",
    _ => "no match",
  );

  assert_eq!(result, "matched specific type");
}

#[test]
fn test_type_param_with_lifetime() {
  let path: syn::Path = syn::parse_quote!(std::borrow::Cow<foo, str>);

  let result = path_match!(
    &path,
    std?::borrow?::Cow<_, str> => "matched",
    _ => "no match",
  );

  assert_eq!(result, "matched");
}

#[test]
fn test_type_param_slice() {
  let path: syn::Path = syn::parse_quote!(std::borrow::Cow<foo, [u8]>);

  let result = path_match!(
    &path,
    std?::borrow?::Cow<_, [u8]> => "matched",
    _ => "no match",
  );

  assert_eq!(result, "matched");
}

#[test]
fn test_type_param_slice_with_binding() {
  let path: syn::Path = syn::parse_quote!(std::borrow::Cow<foo, [u8]>);

  let result = path_match!(
    &path,
    std?::borrow?::Cow<_, [$foo]> => foo.to_token_stream().to_string(),
    _ => "no match".to_string(),
  );

  assert_eq!(result, "u8");
}

#[test]
fn test_type_param_slice_with_complex_binding() {
  let path: syn::Path = syn::parse_quote!(std::borrow::Cow<foo, [Vec<String>]>);

  let result = path_match!(
    &path,
    std?::borrow?::Cow<_, [$elem_type]> => {
      if let syn::Type::Path(type_path) = elem_type {
        format!("slice of {}", type_path.path.segments.last().unwrap().ident)
      } else {
        "not a path type".to_string()
      }
    },
    _ => "no match".to_string(),
  );

  assert_eq!(result, "slice of Vec");
}

#[test]
fn test_lifetime_binding_simple() {
  let path: syn::Path = syn::parse_quote!(Cow<'static, str>);
  let result = path_match!(
    &path,
    Cow<$'lt, str> => format!("lifetime: {}", lt.ident),
    _ => "no match".to_string(),
  );
  assert_eq!(result, "lifetime: static");
}

#[test]
fn test_lifetime_vs_type_binding() {
  let path: syn::Path = syn::parse_quote!(Foo<'a, String>);
  let result = path_match!(
    &path,
    Foo<$'lifetime, $ty> => {
      let lifetime_str = lifetime.ident.to_string();
      let type_str = if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = ty {
        type_path.path.segments.last().unwrap().ident.to_string()
      } else {
        "not_a_path".to_string()
      };
      format!("lifetime: {}, type: {}", lifetime_str, type_str)
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result, "lifetime: a, type: String");
}

#[test]
fn test_lifetime_vs_type_binding_wild() {
  let path: syn::Path = syn::parse_quote!(Foo<'a, String>);
  let result = path_match!(
    &path,
    Foo<'_, $ty> => {
      let type_str = if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = ty {
        type_path.path.segments.last().unwrap().ident.to_string()
      } else {
        "not_a_path".to_string()
      };
      format!("lifetime wildcard, type: {}", type_str)
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result, "lifetime wildcard, type: String");
}

#[test]
fn test_lifetime_wildcard_functionality() {
  let path1: syn::Path = syn::parse_quote!(Cow<'static, str>);
  let path2: syn::Path = syn::parse_quote!(Cow<'a, str>);
  let path3: syn::Path = syn::parse_quote!(Cow<'_, str>);

  for path in [&path1, &path2, &path3] {
    let result = path_match!(
      path,
      Cow<'_, str> => "matched lifetime wildcard",
      _ => "no match",
    );
    assert_eq!(result, "matched lifetime wildcard");
  }

  let path_with_type: syn::Path = syn::parse_quote!(Cow<String, str>);
  let result = path_match!(
    &path_with_type,
    Cow<'_, str> => "matched",
    _ => "no match",
  );
  assert_eq!(result, "no match");
}

#[test]
fn test_optional_binding() {
  let path: syn::Path = syn::parse_quote!(std::str::bar);
  let result = path_match!(
    &path,
    std::str::$any? => {
      if any.is_some() {
        "matched with segment"
      } else {
        "matched without segment"
      }
    },
    _ => "no match",
  );
  assert_eq!(result, "matched with segment");

  let path2: syn::Path = syn::parse_quote!(std::str);
  let result2 = path_match!(
    &path2,
    std::str::$any? => {
      if any.is_some() {
        "matched with segment"
      } else {
        "matched without segment"
      }
    },
    _ => "no match",
  );
  assert_eq!(result2, "matched without segment");
}

#[test]
fn test_optional_binding_with_extraction() {
  let path: syn::Path = syn::parse_quote!(std::collections::HashMap);
  let result = path_match!(
    &path,
    std::$middle?::HashMap => {
      if let Some(seg) = middle {
        format!("Found middle: {}", seg.ident)
      } else {
        "No middle segment".to_string()
      }
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result, "Found middle: collections");

  let path2: syn::Path = syn::parse_quote!(std::HashMap);
  let result2 = path_match!(
    &path2,
    std::$middle?::HashMap => {
      if let Some(seg) = middle {
        format!("Found middle: {}", seg.ident)
      } else {
        "No middle segment".to_string()
      }
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result2, "No middle segment");
}

#[test]
fn test_optional_conflict_with_normal_binding() {
  let path: syn::Path = syn::parse_quote!(str);
  let result = path_match!(
    &path,
    std?::$name => {
      format!("captured: {}", name.ident)
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result, "captured: str");

  let path2: syn::Path = syn::parse_quote!(std::String);
  let result2 = path_match!(
    &path2,
    std?::$name => {
      format!("captured: {}", name.ident)
    },
    _ => "no match".to_string(),
  );
  assert_eq!(result2, "captured: String");
}
