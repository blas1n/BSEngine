# syn-match

A Rust procedural macro for pattern matching on `syn::Path` structures with binding capabilities.

## Overview

`syn-match` provides a `path_match!` macro that allows you to pattern match against Rust path expressions (like `std::collections::HashMap` or `Option<String>`) with support for:

- Exact path matching
- Optional path segments
- Variable bindings
- Generic argument matching
- Wildcard patterns
- Multi-segment bindings
- Associated type matching
- Lifetime matching

## Basic Usage

```rust
use syn_match::path_match;
use syn::Path;

let path: Path = syn::parse_quote!(std::collections::HashMap);

let result = path_match!(&path,
    std::collections::HashMap => "found HashMap",
    std::vec::Vec => "found Vec",
    _ => "something else"
);

assert_eq!(result, "found HashMap");
```

## Pattern Syntax

### 1. Exact Path Matching

```rust
let path: Path = syn::parse_quote!(String);
let result = path_match!(&path,
    String => "matched String",
    _ => "no match"
);
```

### 2. Optional Segments

Use `?` to make path segments optional:

```rust
let path1: Path = syn::parse_quote!(std::str::String);
let path2: Path = syn::parse_quote!(str::String);
let path3: Path = syn::parse_quote!(String);

for path in [&path1, &path2, &path3] {
    let result = path_match!(path,
        std?::str?::String => "matched",
        _ => "no match"
    );
    assert_eq!(result, "matched"); // All match!
}
```

### 3. Variable Bindings

Bind path segments to variables using `$name`:

```rust
let path: Path = syn::parse_quote!(std::collections::HashMap);
let result = path_match!(&path,
    std::$module::$ty => format!("{}::{}", module.ident, ty.ident),
    _ => "no match".to_string()
);
assert_eq!(result, "collections::HashMap");
```

### 4. Multi-Segment Bindings

Capture multiple segments using `$name*`:

```rust
let path: Path = syn::parse_quote!(std::collections::HashMap);
let result = path_match!(&path,
    $prefix*::HashMap => {
        prefix.iter()
            .map(|seg| seg.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    },
    _ => "no match".to_string()
);
assert_eq!(result, "std::collections");
```

### 5. Optional Bindings

Capture segments that may or may not exist using `$name?`:

```rust
let path: Path = syn::parse_quote!(std::collections::HashMap);
let result = path_match!(&path,
    std::$middle?::HashMap => {
        if let Some(seg) = middle {
            format!("Found middle: {}", seg.ident)
        } else {
            "No middle segment".to_string()
        }
    },
    _ => "no match".to_string()
);
assert_eq!(result, "Found middle: collections");
```

### 6. Generic Arguments

Match and bind generic type arguments:

```rust
let path: Path = syn::parse_quote!(Option<String>);
let result = path_match!(&path,
    Option<$inner> => {
        if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = inner {
            type_path.path.segments.last().unwrap().ident.to_string()
        } else {
            "not a path".to_string()
        }
    },
    _ => "no match".to_string()
);
assert_eq!(result, "String");
```

### 7. Path Coercion in Generics

Use `::$name` to bind only path-type generic arguments:

```rust
let path: Path = syn::parse_quote!(Vec<String>);
let result = path_match!(&path,
    Vec<::$ty> => ty.segments.last().unwrap().ident.to_string(),
    _ => "no match".to_string()
);
assert_eq!(result, "String");

// Won't match non-path types:
let path2: Path = syn::parse_quote!(Vec<[u8]>);
let result2 = path_match!(&path2,
    Vec<::$ty> => "matched path",
    _ => "no match"
);
assert_eq!(result2, "no match");
```

### 8. Associated Types

Match associated types in generic parameters:

```rust
let path: Path = syn::parse_quote!(Future<Output = String>);
let result = path_match!(&path,
    Future<Output = $output> => {
        if let syn::Type::Path(type_path) = output {
            format!("Future output: {}", type_path.path.segments.last().unwrap().ident)
        } else {
            "not a path".to_string()
        }
    },
    _ => "no match".to_string()
);
assert_eq!(result, "Future output: String");
```

### 9. Lifetime Matching

Match and bind lifetimes using `$'name`:

```rust
let path: Path = syn::parse_quote!(Cow<'static, str>);
let result = path_match!(&path,
    Cow<$'lt, str> => format!("lifetime: {}", lt.ident),
    _ => "no match".to_string()
);
assert_eq!(result, "lifetime: static");
```

Use `'_` for lifetime wildcards:

```rust
let path: Path = syn::parse_quote!(Cow<'a, str>);
let result = path_match!(&path,
    Cow<'_, str> => "matched any lifetime",
    _ => "no match"
);
assert_eq!(result, "matched any lifetime");
```

**Note:** Lifetime bindings do not support optional patterns (`$'name?`). Only regular lifetime bindings (`$'name`) and wildcards (`'_`) are supported.

### 10. Slice Type Matching

Match slice types in generic arguments:

```rust
let path: Path = syn::parse_quote!(std::borrow::Cow<foo, [u8]>);
let result = path_match!(&path,
    std?::borrow?::Cow<_, [$elem]> => elem.to_token_stream().to_string(),
    _ => "no match".to_string()
);
assert_eq!(result, "u8");
```

### 11. Multiple Patterns

Use `|` to match multiple patterns in a single arm:

```rust
let path: Path = syn::parse_quote!(HashMap);
let result = path_match!(&path,
    String | HashMap | Vec => "collection type",
    _ => "other"
);
assert_eq!(result, "collection type");
```

### 12. Wildcards

Use `_` to match any remaining patterns:

```rust
let path: Path = syn::parse_quote!(Something::Unknown);
let result = path_match!(&path,
    String => "string",
    Vec => "vector", 
    _ => "wildcard match"
);
assert_eq!(result, "wildcard match");
```

## Advanced Examples

### Complex Generic Matching

```rust
let path: Path = syn::parse_quote!(Result<std::collections::HashMap, io::Error>);
let result = path_match!(&path,
    Result<std::$module::$ty, io::Error> => {
        format!("Result with {}::{}", module.ident, ty.ident)
    },
    _ => "no match".to_string()
);
assert_eq!(result, "Result with collections::HashMap");
```

### Nested Generic Patterns

```rust
let path: Path = syn::parse_quote!(Option<Outer<Other<More<Yet<String>>>>>);
let result = path_match!(&path,
    Option<Outer<Other<More<Yet<$inner>>>>> => {
        if let syn::GenericArgument::Type(syn::Type::Path(type_path)) = inner {
            type_path.path.segments.last().unwrap().ident.to_string()
        } else {
            "not a path".to_string()
        }
    },
    _ => "no match".to_string()
);
assert_eq!(result, "String");
```

### Multiple Bindings with Wildcards

```rust
let path: Path = syn::parse_quote!(Result<String, Error>);
let result = path_match!(&path,
    $_package*::Result<$ok> | $_package*::Result<$ok, _> => {
        if let syn::GenericArgument::Type(syn::Type::Path(p)) = ok {
            format!("Result<{}, ?>", p.path.segments.last().unwrap().ident)
        } else {
            "?".to_string()
        }
    },
    _ => "no match".to_string()
);
assert_eq!(result, "Result<String, ?>");
```
