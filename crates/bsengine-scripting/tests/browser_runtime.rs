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
use wasm_bindgen::{JsCast, JsValue};
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

include!("../src/parity_cases.rs");

/// The JavaScript value an expression from the shared table evaluates to.
/// Parenthesised, so `{}` is an object and not a block.
fn value_of(expression: &str) -> JsValue {
    js_sys::eval(&format!("({expression})")).expect(expression)
}

/// The browser build reads every case of the table the V8 build is held to
/// (`src/parity_cases.rs`) as V8 does: Number and BigInt only for numbers,
/// `true` only for `bool`, `""` for a non-string name. Until #2067 measured
/// V8 this applied ToNumber and ToBoolean, so `"2.5"` was 2.5 here and a
/// `TypeError` natively.
#[wasm_bindgen_test]
fn arguments_are_read_as_v8_reads_them() {
    for &(expression, expected) in F32_CASES {
        let got = op_args::number(&value_of(expression), "f32").map(|n| n as f32);
        match expected {
            Some(expected) => assert_eq!(got.ok(), Some(expected), "{expression}"),
            None => {
                let err = got.expect_err(expression);
                let err: &js_sys::Error = err.unchecked_ref();
                assert_eq!(
                    String::from(err.to_string()),
                    "TypeError: expected f32",
                    "{expression}"
                );
            }
        }
    }
    assert!(op_args::number(&value_of("NaN"), "f32").unwrap().is_nan());
    for &(expression, expected) in U32_CASES {
        assert_eq!(
            op_args::uint32(&value_of(expression)).ok(),
            expected,
            "{expression}"
        );
    }
    for &(expression, expected) in BOOL_CASES {
        assert_eq!(
            op_args::boolean(&value_of(expression)),
            expected,
            "{expression}"
        );
    }
    for &(expression, expected) in STRING_CASES {
        assert_eq!(
            op_args::string(&value_of(expression)),
            expected,
            "{expression}"
        );
    }

    let args = js_sys::Array::of1(&JsValue::from_f64(3.0));
    assert_eq!(op_args::arg(&args, 0).as_f64(), Some(3.0));
    assert!(
        op_args::arg(&args, 1).is_undefined(),
        "past the end is undefined"
    );
}

/// And through a real op: a string where a number goes throws the
/// `TypeError` V8 throws, which the script can catch, and nothing runs on
/// with a made-up number.
#[wasm_bindgen_test]
fn an_op_given_a_string_for_a_number_throws_a_type_error() {
    let mut rt = ScriptRuntime::new_with_ops();
    assert_eq!(
        rt.eval(
            "(() => { try { Deno.core.ops.bsengine_set_ao_radius('cam', '2.5'); return 'no throw'; } \
             catch (e) { return String(e); } })()"
        )
        .unwrap(),
        "TypeError: expected f32"
    );
    assert_eq!(
        rt.eval(
            "(() => { try { Deno.core.ops.bsengine_set_ao_sample_count('cam'); return 'no throw'; } \
             catch (e) { return String(e); } })()"
        )
        .unwrap(),
        "TypeError: expected u32",
        "a missing argument is undefined, which is not a number"
    );
}
