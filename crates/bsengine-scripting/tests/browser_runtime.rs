//! The browser build's script engine, run for real: compiled to wasm32 and
//! executed by `wasm-bindgen-test` (in Node, which is a JavaScript engine
//! without V8's embedding API -- the situation a browser build is in).
//!
//! What has to hold is that a script cannot tell the two builds apart: the
//! ops are `Deno.core.ops.<name>`, arguments are coerced as `deno_core`
//! coerces them, results come back in the shapes `serde_v8` gives, the
//! prelude's `var`s land on the global object, and errors are reported as
//! `deno_core` reports them.
//!
//! Run with
//! `cargo test -p bsengine-scripting --target wasm32-unknown-unknown --test browser_runtime`
//! and `wasm-bindgen-test-runner` as the target's runner.

#![cfg(target_arch = "wasm32")]

use bsengine_scripting::ops::BOOTSTRAP_JS;
use bsengine_scripting::runtime::{op_args, ScriptRuntime};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test;

/// Every op is reachable as `Deno.core.ops.<name>`, under the name the
/// native build registers it by, and answers as the native op does.
#[wasm_bindgen_test]
fn ops_are_deno_core_ops() {
    let mut rt = ScriptRuntime::new_with_ops();
    assert_eq!(
        rt.eval("typeof Deno.core.ops.bsengine_get_transform")
            .unwrap(),
        "function"
    );
    assert_eq!(
        rt.eval("Deno.core.ops.bsengine_version()").unwrap(),
        "0.1.0"
    );
}

/// A script's top-level `var` is a property of the global object, as it is
/// in a V8 script: the prelude declares `Bsengine` that way and every later
/// script reads it from there.
#[wasm_bindgen_test]
fn a_scripts_var_lands_on_the_global_object() {
    let mut rt = ScriptRuntime::new();
    rt.exec_source("var bse_browser_global = 41 + 1;", "<test>")
        .unwrap();
    assert_eq!(rt.eval("globalThis.bse_browser_global").unwrap(), "42");
}

/// The prelude runs, and its API reaches the ops: an entity that does not
/// exist has no transform, which `serde_v8` hands back as `null` --
/// `undefined` would be what serde-wasm-bindgen gives by default, and a
/// script checking `=== null` would then think it had one.
#[wasm_bindgen_test]
fn the_prelude_runs_and_a_missing_value_is_null() {
    let mut rt = ScriptRuntime::new_with_ops();
    rt.exec_source(BOOTSTRAP_JS, "prelude")
        .expect("the prelude runs");
    assert_eq!(rt.eval("typeof Bsengine").unwrap(), "object");
    assert_eq!(
        rt.eval("String(Deno.core.ops.bsengine_get_transform('nobody') === null)")
            .unwrap(),
        "true"
    );
}

/// A string argument that is not a string is `""`, as `deno_core` coerces
/// it -- not a thrown `TypeError`, which is what wasm-bindgen's own `String`
/// parameter would give.
#[wasm_bindgen_test]
fn a_non_string_name_is_an_empty_string() {
    let mut rt = ScriptRuntime::new_with_ops();
    assert_eq!(
        rt.eval("String(Deno.core.ops.bsengine_get_transform(undefined))")
            .unwrap(),
        "null"
    );
}

/// A structured argument that does not fit its type throws, as
/// `deno_core`'s `TypeError` does, and the error reaches the caller as the
/// exception's text.
#[wasm_bindgen_test]
fn a_bad_structured_argument_throws() {
    let mut rt = ScriptRuntime::new_with_ops();
    let err = rt
        .exec_source("Deno.core.ops.bsengine_instantiate_prefab(42);", "<test>")
        .expect_err("a number is not prefab parameters");
    // A `TypeError`, as deno_core throws -- not the `RuntimeError:
    // unreachable` a Rust panic would surface as, which also leaves the
    // module in whatever state the panic interrupted.
    assert!(err.starts_with("TypeError"), "{err:?}");
    assert_eq!(
        rt.eval("1 + 1").unwrap(),
        "2",
        "and scripts run on afterwards"
    );
}

/// An exception is reported by its `toString()`, as `deno_core` reports it.
#[wasm_bindgen_test]
fn an_exception_is_reported_by_its_text() {
    let mut rt = ScriptRuntime::new();
    let err = rt
        .exec_source("throw new RangeError('out of the arena');", "<test>")
        .expect_err("the script throws");
    assert_eq!(err, "RangeError: out of the arena");
}

/// Every op the prelude calls is there: the browser registry and the native
/// extension are made from one list, and this is the side a script sees.
#[wasm_bindgen_test]
fn every_op_the_prelude_calls_is_registered() {
    let mut rt = ScriptRuntime::new_with_ops();
    let mut names: Vec<&str> = BOOTSTRAP_JS
        .split("Deno.core.ops.")
        .skip(1)
        .map(|rest| {
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            &rest[..end]
        })
        .collect();
    names.sort_unstable();
    names.dedup();
    assert!(
        names.len() > 200,
        "premise: the prelude calls many ops: {}",
        names.len()
    );
    for name in names {
        assert_eq!(
            rt.eval(&format!("typeof Deno.core.ops.{name}")).unwrap(),
            "function",
            "{name}"
        );
    }
}

/// Numbers are read as V8 reads them: ToNumber for floats, ToUint32 for
/// `u32` (so -1 is 4294967295, not 0), ToBoolean for `bool`.
#[wasm_bindgen_test]
fn arguments_are_coerced_as_v8_coerces_them() {
    assert_eq!(op_args::number(&JsValue::from_str("2.5")), 2.5);
    assert_eq!(op_args::number(&JsValue::NULL), 0.0);
    assert!(op_args::number(&JsValue::UNDEFINED).is_nan());
    assert_eq!(op_args::number(&JsValue::TRUE), 1.0);

    assert_eq!(op_args::uint32(&JsValue::from_f64(-1.0)), u32::MAX);
    assert_eq!(op_args::uint32(&JsValue::from_f64(4_294_967_297.0)), 1);
    assert_eq!(op_args::uint32(&JsValue::from_f64(2.9)), 2);
    assert_eq!(op_args::uint32(&JsValue::from_f64(-2.9)), u32::MAX - 1);
    assert_eq!(op_args::uint32(&JsValue::from_f64(f64::NAN)), 0);
    assert_eq!(op_args::uint32(&JsValue::from_str("7")), 7);

    assert!(!op_args::boolean(&JsValue::from_str("")));
    assert!(op_args::boolean(&JsValue::from_str("0")));
    assert!(!op_args::boolean(&JsValue::from_f64(0.0)));
    assert!(op_args::boolean(&js_sys::Object::new().into()));

    assert_eq!(op_args::string(&JsValue::from_f64(42.0)), "");
    assert_eq!(op_args::string(&JsValue::from_str("x")), "x");

    let args = js_sys::Array::of1(&JsValue::from_f64(3.0));
    assert_eq!(op_args::arg(&args, 0).as_f64(), Some(3.0));
    assert!(
        op_args::arg(&args, 1).is_undefined(),
        "past the end is undefined"
    );
}
