//! The JavaScript engine scripts run in: V8 through `deno_core` natively,
//! and the page's own engine in a browser build (`deno_core` has no wasm
//! target). Both expose the same `ScriptRuntime` -- evaluate, execute, call a
//! named function -- which is all the plugin uses, and both give scripts the
//! ops as `Deno.core.ops`.

#[cfg(target_arch = "wasm32")]
pub use browser::ScriptRuntime;

/// How a browser build's ops read their arguments: the conversions
/// `deno_core` applies, so a script passing the same values gets the same
/// result in both builds. `#[script_op]`'s generated registrations call
/// these; public only for the browser runtime's tests.
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub mod op_args {
    use wasm_bindgen::JsValue;

    /// Argument `index`, `undefined` past the end -- as a V8 op sees a
    /// missing argument.
    pub fn arg(args: &js_sys::Array, index: u32) -> JsValue {
        js_sys::Reflect::get_u32(args, index).unwrap_or(JsValue::UNDEFINED)
    }

    /// ToNumber (JavaScript's unary `+`), as V8's `number_value`: `"2"` is
    /// 2, `null` 0, `undefined` NaN.
    pub fn number(v: &JsValue) -> f64 {
        v.unchecked_into_f64()
    }

    /// ToUint32: ToNumber, truncated, wrapped modulo 2^32; NaN and the
    /// infinities are 0. So -1 is 4294967295, as a V8 `u32` op reads it.
    pub fn uint32(v: &JsValue) -> u32 {
        let n = number(v);
        if n.is_finite() {
            (n.trunc() as i128).rem_euclid(1i128 << 32) as u32
        } else {
            0
        }
    }

    /// ToBoolean: JavaScript truthiness.
    pub fn boolean(v: &JsValue) -> bool {
        v.is_truthy()
    }

    /// A `#[string]` argument: the string, or `""` for anything that is not
    /// one -- what `deno_core` gives, where wasm-bindgen's own `String`
    /// parameter would throw.
    pub fn string(v: &JsValue) -> String {
        v.as_string().unwrap_or_default()
    }
}
#[cfg(not(target_arch = "wasm32"))]
pub use native::ScriptRuntime;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use deno_core::{JsRuntime, RuntimeOptions};
    use std::sync::Once;

    // V8 auto-detects its internal JS-stack limit from the current thread's OS
    // stack at isolate-creation time, but on Windows this detection can fall back
    // to a tiny default (~984 KB) regardless of the actual reserved thread stack
    // (see .cargo/config.toml's /STACK:67108864), causing V8 to abort with
    // "Check failed: IsOnCentralStack()" while compiling even trivial scripts.
    // Set --stack-size explicitly (in KB) so V8's own limit is generous, well
    // within the real 64 MB OS-reserved stack.
    static INIT_V8_FLAGS: Once = Once::new();

    fn ensure_v8_flags() {
        INIT_V8_FLAGS.call_once(|| {
            deno_core::v8_set_flags(vec![
                "bsengine".to_string(),
                "--stack-size=16384".to_string(),
            ]);
        });
    }

    /// A single V8 isolate wrapping a `deno_core::JsRuntime`, used to execute
    /// script-defined behavior for one entity or subsystem.
    pub struct ScriptRuntime {
        runtime: JsRuntime,
    }

    impl ScriptRuntime {
        /// Create a bare runtime with no BSEngine ops registered (useful for
        /// plain JS evaluation in tests).
        pub fn new() -> Self {
            ensure_v8_flags();
            let runtime = JsRuntime::new(RuntimeOptions {
                ..Default::default()
            });
            Self { runtime }
        }

        /// Create a runtime with the full `bsengine_ops` extension registered,
        /// exposing ECS operations to scripts.
        pub fn new_with_ops() -> Self {
            ensure_v8_flags();
            let runtime = JsRuntime::new(RuntimeOptions {
                extensions: vec![crate::ops::bsengine_ops::init()],
                ..Default::default()
            });
            Self { runtime }
        }

        /// Evaluate a JS expression and return its result stringified.
        pub fn eval(&mut self, src: &str) -> Result<String, String> {
            let result = self
                .runtime
                .execute_script("<eval>", src.to_string())
                .map_err(|e| e.to_string())?;

            deno_core::scope!(scope, self.runtime);
            let value = result.open(scope);
            Ok(value.to_rust_string_lossy(scope))
        }

        /// Execute a script without capturing its return value. Used for loading definitions.
        pub fn exec_source(&mut self, src: &str, _name: &str) -> Result<(), String> {
            self.runtime
                .execute_script("<source>", src.to_string())
                .map(|_| ())
                .map_err(|e| e.to_string())
        }

        /// Call a named JS function if it exists, ignoring return value.
        pub fn call_fn(&mut self, fn_name: &str) -> Result<(), String> {
            let src = format!("if (typeof {fn_name} === 'function') {{ {fn_name}(); }}");
            self.exec_source(&src, "<call_fn>")
        }
    }

    impl Default for ScriptRuntime {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::{JsCast, JsValue};

    /// The page's JavaScript engine, which every script in a browser build
    /// shares -- there is one global scope, as there is one V8 isolate
    /// natively.
    pub struct ScriptRuntime {
        _private: (),
    }

    /// The *indirect* `eval` -- `globalThis.eval` called as a plain function
    /// -- which runs a script in the global scope, as `deno_core` runs one
    /// in the isolate's. `js_sys::eval` is a direct call from inside
    /// wasm-bindgen's glue, so a `var` in the prelude would land in that
    /// glue function's scope instead of on `globalThis`.
    fn global_eval(src: &str) -> Result<JsValue, String> {
        let global = js_sys::global();
        let eval: js_sys::Function = js_sys::Reflect::get(&global, &JsValue::from_str("eval"))
            .map_err(describe)?
            .dyn_into()
            .map_err(describe)?;
        eval.call1(&JsValue::UNDEFINED, &JsValue::from_str(src))
            .map_err(describe)
    }

    /// An exception as `deno_core` reports one: an `Error`'s `toString()`
    /// ("TypeError: ..."), or a thrown non-error's own string.
    fn describe(e: JsValue) -> String {
        if let Some(err) = e.dyn_ref::<js_sys::Error>() {
            return String::from(err.to_string());
        }
        e.as_string().unwrap_or_else(|| format!("{e:?}"))
    }

    impl ScriptRuntime {
        /// A runtime with no ops installed.
        pub fn new() -> Self {
            Self { _private: () }
        }

        /// A runtime whose scripts see every `#[script_op]` as
        /// `Deno.core.ops.<name>`, under the names the native build registers
        /// them by (`crate::ops::register_ops`).
        pub fn new_with_ops() -> Self {
            let ops = js_sys::Object::new();
            crate::ops::register_ops(&ops);
            let core = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&core, &JsValue::from_str("ops"), &ops);
            let deno = js_sys::Object::new();
            let _ = js_sys::Reflect::set(&deno, &JsValue::from_str("core"), &core);
            let _ = js_sys::Reflect::set(&js_sys::global(), &JsValue::from_str("Deno"), &deno);
            Self { _private: () }
        }

        /// Evaluates a JS expression and returns its result stringified, as
        /// `String(value)` would -- what V8's `to_rust_string_lossy` gives.
        /// (`JsString::from` is an unchecked cast, not a conversion: a number
        /// through it is not a string and cannot be read as one.)
        pub fn eval(&mut self, src: &str) -> Result<String, String> {
            let value = global_eval(src)?;
            let to_string: js_sys::Function =
                js_sys::Reflect::get(&js_sys::global(), &JsValue::from_str("String"))
                    .map_err(describe)?
                    .dyn_into()
                    .map_err(describe)?;
            to_string
                .call1(&JsValue::UNDEFINED, &value)
                .map_err(describe)?
                .as_string()
                .ok_or_else(|| "String() did not return a string".to_string())
        }

        /// Executes a script without capturing its return value.
        pub fn exec_source(&mut self, src: &str, _name: &str) -> Result<(), String> {
            global_eval(src).map(|_| ())
        }

        /// Calls a named JS function if it exists, ignoring its return value.
        pub fn call_fn(&mut self, fn_name: &str) -> Result<(), String> {
            let src = format!("if (typeof {fn_name} === 'function') {{ {fn_name}(); }}");
            self.exec_source(&src, "<call_fn>")
        }
    }

    impl Default for ScriptRuntime {
        fn default() -> Self {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_executes_simple_js() {
        let mut rt = ScriptRuntime::new();
        let result = rt.eval("1 + 2").expect("eval failed");
        assert_eq!(result, "3");
    }

    #[test]
    fn runtime_executes_string_expression() {
        let mut rt = ScriptRuntime::new();
        let result = rt.eval(r#""hello" + " world""#).expect("eval failed");
        // Result may be quoted or unquoted depending on value type
        assert!(
            result.contains("hello world"),
            "Expected 'hello world' in: {result}"
        );
    }

    #[test]
    fn runtime_returns_error_on_syntax_error() {
        let mut rt = ScriptRuntime::new();
        let result = rt.eval("this is not valid JS !!!");
        assert!(result.is_err());
    }

    #[test]
    fn runtime_executes_multiline_script() {
        let mut rt = ScriptRuntime::new();
        let script = "let x = 10; let y = 20; x + y";
        let result = rt.eval(script).expect("eval failed");
        assert_eq!(result, "30");
    }
}
