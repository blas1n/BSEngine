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
    AddPosition {
        name: String,
        dx: f32,
        dy: f32,
        dz: f32,
    },
    AddPositionLocal {
        name: String,
        dx: f32,
        dy: f32,
        dz: f32,
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
    SetMetallic {
        name: String,
        value: f32,
    },
    SetRoughness {
        name: String,
        value: f32,
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
    AddImpulseAtPoint {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
        px: f32,
        py: f32,
        pz: f32,
    },
    AddForce {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
    },
    AddForceAtPoint {
        name: String,
        fx: f32,
        fy: f32,
        fz: f32,
        px: f32,
        py: f32,
        pz: f32,
    },
    SetVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetVelocityX {
        name: String,
        vx: f32,
    },
    SetVelocityY {
        name: String,
        vy: f32,
    },
    SetVelocityZ {
        name: String,
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
    SetAngularVelocityX {
        name: String,
        vx: f32,
    },
    SetAngularVelocityY {
        name: String,
        vy: f32,
    },
    SetAngularVelocityZ {
        name: String,
        vz: f32,
    },
    AddVelocity {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    AddAngularVelocity {
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
    AddTorque {
        name: String,
        vx: f32,
        vy: f32,
        vz: f32,
    },
    SetCCDEnabled {
        name: String,
        enabled: bool,
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
    SetKinematic {
        name: String,
        kinematic: bool,
    },
    SetGravityScale {
        name: String,
        scale: f32,
    },
    SetColliderSensor {
        name: String,
        sensor: bool,
    },
    SetRestitution {
        name: String,
        restitution: f32,
    },
    SetFriction {
        name: String,
        friction: f32,
    },
    LockRotation {
        name: String,
        lock_x: bool,
        lock_y: bool,
        lock_z: bool,
    },
    LockTranslation {
        name: String,
        lock_x: bool,
        lock_y: bool,
        lock_z: bool,
    },
    WakeUp {
        name: String,
    },
    PutToSleep {
        name: String,
    },
    SetPositionX {
        name: String,
        x: f32,
    },
    SetPositionY {
        name: String,
        y: f32,
    },
    SetPositionZ {
        name: String,
        z: f32,
    },
    AddPositionX {
        name: String,
        dx: f32,
    },
    AddPositionY {
        name: String,
        dy: f32,
    },
    AddPositionZ {
        name: String,
        dz: f32,
    },
    RotateBy {
        name: String,
        rx: f32,
        ry: f32,
        rz: f32,
        rw: f32,
    },
    RotateAroundAxis {
        name: String,
        ax: f32,
        ay: f32,
        az: f32,
        angle_deg: f32,
    },
    AddRotationEuler {
        name: String,
        pitch: f32,
        yaw: f32,
        roll: f32,
    },
    AddRotationEulerX {
        name: String,
        deg: f32,
    },
    AddRotationEulerY {
        name: String,
        deg: f32,
    },
    AddRotationEulerZ {
        name: String,
        deg: f32,
    },
    SetScaleX {
        name: String,
        x: f32,
    },
    SetScaleY {
        name: String,
        y: f32,
    },
    SetScaleZ {
        name: String,
        z: f32,
    },
    AddScaleX {
        name: String,
        dx: f32,
    },
    AddScaleY {
        name: String,
        dy: f32,
    },
    AddScaleZ {
        name: String,
        dz: f32,
    },
    SetRotationEuler {
        name: String,
        pitch_deg: f32,
        yaw_deg: f32,
        roll_deg: f32,
    },
    AddScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    SetRotationEulerX {
        name: String,
        deg: f32,
    },
    SetRotationEulerY {
        name: String,
        deg: f32,
    },
    SetRotationEulerZ {
        name: String,
        deg: f32,
    },
    MultiplyScale {
        name: String,
        sx: f32,
        sy: f32,
        sz: f32,
    },
}

thread_local! {
    pub(crate) static TRANSFORM_SNAPSHOT: RefCell<HashMap<String, (Vec3, Quat, Vec3)>> =
        RefCell::new(HashMap::new());
    pub(crate) static WORLD_TRANSFORM_SNAPSHOT: RefCell<HashMap<String, (Vec3, Quat, Vec3)>> =
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

    // name → base_color [r, g, b] (only for entities with a Material component)
    pub(crate) static MATERIAL_COLOR_SNAPSHOT: RefCell<HashMap<String, [f32; 3]>> =
        RefCell::new(HashMap::new());

    // name → emissive [r, g, b] (only for entities with a Material component)
    pub(crate) static MATERIAL_EMISSIVE_SNAPSHOT: RefCell<HashMap<String, [f32; 3]>> =
        RefCell::new(HashMap::new());

    // name → metallic (only for entities with a Material component)
    pub(crate) static MATERIAL_METALLIC_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → roughness (only for entities with a Material component)
    pub(crate) static MATERIAL_ROUGHNESS_SNAPSHOT: RefCell<HashMap<String, f32>> =
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

    // name → gravity scale (only for entities with a physics body)
    pub(crate) static GRAVITY_SCALE_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → is_kinematic (only for entities with a physics body)
    pub(crate) static BODY_TYPE_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → is_sensor (only for entities with at least one collider)
    pub(crate) static COLLIDER_SENSOR_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → is_sleeping (only for entities with a physics body)
    pub(crate) static SLEEP_SNAPSHOT: RefCell<HashMap<String, bool>> =
        RefCell::new(HashMap::new());

    // name → linear damping (only for entities with a physics body)
    pub(crate) static LINEAR_DAMPING_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → angular damping (only for entities with a physics body)
    pub(crate) static ANGULAR_DAMPING_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → restitution (only for entities with at least one collider)
    pub(crate) static RESTITUTION_SNAPSHOT: RefCell<HashMap<String, f32>> =
        RefCell::new(HashMap::new());

    // name → friction (only for entities with at least one collider)
    pub(crate) static FRICTION_SNAPSHOT: RefCell<HashMap<String, f32>> =
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

#[op2]
#[serde]
pub fn bsengine_get_forward_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::NEG_Z);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_right_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::X);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_up_vector(#[string] name: String) -> Option<Vec<f32>> {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map(|(_, rot, _)| {
            let v = rot.mul_vec3(Vec3::Y);
            vec![v.x, v.y, v.z]
        })
    })
}

#[op2]
#[serde]
pub fn bsengine_get_world_transform(#[string] name: String) -> Option<TransformJson> {
    WORLD_TRANSFORM_SNAPSHOT.with(|s| {
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
pub fn bsengine_distance_to(#[string] name_a: String, #[string] name_b: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        let pos_a = snap.get(&name_a).map(|(p, _, _)| *p);
        let pos_b = snap.get(&name_b).map(|(p, _, _)| *p);
        match (pos_a, pos_b) {
            (Some(a), Some(b)) => a.distance(b),
            _ => -1.0,
        }
    })
}

#[op2(fast)]
pub fn bsengine_distance_to_point(#[string] name: String, x: f32, y: f32, z: f32) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow()
            .get(&name)
            .map(|(pos, _, _)| pos.distance(Vec3::new(x, y, z)))
            .unwrap_or(-1.0)
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
pub fn bsengine_set_rotation_euler(
    #[string] name: String,
    pitch_deg: f32,
    yaw_deg: f32,
    roll_deg: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetRotationEuler {
            name,
            pitch_deg,
            yaw_deg,
            roll_deg,
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
pub fn bsengine_add_position(#[string] name: String, dx: f32, dy: f32, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPosition { name, dx, dy, dz });
    });
}

#[op2(fast)]
pub fn bsengine_rotate_by(#[string] name: String, rx: f32, ry: f32, rz: f32, rw: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::RotateBy {
            name,
            rx,
            ry,
            rz,
            rw,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_local(#[string] name: String, dx: f32, dy: f32, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionLocal { name, dx, dy, dz });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_x(#[string] name: String, x: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionX { name, x });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_y(#[string] name: String, y: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionY { name, y });
    });
}

#[op2(fast)]
pub fn bsengine_set_position_z(#[string] name: String, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetPositionZ { name, z });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_x(#[string] name: String, dx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionX { name, dx });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_y(#[string] name: String, dy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionY { name, dy });
    });
}

#[op2(fast)]
pub fn bsengine_add_position_z(#[string] name: String, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddPositionZ { name, dz });
    });
}

#[op2(fast)]
pub fn bsengine_rotate_around_axis(
    #[string] name: String,
    ax: f32,
    ay: f32,
    az: f32,
    angle_deg: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::RotateAroundAxis {
            name,
            ax,
            ay,
            az,
            angle_deg,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler(#[string] name: String, pitch: f32, yaw: f32, roll: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddRotationEuler {
            name,
            pitch,
            yaw,
            roll,
        });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_x(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerX { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_y(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerY { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_add_rotation_euler_z(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddRotationEulerZ { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_x(#[string] name: String, x: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleX { name, x });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_y(#[string] name: String, y: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleY { name, y });
    });
}

#[op2(fast)]
pub fn bsengine_set_scale_z(#[string] name: String, z: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::SetScaleZ { name, z });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_x(#[string] name: String, dx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleX { name, dx });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_y(#[string] name: String, dy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleY { name, dy });
    });
}

#[op2(fast)]
pub fn bsengine_add_scale_z(#[string] name: String, dz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddScaleZ { name, dz });
    });
}

#[op2(fast)]
pub fn bsengine_get_position_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.x))
}

#[op2(fast)]
pub fn bsengine_get_position_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.y))
}

#[op2(fast)]
pub fn bsengine_get_position_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.0.z))
}

#[op2(fast)]
pub fn bsengine_get_scale_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.x))
}

#[op2(fast)]
pub fn bsengine_get_scale_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.y))
}

#[op2(fast)]
pub fn bsengine_get_scale_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |t| t.2.z))
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_x(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (x, _, _) = t.1.to_euler(glam::EulerRot::XYZ);
            x.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_y(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (_, y, _) = t.1.to_euler(glam::EulerRot::XYZ);
            y.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_get_rotation_euler_z(#[string] name: String) -> f32 {
    TRANSFORM_SNAPSHOT.with(|s| {
        s.borrow().get(&name).map_or(f32::NAN, |t| {
            let (_, _, z) = t.1.to_euler(glam::EulerRot::XYZ);
            z.to_degrees()
        })
    })
}

#[op2(fast)]
pub fn bsengine_add_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddScale { name, sx, sy, sz });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_x(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerX { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_y(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerY { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_set_rotation_euler_z(#[string] name: String, deg: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRotationEulerZ { name, deg });
    });
}

#[op2(fast)]
pub fn bsengine_multiply_scale(#[string] name: String, sx: f32, sy: f32, sz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::MultiplyScale { name, sx, sy, sz });
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

#[op2]
#[string]
pub fn bsengine_get_entities_in_radius(x: f32, y: f32, z: f32, radius: f32) -> String {
    let center = Vec3::new(x, y, z);
    let r2 = radius * radius;
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        let names: Vec<&str> = snap
            .iter()
            .filter(|(_, (pos, _, _))| pos.distance_squared(center) <= r2)
            .map(|(name, _)| name.as_str())
            .collect();
        serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string())
    })
}

#[op2]
#[string]
pub fn bsengine_get_closest_entity(x: f32, y: f32, z: f32) -> String {
    let center = Vec3::new(x, y, z);
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        snap.iter()
            .min_by(|(_, (pa, _, _)), (_, (pb, _, _))| {
                pa.distance_squared(center)
                    .partial_cmp(&pb.distance_squared(center))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
    })
}

#[op2]
#[string]
pub fn bsengine_get_closest_entity_with_tag(
    x: f32,
    y: f32,
    z: f32,
    #[string] tag: String,
) -> String {
    let center = Vec3::new(x, y, z);
    let candidates: Vec<String> =
        TAG_SNAPSHOT.with(|s| s.borrow().get(&tag).cloned().unwrap_or_default());
    TRANSFORM_SNAPSHOT.with(|s| {
        let snap = s.borrow();
        candidates
            .iter()
            .filter_map(|name| snap.get(name).map(|(pos, _, _)| (name, *pos)))
            .min_by(|(_, pa), (_, pb)| {
                pa.distance_squared(center)
                    .partial_cmp(&pb.distance_squared(center))
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(name, _)| name.clone())
            .unwrap_or_default()
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

#[op2]
#[serde]
pub fn bsengine_get_material_color(#[string] name: String) -> Option<Vec<f32>> {
    MATERIAL_COLOR_SNAPSHOT.with(|s| s.borrow().get(&name).map(|c| c.to_vec()))
}

#[op2]
#[serde]
pub fn bsengine_get_material_emissive(#[string] name: String) -> Option<Vec<f32>> {
    MATERIAL_EMISSIVE_SNAPSHOT.with(|s| s.borrow().get(&name).map(|c| c.to_vec()))
}

#[op2(fast)]
pub fn bsengine_set_metallic(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetMetallic { name, value });
    });
}

#[op2(fast)]
pub fn bsengine_get_metallic(#[string] name: String) -> f32 {
    MATERIAL_METALLIC_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(f32::NAN))
}

#[op2(fast)]
pub fn bsengine_set_roughness(#[string] name: String, value: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRoughness { name, value });
    });
}

#[op2(fast)]
pub fn bsengine_get_roughness(#[string] name: String) -> f32 {
    MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(f32::NAN))
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

#[op2]
#[serde]
pub fn bsengine_get_linear_speed(#[string] name: String) -> Option<Vec<f32>> {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map(|v| vec![v.length()]))
}

#[op2(fast)]
pub fn bsengine_get_velocity_x(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.x))
}

#[op2(fast)]
pub fn bsengine_get_velocity_y(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.y))
}

#[op2(fast)]
pub fn bsengine_get_velocity_z(#[string] name: String) -> f32 {
    VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.z))
}

#[op2(fast)]
pub fn bsengine_add_impulse(#[string] name: String, fx: f32, fy: f32, fz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddImpulse { name, fx, fy, fz });
    });
}

#[op2(fast)]
pub fn bsengine_apply_impulse_at_point(
    #[string] name: String,
    fx: f32,
    fy: f32,
    fz: f32,
    px: f32,
    py: f32,
    pz: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddImpulseAtPoint {
            name,
            fx,
            fy,
            fz,
            px,
            py,
            pz,
        });
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
pub fn bsengine_add_force_at_point(
    #[string] name: String,
    fx: f32,
    fy: f32,
    fz: f32,
    px: f32,
    py: f32,
    pz: f32,
) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::AddForceAtPoint {
            name,
            fx,
            fy,
            fz,
            px,
            py,
            pz,
        });
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
pub fn bsengine_set_velocity_x(#[string] name: String, vx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityX { name, vx });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity_y(#[string] name: String, vy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityY { name, vy });
    });
}

#[op2(fast)]
pub fn bsengine_set_velocity_z(#[string] name: String, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetVelocityZ { name, vz });
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
pub fn bsengine_get_angular_velocity_x(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.x))
}

#[op2(fast)]
pub fn bsengine_get_angular_velocity_y(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.y))
}

#[op2(fast)]
pub fn bsengine_get_angular_velocity_z(#[string] name: String) -> f32 {
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow().get(&name).map_or(f32::NAN, |v| v.z))
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_x(#[string] name: String, vx: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityX { name, vx });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_y(#[string] name: String, vy: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityY { name, vy });
    });
}

#[op2(fast)]
pub fn bsengine_set_angular_velocity_z(#[string] name: String, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetAngularVelocityZ { name, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddVelocity { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_add_angular_velocity(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddAngularVelocity { name, vx, vy, vz });
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
pub fn bsengine_add_torque(#[string] name: String, vx: f32, vy: f32, vz: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::AddTorque { name, vx, vy, vz });
    });
}

#[op2(fast)]
pub fn bsengine_set_ccd_enabled(#[string] name: String, enabled: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetCCDEnabled { name, enabled });
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
pub fn bsengine_get_gravity_scale(#[string] name: String) -> f32 {
    GRAVITY_SCALE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(1.0))
}

#[op2(fast)]
pub fn bsengine_is_kinematic(#[string] name: String) -> bool {
    BODY_TYPE_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_is_sleeping(#[string] name: String) -> bool {
    SLEEP_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_wake_up(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::WakeUp { name }));
}

#[op2(fast)]
pub fn bsengine_sleep(#[string] name: String) {
    COMMAND_BUFFER.with(|c| c.borrow_mut().push(ScriptCommand::PutToSleep { name }));
}

#[op2(fast)]
pub fn bsengine_is_collider_sensor(#[string] name: String) -> bool {
    COLLIDER_SENSOR_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(false))
}

#[op2(fast)]
pub fn bsengine_get_linear_damping(#[string] name: String) -> f32 {
    LINEAR_DAMPING_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_angular_damping(#[string] name: String) -> f32 {
    ANGULAR_DAMPING_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_get_restitution(#[string] name: String) -> f32 {
    RESTITUTION_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_restitution(#[string] name: String, restitution: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetRestitution { name, restitution });
    });
}

#[op2(fast)]
pub fn bsengine_get_friction(#[string] name: String) -> f32 {
    FRICTION_SNAPSHOT.with(|s| s.borrow().get(&name).copied().unwrap_or(0.0))
}

#[op2(fast)]
pub fn bsengine_set_friction(#[string] name: String, friction: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetFriction { name, friction });
    });
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
pub fn bsengine_set_kinematic(#[string] name: String, kinematic: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetKinematic { name, kinematic });
    });
}

#[op2]
#[string]
pub fn bsengine_get_tags(#[string] name: String) -> String {
    ENTITY_TAGS_SNAPSHOT.with(|s| {
        let map = s.borrow();
        let labels = map.get(&name).cloned().unwrap_or_default();
        serde_json::to_string(&labels).unwrap_or_else(|_| "[]".to_string())
    })
}

#[op2(fast)]
pub fn bsengine_set_gravity_scale(#[string] name: String, scale: f32) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetGravityScale { name, scale });
    });
}

#[op2(fast)]
pub fn bsengine_set_collider_sensor(#[string] name: String, sensor: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut()
            .push(ScriptCommand::SetColliderSensor { name, sensor });
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
pub fn bsengine_lock_translation(#[string] name: String, lock_x: bool, lock_y: bool, lock_z: bool) {
    COMMAND_BUFFER.with(|c| {
        c.borrow_mut().push(ScriptCommand::LockTranslation {
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
        bsengine_get_forward_vector,
        bsengine_get_right_vector,
        bsengine_get_up_vector,
        bsengine_distance_to,
        bsengine_distance_to_point,
        bsengine_get_world_transform,
        bsengine_set_transform,
        bsengine_set_rotation,
        bsengine_set_rotation_euler,
        bsengine_set_scale,
        bsengine_add_position,
        bsengine_add_position_local,
        bsengine_set_position_x,
        bsengine_set_position_y,
        bsengine_set_position_z,
        bsengine_add_position_x,
        bsengine_add_position_y,
        bsengine_add_position_z,
        bsengine_rotate_by,
        bsengine_rotate_around_axis,
        bsengine_add_rotation_euler,
        bsengine_add_rotation_euler_x,
        bsengine_add_rotation_euler_y,
        bsengine_add_rotation_euler_z,
        bsengine_set_scale_x,
        bsengine_set_scale_y,
        bsengine_set_scale_z,
        bsengine_add_scale_x,
        bsengine_add_scale_y,
        bsengine_add_scale_z,
        bsengine_get_position_x,
        bsengine_get_position_y,
        bsengine_get_position_z,
        bsengine_get_scale_x,
        bsengine_get_scale_y,
        bsengine_get_scale_z,
        bsengine_get_rotation_euler_x,
        bsengine_get_rotation_euler_y,
        bsengine_get_rotation_euler_z,
        bsengine_add_scale,
        bsengine_set_rotation_euler_x,
        bsengine_set_rotation_euler_y,
        bsengine_set_rotation_euler_z,
        bsengine_multiply_scale,
        bsengine_is_key_pressed,
        bsengine_is_key_down,
        bsengine_is_key_up,
        bsengine_get_entity_names,
        bsengine_get_entities_by_tag,
        bsengine_get_entities_in_radius,
        bsengine_get_closest_entity,
        bsengine_get_closest_entity_with_tag,
        bsengine_has_tag,
        bsengine_add_tag,
        bsengine_remove_tag,
        bsengine_set_kinematic,
        bsengine_get_tags,
        bsengine_set_gravity_scale,
        bsengine_set_collider_sensor,
        bsengine_set_emissive,
        bsengine_set_color,
        bsengine_spawn,
        bsengine_destroy,
        bsengine_set_visible,
        bsengine_get_visible,
        bsengine_get_material_color,
        bsengine_get_material_emissive,
        bsengine_set_metallic,
        bsengine_get_metallic,
        bsengine_set_roughness,
        bsengine_get_roughness,
        bsengine_look_at,
        bsengine_get_time,
        bsengine_get_delta_time,
        bsengine_get_screen_size,
        bsengine_set_parent,
        bsengine_clear_parent,
        bsengine_get_parent,
        bsengine_get_children,
        bsengine_get_velocity,
        bsengine_get_linear_speed,
        bsengine_get_velocity_x,
        bsengine_get_velocity_y,
        bsengine_get_velocity_z,
        bsengine_add_impulse,
        bsengine_apply_impulse_at_point,
        bsengine_add_force,
        bsengine_add_force_at_point,
        bsengine_set_velocity,
        bsengine_set_velocity_x,
        bsengine_set_velocity_y,
        bsengine_set_velocity_z,
        bsengine_get_gravity,
        bsengine_set_gravity,
        bsengine_get_angular_velocity,
        bsengine_get_angular_velocity_x,
        bsengine_get_angular_velocity_y,
        bsengine_get_angular_velocity_z,
        bsengine_set_angular_velocity,
        bsengine_set_angular_velocity_x,
        bsengine_set_angular_velocity_y,
        bsengine_set_angular_velocity_z,
        bsengine_add_velocity,
        bsengine_add_angular_velocity,
        bsengine_add_angular_impulse,
        bsengine_add_torque,
        bsengine_set_ccd_enabled,
        bsengine_set_linear_damping,
        bsengine_set_angular_damping,
        bsengine_get_mass,
        bsengine_set_mass,
        bsengine_get_gravity_scale,
        bsengine_is_kinematic,
        bsengine_is_sleeping,
        bsengine_wake_up,
        bsengine_sleep,
        bsengine_is_collider_sensor,
        bsengine_get_linear_damping,
        bsengine_get_angular_damping,
        bsengine_get_restitution,
        bsengine_set_restitution,
        bsengine_get_friction,
        bsengine_set_friction,
        bsengine_lock_rotation,
        bsengine_lock_translation,
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
    getTransform:      (name)                 => Deno.core.ops.bsengine_get_transform(name),
    getPosition:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.x, y: t.y, z: t.z } : null; },
    getRotation:       (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.rx, y: t.ry, z: t.rz, w: t.rw } : null; },
    getScale:          (name)                 => { const t = Deno.core.ops.bsengine_get_transform(name); return t ? { x: t.sx, y: t.sy, z: t.sz } : null; },
    getForwardVector:  (name)                 => Deno.core.ops.bsengine_get_forward_vector(name),
    getRightVector:    (name)                 => Deno.core.ops.bsengine_get_right_vector(name),
    getUpVector:       (name)                 => Deno.core.ops.bsengine_get_up_vector(name),
    distanceTo:        (nameA, nameB)         => Deno.core.ops.bsengine_distance_to(nameA, nameB),
    distanceToPoint:   (name, x, y, z)       => Deno.core.ops.bsengine_distance_to_point(name, x, y, z),
    getWorldTransform: (name)                 => Deno.core.ops.bsengine_get_world_transform(name),
    getWorldPosition:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.x, y: t.y, z: t.z } : null; },
    getWorldRotation:  (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.rx, y: t.ry, z: t.rz, w: t.rw } : null; },
    getWorldScale:     (name)                 => { const t = Deno.core.ops.bsengine_get_world_transform(name); return t ? { x: t.sx, y: t.sy, z: t.sz } : null; },
    setTransform:   (name, x, y, z)        => Deno.core.ops.bsengine_set_transform(name, x, y, z),
    setRotation:      (name, rx, ry, rz, rw)        => Deno.core.ops.bsengine_set_rotation(name, rx, ry, rz, rw),
    setRotationEuler: (name, pitch, yaw, roll)      => Deno.core.ops.bsengine_set_rotation_euler(name, pitch, yaw, roll),
    setScale:            (name, sx, sy, sz)     => Deno.core.ops.bsengine_set_scale(name, sx, sy, sz),
    addPosition:         (name, dx, dy, dz)     => Deno.core.ops.bsengine_add_position(name, dx, dy, dz),
    addPositionLocal:    (name, dx, dy, dz)     => Deno.core.ops.bsengine_add_position_local(name, dx, dy, dz),
    setPositionX:        (name, x)              => Deno.core.ops.bsengine_set_position_x(name, x),
    setPositionY:        (name, y)              => Deno.core.ops.bsengine_set_position_y(name, y),
    setPositionZ:        (name, z)              => Deno.core.ops.bsengine_set_position_z(name, z),
    addPositionX:        (name, dx)             => Deno.core.ops.bsengine_add_position_x(name, dx),
    addPositionY:        (name, dy)             => Deno.core.ops.bsengine_add_position_y(name, dy),
    addPositionZ:        (name, dz)             => Deno.core.ops.bsengine_add_position_z(name, dz),
    rotateBy:          (name, rx, ry, rz, rw)   => Deno.core.ops.bsengine_rotate_by(name, rx, ry, rz, rw),
    rotateAroundAxis:  (name, ax, ay, az, deg)  => Deno.core.ops.bsengine_rotate_around_axis(name, ax, ay, az, deg),
    addRotationEuler:  (name, pitch, yaw, roll) => Deno.core.ops.bsengine_add_rotation_euler(name, pitch, yaw, roll),
    addRotationEulerX: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_x(name, deg),
    addRotationEulerY: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_y(name, deg),
    addRotationEulerZ: (name, deg) => Deno.core.ops.bsengine_add_rotation_euler_z(name, deg),
    setScaleX:         (name, x)               => Deno.core.ops.bsengine_set_scale_x(name, x),
    setScaleY:         (name, y)               => Deno.core.ops.bsengine_set_scale_y(name, y),
    setScaleZ:         (name, z)               => Deno.core.ops.bsengine_set_scale_z(name, z),
    addScaleX:         (name, dx)              => Deno.core.ops.bsengine_add_scale_x(name, dx),
    addScaleY:         (name, dy)              => Deno.core.ops.bsengine_add_scale_y(name, dy),
    addScaleZ:         (name, dz)              => Deno.core.ops.bsengine_add_scale_z(name, dz),
    getPositionX:      (name)                 => Deno.core.ops.bsengine_get_position_x(name),
    getPositionY:      (name)                 => Deno.core.ops.bsengine_get_position_y(name),
    getPositionZ:      (name)                 => Deno.core.ops.bsengine_get_position_z(name),
    getScaleX:         (name)                 => Deno.core.ops.bsengine_get_scale_x(name),
    getScaleY:         (name)                 => Deno.core.ops.bsengine_get_scale_y(name),
    getScaleZ:         (name)                 => Deno.core.ops.bsengine_get_scale_z(name),
    getRotationEulerX: (name) => Deno.core.ops.bsengine_get_rotation_euler_x(name),
    getRotationEulerY: (name) => Deno.core.ops.bsengine_get_rotation_euler_y(name),
    getRotationEulerZ: (name) => Deno.core.ops.bsengine_get_rotation_euler_z(name),
    addScale:          (name, sx, sy, sz)       => Deno.core.ops.bsengine_add_scale(name, sx, sy, sz),
    setRotationEulerX: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_x(name, deg),
    setRotationEulerY: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_y(name, deg),
    setRotationEulerZ: (name, deg) => Deno.core.ops.bsengine_set_rotation_euler_z(name, deg),
    multiplyScale:     (name, sx, sy, sz) => Deno.core.ops.bsengine_multiply_scale(name, sx, sy, sz),
    isKeyPressed:   (key)                  => Deno.core.ops.bsengine_is_key_pressed(key),
    isKeyDown:      (key)                  => Deno.core.ops.bsengine_is_key_down(key),
    isKeyUp:        (key)                  => Deno.core.ops.bsengine_is_key_up(key),
    getEntityNames:      ()    => JSON.parse(Deno.core.ops.bsengine_get_entity_names()),
    getEntitiesByTag:        (tag)           => JSON.parse(Deno.core.ops.bsengine_get_entities_by_tag(tag)),
    getEntitiesInRadius:     (x, y, z, radius) => JSON.parse(Deno.core.ops.bsengine_get_entities_in_radius(x, y, z, radius)),
    getClosestEntity:        (x, y, z)       => Deno.core.ops.bsengine_get_closest_entity(x, y, z),
    getClosestEntityWithTag: (x, y, z, tag)  => Deno.core.ops.bsengine_get_closest_entity_with_tag(x, y, z, tag),
    hasTag:              (name, label) => Deno.core.ops.bsengine_has_tag(name, label),
    addTag:              (name, label) => Deno.core.ops.bsengine_add_tag(name, label),
    removeTag:           (name, label) => Deno.core.ops.bsengine_remove_tag(name, label),
    setKinematic:        (name, kinematic) => Deno.core.ops.bsengine_set_kinematic(name, kinematic),
    getTags:             (name)            => JSON.parse(Deno.core.ops.bsengine_get_tags(name)),
    setGravityScale:     (name, scale)     => Deno.core.ops.bsengine_set_gravity_scale(name, scale),
    setColliderSensor:   (name, sensor)    => Deno.core.ops.bsengine_set_collider_sensor(name, sensor),
    setEmissive:    (name, r, g, b)        => Deno.core.ops.bsengine_set_emissive(name, r, g, b),
    setColor:       (name, r, g, b)        => Deno.core.ops.bsengine_set_color(name, r, g, b),
    spawn:          (params)               => Deno.core.ops.bsengine_spawn(params),
    destroy:        (name)                 => Deno.core.ops.bsengine_destroy(name),
    setVisible:     (name, v)              => Deno.core.ops.bsengine_set_visible(name, v),
    getVisible:     (name)                 => Deno.core.ops.bsengine_get_visible(name),
    getMaterialColor:   (name) => { const v = Deno.core.ops.bsengine_get_material_color(name); return v ? { r: v[0], g: v[1], b: v[2] } : null; },
    getMaterialEmissive:(name) => { const v = Deno.core.ops.bsengine_get_material_emissive(name); return v ? { r: v[0], g: v[1], b: v[2] } : null; },
    setMetallic:    (name, value)          => Deno.core.ops.bsengine_set_metallic(name, value),
    getMetallic:    (name)                 => Deno.core.ops.bsengine_get_metallic(name),
    setRoughness:   (name, value)          => Deno.core.ops.bsengine_set_roughness(name, value),
    getRoughness:   (name)                 => Deno.core.ops.bsengine_get_roughness(name),
    lookAt:         (name, tx, ty, tz)     => Deno.core.ops.bsengine_look_at(name, tx, ty, tz),

    // Time
    getTime:        ()                     => Deno.core.ops.bsengine_get_time(),
    getDeltaTime:   ()                     => Deno.core.ops.bsengine_get_delta_time(),
    getScreenSize:  ()                     => { const [w, h] = Deno.core.ops.bsengine_get_screen_size(); return { width: w, height: h }; },
    setParent:      (child, parent)        => Deno.core.ops.bsengine_set_parent(child, parent),
    clearParent:      (child)   => Deno.core.ops.bsengine_clear_parent(child),
    getParent:        (name)    => { const r = Deno.core.ops.bsengine_get_parent(name); return JSON.parse(r); },
    getChildren:         (name)         => JSON.parse(Deno.core.ops.bsengine_get_children(name)),
    getChildrenCount:    (name)         => JSON.parse(Deno.core.ops.bsengine_get_children(name)).length,
    getChildAt:          (name, index)  => { const c = JSON.parse(Deno.core.ops.bsengine_get_children(name)); return c[index] ?? null; },
    getVelocity:      (name)    => { const v = Deno.core.ops.bsengine_get_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    getLinearSpeed:   (name)    => { const s = Deno.core.ops.bsengine_get_linear_speed(name); return s !== null && s !== undefined ? s[0] : -1; },
    getVelocityX:     (name) => Deno.core.ops.bsengine_get_velocity_x(name),
    getVelocityY:     (name) => Deno.core.ops.bsengine_get_velocity_y(name),
    getVelocityZ:     (name) => Deno.core.ops.bsengine_get_velocity_z(name),
    addImpulse:       (name, fx, fy, fz) => Deno.core.ops.bsengine_add_impulse(name, fx, fy, fz),
    applyImpulseAtPoint: (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_apply_impulse_at_point(name, fx, fy, fz, px, py, pz),
    addForce:         (name, fx, fy, fz) => Deno.core.ops.bsengine_add_force(name, fx, fy, fz),
    addForceAtPoint:  (name, fx, fy, fz, px, py, pz) => Deno.core.ops.bsengine_add_force_at_point(name, fx, fy, fz, px, py, pz),
    setVelocity:      (name, vx, vy, vz) => Deno.core.ops.bsengine_set_velocity(name, vx, vy, vz),
    setVelocityX:     (name, vx) => Deno.core.ops.bsengine_set_velocity_x(name, vx),
    setVelocityY:     (name, vy) => Deno.core.ops.bsengine_set_velocity_y(name, vy),
    setVelocityZ:     (name, vz) => Deno.core.ops.bsengine_set_velocity_z(name, vz),
    getGravity:           ()                     => Deno.core.ops.bsengine_get_gravity(),
    setGravity:           (magnitude)             => Deno.core.ops.bsengine_set_gravity(magnitude),
    getAngularVelocity:   (name)                  => { const v = Deno.core.ops.bsengine_get_angular_velocity(name); return v ? { x: v[0], y: v[1], z: v[2] } : null; },
    getAngularVelocityX:  (name) => Deno.core.ops.bsengine_get_angular_velocity_x(name),
    getAngularVelocityY:  (name) => Deno.core.ops.bsengine_get_angular_velocity_y(name),
    getAngularVelocityZ:  (name) => Deno.core.ops.bsengine_get_angular_velocity_z(name),
    setAngularVelocity:   (name, vx, vy, vz)      => Deno.core.ops.bsengine_set_angular_velocity(name, vx, vy, vz),
    setAngularVelocityX:  (name, vx) => Deno.core.ops.bsengine_set_angular_velocity_x(name, vx),
    setAngularVelocityY:  (name, vy) => Deno.core.ops.bsengine_set_angular_velocity_y(name, vy),
    setAngularVelocityZ:  (name, vz) => Deno.core.ops.bsengine_set_angular_velocity_z(name, vz),
    addVelocity:          (name, vx, vy, vz) => Deno.core.ops.bsengine_add_velocity(name, vx, vy, vz),
    addAngularVelocity:   (name, vx, vy, vz) => Deno.core.ops.bsengine_add_angular_velocity(name, vx, vy, vz),
    addAngularImpulse:    (name, vx, vy, vz)      => Deno.core.ops.bsengine_add_angular_impulse(name, vx, vy, vz),
    addTorque:            (name, vx, vy, vz)      => Deno.core.ops.bsengine_add_torque(name, vx, vy, vz),
    setCCDEnabled:        (name, enabled)           => Deno.core.ops.bsengine_set_ccd_enabled(name, enabled),
    setLinearDamping:     (name, damping)          => Deno.core.ops.bsengine_set_linear_damping(name, damping),
    setAngularDamping:    (name, damping)          => Deno.core.ops.bsengine_set_angular_damping(name, damping),
    getMass:              (name)                   => Deno.core.ops.bsengine_get_mass(name),
    setMass:              (name, mass)             => Deno.core.ops.bsengine_set_mass(name, mass),
    getGravityScale:      (name)                   => Deno.core.ops.bsengine_get_gravity_scale(name),
    isKinematic:          (name)                   => Deno.core.ops.bsengine_is_kinematic(name),
    isSleeping:           (name)                   => Deno.core.ops.bsengine_is_sleeping(name),
    wakeUp:               (name)                   => Deno.core.ops.bsengine_wake_up(name),
    sleep:                (name)                   => Deno.core.ops.bsengine_sleep(name),
    isColliderSensor:     (name)                   => Deno.core.ops.bsengine_is_collider_sensor(name),
    getLinearDamping:     (name)                   => Deno.core.ops.bsengine_get_linear_damping(name),
    getAngularDamping:    (name)                   => Deno.core.ops.bsengine_get_angular_damping(name),
    getRestitution:       (name)                   => Deno.core.ops.bsengine_get_restitution(name),
    setRestitution:       (name, v)                => Deno.core.ops.bsengine_set_restitution(name, v),
    getFriction:          (name)                   => Deno.core.ops.bsengine_get_friction(name),
    setFriction:          (name, v)                => Deno.core.ops.bsengine_set_friction(name, v),
    lockRotation:         (name, lockX, lockY, lockZ) => Deno.core.ops.bsengine_lock_rotation(name, lockX, lockY, lockZ),
    lockTranslation:      (name, lockX, lockY, lockZ) => Deno.core.ops.bsengine_lock_translation(name, lockX, lockY, lockZ),
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

    // Math utilities — pure JS, no round-trips to Rust.
    math: {
        lerp:      (a, b, t)  => a + (b - a) * t,
        clamp:     (v, lo, hi) => Math.min(Math.max(v, lo), hi),
        magnitude: (v)         => Math.sqrt(v.x*v.x + v.y*v.y + v.z*v.z),
        normalize: (v)         => { const l = Math.sqrt(v.x*v.x+v.y*v.y+v.z*v.z); return l>0?{x:v.x/l,y:v.y/l,z:v.z/l}:{x:0,y:0,z:0}; },
        dot:       (a, b)      => a.x*b.x + a.y*b.y + a.z*b.z,
        cross:     (a, b)      => ({x:a.y*b.z-a.z*b.y, y:a.z*b.x-a.x*b.z, z:a.x*b.y-a.y*b.x}),
        lerpVec:   (a, b, t)   => ({x:a.x+(b.x-a.x)*t, y:a.y+(b.y-a.y)*t, z:a.z+(b.z-a.z)*t}),
    },

    // Convenience helpers built on existing ops.
    lookAtEntity(name, targetName) {
        const t = this.getPosition(targetName);
        if (t) this.lookAt(name, t.x, t.y, t.z);
    },
    moveToward(name, tx, ty, tz, speed) {
        const pos = this.getPosition(name);
        if (!pos) return;
        const dx = tx-pos.x, dy = ty-pos.y, dz = tz-pos.z;
        const dist = Math.sqrt(dx*dx+dy*dy+dz*dz);
        if (dist < 1e-6) return;
        const step = Math.min(speed * this.getDeltaTime(), dist) / dist;
        this.setTransform(name, pos.x+dx*step, pos.y+dy*step, pos.z+dz*step);
    },
    getAngularSpeed(name) {
        const v = this.getAngularVelocity(name);
        return v ? Math.sqrt(v.x*v.x+v.y*v.y+v.z*v.z) : 0;
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
    fn distance_to_returns_correct_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "A".to_string(),
                (
                    glam::Vec3::new(0.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "B".to_string(),
                (
                    glam::Vec3::new(3.0, 4.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.distanceTo("A", "B")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let dist: f32 = r.parse().expect("should be a number");
        assert!((dist - 5.0).abs() < 1e-4, "expected 5.0, got {dist}");
    }

    #[test]
    fn distance_to_returns_neg1_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.distanceTo("Ghost", "Unknown")"#)
            .unwrap();
        assert_eq!(r.trim(), "-1", "expected -1 for unknown: {r}");
    }

    #[test]
    fn distance_to_point_returns_correct_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"Bsengine.distanceToPoint("Player", 4.0, 0.0, 0.0)"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let dist: f32 = r.parse().expect("should be a number");
        assert!((dist - 3.0).abs() < 1e-4, "expected 3.0, got {dist}");
    }

    #[test]
    fn get_world_transform_returns_null_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getWorldTransform("NoSuchEntity"))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_forward_vector_returns_neg_z_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getForwardVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("-1") || r.contains("-0"),
            "expected -Z forward: {r}"
        );
    }

    #[test]
    fn get_world_position_reflects_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::WORLD_TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Child".to_string(),
                (
                    glam::Vec3::new(3.0, 4.0, 5.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getWorldPosition("Child"))"#)
            .unwrap();
        super::WORLD_TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("\"x\":3") || r.contains("\"x\":3.0"),
            "pos x: {r}"
        );
        assert!(
            r.contains("\"y\":4") || r.contains("\"y\":4.0"),
            "pos y: {r}"
        );
        assert!(
            r.contains("\"z\":5") || r.contains("\"z\":5.0"),
            "pos z: {r}"
        );
    }

    #[test]
    fn get_right_vector_returns_x_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getRightVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(
            r.contains("1") && !r.contains("-1"),
            "expected +X right: {r}"
        );
    }

    #[test]
    fn get_up_vector_returns_y_for_identity_rotation() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                (glam::Vec3::ZERO, glam::Quat::IDENTITY, glam::Vec3::ONE),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getUpVector("Player"))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("1"), "expected +Y up: {r}");
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
    fn math_lerp_midpoint() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval("Bsengine.math.lerp(0, 10, 0.5)").unwrap();
        let v: f32 = r.trim().parse().unwrap();
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0: {r}");
    }

    #[test]
    fn math_normalize_unit_vector() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.math.normalize({x:3,y:4,z:0}))"#)
            .unwrap();
        assert!(
            r.contains("\"x\":0.6") || r.contains("\"x\":0.60"),
            "x: {r}"
        );
        assert!(
            r.contains("\"y\":0.8") || r.contains("\"y\":0.80"),
            "y: {r}"
        );
    }

    #[test]
    fn math_dot_product() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.math.dot({x:1,y:0,z:0},{x:0,y:1,z:0})"#)
            .unwrap();
        let v: f32 = r.trim().parse().unwrap();
        assert!((v - 0.0).abs() < 1e-4, "expected 0 for perpendicular: {r}");
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
    fn get_material_color_returns_null_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getMaterialColor("Ghost") === null ? "null" : "not-null""#)
            .unwrap();
        assert!(r.contains("null"), "expected null: {r}");
    }

    #[test]
    fn get_material_color_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_COLOR_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), [0.5, 0.25, 1.0]);
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMaterialColor("Cube"))"#)
            .unwrap();
        super::MATERIAL_COLOR_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.5"), "expected r=0.5: {r}");
    }

    #[test]
    fn get_material_emissive_returns_null_for_unknown_entity() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getMaterialEmissive("Ghost") === null ? "null" : "not-null""#)
            .unwrap();
        assert!(r.contains("null"), "expected null: {r}");
    }

    #[test]
    fn get_material_emissive_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_EMISSIVE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), [0.75, 0.0, 0.0]);
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getMaterialEmissive("Cube"))"#)
            .unwrap();
        super::MATERIAL_EMISSIVE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected r=0.75: {r}");
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
    fn get_closest_entity_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getClosestEntity(0, 0, 0)"#).unwrap();
        assert!(r.trim_matches('"').is_empty(), "expected empty: {r}");
    }

    #[test]
    fn get_closest_entity_returns_nearest() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "Near".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "Far".to_string(),
                (
                    glam::Vec3::new(100.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getClosestEntity(0, 0, 0)"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("Near"), "expected Near: {r}");
    }

    #[test]
    fn get_entities_in_radius_returns_nearby_entities() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "Near".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "Far".to_string(),
                (
                    glam::Vec3::new(100.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesInRadius(0.0, 0.0, 0.0, 5.0))"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("Near"), "expected Near: {r}");
        assert!(!r.contains("Far"), "should not contain Far: {r}");
    }

    #[test]
    fn get_entities_in_radius_returns_empty_when_none_in_range() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getEntitiesInRadius(0.0, 0.0, 0.0, 1.0))"#)
            .unwrap();
        assert_eq!(r.trim(), "[]", "expected empty array: {r}");
    }

    #[test]
    fn get_closest_entity_with_tag_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"Bsengine.getClosestEntityWithTag(0, 0, 0, "enemy")"#)
            .unwrap();
        assert!(r.trim_matches('"').is_empty(), "expected empty: {r}");
    }

    #[test]
    fn get_closest_entity_with_tag_returns_nearest_with_tag() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            let mut m = s.borrow_mut();
            m.insert(
                "NearEnemy".to_string(),
                (
                    glam::Vec3::new(2.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "FarEnemy".to_string(),
                (
                    glam::Vec3::new(50.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
            m.insert(
                "NearAlly".to_string(),
                (
                    glam::Vec3::new(1.0, 0.0, 0.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        super::TAG_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "enemy".to_string(),
                vec!["NearEnemy".to_string(), "FarEnemy".to_string()],
            );
        });
        let r = rt
            .eval(r#"Bsengine.getClosestEntityWithTag(0, 0, 0, "enemy")"#)
            .unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        super::TAG_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("NearEnemy"), "expected NearEnemy: {r}");
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
    fn set_kinematic_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setKinematic("Box", true);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetKinematic { name, kinematic }
                    if name == "Box" && *kinematic)
            });
            assert!(found, "SetKinematic not in buffer");
        });
    }

    #[test]
    fn get_tags_returns_empty_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getTags("Player"))"#)
            .unwrap();
        assert!(r.contains("[]"), "expected empty array: {r}");
    }

    #[test]
    fn get_tags_returns_list_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ENTITY_TAGS_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Player".to_string(),
                vec!["hero".to_string(), "alive".to_string()],
            );
        });
        let r = rt
            .eval(r#"JSON.stringify(Bsengine.getTags("Player"))"#)
            .unwrap();
        super::ENTITY_TAGS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("hero"), "expected hero: {r}");
        assert!(r.contains("alive"), "expected alive: {r}");
    }

    #[test]
    fn set_gravity_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setGravityScale("Ball", 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetGravityScale { name, scale }
                    if name == "Ball" && (*scale - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetGravityScale not in buffer");
        });
    }

    #[test]
    fn set_collider_sensor_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setColliderSensor("Zone", true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetColliderSensor { name, sensor }
                    if name == "Zone" && *sensor)
            });
            assert!(found, "SetColliderSensor not in buffer");
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
    fn apply_impulse_at_point_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.applyImpulseAtPoint("Ball", 0, 10, 0, 1, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddImpulseAtPoint { name, fy, px, .. }
                    if name == "Ball" && (*fy - 10.0).abs() < 1e-6 && (*px - 1.0).abs() < 1e-6)
            });
            assert!(found, "AddImpulseAtPoint not in buffer");
        });
    }

    #[test]
    fn add_force_at_point_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addForceAtPoint("Ball", 0, 5, 0, 1, 0, 0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddForceAtPoint { name, fy, px, .. }
                    if name == "Ball" && (*fy - 5.0).abs() < 1e-6 && (*px - 1.0).abs() < 1e-6)
            });
            assert!(found, "AddForceAtPoint not in buffer");
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
    fn set_velocity_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityX("Ball", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityX { name, vx }
                    if name == "Ball" && (*vx - 3.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityX not in buffer");
        });
    }

    #[test]
    fn set_velocity_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityY("Ball", 4.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityY { name, vy }
                    if name == "Ball" && (*vy - 4.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityY not in buffer");
        });
    }

    #[test]
    fn set_velocity_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setVelocityZ("Ball", 2.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetVelocityZ { name, vz }
                    if name == "Ball" && (*vz - 2.0).abs() < 1e-6)
            });
            assert!(found, "SetVelocityZ not in buffer");
        });
    }

    #[test]
    fn add_position_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPosition("Player", 1, 2, 3);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddPosition { name, dx, dy, dz }
                    if name == "Player"
                        && (*dx - 1.0).abs() < 1e-6
                        && (*dy - 2.0).abs() < 1e-6
                        && (*dz - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddPosition not in buffer");
        });
    }

    #[test]
    fn add_position_local_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionLocal("Player", 0, 0, -1);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddPositionLocal { name, dz, .. }
                    if name == "Player" && (*dz - (-1.0)).abs() < 1e-6)
            });
            assert!(found, "AddPositionLocal not in buffer");
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
    fn set_angular_velocity_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityX("Top", 1.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityX { name, vx }
                    if name == "Top" && (*vx - 1.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityX not in buffer");
        });
    }

    #[test]
    fn set_angular_velocity_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityY("Top", 2.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityY { name, vy }
                    if name == "Top" && (*vy - 2.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityY not in buffer");
        });
    }

    #[test]
    fn set_angular_velocity_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setAngularVelocityZ("Top", 3.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetAngularVelocityZ { name, vz }
                    if name == "Top" && (*vz - 3.5).abs() < 1e-6)
            });
            assert!(found, "SetAngularVelocityZ not in buffer");
        });
    }

    #[test]
    fn add_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addVelocity("Ball", 1.0, 2.0, 3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddVelocity { name, vx, vy, vz }
                    if name == "Ball" && (*vx - 1.0).abs() < 1e-6
                        && (*vy - 2.0).abs() < 1e-6
                        && (*vz - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddVelocity not in buffer");
        });
    }

    #[test]
    fn add_angular_velocity_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addAngularVelocity("Top", 0.1, 0.2, 0.3);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddAngularVelocity { name, vx, vy, vz }
                    if name == "Top" && (*vx - 0.1).abs() < 1e-6
                        && (*vy - 0.2).abs() < 1e-6
                        && (*vz - 0.3).abs() < 1e-6)
            });
            assert!(found, "AddAngularVelocity not in buffer");
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
    fn add_torque_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addTorque("Gyro", 0, 3, 0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddTorque { name, vy, .. }
                    if name == "Gyro" && (*vy - 3.0).abs() < 1e-6)
            });
            assert!(found, "AddTorque not in buffer");
        });
    }

    #[test]
    fn set_ccd_enabled_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setCCDEnabled("Bullet", true);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetCCDEnabled { name, enabled }
                    if name == "Bullet" && *enabled)
            });
            assert!(found, "SetCCDEnabled not in buffer");
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
    fn get_gravity_scale_returns_default_when_no_snapshot() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getGravityScale("Cube")"#).unwrap();
        assert!(r.contains('1'), "expected 1: {r}");
    }

    #[test]
    fn get_gravity_scale_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::GRAVITY_SCALE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), 2.5);
        });
        let r = rt.eval(r#"Bsengine.getGravityScale("Cube")"#).unwrap();
        super::GRAVITY_SCALE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("2.5"), "expected 2.5: {r}");
    }

    #[test]
    fn is_kinematic_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"String(Bsengine.isKinematic("Cube"))"#).unwrap();
        assert_eq!(r, "false");
    }

    #[test]
    fn is_kinematic_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::BODY_TYPE_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Cube".to_string(), true);
        });
        let r = rt.eval(r#"String(Bsengine.isKinematic("Cube"))"#).unwrap();
        super::BODY_TYPE_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(r, "true");
    }

    #[test]
    fn is_collider_sensor_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.isColliderSensor("Zone"))"#)
            .unwrap();
        assert_eq!(r, "false");
    }

    #[test]
    fn is_collider_sensor_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::COLLIDER_SENSOR_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Zone".to_string(), true);
        });
        let r = rt
            .eval(r#"String(Bsengine.isColliderSensor("Zone"))"#)
            .unwrap();
        super::COLLIDER_SENSOR_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(r, "true");
    }

    #[test]
    fn get_linear_damping_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getLinearDamping("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_linear_damping_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::LINEAR_DAMPING_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.3);
        });
        let r = rt.eval(r#"Bsengine.getLinearDamping("Ball")"#).unwrap();
        super::LINEAR_DAMPING_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.3"), "expected 0.3: {r}");
    }

    #[test]
    fn get_angular_damping_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getAngularDamping("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_angular_damping_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_DAMPING_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getAngularDamping("Ball")"#).unwrap();
        super::ANGULAR_DAMPING_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected 0.75: {r}");
    }

    #[test]
    fn get_restitution_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getRestitution("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_restitution_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::RESTITUTION_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getRestitution("Ball")"#).unwrap();
        super::RESTITUTION_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.75"), "expected 0.75: {r}");
    }

    #[test]
    fn set_restitution_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRestitution("Ball", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetRestitution { name, restitution }
                    if name == "Ball" && (*restitution - 0.5).abs() < 1e-6)
            });
            assert!(found, "SetRestitution not in buffer");
        });
    }

    #[test]
    fn get_friction_returns_zero_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getFriction("Ball")"#).unwrap();
        assert!(r.contains('0'), "expected 0: {r}");
    }

    #[test]
    fn get_friction_returns_value_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::FRICTION_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Ball".to_string(), 0.5);
        });
        let r = rt.eval(r#"Bsengine.getFriction("Ball")"#).unwrap();
        super::FRICTION_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("0.5"), "expected 0.5: {r}");
    }

    #[test]
    fn set_friction_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setFriction("Ball", 0.25);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::SetFriction { name, friction }
                    if name == "Ball" && (*friction - 0.25).abs() < 1e-6)
            });
            assert!(found, "SetFriction not in buffer");
        });
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
    fn lock_translation_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.lockTranslation("Player", false, true, false);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::LockTranslation { name, lock_x, lock_y, lock_z }
                    if name == "Player" && !*lock_x && *lock_y && !*lock_z)
            });
            assert!(found, "LockTranslation not in buffer");
        });
    }

    #[test]
    fn wake_up_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.wakeUp("Rock");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf
                .iter()
                .any(|cmd| matches!(cmd, super::ScriptCommand::WakeUp { name } if name == "Rock"));
            assert!(found, "WakeUp not in buffer");
        });
    }

    #[test]
    fn sleep_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.sleep("Rock");"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(
                |cmd| matches!(cmd, super::ScriptCommand::PutToSleep { name } if name == "Rock"),
            );
            assert!(found, "PutToSleep not in buffer");
        });
    }

    #[test]
    fn is_sleeping_returns_false_by_default() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let v = rt.eval(r#"String(Bsengine.isSleeping("Rock"))"#).unwrap();
        assert_eq!(v, "false");
    }

    #[test]
    fn is_sleeping_returns_true_when_snapshot_set() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::SLEEP_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Rock".to_string(), true);
        });
        let v = rt.eval(r#"String(Bsengine.isSleeping("Rock"))"#).unwrap();
        super::SLEEP_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert_eq!(v, "true");
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

    #[test]
    fn set_position_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionX("Player", 5.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionX { name, x } => {
                    assert_eq!(name, "Player");
                    assert!((x - 5.0).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_position_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionY("Player", -3.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionY { name, y } => {
                    assert_eq!(name, "Player");
                    assert!((y - (-3.0)).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_position_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setPositionZ("Player", 10.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetPositionZ { name, z } => {
                    assert_eq!(name, "Player");
                    assert!((z - 10.0).abs() < 1e-4);
                }
                _ => panic!("expected SetPositionZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn rotate_by_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.rotateBy("Cube", 0.0, 0.707, 0.0, 0.707);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::RotateBy { name, ry, rw, .. } => {
                    assert_eq!(name, "Cube");
                    assert!((ry - 0.707).abs() < 1e-4);
                    assert!((rw - 0.707).abs() < 1e-4);
                }
                _ => panic!("expected RotateBy command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn rotate_around_axis_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.rotateAroundAxis("Cube", 0.0, 1.0, 0.0, 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::RotateAroundAxis {
                    name,
                    ay,
                    angle_deg,
                    ..
                } => {
                    assert_eq!(name, "Cube");
                    assert!((ay - 1.0).abs() < 1e-4);
                    assert!((angle_deg - 90.0).abs() < 1e-4);
                }
                _ => panic!("expected RotateAroundAxis command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleX("Cube", 2.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleX { name, x } => {
                    assert_eq!(name, "Cube");
                    assert!((x - 2.0).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleY("Cube", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleY { name, y } => {
                    assert_eq!(name, "Cube");
                    assert!((y - 3.0).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_scale_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setScaleZ("Cube", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetScaleZ { name, z } => {
                    assert_eq!(name, "Cube");
                    assert!((z - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected SetScaleZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_linear_speed_returns_neg1_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let raw = rt
            .eval(r#"String(Deno.core.ops.bsengine_get_linear_speed("__no_such__"))"#)
            .unwrap();
        assert!(
            raw.contains("null") || raw.contains("undefined"),
            "op should return null for unknown entity: {raw}"
        );
        let wrapped = rt
            .eval(r#"Bsengine.getLinearSpeed("__no_such__")"#)
            .unwrap();
        let v: f32 = wrapped.trim().parse().expect("should be a number");
        assert!(
            v < 0.0,
            "wrapper should return -1 for unknown entity, got: {v}"
        );
    }

    #[test]
    fn get_linear_speed_returns_magnitude() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(3.0, 4.0, 0.0));
        });
        let r = rt.eval(r#"Bsengine.getLinearSpeed("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let speed: f32 = r.trim().parse().expect("expected a number");
        assert!((speed - 5.0).abs() < 1e-4, "expected 5.0, got {speed}");
    }

    #[test]
    fn get_children_count_returns_zero_for_unknown() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt.eval(r#"Bsengine.getChildrenCount("NoEntity")"#).unwrap();
        assert_eq!(r.trim(), "0", "expected 0: {r}");
    }

    #[test]
    fn get_child_at_returns_null_for_out_of_bounds() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        let r = rt
            .eval(r#"String(Bsengine.getChildAt("NoEntity", 0))"#)
            .unwrap();
        assert!(
            r.contains("null") || r.contains("undefined"),
            "expected null: {r}"
        );
    }

    #[test]
    fn get_child_at_returns_correct_child() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Root".to_string(),
                vec!["ChildA".to_string(), "ChildB".to_string()],
            );
        });
        let r = rt.eval(r#"Bsengine.getChildAt("Root", 1)"#).unwrap();
        super::CHILDREN_SNAPSHOT.with(|s| s.borrow_mut().clear());
        assert!(r.contains("ChildB"), "expected ChildB: {r}");
    }

    #[test]
    fn set_rotation_euler_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEuler("Cube", 45.0, 90.0, 0.0);"#)
            .unwrap();

        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEuler {
                    name,
                    pitch_deg,
                    yaw_deg,
                    roll_deg,
                } => {
                    assert_eq!(name, "Cube");
                    assert!((pitch_deg - 45.0).abs() < 1e-4);
                    assert!((yaw_deg - 90.0).abs() < 1e-4);
                    assert!((roll_deg - 0.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEuler("Cube", 30.0, 45.0, 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddRotationEuler {
                    name,
                    pitch,
                    yaw,
                    roll,
                } => {
                    assert_eq!(name, "Cube");
                    assert!((pitch - 30.0).abs() < 1e-4, "pitch: {pitch}");
                    assert!((yaw - 45.0).abs() < 1e-4, "yaw: {yaw}");
                    assert!((roll - 90.0).abs() < 1e-4, "roll: {roll}");
                }
                _ => panic!("expected AddRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerX("Cube", 45.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerX { name, deg }
                    if name == "Cube" && (*deg - 45.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerX not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerY("Cube", 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerY { name, deg }
                    if name == "Cube" && (*deg - 90.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerY not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_rotation_euler_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addRotationEulerZ("Cube", 30.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            let found = buf.iter().any(|cmd| {
                matches!(cmd, super::ScriptCommand::AddRotationEulerZ { name, deg }
                    if name == "Cube" && (*deg - 30.0).abs() < 1e-4)
            });
            assert!(found, "AddRotationEulerZ not in buffer");
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScale("Obj", 0.5, 0.5, 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScale { name, sx, sy, sz } => {
                    assert_eq!(name, "Obj");
                    assert!((sx - 0.5).abs() < 1e-4, "sx: {sx}");
                    assert!((sy - 0.5).abs() < 1e-4, "sy: {sy}");
                    assert!((sz - 0.5).abs() < 1e-4, "sz: {sz}");
                }
                _ => panic!("expected AddScale command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_zero_enqueues_identity_angles() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEuler("Box", 0.0, 0.0, 0.0);"#)
            .unwrap();

        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEuler {
                    pitch_deg,
                    yaw_deg,
                    roll_deg,
                    ..
                } => {
                    assert!((pitch_deg).abs() < 1e-4);
                    assert!((yaw_deg).abs() < 1e-4);
                    assert!((roll_deg).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEuler command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerX("Box", 45.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerX { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 45.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerY("Box", 90.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerY { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 90.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_rotation_euler_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRotationEulerZ("Box", 180.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRotationEulerZ { name, deg } => {
                    assert_eq!(name, "Box");
                    assert!((deg - 180.0).abs() < 1e-4);
                }
                _ => panic!("expected SetRotationEulerZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn multiply_scale_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.multiplyScale("Obj", 2.0, 3.0, 0.5);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::MultiplyScale { name, sx, sy, sz } => {
                    assert_eq!(name, "Obj");
                    assert!((sx - 2.0).abs() < 1e-4);
                    assert!((sy - 3.0).abs() < 1e-4);
                    assert!((sz - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected MultiplyScale command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_position_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn get_position_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0, got {v}");
    }

    #[test]
    fn get_position_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::new(3.0, 5.0, 7.0),
                    glam::Quat::IDENTITY,
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getPositionZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 7.0).abs() < 1e-4, "expected 7.0, got {v}");
    }

    #[test]
    fn get_scale_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_scale_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn get_scale_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::IDENTITY,
                    glam::Vec3::new(2.0, 3.0, 4.0),
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getScaleZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 4.0).abs() < 1e-4, "expected 4.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 30f32.to_radians(), 0.0, 0.0),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerX("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 30.0).abs() < 1e-3, "expected 30.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 0.0, 45f32.to_radians(), 0.0),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerY("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 45.0).abs() < 1e-3, "expected 45.0, got {v}");
    }

    #[test]
    fn get_rotation_euler_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| {
            s.borrow_mut().insert(
                "Obj".to_string(),
                (
                    glam::Vec3::ZERO,
                    glam::Quat::from_euler(glam::EulerRot::XYZ, 0.0, 0.0, 90f32.to_radians()),
                    glam::Vec3::ONE,
                ),
            );
        });
        let r = rt.eval(r#"Bsengine.getRotationEulerZ("Obj")"#).unwrap();
        super::TRANSFORM_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 90.0).abs() < 1e-3, "expected 90.0, got {v}");
    }

    #[test]
    fn get_velocity_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityX("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 5.0).abs() < 1e-4, "expected 5.0, got {v}");
    }

    #[test]
    fn get_velocity_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityY("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_velocity_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Ball".to_string(), glam::Vec3::new(5.0, 2.0, -3.0));
        });
        let r = rt.eval(r#"Bsengine.getVelocityZ("Ball")"#).unwrap();
        super::VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - (-3.0)).abs() < 1e-4, "expected -3.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_x_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityX("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 1.0).abs() < 1e-4, "expected 1.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_y_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityY("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 2.0).abs() < 1e-4, "expected 2.0, got {v}");
    }

    #[test]
    fn get_angular_velocity_z_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| {
            s.borrow_mut()
                .insert("Wheel".to_string(), glam::Vec3::new(1.0, 2.0, 3.0));
        });
        let r = rt.eval(r#"Bsengine.getAngularVelocityZ("Wheel")"#).unwrap();
        super::ANGULAR_VELOCITY_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 3.0).abs() < 1e-4, "expected 3.0, got {v}");
    }

    #[test]
    fn add_position_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionX("Player", 5.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionX { name, dx } => {
                    assert_eq!(name, "Player");
                    assert!((dx - 5.0).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_position_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionY("Player", -2.0);"#)
            .unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionY { name, dy } => {
                    assert_eq!(name, "Player");
                    assert!((dy - (-2.0)).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_position_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addPositionZ("Player", 3.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddPositionZ { name, dz } => {
                    assert_eq!(name, "Player");
                    assert!((dz - 3.0).abs() < 1e-4);
                }
                _ => panic!("expected AddPositionZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_x_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleX("Box", 0.5);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleX { name, dx } => {
                    assert_eq!(name, "Box");
                    assert!((dx - 0.5).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleX command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_y_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleY("Box", 1.0);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleY { name, dy } => {
                    assert_eq!(name, "Box");
                    assert!((dy - 1.0).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleY command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn add_scale_z_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.addScaleZ("Box", -0.25);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::AddScaleZ { name, dz } => {
                    assert_eq!(name, "Box");
                    assert!((dz - (-0.25)).abs() < 1e-4);
                }
                _ => panic!("expected AddScaleZ command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn set_metallic_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setMetallic("Sphere", 0.8);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetMetallic { name, value } => {
                    assert_eq!(name, "Sphere");
                    assert!((value - 0.8).abs() < 1e-4);
                }
                _ => panic!("expected SetMetallic command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_metallic_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_METALLIC_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Sphere".to_string(), 0.75);
        });
        let r = rt.eval(r#"Bsengine.getMetallic("Sphere")"#).unwrap();
        super::MATERIAL_METALLIC_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 0.75).abs() < 1e-4, "expected 0.75, got {v}");
    }

    #[test]
    fn set_roughness_enqueues_command() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        rt.eval(r#"Bsengine.setRoughness("Sphere", 0.3);"#).unwrap();
        super::COMMAND_BUFFER.with(|c| {
            let buf = c.borrow();
            assert_eq!(buf.len(), 1);
            match &buf[0] {
                super::ScriptCommand::SetRoughness { name, value } => {
                    assert_eq!(name, "Sphere");
                    assert!((value - 0.3).abs() < 1e-4);
                }
                _ => panic!("expected SetRoughness command"),
            }
        });
        super::COMMAND_BUFFER.with(|c| c.borrow_mut().clear());
    }

    #[test]
    fn get_roughness_returns_value() {
        let mut rt = ScriptRuntime::new_with_ops();
        rt.exec_source(super::BOOTSTRAP_JS, "<bootstrap>").unwrap();
        super::MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| {
            s.borrow_mut().insert("Sphere".to_string(), 0.4);
        });
        let r = rt.eval(r#"Bsengine.getRoughness("Sphere")"#).unwrap();
        super::MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| s.borrow_mut().clear());
        let v: f32 = r.trim().parse().expect("expected a number");
        assert!((v - 0.4).abs() < 1e-4, "expected 0.4, got {v}");
    }
}
