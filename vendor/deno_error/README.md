# deno_error

[![Build Status - Cirrus][]][Build status] [![Twitter handle][]][Twitter badge]
[![Discord Chat](https://img.shields.io/discord/684898665143206084?logo=discord&style=social)](https://discord.gg/deno)

Trait and macros to represent Rust errors in JS.

## Usage

The `JsErrorClass` trait is available to be manually implemented, which includes
3 functions: `get_class`, `get_message` and `get_additional_properties`.

It is however advised to not implement this trait manually, but rather go
through the `JsError` derive macro that is exposed, which provides the following
functionality:

- Define the class via the `#[class()]` attribute, that can be defined on either
  variants of an enum, or the top-level of an enum to define for all variants,
  where on individual variants overwrite can still be applied. Structs are also
  supported.

  This attribute accepts 3 possible kinds of value:
  1. `GENERIC`, `TYPE`, and a few more that are defined in the `builtin_classes`
     module, without the `_ERROR` suffix.
  2. A text value ie `"NotFound"`. If a text value is passed that is a valid
     builtin (see the previous point), it will error out as the special
     identifiers are preferred to avoid mistakes.
  3. `inherit`: this will inherit the class from whatever field is marked with
     the `#[inherit]` attribute. Alternatively, the `#[inherit]` attribute can
     be omitted if only one field is present in the enum variant or struct.
     This value is inferred if the class attribute is missing and only a single
     field is present on a struct, however for enums this inferring is not done.

- Define additional properties via the `#[property]` attribute that can be
  defined on individual fields. The type of the field needs to implement a
  `.to_string()` function for it being able to be inherited.
- Inherit class and properties via the `#[inherit]` attribute which can be
  specified on fields that contain a value that implements `JsErrorClass`. This
  is inferred if only one field is present in the enum variant or struct.

The macro does not provide functionality to related to the `get_message`
function, as one can combine the
[`thiserror`](https://crates.io/crates/thiserror) well with this macro.

There also is the `js_error_wrapper` macro which lets you wrap an existing error
in a new error that implements the `JsErrorClass` trait. This macro however does
currently not support the special identifiers that the `JsError` macro supports.
Here are two examples on how to use it:

```rust
js_error_wrapper!(std::net::AddrParseError, JsAddrParseError, "TypeError");
```

```rust
js_error_wrapper!(std::net::AddrParseError, JsAddrParseError, |err| {
  // match or do some logic to get the error class
});
```

Additionally, this crate provides some features which related to some commonly
used crates in Deno, which implements the `JsErrorClass` trait on some of their
errors.

## Versioning Strategy

This crate does not follow semver so make sure to pin it to a patch version.
Instead a versioning strategy that optimizes for more efficient maintenance is
used:

- Does [deno_graph](https://github.com/denoland/deno_deno_graph) still compile
  in the [Deno](https://github.com/denoland/deno) repo?
  - If yes, is this a change that would break something at runtime?
    - If yes, it's a minor release.
    - If no, it's a patch release.
  - If no, it's a minor release.

### Contributing

We appreciate your help!

To contribute, please read our
[contributing instructions](https://deno.land/manual/contributing).

[Build Status - Cirrus]: https://github.com/denoland/deno_error/workflows/ci/badge.svg?branch=main&event=push
[Build status]: https://github.com/denoland/deno_error/actions
[Twitter badge]: https://twitter.com/intent/follow?screen_name=deno_land
[Twitter handle]: https://img.shields.io/twitter/follow/deno_land.svg?style=social&label=Follow
