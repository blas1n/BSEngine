# `capacity_builder`

Builders where the code to calculate the capacity is the same as the code to
write what's being built.

## Overview

Sometimes you have some complex code that would be a bit of a pain to calculate
the capacity of or could risk easily getting out of sync with the
implementation. This crate makes keeping it in sync easier because it's the same
code.

### `StringBuilder`

```rs
use capacity_builder::StringBuilder;

let text = StringBuilder::<String>::build(|builder| {
  for (i, import_module) in import_modules.iter().enumerate() {
    builder.append("// ");
    builder.append(i);
    builder.append(" import\n");
    builder.append("import \"");
    builder.append(import_module);
    builder.append("\";\n");
  }
})?;
```

Behind the scenes it runs the closure once to compute the capacity and a second
time to write the string.

Note that providing an owned value will cause an error at compile time in order
to prevent doing any allocation twice instead of once:

```rs
let text = StringBuilder::<String>::build(|builder| {
  builder.append("some allocated value".to_string());
                 ^-- Lifetime compile time error
})?;
```

To fix this, allocate outside the closure:

```rs
let value = "some allocated value".to_string();
let text = StringBuilder::<String>::build(|builder| {
  builder.append(&value); // ok
})?;
```

### `BytesBuilder`

The bytes builder is similar to the `StringBuilder`:

```rs
use capacity_builder::BytesBuilder;

let bytes = BytesBuilder::<Vec<u8>>::build(|builder| {
  builder.append_le(123);
  builder.append("example");
  builder.append(other_bytes);
})?;
```

## Making an object "appendable"

Custom types can be appended to builders by implementing the `BytesAppendable`
or `StringAppendable` trait.

For example:

```rs
use capacity_builder::BytesAppendable;
use capacity_builder::BytesBuilder;
use capacity_builder::BytesType;

struct MyStruct;

impl<'a> BytesAppendable<'a> for &'a MyStruct {
  fn append_to_builder<TBytes: BytesType>(self, builder: &mut BytesBuilder<'a, TBytes>) {
    builder.append("Hello");
    builder.append(" there!");
  }
}

let bytes = BytesBuilder::<Vec<u8>>::build(|builder| {
  builder.append(&MyStruct); // works
})
.unwrap();
assert_eq!(bytes, b"Hello there!");
```

Or with a string:

```rs
use capacity_builder::StringAppendable;
use capacity_builder::StringBuilder;
use capacity_builder::StringType;

#[derive(Debug)]
pub struct Version {
  // ...
}

impl<'a> StringAppendable for &'a Version {
  fn append_to_builder<TString: StringType>(
    &'a self,
    builder: &mut StringBuilder<'a, TString>,
  ) {
    builder.append(version.major);
    builder.append('.');
    builder.append(version.minor);
    builder.append('.');
    builder.append(version.patch);
    if !version.pre.is_empty() {
      builder.append('-');
      for (i, part) in version.pre.iter().enumerate() {
        if i > 0 {
          builder.append('.');
        }
        builder.append(part);
      }
    }
    if !version.build.is_empty() {
      builder.append('+');
      for (i, part) in version.build.iter().enumerate() {
        if i > 0 {
          builder.append('.');
        }
        builder.append(part);
      }
    }
  }
}
```

## Implementing faster `.to_string()` and `std::fmt::Display`

The default `.to_string()` implementation reuses `std::fmt::Display`. This is
slow because the capacity isn't set.

This crate provides a `#[derive(CapacityDisplay)]` macro for implementing
`.to_string()` and `std::fmt::Display` reusing the implementation in
`StringAppendable`.

```rs
use capacity_builder::CapacityDisplay;
use capacity_builder::StringAppendable;

#[derive(CapacityDisplay)] // <-- add this
pub struct Version {
  // ...
}

impl<'a> StringAppendable for &'a Version {
  // ...see above for example implementation
}
```

Now `version.to_string()` will be fast and return a string that has an accurate
capacity. Additionally you can use the struct in format strings, which falls
back to just writing to the formatter which should run with about the same
performance as before.

Side note: You may have noticed that the builders don't seem to surface format
errors. This is because errors when formatting are really rare and if an error
is encountered it will store it to surface at the end and the rest of the
`append` statements stop formatting.

## Cargo Features

- [`ecow`](https://crates.io/crates/ecow)
- [`hipstr`](https://crates.io/crates/hipstr)

Example:

```toml
# Cargo.toml
capacity_builder = { version = "...", features = ["ecow"] }
```

```rs
let text = StringBuilder::<ecow::EcoString>::build(|builder| {
  // ...
}).unwrap();
```

## Tips

- Do any necessary allocations before running the closure.
- Measure before and after using this crate to ensure you're not slower.
