use deno_core::{JsRuntime, RuntimeOptions};

pub struct ScriptRuntime {
    runtime: JsRuntime,
}

impl ScriptRuntime {
    pub fn new() -> Self {
        let runtime = JsRuntime::new(RuntimeOptions {
            ..Default::default()
        });
        Self { runtime }
    }

    pub fn new_with_ops() -> Self {
        let runtime = JsRuntime::new(RuntimeOptions {
            extensions: vec![crate::ops::bsengine_ops::init()],
            ..Default::default()
        });
        Self { runtime }
    }

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
        let src = format!(
            "if (typeof {fn_name} === 'function') {{ {fn_name}(); }}"
        );
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
