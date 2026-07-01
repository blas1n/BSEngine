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
    SetVisible {
        name: String,
        visible: bool,
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
    SetParent {
        child: String,
        parent: String,
    },
    ClearParent {
        child: String,
    },
    SetCursorVisible {
        visible: bool,
    },
    SetCursorLocked {
        locked: bool,
    },
    AddImpulse {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
    },
    AddForce {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
    },
    SetVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetGravity {
        magnitude: f32,
    },
    SetAngularVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AddAngularImpulse {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetLinearDamping {
        name: String,
        damping: f32,
    },
    SetAngularDamping {
        name: String,
        damping: f32,
    },
    SetMass {
        name: String,
        mass: f32,
    },
    AddTag {
        name: String,
        label: String,
    },
    RemoveTag {
        name: String,
        label: String,
    },
    LockRotation {
        name: String,
        lock_x: bool,
        lock_y: bool,
        lock_z: bool,
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

    pub(crate) static VISIBLE_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    pub(crate) static TIME_ELAPSED_SNAPSHOT: RefCell<f32> = RefCell::new(0.0);
    pub(crate) static TIME_DELTA_SNAPSHOT: RefCell<f32> = RefCell::new(0.0);

    pub(crate) static SCREEN_SIZE_SNAPSHOT: RefCell<(u32, u32)> = RefCell::new((1280, 720));

    // name → linear velocity Vec3 (only for entities with a physics body)
    pub(crate) static VELOCITY_SNAPSHOT: RefCell<HashMap<String, Vec3>> =
        RefCell::new(HashMap::new());

    pub(crate) static GRAVITY_SNAPSHOT: RefCell<f32> = RefCell::new(9.81);

    // name → angular velocity Vec3 (only for entities with a physics body)
    pub(crate) static ANGULAR_VELOCITY_SNAPSHOT: RefCell<HashMap<String, Vec3>> =
        RefCell::new(HashMap::new());

    // name → mass (only for entities with a physics body)
    pub(crate) static MASS_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // child_name → parent_name (only for entities that have a Parent component)
    pub(crate) static PARENT_SNAPSHOT: RefCell<HashMap<String, String>> =
        RefCell::new(HashMap::new());
    // parent_name → [child_names]
    pub(crate) static CHILDREN_SNAPSHOT: RefCell<HashMap<String, Vec<String>>> =
        RefCell::new(HashMap::new());

    // tag label → [entity names]
    pub(crate) static TAG_SNAPSHOT: RefCell<HashMap<String, Vec<String>>> =
        RefCell::new(HashMap::new());

    // entity name → [tag labels]
    pub(crate) static ENTITY_TAGS_SNAPSHOT: RefCell<HashMap<String, Vec<String>>> =
        RefCell::new(HashMap::new());
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

#[op2]
#[string]
pub fn bsengine_get_entities_by_tag(#[string] tag: String) -> String {
    TAG_SNAPSHOT.with(|s| {
        let map = s.borrow();
        let names = map.get(&tag).cloned().unwrap_or_default();
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    })
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
pub fn bsengine_set_visible(#[string] name: String, visible: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVisible { name, visible });
    });
}

#[op2(fast)]
pub fn bsengine_get_visible(#[string] name: String) -> bool {
    VISIBLE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(true))
}

#[op2(fast)]
pub fn bsengine_look_at(#[string] name: String, tx: f32, ty: f32, tz: f32) {
    let origin = TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map(|(pos, _, _)| *pos));
    if let Some(pos) = origin {
        let dir = Vec3::new(tx - pos.x, ty - pos.y, tz - pos.z);
        if dir.length_squared() < 1e-10 {
            return;
        }
        let rot = Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize());
        COMMAND_BUFFER.with(|c| {
            c.borrow_mut().push(ScriptCommand::SetRotation {
                name,
                rx: rot.x,
                ry: rot.y,
                rz: rot.z,
                rw: rot.w,
            });
        });
    }
}

#[op2(fast)]
pub fn bsengine_get_time() -> f32 {
    TIME_ELAPSED_SNAPSHOT.with(|s| *s.borrow())
}

#[op2(fast)]
pub fn bsengine_get_delta_time() -> f32 {
    TIME_DELTA_SNAPSHOT.with(|s| *s.borrow())
}

#[op2]
#[serde]
pub fn bsengine_get_screen_size() -> Vec<u32> {
    SCREEN_SIZE_SNAPSHOT.with(|s| {
        let (w, h) = *s.borrow();
        vec![w, h]
    })
}

#[op2(fast)]
pub fn bsengine_set_parent(#[string] child: String, #[string] parent: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetParent { child, parent });
    });
}

#[op2(fast)]
pub fn bsengine_clear_parent(#[string] child: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::ClearParent { child });
    });
}

#[op2]
#[string]
pub fn bsengine_get_parent(#[string] name: String) -> String {
    PARENT_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .cloned()
            .map(|p| format!("\"{p}\""))
            .unwrap_or_else(|| "null".to_string())
    })
}

#[op2]
#[string]
pub fn bsengine_get_children(#[string] name: String) -> String {
    CHILDREN_SNAPSHOT.with(|s| {
        serde_json::to_string(s.borrow().get(&name).unwrap_or(&Vec::new()))
            .unwrap_or_else(|_| "[]".to_string())
    })
}

#[op2]
#[serde]
pub fn bsengine_get_velocity(#[string] name: String) -> Option<Vec<f32>> {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.x, v.y, v.z]))
}

#[op2(fast)]
pub fn bsengine_add_impulse(#[string] name: String, fx: f32, fy: f32, fz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddImpulse { name, fx, fy, fz });
    });
}

#[op2(fast)]
pub fn bsengine_add_force(#[string] name: String, fx: f32, fy: f32, fz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddForce { name, fx, fy, fz });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_get_gravity() -> f32 {
    GRAVITY_SNAPSHOT.with(|s| *s.borrow())
}

#[op2(fast)]
pub fn bsengine_set_gravity(magnitude: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetGravity { magnitude });
    });
}

#[op2]
#[serde]
pub fn bsengine_get_angular_velocity(#[string] name: String) -> Option<Vec<f32>> {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.x, v.y, v.z]))
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_angular_impulse(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddAngularImpulse { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_linear_damping(#[string] name: String, damping: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetLinearDamping { name, damping });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_damping(#[string] name: String, damping: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularDamping { name, damping });
    });
}

#[op2(fast)]
pub fn bsengine_get_mass(#[string] name: String) -> f32 {
    MASS_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_mass(#[string] name: String, mass: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetMass { name, mass });
    });
}

#[op2(fast)]
pub fn bsengine_has_tag(#[string] name: String, #[string] label: String) -> bool {
    ENTITY_TAGS_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|labels| labels.iter().any(|l| l == &label))
            .unwrap_or(false)
    })
}

#[op2(fast)]
pub fn bsengine_add_tag(#[string] name: String, #[string] label: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddTag { name, label });
    });
}

#[op2(fast)]
pub fn bsengine_remove_tag(#[string] name: String, #[string] label: String) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::RemoveTag { name, label });
    });
}

#[op2(fast)]
pub fn bsengine_lock_rotation(#[string] name: String, lock_x: bool, lock_y: bool, lock_z: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::LockRotation {
            name,
            lock_x,
            lock_y,
            lock_z,
        });
    });
}

#[op2(fast)]
pub fn bsengine_set_cursor_visible(visible: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCursorVisible { visible });
    });
}

#[op2(fast)]
pub fn bsengine_set_cursor_locked(locked: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCursorLocked { locked });
    });
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
        bsengine_get_entities_by_tag,
        bsengine_has_tag,
        bsengine_add_tag,
        bsengine_remove_tag,
        bsengine_set_emissive,
        bsengine_set_color,
        bsengine_spawn,
        bsengine_destroy,
        bsengine_set_visible,
        bsengine_get_visible,
        bsengine_look_at,
        bsengine_get_time,
        bsengine_get_delta_time,
        bsengine_get_screen_size,
        bsengine_set_parent,
        bsengine_clear_parent,
        bsengine_get_parent,
        bsengine_get_children,
        bsengine_get_velocity,
        bsengine_add_impulse,
        bsengine_add_force,
        bsengine_set_velocity,
        bsengine_get_gravity,
        bsengine_set_gravity,
        bsengine_get_angular_velocity,
        bsengine_set_angular_velocity,
        bsengine_add_angular_impulse,
        bsengine_set_linear_damping,
        bsengine_set_angular_damping,
        bsengine_get_mass,
        bsengine_set_mass,
        bsengine_lock_rotation,
        bsengine_set_cursor_visible,
        bsengine_set_cursor_locked,
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
    getEntityNames:      ()    => JSON.parse(Deno.core.ops.bsengine_get_entity_names()),
    getEntitiesByTag:    (tag) => JSON.parse(Deno.core.ops.bsengine_get_entities_by_tag(tag)),
    hasTag:              (name, label) => Deno.core.ops.bsengine_has_tag(name, label),
    addTag:              (name, label) => Deno.core.ops.bsengine_add_tag(name, label),
    removeTag:           (name, label) => Deno.core.ops.bsengine_remove_tag(name, label),
    setEmissive:    (name, r, g, b)        => Deno.core.ops.bsengine_set_emissive(name, r, g, b),
    setColor:       (name, r, g, b)        => Deno.core.ops.bsengine_set_color(name, r, g, b),
    spawn:          (params)               => Deno.core.ops.bsengine_spawn(params),
    destroy:        (name)                 => Deno.core.ops.bsengine_destroy(name),
    setVisible:     (name, v)              => Deno.core.ops.bsengine_set_visible(name, v),
    getVisible:     (name)                 => Deno.core.ops.bsengine_get_visible(name),
    lookAt:         (name, tx, ty, tz)     => Deno.core.ops.bsengine_look_at(name, tx, ty, tz),

    // Time
    getTime:        ()                     => Deno.core.ops.bsengine_get_time(),
    getDeltaTime:   ()                     => Deno.core.ops.bsengine_get_delta_time(),
    getScreenSize:  ()                     => { const [w, h] = Deno.core.ops.bsengine_get_screen_size(); return { width: w, height: h }; },
    setParent:      (child, parent)        => Deno.core.ops.bsengine_set_parent(child, parent),
    clearParent:      (child)   => Deno.core.ops.bsengine_clear_parent(child),
    getParent:        (name)    => { const r = Deno.core.ops.bsengine_get_parent(name); return JSON.parse(r); },
    getChildren:      (name)    => JSON.parse(Deno.core.ops.bsengine_get_children(name)),
    getVelocity:      (name)    => { const v = Deno.core.ops.bsengine_get_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    addImpulse:       (name, fx, fy, fz) => Deno.core.ops.bsengine_add_impulse(name, fx, fy, fz),
    addForce:         (name, fx, fy, fz) => Deno.core.ops.bsengine_add_force(name, fx, fy, fz),
    setVelocity:      (name, vx, vy, vz) => Deno.core.ops.bsengine_set_velocity(name, vx, vy, vz),
    getGravity:           ()                     => Deno.core.ops.bsengine_get_gravity(),
    setGravity:           (magnitude)             => Deno.core.ops.bsengine_set_gravity(magnitude),
    getAngularVelocity:   (name)                  => { const v = Deno.core.ops.bsengine_get_angular_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    setAngularVelocity:   (name, vx, vy, vz)      => Deno.core.ops.bsengine_set_angular_velocity(name, vx, vy, vz),
    addAngularImpulse:    (name, vx, vy, vz)      => Deno.core.ops.bsengine_add_angular_impulse(name, vx, vy, vz),
    setLinearDamping:     (name, damping)          => Deno.core.ops.bsengine_set_linear_damping(name, damping),
    setAngularDamping:    (name, damping)          => Deno.core.ops.bsengine_set_angular_damping(name, damping),
    getMass:              (name)                   => Deno.core.ops.bsengine_get_mass(name),
    setMass:              (name, mass)             => Deno.core.ops.bsengine_set_mass(name, mass),
    lockRotation:         (name, lockX, lockY, lockZ) => Deno.core.ops.bsengine_lock_rotation(name, lockX, lockY, lockZ),
    setCursorVisible: (visible) => Deno.core.ops.bsengine_set_cursor_visible(visible),
    setCursorLocked:  (locked)  => Deno.core.ops.bsengine_set_cursor_locked(locked),
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

    // Mouse event callbacks (btn: 0=Left, 1=Right, 2=Middle)
    _mouseDownHandlers: {},
    _mouseUpHandlers: {},
    onMouseDown(btn, fn) { (this._mouseDownHandlers[btn] ??= []).push(fn); },
    onMouseUp(btn, fn)   { (this._mouseUpHandlers[btn]   ??= []).push(fn); },
    _dispatchMouseEvents() {
        for (let btn = 0; btn < 3; btn++) {
            if (Deno.core.ops.bsengine_is_mouse_down(btn)) {
                for (const fn of (this._mouseDownHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[mouseDown:' + btn + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_mouse_up(btn)) {
                for (const fn of (this._mouseUpHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[mouseUp:' + btn + '] ' + e); }
                }
            }
        }
    },

    // Gamepad event callbacks (btn: 0=South/A..15=DPadRight)
    _gamepadDownHandlers: {},
    _gamepadUpHandlers: {},
    onGamepadButtonDown(btn, fn) { (this._gamepadDownHandlers[btn] ??= []).push(fn); },
    onGamepadButtonUp(btn, fn)   { (this._gamepadUpHandlers[btn]   ??= []).push(fn); },
    _dispatchGamepadEvents() {
        for (let btn = 0; btn < 16; btn++) {
            if (Deno.core.ops.bsengine_is_gamepad_button_down(btn)) {
                for (const fn of (this._gamepadDownHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[gamepadDown:' + btn + '] ' + e); }
                }
            }
            if (Deno.core.ops.bsengine_is_gamepad_button_up(btn)) {
                for (const fn of (this._gamepadUpHandlers[btn] || [])) {
                    try { fn(btn); } catch (e) { this.log('[gamepadUp:' + btn + '] ' + e); }
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

    // --- Messaging ---
    _messageHandlers: {},

    // Register a handler for messages of `key` addressed to `entityName`.
    onMessage(entityName, key, fn) {
        const k = `${entityName}::${key}`;
        (this._messageHandlers[k] ??= []).push(fn);
    },

    // Dispatch a message synchronously to all handlers registered for `target`+`key`.
    sendMessage(target, key, data) {
        const handlers = this._messageHandlers[`${target}::${key}`] || [];
        for (const fn of handlers) {
            try { fn(data); } catch (e) { this.log(`[msg:${target}:${key}] ${e}`); }
        }
    },

    // Dispatch `key` to every entity that has a handler registered for it.
    broadcast(key, data) {
        const suffix = `::${key}`;
        for (const k of Object.keys(this._messageHandlers)) {
            if (k.endsWith(suffix)) {
                for (const fn of this._messageHandlers[k]) {
                    try { fn(data); } catch (e) { this.log(`[broadcast:${key}] ${e}`); }
                }
            }
        }
    },

    // Called each frame by the engine with [[id, name], ...] for all scripted entities.
    _runAll(entities) {
        this._tickTimers();
        this._dispatchKeyEvents();
        this._dispatchMouseEvents();
        this._dispatchGamepadEvents();
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
    fn set_parent_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setParent("Child", "Root");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetParent { child, parent }
                    if child == "Child" && parent == "Root")
            });
            assert!(found, "SetParent not in buffer");
        });
    }

    #[test]
    fn clear_parent_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.clearParent("Child");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::ClearParent { child } if child == "Child"));
            assert!(found, "ClearParent not in buffer");
        });
    }

    #[test]
    fn broadcast_fires_all_registered_handlers() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(
                r#"
let hits = 0;
Bsengine.onMessage("A", "boom", () => { hits++; });
Bsengine.onMessage("B", "boom", () => { hits++; });
Bsengine.onMessage("A", "other", () => { hits += 100; });
Bsengine.broadcast("boom", {});
hits
"#,
            )
            .unwrap();
        assert_eq!(result.trim(), "2", "expected 2 hits: {result}");
    }

    #[test]
    fn broadcast_no_op_when_no_handlers() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.broadcast("nobody", {}); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "threw: {r}");
    }

    #[test]
    fn get_screen_size_returns_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval("JSON.stringify(Bsengine.getScreenSize())").unwrap();
        assert!(r.contains("\"width\":1280"), "unexpected: {r}");
        assert!(r.contains("\"height\":720"), "unexpected: {r}");
    }

    #[test]
    fn messaging_delivers_to_handler() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let result = rt
            .eval(
                r#"
let received = null;
Bsengine.onMessage("Box", "hit", (data) => { received = data; });
Bsengine.sendMessage("Box", "hit", { damage: 5 });
JSON.stringify(received)
"#,
            )
            .unwrap();
        assert!(
            result.contains("\"damage\":5"),
            "message not delivered: {result}"
        );
    }

    #[test]
    fn messaging_no_op_for_unknown_recipient() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.sendMessage("NoOne", "event", {}); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "threw: {r}");
    }

    #[test]
    fn look_at_no_op_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // No transform snapshot → should not crash
        let r = rt
            .eval(r#"Bsengine.lookAt("NoEntity", 1, 0, 0); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "lookAt threw: {r}");
    }

    #[test]
    fn look_at_no_op_when_zero_dir() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Seed snapshot: entity at (1, 0, 0), target also (1, 0, 0) → zero dir
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Turret".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"Bsengine.lookAt("Turret", 1.0, 0.0, 0.0); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "lookAt zero-dir threw: {r}");
    }

    #[test]
    fn get_time_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // No time snapshot set → returns 0.0
        let r = rt
            .eval(r#"Bsengine.getTime() === 0.0 ? "zero" : "nonzero""#)
            .unwrap();
        assert!(r.contains("zero"), "expected 0.0: {r}");
    }

    #[test]
    fn get_delta_time_returns_zero_initially() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getDeltaTime() === 0.0 ? "zero" : "nonzero""#)
            .unwrap();
        assert!(r.contains("zero"), "expected 0.0: {r}");
    }

    #[test]
    fn get_visible_returns_true_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Unknown entity defaults to visible=true
        let r = rt
            .eval(r#"Bsengine.getVisible("NonExistent") ? "visible" : "hidden""#)
            .unwrap();
        assert!(r.contains("visible"), "expected visible by default: {r}");
    }

    #[test]
    fn set_visible_queues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        // Should not throw
        let r = rt
            .eval(r#"Bsengine.setVisible("Cube", false); "ok""#)
            .unwrap();
        assert!(r.contains("ok"), "setVisible threw: {r}");
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
    fn get_parent_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getParent("Child"))"#).unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_parent_returns_name_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::PARENT_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Child".to_string(), "Root".to_string());
        });
        let r = rt.eval(r#"Bsengine.getParent("Child")"#).unwrap();
        assert!(r.contains("Root"), "expected Root: {r}");
    }

    #[test]
    fn get_children_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getChildren("Root"))"#)
            .unwrap();
        assert!(r.contains("[]"), "expected empty array: {r}");
    }

    #[test]
    fn get_children_returns_list_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Root".to_string(),
                vec!["ChildA".to_string(), "ChildB".to_string()],
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getChildren("Root"))"#)
            .unwrap();
        assert!(r.contains("ChildA"), "expected ChildA: {r}");
        assert!(r.contains("ChildB"), "expected ChildB: {r}");
    }

    #[test]
    fn get_entities_by_tag_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesByTag("enemy"))"#)
            .unwrap();
        assert!(r.contains("[]"), "expected empty array: {r}");
    }

    #[test]
    fn get_entities_by_tag_returns_list_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TAG_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "enemy".to_string(),
                vec!["Goblin".to_string(), "Orc".to_string()],
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesByTag("enemy"))"#)
            .unwrap();
        super::TAG_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("Goblin"), "expected Goblin: {r}");
        assert!(r.contains("Orc"), "expected Orc: {r}");
    }

    #[test]
    fn has_tag_returns_false_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.hasTag("Player", "enemy")"#).unwrap();
        assert!(r.contains("false"), "expected false: {r}");
    }

    #[test]
    fn has_tag_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ENTITY_TAGS_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Player".to_string(), vec!["hero".to_string()]);
        });
        let r = rt.eval(r#"Bsengine.hasTag("Player", "hero")"#).unwrap();
        super::ENTITY_TAGS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("true"), "expected true: {r}");
    }

    #[test]
    fn add_tag_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addTag("Enemy", "stunned");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddTag { name, label }
                    if name == "Enemy" && label == "stunned")
            });
            assert!(found, "AddTag not in buffer");
        });
    }

    #[test]
    fn remove_tag_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.removeTag("Enemy", "stunned");"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::RemoveTag { name, label }
                    if name == "Enemy" && label == "stunned")
            });
            assert!(found, "RemoveTag not in buffer");
        });
    }

    #[test]
    fn get_velocity_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.getVelocity("Ball"))"#).unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_velocity_returns_vec_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getVelocity("Ball"))"#)
            .unwrap();
        assert!(r.contains("\"x\":1"), "expected x=1: {r}");
        assert!(r.contains("\"y\":2"), "expected y=2: {r}");
    }

    #[test]
    fn add_impulse_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addImpulse("Ball", 0, 10, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddImpulse { name, fy, .. }
                    if name == "Ball" && (*fy - 10.0).abs() < 1e-6)
            });
            assert!(found, "AddImpulse not in buffer");
        });
    }

    #[test]
    fn set_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocity("Ball", 5, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocity { name, vx, .. }
                    if name == "Ball" && (*vx - 5.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocity not in buffer");
        });
    }

    #[test]
    fn set_cursor_visible_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCursorVisible(false);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::SetCursorVisible { visible } if !visible),
            );
            assert!(found, "SetCursorVisible not in buffer");
        });
    }

    #[test]
    fn set_cursor_locked_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCursorLocked(true);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::SetCursorLocked { locked } if *locked),
            );
            assert!(found, "SetCursorLocked not in buffer");
        });
    }

    #[test]
    fn get_gravity_returns_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getGravity()"#).unwrap();
        assert!(
            r.contains("9.81") || r.contains("9.8"),
            "expected ~9.81: {r}"
        );
    }

    #[test]
    fn get_gravity_returns_snapshot_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = 20.0);
        let r = rt.eval(r#"Bsengine.getGravity()"#).unwrap();
        super::GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = 9.81);
        assert!(r.contains("20"), "expected 20: {r}");
    }

    #[test]
    fn set_gravity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setGravity(0.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetGravity { magnitude }
                    if (*magnitude).abs() < 1e-6)
            });
            assert!(found, "SetGravity not in buffer");
        });
    }

    #[test]
    fn get_angular_velocity_returns_null_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getAngularVelocity("Spinner"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_angular_velocity_returns_vec_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Spinner".to_string(), glam::Vec3::new(0.0, 2.5, 0.0));
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getAngularVelocity("Spinner"))"#)
            .unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("\"y\":2.5"), "expected y=2.5: {r}");
    }

    #[test]
    fn set_angular_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocity("Top", 0, 5, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocity { name, vy, .. }
                    if name == "Top" && (*vy - 5.0).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocity not in buffer");
        });
    }

    #[test]
    fn add_angular_impulse_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addAngularImpulse("Top", 0, 2, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddAngularImpulse { name, vy, .. }
                    if name == "Top" && (*vy - 2.0).abs() < 1e-6)
            });
            assert!(found, "AddAngularImpulse not in buffer");
        });
    }

    #[test]
    fn set_linear_damping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setLinearDamping("Ball", 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetLinearDamping { name, damping }
                    if name == "Ball" && (*damping - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetLinearDamping not in buffer");
        });
    }

    #[test]
    fn set_angular_damping_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularDamping("Ball", 0.8);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularDamping { name, damping }
                    if name == "Ball" && (*damping - 0.8).abs() < 1e-6)
            });
            assert!(found, "SetAngularDamping not in buffer");
        });
    }

    #[test]
    fn get_mass_returns_zero_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getMass("Rock")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_mass_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MASS_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Rock".to_string(), 5.0);
        });
        let r = rt.eval(r#"Bsengine.getMass("Rock")"#).unwrap();
        super::MASS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains('5'), "expected 5: {r}");
    }

    #[test]
    fn set_mass_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setMass("Rock", 10.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetMass { name, mass }
                    if name == "Rock" && (*mass - 10.0).abs() < 1e-6)
            });
            assert!(found, "SetMass not in buffer");
        });
    }

    #[test]
    fn lock_rotation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.lockRotation("Player", true, false, true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::LockRotation { name, lock_x, lock_y, lock_z }
                    if name == "Player" && *lock_x && !*lock_y && *lock_z)
            });
            assert!(found, "LockRotation not in buffer");
        });
    }

    #[test]
    fn on_mouse_down_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseDown(0, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_mouse_down_called_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0b001); // btn 0 = Left
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseDown(0, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        super::MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0);
        assert!(r.contains("called"), "expected called: {r}");
    }

    #[test]
    fn on_mouse_up_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onMouseUp(1, () => { called = true; });
            Bsengine._dispatchMouseEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_gamepad_button_down_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonDown(0, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        assert!(r.contains("not_called"), "expected not_called: {r}");
    }

    #[test]
    fn on_gamepad_button_down_called_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0b0001); // btn 0
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonDown(0, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
            called ? "called" : "not_called"
        "#,
            )
            .unwrap();
        super::GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = 0);
        assert!(r.contains("called"), "expected called: {r}");
    }

    #[test]
    fn on_gamepad_button_up_not_called_when_no_input() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(
                r#"
            let called = false;
            Bsengine.onGamepadButtonUp(3, () => { called = true; });
            Bsengine._dispatchGamepadEvents();
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
