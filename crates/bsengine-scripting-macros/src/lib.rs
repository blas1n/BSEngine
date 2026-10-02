//! `#[script_op]`: a script op written once and bound to whichever JavaScript
//! engine the build has.
//!
//! Natively scripts run in V8 through `deno_core`, and an op is a
//! `#[deno_core::op2]` function. A browser build has no V8 -- `deno_core` does
//! not target wasm -- and its scripts run in the page's own engine. The op's
//! body, its state and its JavaScript name are the same either way; only the
//! glue differs, so this attribute writes the glue:
//!
//! - **Native:** the function is passed to `#[deno_core::op2(..)]` untouched,
//!   argument attributes (`#[string]`, `#[serde]`) and all.
//! - **wasm32:** those attributes are stripped, the function is kept as plain
//!   Rust, and a module of the same name is generated beside it whose
//!   `register(ops)` puts a JavaScript function on `ops.<name>`. That function
//!   converts its arguments the way `deno_core` does -- `ToNumber` for floats,
//!   `ToUint32` for `u32`, `ToBoolean` for `bool`, `""` for a `#[string]` that
//!   is not a string -- and returns `#[serde]` values through
//!   `serde-wasm-bindgen` with `None` as `null` (as `serde_v8` gives it; the
//!   default would be `undefined`). A `#[serde]` argument that does not fit
//!   its type throws a `TypeError`, as `deno_core`'s does.
//!
//! The module shares the op's name without clashing: a module lives in the
//! type namespace and a function in the value namespace. `ops.rs` lists every
//! op once and makes both the native `deno_core` extension and the browser's
//! `register_ops` from that list.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, Attribute, FnArg, ItemFn, Pat, ReturnType, Type};

fn has_attr(attrs: &[Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}

fn strip_op_attrs(attrs: &mut Vec<Attribute>) {
    attrs.retain(|a| !a.path().is_ident("string") && !a.path().is_ident("serde"));
}

fn type_name(ty: &Type) -> String {
    match ty {
        Type::Path(p) => p
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// See the crate documentation. The attribute's own arguments (`fast`) are
/// `op2`'s and are forwarded to it natively.
#[proc_macro_attribute]
pub fn script_op(attr: TokenStream, item: TokenStream) -> TokenStream {
    let op2_args = proc_macro2::TokenStream::from(attr);
    let func = parse_macro_input!(item as ItemFn);

    // The browser build's plain function: the same body and signature, with
    // the `deno_core` argument attributes gone. Called only through its
    // registration below -- natively `op2` turns the function into an
    // `OpDecl` constructor, so nothing outside the crate calls an op as a
    // function in either build, and a `pub` here would expose the ops'
    // private JSON types.
    let mut plain = func.clone();
    strip_op_attrs(&mut plain.attrs);
    plain.vis = syn::parse_quote!(pub(crate));
    for input in plain.sig.inputs.iter_mut() {
        if let FnArg::Typed(pt) = input {
            strip_op_attrs(&mut pt.attrs);
        }
    }

    let name = &func.sig.ident;
    let js_name = name.to_string();

    let mut converts = Vec::new();
    let mut call_args = Vec::new();
    for (i, input) in func.sig.inputs.iter().enumerate() {
        let FnArg::Typed(pt) = input else {
            return syn::Error::new_spanned(input, "a script op takes no `self`")
                .to_compile_error()
                .into();
        };
        let arg = match &*pt.pat {
            Pat::Ident(p) => p.ident.clone(),
            _ => format_ident!("arg{}", i),
        };
        let ty = &pt.ty;
        let index = i as u32;
        let raw = quote! { crate::runtime::op_args::arg(&__bsengine_args, #index) };
        let convert = if has_attr(&pt.attrs, "serde") {
            quote! {
                let #arg: #ty = ::serde_wasm_bindgen::from_value(#raw).map_err(|e| {
                    ::wasm_bindgen::JsValue::from(::js_sys::TypeError::new(&e.to_string()))
                })?;
            }
        } else if has_attr(&pt.attrs, "string") {
            quote! { let #arg: ::std::string::String = crate::runtime::op_args::string(&#raw); }
        } else {
            match type_name(ty).as_str() {
                // The conversions are `crate::runtime::op_args`'s, where
                // the browser runtime's tests pin them down.
                "f32" => quote! { let #arg: f32 = crate::runtime::op_args::number(&#raw) as f32; },
                "f64" => quote! { let #arg: f64 = crate::runtime::op_args::number(&#raw); },
                "u32" => quote! { let #arg: u32 = crate::runtime::op_args::uint32(&#raw); },
                "bool" => quote! { let #arg: bool = crate::runtime::op_args::boolean(&#raw); },
                other => {
                    return syn::Error::new_spanned(
                        ty,
                        format!(
                            "a script op argument of type `{other}` needs #[string] or #[serde], \
                             or a conversion added to bsengine-scripting-macros"
                        ),
                    )
                    .to_compile_error()
                    .into();
                }
            }
        };
        converts.push(convert);
        call_args.push(quote! { #arg });
    }

    let to_js = match &func.sig.output {
        ReturnType::Default => quote! { ::wasm_bindgen::JsValue::UNDEFINED },
        ReturnType::Type(..) if has_attr(&func.attrs, "serde") => quote! {{
            let serializer = ::serde_wasm_bindgen::Serializer::new()
                .serialize_missing_as_null(true);
            ::serde::Serialize::serialize(&value, &serializer)
                .map_err(|e| ::wasm_bindgen::JsValue::from_str(&e.to_string()))?
        }},
        ReturnType::Type(..) => quote! { ::wasm_bindgen::JsValue::from(value) },
    };

    let op2 = if op2_args.is_empty() {
        quote! { #[::deno_core::op2] }
    } else {
        quote! { #[::deno_core::op2(#op2_args)] }
    };
    let expanded = quote! {
        #[cfg(not(target_arch = "wasm32"))]
        #op2
        #func

        #[cfg(target_arch = "wasm32")]
        #plain

        #[cfg(target_arch = "wasm32")]
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::all)]
        pub(crate) mod #name {
            // The op's argument types are named as its signature names them,
            // from the enclosing module.
            #[allow(unused_imports)]
            use super::*;

            /// Puts this op on `ops` under its JavaScript name, as a function
            /// taking any number of arguments.
            // The closure's argument array is `__bsengine_args`, a name no
            // op parameter will have: one op takes a parameter called
            // `args`, which shadowed it.
            pub(crate) fn register(ops: &::js_sys::Object) {
                let body = ::wasm_bindgen::closure::Closure::<
                    dyn Fn(::js_sys::Array) -> ::core::result::Result<
                        ::wasm_bindgen::JsValue,
                        ::wasm_bindgen::JsValue,
                    >,
                >::new(|__bsengine_args: ::js_sys::Array| {
                    #(#converts)*
                    let value = super::#name(#(#call_args),*);
                    ::core::result::Result::Ok(#to_js)
                });
                // `(...a) => f(a)`: the op is called with its arguments
                // spread, as a V8 op is, and the closure takes them as one
                // array, which wasm-bindgen can pass for any arity.
                let spread = ::js_sys::Function::new_with_args("f", "return (...a) => f(a);")
                    .call1(&::wasm_bindgen::JsValue::UNDEFINED, body.as_ref())
                    .expect("wrapping a script op");
                // Ops live as long as the page: the closure is never freed.
                body.forget();
                let _ = ::js_sys::Reflect::set(
                    ops,
                    &::wasm_bindgen::JsValue::from_str(#js_name),
                    &spread,
                );
            }
        }
    };
    expanded.into()
}
