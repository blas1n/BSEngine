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
//! - **QuickJS** (`cfg(bsengine_quickjs)`: Android, iOS, and a desktop build
//!   with `bsengine-scripting`'s `quickjs` feature): the plain function as for
//!   wasm32, and a module whose `register(ctx, ops)` puts an `rquickjs`
//!   function on `ops.<name>`. It reads its arguments as `deno_core` really
//!   does (`crate::runtime::qjs_args`, measured against V8 by the
//!   engine-parity tests): a number is a Number or a BigInt and anything else
//!   a `TypeError`, a `bool` is `true` only for `true`. That is narrower than
//!   the ToNumber/ToBoolean the wasm32 arm applies, which predates the
//!   measurement. A `#[serde]` value crosses as JSON -- `JSON.stringify` into
//!   `serde_json`, `serde_json` into `JSON.parse` -- so `None` is `null` here
//!   too, but a NaN or an infinity inside one arrives as `null`.
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
    // An op's arity is its JavaScript signature's, which `op2` accepts at any
    // length; as a plain function the longest (11 arguments) trips clippy in
    // every build that is not V8's.
    plain
        .attrs
        .push(syn::parse_quote!(#[allow(clippy::too_many_arguments)]));
    for input in plain.sig.inputs.iter_mut() {
        if let FnArg::Typed(pt) = input {
            strip_op_attrs(&mut pt.attrs);
        }
    }

    let name = &func.sig.ident;
    let js_name = name.to_string();

    let mut converts = Vec::new();
    let mut qjs_converts = Vec::new();
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
        let qjs_raw = quote! { crate::runtime::qjs_args::arg(&__bsengine_ctx, &__bsengine_args, #index as usize) };
        let qjs_convert = if has_attr(&pt.attrs, "serde") {
            quote! {
                let #arg: #ty = crate::runtime::qjs_args::serde(&__bsengine_ctx, #qjs_raw)?;
            }
        } else if has_attr(&pt.attrs, "string") {
            quote! { let #arg: ::std::string::String = crate::runtime::qjs_args::string(&#qjs_raw); }
        } else {
            match type_name(ty).as_str() {
                "f32" => {
                    quote! { let #arg: f32 = crate::runtime::qjs_args::number(&__bsengine_ctx, #qjs_raw, "f32")? as f32; }
                }
                "f64" => {
                    quote! { let #arg: f64 = crate::runtime::qjs_args::number(&__bsengine_ctx, #qjs_raw, "f64")?; }
                }
                "u32" => {
                    quote! { let #arg: u32 = crate::runtime::qjs_args::uint32(&__bsengine_ctx, #qjs_raw)?; }
                }
                "bool" => {
                    quote! { let #arg: bool = crate::runtime::qjs_args::boolean(&__bsengine_ctx, #qjs_raw)?; }
                }
                // Every other type is refused by the wasm32 arm below.
                _ => quote! {},
            }
        };
        qjs_converts.push(qjs_convert);
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

    let qjs_to_js = match &func.sig.output {
        ReturnType::Default => quote! { ::rquickjs::Value::new_undefined(__bsengine_ctx.clone()) },
        ReturnType::Type(..) if has_attr(&func.attrs, "serde") => quote! {
            crate::runtime::qjs_args::to_serde(&__bsengine_ctx, &value)?
        },
        ReturnType::Type(..) => quote! {
            ::rquickjs::IntoJs::into_js(value, &__bsengine_ctx)?
        },
    };

    let op2 = if op2_args.is_empty() {
        quote! { #[::deno_core::op2] }
    } else {
        quote! { #[::deno_core::op2(#op2_args)] }
    };
    let expanded = quote! {
        #[cfg(not(any(target_arch = "wasm32", bsengine_quickjs)))]
        #op2
        #func

        #[cfg(any(target_arch = "wasm32", bsengine_quickjs))]
        #plain

        #[cfg(bsengine_quickjs)]
        #[doc(hidden)]
        #[allow(non_snake_case, clippy::all)]
        pub(crate) mod #name {
            #[allow(unused_imports)]
            use super::*;

            /// Puts this op on `ops` under its JavaScript name. The function
            /// takes its arguments as one rest parameter, so a script may
            /// pass fewer or more than the op names, as with V8.
            pub(crate) fn register<'js>(
                ctx: &::rquickjs::Ctx<'js>,
                ops: &::rquickjs::Object<'js>,
            ) -> ::rquickjs::Result<()> {
                let body = ::rquickjs::Function::new(
                    ctx.clone(),
                    |__bsengine_ctx: ::rquickjs::Ctx<'js>,
                     __bsengine_args: ::rquickjs::function::Rest<::rquickjs::Value<'js>>|
                     -> ::rquickjs::Result<::rquickjs::Value<'js>> {
                        #(#qjs_converts)*
                        let value = super::#name(#(#call_args),*);
                        ::core::result::Result::Ok(#qjs_to_js)
                    },
                )?;
                ops.set(#js_name, body)
            }
        }

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
