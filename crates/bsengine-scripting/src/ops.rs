use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use deno_core::op2;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize)]
pub struct SpawnParams {
    pub name: String,
    #[serde(default = "default_primitive")]
    pub primitive: String,
    #[serde(default)]
    pub x: f32,
    #[serde(default)]
    pub y: f32,
    #[serde(default)]
    pub z: f32,
    #[serde(default)]
    pub rx: f32,
    #[serde(default)]
    pub ry: f32,
    #[serde(default)]
    pub rz: f32,
    #[serde(default = "default_one")]
    pub rw: f32,
    #[serde(default = "default_one")]
    pub sx: f32,
    #[serde(default = "default_one")]
    pub sy: f32,
    #[serde(default = "default_one")]
    pub sz: f32,
    pub color: Option<[f32; 3]>,
    pub emissive: Option<[f32; 3]>,
    pub script: Option<String>,
}

fn default_primitive() -> String {
    "Cube".to_string()
}
fn default_one() -> f32 {
    1.0
}

#[derive(Clone)]
pub enum ScriptCommand {
    SetTransform {
        name: String,
        x: f32,
        y: f32,
        z: f32,
    },
    SetRotation {
        name: String,
        rx: f32,
        ry: f32,
        rz: f32,
        rw: f32,
    },
    SetScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
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
    Spawn(SpawnParams),
    Destroy {
        name: String,
    },
    PlaySound {
        id: u32,
        path: String,
        volume: f32,
        loop_: bool,
    },
    StopSound {
        id: u32,
    },
    SetHudText {
        id: String,
        text: String,
    },
    ClearHudText {
        id: String,
    },
    LoadScene {
        path: String,
    },
    SetSkybox {
        path: String,
    },
}

thread_local! {
    pub(crate) static TRANSFORM_SNAPSHOT: RefCell<HashMap<String, (Vec3, Quat, Vec3)>> =
        RefCell::new(HashMap::new());
    pub(crate) static KEY_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static KEY_JUST_PRESSED_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static KEY_JUST_RELEASED_SNAPSHOT: RefCell<HashSet<String>> =
        RefCell::new(HashSet::new());
    pub(crate) static ENTITY_NAMES_SNAPSHOT: RefCell<Vec<String>> =
        RefCell::new(Vec::new());
    pub(crate) static COLLISION_SNAPSHOT: RefCell<Vec<(String, String, bool)>> =
        RefCell::new(Vec::new());
    pub(crate) static COMMAND_BUFFER: RefCell<Vec<ScriptCommand>> =
        RefCell::new(Vec::new());
    pub(crate) static SOUND_ID_COUNTER: RefCell<u32> =
        RefCell::new(0);

    // Mouse state snapshots (bit 0=Left, bit 1=Right, bit 2=Middle)
    pub(crate) static MOUSE_PRESSED_SNAPSHOT: RefCell<u8> = RefCell::new(0);
    pub(crate) static MOUSE_JUST_PRESSED_SNAPSHOT: RefCell<u8> = RefCell::new(0);
    pub(crate) static MOUSE_JUST_RELEASED_SNAPSHOT: RefCell<u8> = RefCell::new(0);
    pub(crate) static MOUSE_POS_SNAPSHOT: RefCell<(f64, f64)> = RefCell::new((0.0, 0.0));
    pub(crate) static MOUSE_DELTA_SNAPSHOT: RefCell<(f64, f64)> = RefCell::new((0.0, 0.0));

    // Raycast: raw pointer to PhysicsWorld, valid only during V8 execution in run_scripts.
    // Safety: set before V8 runs, cleared immediately after. V8 is synchronous.
    pub(crate) static PHYSICS_WORLD_PTR: RefCell<*const bsengine_physics::PhysicsWorld> =
        RefCell::new(std::ptr::null());

    // Entity name lookup for raycast results: entity.to_bits() → name
    pub(crate) static ENTITY_NAME_MAP: RefCell<HashMap<u64, String>> =
        RefCell::new(HashMap::new());

    // Gamepad button state (bit 0=South..15=DPadRight)
    pub(crate) static GAMEPAD_BUTTON_SNAPSHOT: RefCell<u16> = RefCell::new(0);
    pub(crate) static GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT: RefCell<u16> = RefCell::new(0);
    pub(crate) static GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT: RefCell<u16> = RefCell::new(0);
    // (left_x, left_y, right_x, right_y, left_trigger, right_trigger)
    pub(crate) static GAMEPAD_STICKS_SNAPSHOT: RefCell<(f32, f32, f32, f32, f32, f32)> =
        RefCell::new((0.0, 0.0, 0.0, 0.0, 0.0, 0.0));
}

/// Full transform returned to scripts: position + rotation quaternion + scale.
#[derive(Serialize)]
struct TransformJson {
    x: f32,
    y: f32,
    z: f32,
    rx: f32,
    ry: f32,
    rz: f32,
    rw: f32,
    sx: f32,
    sy: f32,
    sz: f32,
}

#[derive(Serialize)]
struct RaycastHitJson {
    entity_name: Option<String>,
    point: [f32; 3],
    normal: [f32; 3],
    distance: f32,
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
pub fn bsengine_get_transform(#[string] name: String) -> Option<TransformJson> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(pos, rot, scale)| TransformJson {
                x: pos.x,
                y: pos.y,
                z: pos.z,
                rx: rot.x,
                ry: rot.y,
                rz: rot.z,
                rw: rot.w,
                sx: scale.x,
                sy: scale.y,
                sz: scale.z,
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
pub fn bsengine_set_rotation(#[string] name: String, rx: f32, ry: f32, rz: f32, rw: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetRotation {
            name,
            rx,
            ry,
            rz,
            rw,
        });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetScale { name, sx, sy, sz });
    });
}

#[op2(fast)]
pub fn bsengine_is_key_pressed(#[string] key: String) -> bool {
    KEY_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2(fast)]
pub fn bsengine_is_key_down(#[string] key: String) -> bool {
    KEY_JUST_PRESSED_SNAPSHOT.with(|k| k.borrow().contains(&key))
}

#[op2(fast)]
pub fn bsengine_is_key_up(#[string] key: String) -> bool {
    KEY_JUST_RELEASED_SNAPSHOT.with(|k| k.borrow().contains(&key))
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

#[op2]
pub fn bsengine_spawn(#[serde] params: SpawnParams) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::Spawn(params)));
}

#[op2(fast)]
pub fn bsengine_destroy(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::Destroy { name }));
}

#[op2(fast)]
pub fn bsengine_play_sound(#[string] path: String, volume: f32, loop_: bool) -> u32 {
    let id = SOUND_ID_COUNTER.with(|c| {
        let id = *c.borrow();
        *c.borrow_mut() = id + 1;
        id
    });
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::PlaySound {
            id,
            path,
            volume,
            loop_,
        });
    });
    id
}

#[op2(fast)]
pub fn bsengine_stop_sound(id: u32) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::StopSound { id }));
}

#[op2(fast)]
pub fn bsengine_set_hud_text(#[string] id: String, #[string] text: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SetHudText { id, text }));
}

#[op2(fast)]
pub fn bsengine_clear_hud_text(#[string] id: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::ClearHudText { id }));
}

#[op2(fast)]
pub fn bsengine_load_scene(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::LoadScene { path }));
}

#[op2(fast)]
pub fn bsengine_set_skybox(#[string] path: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::SetSkybox { path }));
}

// --- Mouse ops ---

#[op2(fast)]
pub fn bsengine_is_mouse_pressed(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_mouse_down(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_mouse_up(button: u32) -> bool {
    if button > 7 {
        return false;
    }
    MOUSE_JUST_RELEASED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2]
#[serde]
pub fn bsengine_get_mouse_pos() -> Vec<f64> {
    MOUSE_POS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

#[op2]
#[serde]
pub fn bsengine_get_mouse_delta() -> Vec<f64> {
    MOUSE_DELTA_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

// --- Raycast op ---

#[op2]
#[serde]
pub fn bsengine_raycast(
    ox: f32,
    oy: f32,
    oz: f32,
    dx: f32,
    dy: f32,
    dz: f32,
    max_dist: f32,
) -> Option<RaycastHitJson> {
    PHYSICS_WORLD_PTR.with(|p| {
        let ptr = *p.borrow();
        if ptr.is_null() {
            return None;
        }
        // SAFETY: ptr is valid for the duration of V8 execution (see plugin.rs run_scripts).
        let pw = unsafe { &*ptr };
        let dir_raw = Vec3::new(dx, dy, dz);
        let len = dir_raw.length();
        if len < 1e-7 {
            return None;
        }
        let origin = Vec3::new(ox, oy, oz);
        let dir = dir_raw / len;
        pw.cast_ray(origin, dir, max_dist).map(|hit| {
            let entity_name = hit
                .entity
                .and_then(|e| ENTITY_NAME_MAP.with(|m| m.borrow().get(&e.to_bits()).cloned()));
            RaycastHitJson {
                entity_name,
                point: [hit.point.x, hit.point.y, hit.point.z],
                normal: [hit.normal.x, hit.normal.y, hit.normal.z],
                distance: hit.distance,
            }
        })
    })
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button_down(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2(fast)]
pub fn bsengine_is_gamepad_button_up(button: u32) -> bool {
    if button > 15 {
        return false;
    }
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT.with(|s| ((*s.borrow()) >> button) & 1 != 0)
}

#[op2]
#[serde]
pub fn bsengine_get_left_stick() -> Vec<f32> {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.0, v.1]
    })
}

#[op2]
#[serde]
pub fn bsengine_get_right_stick() -> Vec<f32> {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        vec![v.2, v.3]
    })
}

#[op2(fast)]
pub fn bsengine_get_gamepad_trigger(side: u32) -> f32 {
    GAMEPAD_STICKS_SNAPSHOT.with(|s| {
        let v = *s.borrow();
        if side == 0 {
            v.4
        } else {
            v.5
        }
    })
}

deno_core::extension!(
    bsengine_ops,
    ops = [
        bsengine_log,
        bsengine_version,
        bsengine_get_transform,
        bsengine_set_transform,
        bsengine_set_rotation,
        bsengine_set_scale,
        bsengine_is_key_pressed,
        bsengine_is_key_down,
        bsengine_is_key_up,
        bsengine_get_entity_names,
        bsengine_set_emissive,
        bsengine_set_color,
        bsengine_spawn,
        bsengine_destroy,
        bsengine_play_sound,
        bsengine_stop_sound,
        bsengine_set_hud_text,
        bsengine_clear_hud_text,
        bsengine_load_scene,
        bsengine_is_mouse_pressed,
        bsengine_is_mouse_down,
        bsengine_is_mouse_up,
        bsengine_get_mouse_pos,
        bsengine_get_mouse_delta,
        bsengine_raycast,
        bsengine_is_gamepad_button,
        bsengine_is_gamepad_button_down,
        bsengine_is_gamepad_button_up,
        bsengine_get_left_stick,
        bsengine_get_right_stick,
        bsengine_get_gamepad_trigger,
        bsengine_set_skybox,
    ],
);

pub const BOOTSTRAP_JS: &str = r#"
const Bsengine = {
    log:            (msg)                  => Deno.core.ops.bsengine_log(msg),
    version:        ()                     => Deno.core.ops.bsengine_version(),
    getTransform:   (name)                 => Deno.core.ops.bsengine_get_transform(name),
    setTransform:   (name, x, y, z)        => Deno.core.ops.bsengine_set_transform(name, x, y, z),
    setRotation:    (name, rx, ry, rz, rw) => Deno.core.ops.bsengine_set_rotation(name, rx, ry, rz, rw),
    setScale:       (name, sx, sy, sz)     => Deno.core.ops.bsengine_set_scale(name, sx, sy, sz),
    isKeyPressed:   (key)                  => Deno.core.ops.bsengine_is_key_pressed(key),
    isKeyDown:      (key)                  => Deno.core.ops.bsengine_is_key_down(key),
    isKeyUp:        (key)                  => Deno.core.ops.bsengine_is_key_up(key),
    getEntityNames: ()                     => JSON.parse(Deno.core.ops.bsengine_get_entity_names()),
    setEmissive:    (name, r, g, b)        => Deno.core.ops.bsengine_set_emissive(name, r, g, b),
    setColor:       (name, r, g, b)        => Deno.core.ops.bsengine_set_color(name, r, g, b),
    spawn:          (params)               => Deno.core.ops.bsengine_spawn(params),
    destroy:        (name)                 => Deno.core.ops.bsengine_destroy(name),
    playSound:      (path, opts) => {
        const v = (opts && opts.volume !== undefined) ? opts.volume : 1.0;
        const l = (opts && opts.loop) ? true : false;
        return Deno.core.ops.bsengine_play_sound(path, v, l);
    },
    stopSound:      (id)                   => Deno.core.ops.bsengine_stop_sound(id),
    setHudText:     (id, text)             => Deno.core.ops.bsengine_set_hud_text(id, String(text)),
    clearHudText:   (id)                   => Deno.core.ops.bsengine_clear_hud_text(id),
    loadScene:      (path)                 => Deno.core.ops.bsengine_load_scene(path),

    // Mouse input (btn: 0=Left, 1=Right, 2=Middle)
    isMousePressed: (btn)  => Deno.core.ops.bsengine_is_mouse_pressed(btn),
    isMouseDown:    (btn)  => Deno.core.ops.bsengine_is_mouse_down(btn),
    isMouseUp:      (btn)  => Deno.core.ops.bsengine_is_mouse_up(btn),
    getMousePos:    ()     => { const v = Deno.core.ops.bsengine_get_mouse_pos(); return { x: v[0], y: v[1] }; },
    getMouseDelta:  ()     => { const v = Deno.core.ops.bsengine_get_mouse_delta(); return { x: v[0], y: v[1] }; },

    // Raycast: origin/{x,y,z}, dir/{x,y,z}, maxDist → {entityName, point, normal, distance} or null
    raycast:        (origin, dir, maxDist) =>
        Deno.core.ops.bsengine_raycast(origin.x, origin.y, origin.z, dir.x, dir.y, dir.z, maxDist),

    // Gamepad (btn: 0=South/A..15=DPadRight; side: 0=L2, 1=R2)
    isGamepadButton:     (btn)  => Deno.core.ops.bsengine_is_gamepad_button(btn),
    isGamepadButtonDown: (btn)  => Deno.core.ops.bsengine_is_gamepad_button_down(btn),
    isGamepadButtonUp:   (btn)  => Deno.core.ops.bsengine_is_gamepad_button_up(btn),
    getLeftStick:        ()     => { const v = Deno.core.ops.bsengine_get_left_stick(); return { x: v[0], y: v[1] }; },
    getRightStick:       ()     => { const v = Deno.core.ops.bsengine_get_right_stick(); return { x: v[0], y: v[1] }; },
    getGamepadTrigger:   (side) => Deno.core.ops.bsengine_get_gamepad_trigger(side),

    // Skybox
    setSkybox:           (path) => Deno.core.ops.bsengine_set_skybox(path),

    // Key event callbacks (event-based alternative to polling)
    _keyDownHandlers: {},
    _keyUpHandlers: {},
    onKeyDown(key, fn) { (this._keyDownHandlers[key] ??= []).push(fn); },
    onKeyUp(key, fn)   { (this._keyUpHandlers[key]   ??= []).push(fn); },
    _dispatchKeyEvents() {
        const keys = ['W','A','S','D','Space','Enter','Escape','Up','Down','Left','Right'];
        for (const key of keys) {
            if (Deno.core.ops.bsengine_is_key_down(key)) {
                for (const fn of (this._keyDownHandlers[key] || [])) {
                    try { fn(); } catch(e) { this.log('[keyDown:' + key + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_key_up(key)) {
                for (const fn of (this._keyUpHandlers[key] || [])) {
                    try { fn(); } catch(e) { this.log('[keyUp:' + key + '] ' + e); }
                }
            }
        }
    },

    // Timers — frame-based (1 frame ≈ 1 tick)
    _timers: [],
    _nextTimerId: 0,
    setTimeout(callback, frames) {
        const id = this._nextTimerId++;
        this._timers.push({ id, callback, remaining: frames });
        return id;
    },
    clearTimeout(id) {
        this._timers = this._timers.filter(t => t.id !== id);
    },
    _tickTimers() {
        const toFire = [];
        const keep = [];
        for (const t of this._timers) {
            t.remaining--;
            (t.remaining <= 0 ? toFire : keep).push(t);
        }
        this._timers = keep;
        for (const t of toFire) {
            try { t.callback(); } catch (e) { this.log('[timer] ' + e); }
        }
    },

    // Entity messaging — same-frame publish/subscribe event bus
    _listeners: {},
    sendMessage(target, event, data) {
        const key = target + '\x00' + event;
        const handlers = this._listeners[key];
        if (handlers) {
            for (const cb of handlers) {
                try { cb(data); } catch (e) { this.log('[msg] ' + e); }
            }
        }
    },
    onMessage(target, event, callback) {
        const key = target + '\x00' + event;
        if (!this._listeners[key]) this._listeners[key] = [];
        this._listeners[key].push(callback);
    },

    // Physics collision callbacks — keyed by entity name
    _collisionHandlers: {},
    onCollision(entityName, callback) {
        if (!this._collisionHandlers[entityName]) this._collisionHandlers[entityName] = [];
        this._collisionHandlers[entityName].push(callback);
    },
    _runCollisions(events) {
        for (const { nameA, nameB, started } of events) {
            for (const cb of (this._collisionHandlers[nameA] || [])) {
                try { cb(nameB, started); } catch (e) { this.log('[collision] ' + e); }
            }
            for (const cb of (this._collisionHandlers[nameB] || [])) {
                try { cb(nameA, started); } catch (e) { this.log('[collision] ' + e); }
            }
        }
    },

    // Per-entity script registry. Keys are entity bit-IDs (strings).
    _scripts: {},

    // Called each frame by the engine with [[id, name], ...] for all scripted entities.
    _runAll(entities) {
        this._tickTimers();
        this._dispatchKeyEvents();
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

    #[test]
    fn is_key_down_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyDown("Space") ? "down" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not down: {r}");
    }

    #[test]
    fn is_key_up_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isKeyUp("Space") ? "up" : "not""#)
            .unwrap();
        assert!(r.contains("not"), "expected not up: {r}");
    }

    #[test]
    fn is_mouse_pressed_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isMousePressed(0) ? "yes" : "no""#)
            .unwrap();
        assert!(r.contains("no"), "expected not pressed: {r}");
    }

    #[test]
    fn get_mouse_pos_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMousePos())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn get_mouse_delta_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMouseDelta())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn raycast_returns_null_without_physics_world() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.raycast({x:0,y:0,z:0}, {x:0,y:-1,z:0}, 100.0))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null when no physics: {r}"
        );
    }

    #[test]
    fn set_timeout_fires_after_frames() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let fired = false;
            Bsengine.setTimeout(() => { fired = true; }, 2);
            "#,
            "<test>",
        )
        .unwrap();
        rt.exec_source("Bsengine._runAll([]);", "<tick>").unwrap();
        let r = rt.eval("fired ? 'yes' : 'no'").unwrap();
        assert!(r.contains("no"), "should not fire on frame 1: {r}");
        rt.exec_source("Bsengine._runAll([]);", "<tick>").unwrap();
        let r = rt.eval("fired ? 'yes' : 'no'").unwrap();
        assert!(r.contains("yes"), "should fire on frame 2: {r}");
    }

    #[test]
    fn send_message_delivers_to_listener() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let received = null;
            Bsengine.onMessage("Enemy", "hit", (data) => { received = data; });
            Bsengine.sendMessage("Enemy", "hit", 42);
            "#,
            "<test>",
        )
        .unwrap();
        let r = rt.eval("String(received)").unwrap();
        assert!(r.contains("42"), "expected 42: {r}");
    }

    #[test]
    fn is_gamepad_button_returns_false_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.isGamepadButton(0) ? "yes" : "no""#)
            .unwrap();
        assert!(r.contains("no"), "expected not pressed: {r}");
    }

    #[test]
    fn get_left_stick_returns_zeros_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getLeftStick())"#)
            .unwrap();
        assert!(r.contains("0"), "expected zeros: {r}");
    }

    #[test]
    fn get_gamepad_trigger_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getGamepadTrigger(0))"#).unwrap();
        assert!(r.contains("0"), "expected 0: {r}");
    }

    #[test]
    fn on_collision_handler_dispatches() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.exec_source(
            r#"
            let hit = null;
            Bsengine.onCollision("Ball", (other, started) => { hit = other; });
            Bsengine._runCollisions([{nameA: "Ball", nameB: "Floor", started: true}]);
            "#,
            "<test>",
        )
        .unwrap();
        let r = rt.eval("hit").unwrap();
        assert!(r.contains("Floor"), "expected Floor: {r}");
    }

    #[test]
    fn on_key_down_registers_and_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Register handler; since no key snapshot, handler must NOT be called
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onKeyDown('Space', () => { called = true; });
            Bsengine._dispatchKeyEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_key_up_registers_and_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onKeyUp('Enter', () => { called = true; });
            Bsengine._dispatchKeyEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn spawn_params_rotation_defaults_to_identity() {
        use crate::ops::SpawnParams;
        let p: SpawnParams =
            serde_json::from_str(r#"{"name":"Cube1","primitive":"Cube","x":0,"y":0,"z":0}"#)
                .unwrap();
        assert_eq!(p.rx, 0.0);
        assert_eq!(p.ry, 0.0);
        assert_eq!(p.rz, 0.0);
        assert_eq!(p.rw, 1.0, "rw should default to 1 (identity quaternion)");
    }

    #[test]
    fn spawn_params_rotation_accepted() {
        use crate::ops::SpawnParams;
        let p: SpawnParams = serde_json::from_str(
            r#"{"name":"Tilted","primitive":"Cube","x":0,"y":0,"z":0,
               "rx":0.0,"ry":0.707,"rz":0.0,"rw":0.707}"#,
        )
        .unwrap();
        assert!((p.ry - 0.707).abs() < 1e-3);
        assert!((p.rw - 0.707).abs() < 1e-3);
    }
}
