use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use deno_core::op2;
use glam::Vec3;
use serde::Serialize;

#[derive(Clone)]
pub enum ScriptCommand {
    SetTransform {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    SetEmissive {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
    SetColor {
        name: String,
        r: f32,
        g: f32,
        b: f32,
    },
}

thread_local! {
    pub(crate) static TRANSFORM_SNAPSHOT: RefCell<HashMap<String, Vec3>> =
        RefCell::new(HashMap::new());
    pub(crate) static KEY_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static ENTITY_NAMES_SNAPSHOT: RefCell<Vec<String>> =
        RefCell::new(Vec::new());
    pub(crate) static COMMAND_BUFFER: RefCell<Vec<ScriptCommand>> =
        RefCell::new(Vec::new());
}

#[derive(Serialize)]
struct Vec3Json {
    x: f32,
    y: f32,
    z: f32,
}

#[op2(fast)]
pub fn bsengine_log(#[string] msg: String) {
    tracing::info!("[script] {}", msg);
}

#[op2]
#[string]
pub fn bsengine_version() -> String {
    "0.1.0".to_string()
}

#[op2]
#[serde]
pub fn bsengine_get_transform(#[string] name: String) -> Option<Vec3Json> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|t| Vec3Json {
            x: t.x,
            y: t.y,
            z: t.z,
        })
    })
}

#[op2(fast)]
pub fn bsengine_set_transform(#[string] name: String, x: f32, y: f32, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetTransform { name, x, y, z });
    });
}

#[op2(fast)]
pub fn bsengine_is_key_pressed(#[string] key: String) -> bool {
    KEY_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2]
#[string]
pub fn bsengine_get_entity_names() -> String {
    ENTITY_NAMES_SNAPSHOT
        .with(|s| serde_json::to_string(&*s.borrow()).unwrap_or_else(|_| "[]".to_string()))
}

#[op2(fast)]
pub fn bsengine_set_emissive(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetEmissive { name, r, g, b });
    });
}

#[op2(fast)]
pub fn bsengine_set_color(#[string] name: String, r: f32, g: f32, b: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetColor { name, r, g, b });
    });
}

deno_core::extension!(
    bsengine_ops,
    ops = [
        bsengine_log,
        bsengine_version,
        bsengine_get_transform,
        bsengine_set_transform,
        bsengine_is_key_pressed,
        bsengine_get_entity_names,
        bsengine_set_emissive,
        bsengine_set_color,
    ],
);

/// Bootstrap JS loaded before any user script — exposes the `Bsengine` global.
pub const BOOTSTRAP_JS: &str = r#"
const Bsengine = {
    log:            (msg)           => Deno.core.ops.bsengine_log(msg),
    version:        ()              => Deno.core.ops.bsengine_version(),
    getTransform:   (name)          => Deno.core.ops.bsengine_get_transform(name),
    setTransform:   (name, x, y, z) => Deno.core.ops.bsengine_set_transform(name, x, y, z),
    isKeyPressed:   (key)           => Deno.core.ops.bsengine_is_key_pressed(key),
    getEntityNames: ()              => JSON.parse(Deno.core.ops.bsengine_get_entity_names()),
    setEmissive:    (name, r, g, b) => Deno.core.ops.bsengine_set_emissive(name, r, g, b),
    setColor:       (name, r, g, b) => Deno.core.ops.bsengine_set_color(name, r, g, b),

    // Per-entity script registry. Keys are entity bit-IDs (strings).
    _scripts: {},

    // Called each frame by the engine with [[id, name], ...] for all scripted entities.
    _runAll(entities) {
        for (const [id, name] of entities) {
            const s = this._scripts[id];
            if (s && s.onUpdate) {
                try {
                    s.onUpdate(name);
                } catch (e) {
                    this.log(`[${name}] onUpdate error: ${e}`);
                }
            }
        }
    },
};
"#;

#[cfg(test)]
mod tests {
    use crate::runtime::ScriptRuntime;

    #[test]
    fn op_log_callable_from_script() {
        let mut rt = ScriptRuntime::new_with_ops();
        let result = rt.eval(r#"Deno.core.ops.bsengine_log("hello from script"); "ok""#);
        assert!(result.is_ok(), "op call failed: {:?}", result);
        assert!(result.unwrap().contains("ok"));
    }

    #[test]
    fn op_version_returns_string() {
        let mut rt = ScriptRuntime::new_with_ops();
        let result = rt.eval(r#"Deno.core.ops.bsengine_version()"#);
        assert!(result.is_ok(), "version op failed: {:?}", result);
        let v = result.unwrap();
        assert!(v.contains("0.1"), "unexpected version: {v}");
    }

    #[test]
    fn bsengine_global_exposed_after_bootstrap() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"typeof Bsengine !== "undefined" ? "ok" : "missing""#)
            .unwrap();
        assert!(r.contains("ok"), "Bsengine global missing: {r}");
    }

    #[test]
    fn get_transform_returns_null_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getTransform("NoSuchEntity"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn is_key_pressed_returns_false_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyPressed("W") ? "pressed" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not pressed: {r}");
    }
}
