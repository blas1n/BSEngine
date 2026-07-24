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
