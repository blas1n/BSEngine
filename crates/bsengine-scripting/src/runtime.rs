//! The JavaScript engine scripts run in: V8 through `deno_core` natively,
//! the page's own engine in a browser build (`deno_core` has no wasm
//! target), and QuickJS on Android and iOS (rusty_v8 ships no library for
//! either, and iOS forbids an app's own JIT) -- or on a desktop built with the
//! `quickjs` feature, which is how that engine is tested. All three expose the
//! same `ScriptRuntime` -- evaluate, execute, call a named function -- which
//! is all the plugin uses, and all give scripts the ops as `Deno.core.ops`.

#[cfg(target_arch = "wasm32")]
pub use browser::{ScriptRuntime, ENGINE};

/// How a browser build's ops read their arguments: as `deno_core`'s `op2`
/// reads them in V8 (see `v8_u32` for where these rules come from), so a
/// script passing the same values gets the same result in both builds.
/// `#[script_op]`'s generated registrations call these; public only for the
/// browser runtime's tests.
#[cfg(target_arch = "wasm32")]
#[doc(hidden)]
pub mod op_args {
    use wasm_bindgen::{JsCast, JsValue};

    /// Argument `index`, `undefined` past the end -- as a V8 op sees a
    /// missing argument.
    pub fn arg(args: &js_sys::Array, index: u32) -> JsValue {
        js_sys::Reflect::get_u32(args, index).unwrap_or(JsValue::UNDEFINED)
    }

    /// A BigInt's value wrapped to 64 bits, as V8's `i64_value`; `None` for
    /// anything that is not a BigInt.
    fn big_int(v: &JsValue) -> Option<i64> {
        if !v.is_bigint() {
            return None;
        }
        let wrapped = js_sys::BigInt::as_int_n(64.0, v.unchecked_ref());
        i64::try_from(JsValue::from(wrapped)).ok()
    }

    fn expected(ty: &str) -> JsValue {
        js_sys::TypeError::new(&format!("expected {ty}")).into()
    }

    /// A number argument: a Number as it is, a BigInt by its 64-bit value;
    /// anything else -- `"2"`, `null`, `undefined`, `true`, an object with
    /// a `valueOf` -- throws `TypeError: expected <ty>`. Not ToNumber,
    /// which this function applied until the engine-parity tests measured
    /// V8 (#2067).
    pub fn number(v: &JsValue, ty: &str) -> Result<f64, JsValue> {
        if let Some(n) = v.as_f64() {
            return Ok(n);
        }
        big_int(v).map(|n| n as f64).ok_or_else(|| expected(ty))
    }

    /// A `u32` argument: a Number by `v8_u32`, a BigInt by its low 32 bits;
    /// anything else throws `TypeError: expected u32`.
    pub fn uint32(v: &JsValue) -> Result<u32, JsValue> {
        if let Some(n) = v.as_f64() {
            return Ok(super::v8_u32(n));
        }
        big_int(v).map(|n| n as u32).ok_or_else(|| expected("u32"))
    }

    /// A `bool` argument: `true` and nothing else -- V8's `is_true()`, so
    /// `1` and `"yes"` are false, and nothing throws. Not truthiness.
    pub fn boolean(v: &JsValue) -> bool {
        v.as_bool() == Some(true)
    }

    /// A `#[string]` argument: the string, or `""` for anything that is not
    /// one -- what `deno_core` gives, where wasm-bindgen's own `String`
    /// parameter would throw.
    pub fn string(v: &JsValue) -> String {
        v.as_string().unwrap_or_default()
    }
}

/// `deno_core`'s `to_u32_option` on a Number (0.404, `runtime/ops.rs`),
/// shared by the two backends that are not V8. V8 asks the *value* whether
/// it is a uint32 or an int32 -- QuickJS may hold `-2` as a float, so this
/// cannot ask the representation -- and wraps those (so -1 is 4294967295);
/// any other number goes through `trunc() as u64`, which saturates (so -2.9
/// is 0, NaN 0, Infinity u64::MAX), keeping the low 32 bits (so 4294967297
/// is 1).
///
/// This rule and the number/bool ones beside it were measured against V8,
/// not taken from documentation: they are not JavaScript's
/// ToNumber/ToUint32/ToBoolean, which both backends once assumed and the
/// engine-parity tests (`parity_cases.rs`) caught.
#[cfg(any(target_arch = "wasm32", bsengine_quickjs))]
fn v8_u32(n: f64) -> u32 {
    if n.fract() == 0.0 && n >= f64::from(i32::MIN) && n <= f64::from(u32::MAX) {
        n as i64 as u32
    } else {
        n.trunc() as u64 as u32
    }
}

/// How a QuickJS build's ops read their arguments and return `#[serde]`
/// values: the conversions `deno_core` applies, as `op_args` gives them in a
/// browser. `#[script_op]`'s generated registrations call these.
#[cfg(bsengine_quickjs)]
#[doc(hidden)]
pub mod qjs_args {
    use rquickjs::{Ctx, Exception, Value};

    /// Argument `index`, `undefined` past the end -- as a V8 op sees a
    /// missing argument.
    pub fn arg<'js>(
        ctx: &Ctx<'js>,
        args: &rquickjs::function::Rest<Value<'js>>,
        index: usize,
    ) -> Value<'js> {
        args.0
            .get(index)
            .cloned()
            .unwrap_or_else(|| Value::new_undefined(ctx.clone()))
    }

    // The rules are `deno_core`'s, measured against V8; see `super::v8_u32`.

    /// A number argument: a Number as it is, a BigInt by its 64-bit value;
    /// anything else throws `TypeError: expected <ty>`.
    pub fn number<'js>(ctx: &Ctx<'js>, v: Value<'js>, ty: &str) -> rquickjs::Result<f64> {
        if let Some(n) = v.as_number() {
            return Ok(n);
        }
        if let Some(big) = v.into_big_int() {
            return Ok(big.to_i64()? as f64);
        }
        Err(Exception::throw_type(ctx, &format!("expected {ty}")))
    }

    /// A `u32` argument: a Number or BigInt, as `deno_core` reads one.
    pub fn uint32<'js>(ctx: &Ctx<'js>, v: Value<'js>) -> rquickjs::Result<u32> {
        if let Some(n) = v.as_number() {
            return Ok(super::v8_u32(n));
        }
        if let Some(big) = v.into_big_int() {
            return Ok(big.to_i64()? as u32);
        }
        Err(Exception::throw_type(ctx, "expected u32"))
    }

    /// A `bool` argument: `true` and nothing else -- V8's `is_true()`, so
    /// `1` and `"yes"` are false, and nothing throws.
    pub fn boolean<'js>(_ctx: &Ctx<'js>, v: Value<'js>) -> rquickjs::Result<bool> {
        Ok(v.as_bool() == Some(true))
    }

    /// A `#[string]` argument: the string, or `""` for anything that is not
    /// one -- what `deno_core` gives.
    pub fn string(v: &Value<'_>) -> String {
        v.as_string()
            .and_then(|s| s.to_string().ok())
            .unwrap_or_default()
    }

    /// A `#[serde]` argument, through `JSON.stringify`. A value that does not
    /// fit its type throws a `TypeError`, as `deno_core`'s does; `undefined`
    /// reads as `null`, which is what `serde_v8` makes of it for an `Option`.
    pub fn serde<'js, T: serde::de::DeserializeOwned>(
        ctx: &Ctx<'js>,
        v: Value<'js>,
    ) -> rquickjs::Result<T> {
        let json = match ctx.json_stringify(v)? {
            Some(s) => s.to_string()?,
            None => "null".to_string(),
        };
        serde_json::from_str(&json).map_err(|e| Exception::throw_type(ctx, &e.to_string()))
    }

    /// A `#[serde]` return value, through `JSON.parse`: `None` is `null`, as
    /// `serde_v8` gives it.
    pub fn to_serde<'js, T: serde::Serialize>(
        ctx: &Ctx<'js>,
        value: &T,
    ) -> rquickjs::Result<Value<'js>> {
        let json =
            serde_json::to_string(value).map_err(|e| Exception::throw_type(ctx, &e.to_string()))?;
        ctx.json_parse(json)
    }
}

#[cfg(not(any(target_arch = "wasm32", bsengine_quickjs)))]
pub use native::{ScriptRuntime, ENGINE};

#[cfg(bsengine_quickjs)]
pub use quickjs::{ScriptRuntime, ENGINE};

#[cfg(not(any(target_arch = "wasm32", bsengine_quickjs)))]
mod native {
    use deno_core::{JsRuntime, RuntimeOptions};
    use std::sync::Once;

    /// The engine scripts run in, as the startup log names it.
    pub const ENGINE: &str = "V8";

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

#[cfg(bsengine_quickjs)]
mod quickjs {
    use rquickjs::context::EvalOptions;
    use rquickjs::convert::Coerced;
    use rquickjs::{Context, Ctx, FromJs, Object, Runtime, Value};

    /// The engine scripts run in, as the startup log names it.
    pub const ENGINE: &str = "QuickJS";

    /// How deep a script may recurse before QuickJS throws a `RangeError`
    /// instead of overflowing the thread. QuickJS measures from the stack
    /// address where the runtime was made, so this must stay under the stack
    /// that thread really has: 4 MiB is inside libtest's (RUST_MIN_STACK,
    /// 16 MiB) and the desktop main thread's 64 MiB, and still deeper than
    /// any script here recurses. V8 is given 16 MiB for the same purpose.
    const MAX_STACK_BYTES: usize = 4 * 1024 * 1024;

    /// One QuickJS runtime and context: every script in it shares one
    /// global scope, as in one V8 isolate.
    pub struct ScriptRuntime {
        context: Context,
        // Dropped after the context, which it outlives in QuickJS.
        _runtime: Runtime,
    }

    /// `deno_core`'s `execute_script` runs a classic script in sloppy mode;
    /// `rquickjs`'s default is strict, under which a script assigning an
    /// undeclared global throws here and not in V8.
    fn options(name: &str) -> EvalOptions {
        let mut options = EvalOptions::default();
        options.strict = false;
        options.filename = Some(name.to_string());
        options
    }

    /// An exception as `deno_core` reports one: the thrown value as
    /// `String()` gives it ("TypeError: ..."), with the stack after it when
    /// it is an `Error`.
    fn describe(ctx: &Ctx<'_>, error: rquickjs::Error) -> String {
        if !matches!(error, rquickjs::Error::Exception) {
            return error.to_string();
        }
        let thrown = ctx.catch();
        let text = Coerced::<String>::from_js(ctx, thrown.clone())
            .map(|s| s.0)
            .unwrap_or_else(|_| format!("{thrown:?}"));
        let stack = thrown
            .as_object()
            .and_then(|o| o.get::<_, Option<String>>("stack").ok().flatten())
            .filter(|s| !s.is_empty());
        match stack {
            Some(stack) => format!("{text}\n{stack}"),
            None => text,
        }
    }

    impl ScriptRuntime {
        /// A runtime with no ops installed.
        pub fn new() -> Self {
            let runtime = Runtime::new().expect("creating a QuickJS runtime");
            runtime.set_max_stack_size(MAX_STACK_BYTES);
            let context = Context::full(&runtime).expect("creating a QuickJS context");
            Self {
                context,
                _runtime: runtime,
            }
        }

        /// A runtime whose scripts see every `#[script_op]` as
        /// `Deno.core.ops.<name>`, under the names the V8 build registers
        /// them by (`crate::ops::register_ops`).
        pub fn new_with_ops() -> Self {
            let this = Self::new();
            this.context
                .with(|ctx| -> rquickjs::Result<()> {
                    let ops = Object::new(ctx.clone())?;
                    crate::ops::register_ops(&ctx, &ops)?;
                    let core = Object::new(ctx.clone())?;
                    core.set("ops", ops)?;
                    let deno = Object::new(ctx.clone())?;
                    deno.set("core", core)?;
                    ctx.globals().set("Deno", deno)
                })
                .expect("registering the script ops");
            this
        }

        /// Evaluates a JS expression and returns its result stringified, as
        /// `String(value)` would -- what V8's `to_rust_string_lossy` gives.
        pub fn eval(&mut self, src: &str) -> Result<String, String> {
            self.context.with(|ctx| {
                let value: Value = ctx
                    .eval_with_options(src, options("<eval>"))
                    .map_err(|e| describe(&ctx, e))?;
                Coerced::<String>::from_js(&ctx, value)
                    .map(|s| s.0)
                    .map_err(|e| describe(&ctx, e))
            })
        }

        /// Executes a script without capturing its return value.
        pub fn exec_source(&mut self, src: &str, name: &str) -> Result<(), String> {
            self.context.with(|ctx| {
                ctx.eval_with_options::<(), _>(src, options(name))
                    .map_err(|e| describe(&ctx, e))
            })
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

#[cfg(target_arch = "wasm32")]
mod browser {
    use wasm_bindgen::{JsCast, JsValue};

    /// The engine scripts run in, as the startup log names it.
    pub const ENGINE: &str = "the browser's";

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
