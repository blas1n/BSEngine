use std::collections::{HashMap, HashSet};

use bevy_app::{App, Plugin, PostStartup, Update};
use bevy_ecs::prelude::*;
use bsengine_audio::AudioWorld;
use bsengine_core::{
    CursorConfig, GlobalTransform, HudTexts, Material, Parent, ScreenSize, SkyboxPath, Tag,
    Transform, Visible,
};
use bsengine_input::{GamepadButton, GamepadSticks, Input, KeyCode, MouseButton, MouseState};
use bsengine_physics::CollisionEvent;
use bsengine_physics::PhysicsWorld;
use bsengine_scene::{Name, PendingSceneLoad, Primitive, PrimitiveMesh, ScriptPath};
use glam::{EulerRot, Quat, Vec3};

use crate::ops::{
    ScriptCommand, SpawnParams, ABILITY_SNAPSHOT, ABSORPTION_SNAPSHOT, ALARM_SNAPSHOT,
    AMBIENT_OCCLUSION_SNAPSHOT, AMMO_SNAPSHOT, AMPLIFY_SNAPSHOT, ANCHOR_SNAPSHOT,
    ANGULAR_DAMPING_SNAPSHOT, ANGULAR_VELOCITY_SNAPSHOT, ANIMATION_SNAPSHOT, ARMOR_SNAPSHOT,
    BARRIER_SNAPSHOT, BEACON_SNAPSHOT, BILLBOARD_SNAPSHOT, BLEED_SNAPSHOT, BLIND_SNAPSHOT,
    BLOOM_SNAPSHOT, BODY_TYPE_SNAPSHOT, BOOTSTRAP_JS, BUOYANCY_SNAPSHOT, BURN_SNAPSHOT,
    CHARGE_SNAPSHOT, CHARM_SNAPSHOT, CHILDREN_SNAPSHOT, CHROM_AB_SNAPSHOT,
    COLLIDER_SENSOR_SNAPSHOT, COLLISION_SNAPSHOT, COLOR_GRADING_SNAPSHOT, COMMAND_BUFFER,
    CONCUSS_SNAPSHOT, CONFUSE_SNAPSHOT, COOLDOWN_SNAPSHOT, CORROSION_SNAPSHOT, CRIPPLE_SNAPSHOT,
    CROSSHAIR_SNAPSHOT, CURSE_SNAPSHOT, DAMAGE_SNAPSHOT, DASH_SNAPSHOT, DAZE_SNAPSHOT,
    DEMORALIZE_SNAPSHOT, DEPTH_OF_FIELD_SNAPSHOT, DIALOGUE_SNAPSHOT, DISARM_SNAPSHOT,
    DISSOLVE_SNAPSHOT, DODGE_SNAPSHOT, DOOM_SNAPSHOT, DRAIN_SNAPSHOT, DREAD_SNAPSHOT,
    EMISSIVE_SNAPSHOT, EMPOWER_SNAPSHOT, ENERVATE_SNAPSHOT, ENTANGLE_SNAPSHOT,
    ENTITY_NAMES_SNAPSHOT, ENTITY_NAME_MAP, ENTITY_TAGS_SNAPSHOT, EXHAUSTION_SNAPSHOT,
    EXPERIENCE_SNAPSHOT, EXPOSE_SNAPSHOT, FEAR_SNAPSHOT, FOG_SNAPSHOT, FOLLOW_SNAPSHOT,
    FOOTSTEP_SNAPSHOT, FRACTURE_SNAPSHOT, FREEZE_SNAPSHOT, FRICTION_SNAPSHOT, FROSTBITE_SNAPSHOT,
    FUEL_SNAPSHOT, FURY_SNAPSHOT, GALVANIZE_SNAPSHOT, GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT,
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT, GAMEPAD_BUTTON_SNAPSHOT, GAMEPAD_STICKS_SNAPSHOT,
    GRAPPLE_SNAPSHOT, GRAVITY_SCALE_SNAPSHOT, GRAVITY_SNAPSHOT, GRID_SNAP_SNAPSHOT, HASTE_SNAPSHOT,
    HAVOC_SNAPSHOT, HAZE_SNAPSHOT, HEALTH_SNAPSHOT, HEAT_SNAPSHOT, HEX_SNAPSHOT, HOBBLE_SNAPSHOT,
    IGNITE_SNAPSHOT, IMBUE_SNAPSHOT, IMMUNE_SNAPSHOT, IMPACT_SNAPSHOT, INTERACTABLE_SNAPSHOT,
    INTERCEPT_SNAPSHOT, INTERRUPT_SNAPSHOT, JUMP_SNAPSHOT, KEY_JUST_PRESSED_SNAPSHOT,
    KEY_JUST_RELEASED_SNAPSHOT, KEY_SNAPSHOT, KNOCKBACK_SNAPSHOT, LAYER_SNAPSHOT, LEVEL_SNAPSHOT,
    LIFETIME_SNAPSHOT, LINEAR_DAMPING_SNAPSHOT, LOOK_AT_SNAPSHOT, MANA_SNAPSHOT, MASS_SNAPSHOT,
    MATERIAL_COLOR_SNAPSHOT, MATERIAL_EMISSIVE_SNAPSHOT, MATERIAL_METALLIC_SNAPSHOT,
    MATERIAL_ROUGHNESS_SNAPSHOT, MOTION_BLUR_SNAPSHOT, MOUSE_DELTA_SNAPSHOT,
    MOUSE_JUST_PRESSED_SNAPSHOT, MOUSE_JUST_RELEASED_SNAPSHOT, MOUSE_POS_SNAPSHOT,
    MOUSE_PRESSED_SNAPSHOT, MOVE_SPEED_SNAPSHOT, NAV_SNAPSHOT, OUTLINE_SNAPSHOT, PARENT_SNAPSHOT,
    PHYSICS_WORLD_PTR, POISON_SNAPSHOT, PROJECTILE_SNAPSHOT, REGEN_SNAPSHOT, RESTITUTION_SNAPSHOT,
    ROOT_SNAPSHOT, SCREEN_SHAKE_SNAPSHOT, SCREEN_SIZE_SNAPSHOT, SHIELD_BREAK_SNAPSHOT,
    SHIELD_SNAPSHOT, SLEEP_SNAPSHOT, SLOW_SNAPSHOT, SOUND_POSITION_SNAPSHOT, SOUND_STATE_SNAPSHOT,
    SPAWN_POINT_SNAPSHOT, SPRING_SNAPSHOT, SPRINT_SNAPSHOT, STAMINA_SNAPSHOT,
    STATUS_EFFECT_SNAPSHOT, STUN_SNAPSHOT, TAG_SNAPSHOT, TIMER_SNAPSHOT, TIME_DELTA_SNAPSHOT,
    TIME_ELAPSED_SNAPSHOT, TINT_SNAPSHOT, TONE_MAP_SNAPSHOT, TRANSFORM_SNAPSHOT, TRIGGER_SNAPSHOT,
    TWEEN_SNAPSHOT, VELOCITY_SNAPSHOT, VIGNETTE_SNAPSHOT, VISIBLE_SNAPSHOT, WIND_SNAPSHOT,
    WORLD_TRANSFORM_SNAPSHOT, Z_INDEX_SNAPSHOT,
};
use crate::runtime::ScriptRuntime;

/// Root directory of the current project — used to resolve relative script paths.
#[derive(Resource, Default)]
pub struct ProjectDir(pub String);

/// Loaded JS source for a scripted entity.
#[derive(Component)]
pub struct Script {
    pub source: String,
}

// Not Send/Sync — stored as a non-send resource via insert_non_send_resource.
pub struct ScriptRuntimeResource(pub ScriptRuntime);

/// Stores kira sound handles by script-assigned id for stopSound support.
#[derive(Resource, Default)]
pub struct SoundHandles(pub HashMap<u32, kira::sound::static_sound::StaticSoundHandle>);

#[derive(Resource)]
pub(crate) struct ScriptTimingState {
    startup: std::time::Instant,
    last_frame: std::time::Instant,
}

pub struct ScriptingPlugin {
    pub project_dir: String,
}

impl Default for ScriptingPlugin {
    fn default() -> Self {
        Self {
            project_dir: ".".to_string(),
        }
    }
}

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ProjectDir(self.project_dir.clone()));
        app.insert_resource(HudTexts::default());
        app.insert_resource(SoundHandles::default());
        app.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));
        let now = std::time::Instant::now();
        app.insert_resource(ScriptTimingState {
            startup: now,
            last_frame: now,
        });
        // Register CollisionEvent so EventReader works even without PhysicsPlugin
        app.add_event::<CollisionEvent>();
        app.add_systems(PostStartup, load_scripts);
        app.add_systems(Update, (capture_collision_events, run_scripts).chain());
    }
}

/// Capture collision events each frame into a thread_local snapshot for scripts.
fn capture_collision_events(
    mut events: EventReader<CollisionEvent>,
    name_query: Query<(Entity, &Name)>,
) {
    let name_map: HashMap<Entity, String> =
        name_query.iter().map(|(e, n)| (e, n.0.clone())).collect();

    let collisions: Vec<(String, String, bool)> = events
        .read()
        .filter_map(|ev| {
            let a = name_map.get(&ev.entity_a)?.clone();
            let b = name_map.get(&ev.entity_b)?.clone();
            Some((a, b, ev.started))
        })
        .collect();

    COLLISION_SNAPSHOT.with(|s| *s.borrow_mut() = collisions);
}

pub fn load_scripts(world: &mut World) {
    let project_dir = world
        .get_resource::<ProjectDir>()
        .map(|pd| pd.0.clone())
        .unwrap_or_default();

    let scripts: Vec<(Entity, String)> = {
        let mut q = world.query::<(Entity, &ScriptPath)>();
        q.iter(world)
            .map(|(e, sp)| {
                let path = if project_dir.is_empty() {
                    sp.0.clone()
                } else {
                    format!("{}/{}", project_dir, sp.0)
                };
                (e, path)
            })
            .collect()
    };

    tracing::info!(
        "[scripting] {} scripted entity/entities found",
        scripts.len()
    );

    if scripts.is_empty() {
        return;
    }

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        if let Err(e) = rt.0.exec_source(BOOTSTRAP_JS, "<bootstrap>") {
            tracing::error!("[scripting] bootstrap failed: {e}");
            return;
        }
    }

    for (entity, path) in scripts {
        match std::fs::read_to_string(&path) {
            Ok(source) => {
                let id = entity.to_bits();
                let wrapped = format!(
                    "(function() {{\n{source}\nBsengine._scripts[\"{id}\"] = \
                     {{ onUpdate: typeof onUpdate === 'function' ? onUpdate : null }};\n}})();"
                );
                world.entity_mut(entity).insert(Script { source });
                if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
                    match rt.0.exec_source(&wrapped, &path) {
                        Ok(()) => tracing::info!("[scripting] loaded: {path}"),
                        Err(e) => tracing::error!("[scripting] error in {path}: {e}"),
                    }
                }
            }
            Err(e) => tracing::warn!("[scripting] cannot read {path}: {e}"),
        }
    }
}

const KEY_MAPPINGS: &[(KeyCode, &str)] = &[
    (KeyCode::W, "W"),
    (KeyCode::A, "A"),
    (KeyCode::S, "S"),
    (KeyCode::D, "D"),
    (KeyCode::Space, "Space"),
    (KeyCode::Enter, "Enter"),
    (KeyCode::Escape, "Escape"),
    (KeyCode::Up, "Up"),
    (KeyCode::Down, "Down"),
    (KeyCode::Left, "Left"),
    (KeyCode::Right, "Right"),
];

fn run_scripts(world: &mut World) {
    {
        let mut q = world.query::<&Script>();
        if q.iter(world).next().is_none() {
            return;
        }
    }

    let transform_snapshot: HashMap<String, (Vec3, Quat, Vec3)> = {
        let mut q = world.query::<(&Name, &Transform)>();
        q.iter(world)
            .map(|(n, t)| (n.0.clone(), (t.translation, t.rotation, t.scale)))
            .collect()
    };

    let world_transform_snapshot: HashMap<String, (Vec3, Quat, Vec3)> = {
        let mut q = world.query::<(&Name, &GlobalTransform)>();
        q.iter(world)
            .map(|(n, gt)| {
                let (scale, rot, pos) = gt.0.to_scale_rotation_translation();
                (n.0.clone(), (pos, rot, scale))
            })
            .collect()
    };

    let visible_snapshot: HashMap<String, bool> = {
        let mut q = world.query::<(&Name, &Visible)>();
        q.iter(world)
            .map(|(n, v)| (n.0.clone(), v.is_visible))
            .collect()
    };

    let material_color_snapshot: HashMap<String, [f32; 3]> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.base_color.to_array()))
            .collect()
    };

    let material_emissive_snapshot: HashMap<String, [f32; 3]> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.emissive.to_array()))
            .collect()
    };

    let material_metallic_snapshot: HashMap<String, f32> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.metallic))
            .collect()
    };

    let material_roughness_snapshot: HashMap<String, f32> = {
        let mut q = world.query::<(&Name, &Material)>();
        q.iter(world)
            .map(|(n, m)| (n.0.clone(), m.roughness))
            .collect()
    };

    let (key_snapshot, key_just_pressed, key_just_released): (
        HashSet<String>,
        HashSet<String>,
        HashSet<String>,
    ) = if let Some(input) = world.get_resource::<Input<KeyCode>>() {
        let pressed = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.is_pressed(code))
            .map(|(_, name)| name.to_string())
            .collect();
        let just_pressed = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.just_pressed(code))
            .map(|(_, name)| name.to_string())
            .collect();
        let just_released = KEY_MAPPINGS
            .iter()
            .filter(|(code, _)| input.just_released(code))
            .map(|(_, name)| name.to_string())
            .collect();
        (pressed, just_pressed, just_released)
    } else {
        (HashSet::new(), HashSet::new(), HashSet::new())
    };

    let (mb_pressed, mb_just_pressed, mb_just_released): (u8, u8, u8) =
        if let Some(buttons) = world.get_resource::<Input<MouseButton>>() {
            let mut p = 0u8;
            let mut jp = 0u8;
            let mut jr = 0u8;
            if buttons.is_pressed(&MouseButton::Left) {
                p |= 1;
            }
            if buttons.is_pressed(&MouseButton::Right) {
                p |= 2;
            }
            if buttons.is_pressed(&MouseButton::Middle) {
                p |= 4;
            }
            if buttons.just_pressed(&MouseButton::Left) {
                jp |= 1;
            }
            if buttons.just_pressed(&MouseButton::Right) {
                jp |= 2;
            }
            if buttons.just_pressed(&MouseButton::Middle) {
                jp |= 4;
            }
            if buttons.just_released(&MouseButton::Left) {
                jr |= 1;
            }
            if buttons.just_released(&MouseButton::Right) {
                jr |= 2;
            }
            if buttons.just_released(&MouseButton::Middle) {
                jr |= 4;
            }
            (p, jp, jr)
        } else {
            (0, 0, 0)
        };

    let (mouse_pos, mouse_delta) = world
        .get_resource::<MouseState>()
        .map(|ms| (ms.position, ms.delta))
        .unwrap_or(((0.0, 0.0), (0.0, 0.0)));

    const GAMEPAD_MAPPINGS: &[(GamepadButton, u32)] = &[
        (GamepadButton::South, 0),
        (GamepadButton::East, 1),
        (GamepadButton::West, 2),
        (GamepadButton::North, 3),
        (GamepadButton::LB, 4),
        (GamepadButton::RB, 5),
        (GamepadButton::LT, 6),
        (GamepadButton::RT, 7),
        (GamepadButton::Select, 8),
        (GamepadButton::Start, 9),
        (GamepadButton::LeftStick, 10),
        (GamepadButton::RightStick, 11),
        (GamepadButton::DPadUp, 12),
        (GamepadButton::DPadDown, 13),
        (GamepadButton::DPadLeft, 14),
        (GamepadButton::DPadRight, 15),
    ];

    let (gpad_pressed, gpad_just_pressed, gpad_just_released): (u16, u16, u16) =
        if let Some(gpad) = world.get_resource::<Input<GamepadButton>>() {
            let mut p = 0u16;
            let mut jp = 0u16;
            let mut jr = 0u16;
            for &(btn, bit) in GAMEPAD_MAPPINGS {
                let mask = 1u16 << bit;
                if gpad.is_pressed(&btn) {
                    p |= mask;
                }
                if gpad.just_pressed(&btn) {
                    jp |= mask;
                }
                if gpad.just_released(&btn) {
                    jr |= mask;
                }
            }
            (p, jp, jr)
        } else {
            (0, 0, 0)
        };

    let gamepad_sticks = world
        .get_resource::<GamepadSticks>()
        .map(|s| {
            (
                s.left.0,
                s.left.1,
                s.right.0,
                s.right.1,
                s.left_trigger,
                s.right_trigger,
            )
        })
        .unwrap_or((0.0, 0.0, 0.0, 0.0, 0.0, 0.0));

    let physics_ptr = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| pw as *const PhysicsWorld)
        .unwrap_or(std::ptr::null());

    let entity_name_map: HashMap<u64, String> = {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world)
            .map(|(e, n)| (e.to_bits(), n.0.clone()))
            .collect()
    };

    let velocity_snapshot: HashMap<String, Vec3> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_linvel(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    let angular_velocity_snapshot: HashMap<String, Vec3> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_angvel(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();

    let parent_map: HashMap<String, String> = {
        let mut q = world.query::<(Entity, &Name, &Parent)>();
        q.iter(world)
            .filter_map(|(_, n, p)| {
                entity_name_map
                    .get(&p.0.to_bits())
                    .map(|pn| (n.0.clone(), pn.clone()))
            })
            .collect()
    };
    let children_map: HashMap<String, Vec<String>> = {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for (child, parent) in &parent_map {
            map.entry(parent.clone()).or_default().push(child.clone());
        }
        map
    };

    let scripted: Vec<(String, String)> = {
        let mut q = world.query::<(Entity, &Name, &Script)>();
        q.iter(world)
            .map(|(e, n, _)| (e.to_bits().to_string(), n.0.clone()))
            .collect()
    };

    let all_names: Vec<String> = {
        let mut q = world.query::<&Name>();
        q.iter(world).map(|n| n.0.clone()).collect()
    };

    let collision_json = COLLISION_SNAPSHOT.with(|s| {
        let evs = s.borrow();
        serde_json::to_string(
            &evs.iter()
                .map(|(a, b, started)| {
                    serde_json::json!({"nameA": a, "nameB": b, "started": started})
                })
                .collect::<Vec<_>>(),
        )
        .unwrap_or_else(|_| "[]".to_string())
    });

    TRANSFORM_SNAPSHOT.with(|s| *s.borrow_mut() = transform_snapshot);
    WORLD_TRANSFORM_SNAPSHOT.with(|s| *s.borrow_mut() = world_transform_snapshot);
    VISIBLE_SNAPSHOT.with(|s| *s.borrow_mut() = visible_snapshot);
    MATERIAL_COLOR_SNAPSHOT.with(|s| *s.borrow_mut() = material_color_snapshot);
    MATERIAL_EMISSIVE_SNAPSHOT.with(|s| *s.borrow_mut() = material_emissive_snapshot);
    MATERIAL_METALLIC_SNAPSHOT.with(|s| *s.borrow_mut() = material_metallic_snapshot);
    MATERIAL_ROUGHNESS_SNAPSHOT.with(|s| *s.borrow_mut() = material_roughness_snapshot);

    let (elapsed_secs, delta_secs) =
        if let Some(mut timing) = world.get_resource_mut::<ScriptTimingState>() {
            let now = std::time::Instant::now();
            let elapsed = now.duration_since(timing.startup).as_secs_f32();
            let delta = now.duration_since(timing.last_frame).as_secs_f32();
            timing.last_frame = now;
            (elapsed, delta)
        } else {
            (0.0, 0.0)
        };
    TIME_ELAPSED_SNAPSHOT.with(|s| *s.borrow_mut() = elapsed_secs);
    TIME_DELTA_SNAPSHOT.with(|s| *s.borrow_mut() = delta_secs);
    if let Some(ss) = world.get_resource::<ScreenSize>() {
        SCREEN_SIZE_SNAPSHOT.with(|s| *s.borrow_mut() = (ss.width, ss.height));
    }
    KEY_SNAPSHOT.with(|k| *k.borrow_mut() = key_snapshot);
    KEY_JUST_PRESSED_SNAPSHOT.with(|k| *k.borrow_mut() = key_just_pressed);
    KEY_JUST_RELEASED_SNAPSHOT.with(|k| *k.borrow_mut() = key_just_released);
    ENTITY_NAMES_SNAPSHOT.with(|s| *s.borrow_mut() = all_names);
    MOUSE_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_pressed);
    MOUSE_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_just_pressed);
    MOUSE_JUST_RELEASED_SNAPSHOT.with(|s| *s.borrow_mut() = mb_just_released);
    MOUSE_POS_SNAPSHOT.with(|s| *s.borrow_mut() = mouse_pos);
    MOUSE_DELTA_SNAPSHOT.with(|s| *s.borrow_mut() = mouse_delta);
    let mass_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_mass(entity).map(|m| (name.clone(), m))
                })
                .collect()
        })
        .unwrap_or_default();
    let gravity_scale_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_gravity_scale(entity).map(|s| (name.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default();
    let body_type_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_kinematic(entity).map(|k| (name.clone(), k))
                })
                .collect()
        })
        .unwrap_or_default();
    let collider_sensor_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_collider_sensor(entity).map(|s| (name.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default();
    let linear_damping_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_linear_damping(entity).map(|d| (name.clone(), d))
                })
                .collect()
        })
        .unwrap_or_default();
    let angular_damping_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_angular_damping(entity).map(|d| (name.clone(), d))
                })
                .collect()
        })
        .unwrap_or_default();
    let restitution_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_restitution(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    let friction_snapshot: HashMap<String, f32> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.get_friction(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    let sleep_snapshot: HashMap<String, bool> = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| {
            entity_name_map
                .iter()
                .filter_map(|(&bits, name)| {
                    let entity = bevy_ecs::prelude::Entity::from_bits(bits);
                    pw.is_sleeping(entity).map(|v| (name.clone(), v))
                })
                .collect()
        })
        .unwrap_or_default();
    let (tag_snapshot, entity_tags_snapshot): (
        HashMap<String, Vec<String>>,
        HashMap<String, Vec<String>>,
    ) = {
        let mut by_label: HashMap<String, Vec<String>> = HashMap::new();
        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();
        let mut q = world.query::<(&Name, &Tag)>();
        for (name, tag) in q.iter(world) {
            for label in tag.iter() {
                by_label
                    .entry(label.to_string())
                    .or_default()
                    .push(name.0.clone());
                by_name
                    .entry(name.0.clone())
                    .or_default()
                    .push(label.to_string());
            }
        }
        (by_label, by_name)
    };
    ENTITY_NAME_MAP.with(|m| *m.borrow_mut() = entity_name_map);
    PARENT_SNAPSHOT.with(|s| *s.borrow_mut() = parent_map);
    CHILDREN_SNAPSHOT.with(|s| *s.borrow_mut() = children_map);
    TAG_SNAPSHOT.with(|s| *s.borrow_mut() = tag_snapshot);
    ENTITY_TAGS_SNAPSHOT.with(|s| *s.borrow_mut() = entity_tags_snapshot);
    VELOCITY_SNAPSHOT.with(|s| *s.borrow_mut() = velocity_snapshot);
    ANGULAR_VELOCITY_SNAPSHOT.with(|s| *s.borrow_mut() = angular_velocity_snapshot);
    MASS_SNAPSHOT.with(|s| *s.borrow_mut() = mass_snapshot);
    GRAVITY_SCALE_SNAPSHOT.with(|s| *s.borrow_mut() = gravity_scale_snapshot);
    BODY_TYPE_SNAPSHOT.with(|s| *s.borrow_mut() = body_type_snapshot);
    COLLIDER_SENSOR_SNAPSHOT.with(|s| *s.borrow_mut() = collider_sensor_snapshot);
    LINEAR_DAMPING_SNAPSHOT.with(|s| *s.borrow_mut() = linear_damping_snapshot);
    ANGULAR_DAMPING_SNAPSHOT.with(|s| *s.borrow_mut() = angular_damping_snapshot);
    RESTITUTION_SNAPSHOT.with(|s| *s.borrow_mut() = restitution_snapshot);
    FRICTION_SNAPSHOT.with(|s| *s.borrow_mut() = friction_snapshot);
    SLEEP_SNAPSHOT.with(|s| *s.borrow_mut() = sleep_snapshot);
    let gravity = world
        .get_resource::<PhysicsWorld>()
        .map(|pw| pw.gravity())
        .unwrap_or(9.81);
    GRAVITY_SNAPSHOT.with(|s| *s.borrow_mut() = gravity);
    PHYSICS_WORLD_PTR.with(|p| *p.borrow_mut() = physics_ptr);
    GAMEPAD_BUTTON_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_pressed);
    GAMEPAD_BUTTON_JUST_PRESSED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_pressed);
    GAMEPAD_BUTTON_JUST_RELEASED_SNAPSHOT.with(|s| *s.borrow_mut() = gpad_just_released);
    GAMEPAD_STICKS_SNAPSHOT.with(|s| *s.borrow_mut() = gamepad_sticks);
    if let Some(handles) = world.get_resource::<SoundHandles>() {
        use kira::sound::PlaybackState;
        let mut states = std::collections::HashMap::new();
        let mut positions = std::collections::HashMap::new();
        for (id, handle) in &handles.0 {
            let state = match handle.state() {
                PlaybackState::Playing => "playing",
                PlaybackState::Pausing => "pausing",
                PlaybackState::Paused => "paused",
                PlaybackState::WaitingToResume => "waiting_to_resume",
                PlaybackState::Resuming => "resuming",
                PlaybackState::Stopping => "stopping",
                PlaybackState::Stopped => "stopped",
            };
            states.insert(*id, state.to_string());
            positions.insert(*id, handle.position());
        }
        SOUND_STATE_SNAPSHOT.with(|s| *s.borrow_mut() = states);
        SOUND_POSITION_SNAPSHOT.with(|s| *s.borrow_mut() = positions);
    }
    {
        use bsengine_core::Health;
        let mut health_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Health)>();
        for (name, health) in q.iter(world) {
            health_map.insert(name.0.clone(), (health.current, health.max));
        }
        HEALTH_SNAPSHOT.with(|s| *s.borrow_mut() = health_map);
    }
    {
        use bsengine_core::AnimationPlayer;
        let mut anim_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &AnimationPlayer)>();
        for (name, ap) in q.iter(world) {
            anim_map.insert(
                name.0.clone(),
                (ap.clip.clone(), ap.time, ap.speed, ap.looping, ap.playing),
            );
        }
        ANIMATION_SNAPSHOT.with(|s| *s.borrow_mut() = anim_map);
    }
    {
        use bsengine_core::Lifetime;
        let mut lifetime_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Lifetime)>();
        for (name, lt) in q.iter(world) {
            lifetime_map.insert(name.0.clone(), lt.remaining);
        }
        LIFETIME_SNAPSHOT.with(|s| *s.borrow_mut() = lifetime_map);
    }
    {
        use bsengine_core::Stamina;
        let mut stamina_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stamina)>();
        for (name, st) in q.iter(world) {
            stamina_map.insert(name.0.clone(), (st.current, st.max, st.exhausted));
        }
        STAMINA_SNAPSHOT.with(|s| *s.borrow_mut() = stamina_map);
    }
    {
        use bsengine_core::Mana;
        let mut mana_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Mana)>();
        for (name, mn) in q.iter(world) {
            mana_map.insert(name.0.clone(), (mn.current, mn.max));
        }
        MANA_SNAPSHOT.with(|s| *s.borrow_mut() = mana_map);
    }
    {
        use bsengine_core::MoveSpeed;
        let mut ms_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &MoveSpeed)>();
        for (name, ms) in q.iter(world) {
            ms_map.insert(name.0.clone(), (ms.base, ms.effective()));
        }
        MOVE_SPEED_SNAPSHOT.with(|s| *s.borrow_mut() = ms_map);
    }
    {
        use bsengine_core::Shield;
        let mut shield_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shield)>();
        for (name, sh) in q.iter(world) {
            shield_map.insert(name.0.clone(), (sh.current, sh.max));
        }
        SHIELD_SNAPSHOT.with(|s| *s.borrow_mut() = shield_map);
    }
    {
        use bsengine_core::Experience;
        let mut xp_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Experience)>();
        for (name, xp) in q.iter(world) {
            xp_map.insert(
                name.0.clone(),
                (
                    xp.level as f32,
                    xp.current_xp,
                    xp.progress(),
                    xp.is_max_level(),
                ),
            );
        }
        EXPERIENCE_SNAPSHOT.with(|s| *s.borrow_mut() = xp_map);
    }
    {
        use bsengine_core::Level;
        let mut lvl_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Level)>();
        for (name, lvl) in q.iter(world) {
            lvl_map.insert(
                name.0.clone(),
                (
                    lvl.current as f32,
                    lvl.max as f32,
                    lvl.prestige_level as f32,
                    lvl.is_max_level(),
                    lvl.progress_fraction(),
                ),
            );
        }
        LEVEL_SNAPSHOT.with(|s| *s.borrow_mut() = lvl_map);
    }
    {
        use bsengine_core::Cooldown;
        let mut cd_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Cooldown)>();
        for (name, cd) in q.iter(world) {
            cd_map.insert(name.0.clone(), (cd.remaining, cd.progress(), cd.is_ready()));
        }
        COOLDOWN_SNAPSHOT.with(|s| *s.borrow_mut() = cd_map);
    }
    {
        use bsengine_core::Timer;
        let mut timer_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Timer)>();
        for (name, t) in q.iter(world) {
            timer_map.insert(
                name.0.clone(),
                (
                    t.elapsed(),
                    t.duration(),
                    t.fraction(),
                    t.is_finished(),
                    t.just_finished(),
                ),
            );
        }
        TIMER_SNAPSHOT.with(|s| *s.borrow_mut() = timer_map);
    }
    {
        use bsengine_core::Ammo;
        let mut ammo_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Ammo)>();
        for (name, a) in q.iter(world) {
            ammo_map.insert(
                name.0.clone(),
                (
                    a.current,
                    a.max_capacity,
                    a.reserve,
                    a.reserve_max,
                    a.just_emptied,
                    a.just_reloaded,
                    a.enabled,
                ),
            );
        }
        AMMO_SNAPSHOT.with(|s| *s.borrow_mut() = ammo_map);
    }
    {
        use bsengine_core::Regen;
        let mut regen_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Regen)>();
        for (name, r) in q.iter(world) {
            regen_map.insert(
                name.0.clone(),
                (r.rate, r.delay_after_damage, r.delay_timer, r.enabled),
            );
        }
        REGEN_SNAPSHOT.with(|s| *s.borrow_mut() = regen_map);
    }
    {
        use bsengine_core::Fuel;
        let mut fuel_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Fuel)>();
        for (name, f) in q.iter(world) {
            fuel_map.insert(
                name.0.clone(),
                (
                    f.fuel,
                    f.max_fuel,
                    f.low_threshold,
                    f.just_emptied,
                    f.is_low(),
                    f.enabled,
                ),
            );
        }
        FUEL_SNAPSHOT.with(|s| *s.borrow_mut() = fuel_map);
    }
    {
        use bsengine_core::Charge;
        let mut charge_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Charge)>();
        for (name, c) in q.iter(world) {
            charge_map.insert(
                name.0.clone(),
                (
                    c.current,
                    c.max_charge,
                    c.is_charging(),
                    c.is_fully_charged(),
                    c.enabled,
                ),
            );
        }
        CHARGE_SNAPSHOT.with(|s| *s.borrow_mut() = charge_map);
    }
    {
        use bsengine_core::Armor;
        let mut armor_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Armor)>();
        for (name, a) in q.iter(world) {
            armor_map.insert(
                name.0.clone(),
                (
                    a.flat_reduction,
                    a.percent_reduction,
                    a.durability,
                    a.max_durability,
                    a.enabled,
                ),
            );
        }
        ARMOR_SNAPSHOT.with(|s| *s.borrow_mut() = armor_map);
    }
    {
        use bsengine_core::Jump;
        let mut jump_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Jump)>();
        for (name, j) in q.iter(world) {
            jump_map.insert(
                name.0.clone(),
                (
                    j.impulse,
                    j.max_jumps,
                    j.jumps_remaining,
                    j.wants_jump,
                    j.enabled,
                ),
            );
        }
        JUMP_SNAPSHOT.with(|s| *s.borrow_mut() = jump_map);
    }
    {
        use bsengine_core::Sprint;
        let mut sprint_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Sprint)>();
        for (name, sp) in q.iter(world) {
            sprint_map.insert(
                name.0.clone(),
                (
                    sp.speed_multiplier,
                    sp.is_sprinting(),
                    sp.is_exhausted(),
                    sp.just_started,
                    sp.just_stopped,
                    sp.enabled,
                ),
            );
        }
        SPRINT_SNAPSHOT.with(|s| *s.borrow_mut() = sprint_map);
    }
    {
        use bsengine_core::Dash;
        let mut dash_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Dash)>();
        for (name, d) in q.iter(world) {
            dash_map.insert(
                name.0.clone(),
                (
                    d.speed,
                    d.duration,
                    d.cooldown,
                    d.cooldown_timer,
                    d.max_charges,
                    d.charges,
                    d.is_active(),
                    d.is_invincible(),
                    d.can_dash(),
                    d.enabled,
                ),
            );
        }
        DASH_SNAPSHOT.with(|s| *s.borrow_mut() = dash_map);
    }
    {
        use bsengine_core::{NavAgentState, NavMeshAgent};
        let mut nav_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &NavMeshAgent)>();
        for (name, a) in q.iter(world) {
            let state_u8 = match a.state {
                NavAgentState::Idle => 0u8,
                NavAgentState::Moving => 1u8,
                NavAgentState::Arrived => 2u8,
                NavAgentState::NoPath => 3u8,
            };
            nav_map.insert(
                name.0.clone(),
                (
                    a.speed,
                    a.angular_speed,
                    a.stopping_distance,
                    state_u8,
                    a.enabled,
                ),
            );
        }
        NAV_SNAPSHOT.with(|s| *s.borrow_mut() = nav_map);
    }
    {
        use bsengine_core::Knockback;
        let mut kb_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Knockback)>();
        for (name, k) in q.iter(world) {
            kb_map.insert(
                name.0.clone(),
                (
                    k.force,
                    k.vertical_boost,
                    k.hits_remaining,
                    k.blocks_new,
                    k.enabled,
                ),
            );
        }
        KNOCKBACK_SNAPSHOT.with(|s| *s.borrow_mut() = kb_map);
    }
    {
        use bsengine_core::Projectile;
        let mut proj_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Projectile)>();
        for (name, p) in q.iter(world) {
            proj_map.insert(
                name.0.clone(),
                (
                    p.speed,
                    p.gravity_scale,
                    p.piercing,
                    p.range,
                    p.distance_traveled,
                ),
            );
        }
        PROJECTILE_SNAPSHOT.with(|s| *s.borrow_mut() = proj_map);
    }
    {
        use bsengine_core::{Grapple, GrappleState};
        let mut grapple_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Grapple)>();
        for (name, g) in q.iter(world) {
            let state_u8 = match g.state {
                GrappleState::Idle => 0u8,
                GrappleState::InFlight => 1u8,
                GrappleState::Attached => 2u8,
                GrappleState::Retracting => 3u8,
            };
            grapple_map.insert(
                name.0.clone(),
                (
                    state_u8,
                    g.anchor_point.x,
                    g.anchor_point.y,
                    g.anchor_point.z,
                    g.max_range,
                    g.hook_speed,
                    g.pull_force,
                    g.rope_length,
                    g.enabled,
                ),
            );
        }
        GRAPPLE_SNAPSHOT.with(|s| *s.borrow_mut() = grapple_map);
    }
    {
        use bsengine_core::{InteractTrigger, Interactable};
        let mut ia_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Interactable)>();
        for (name, i) in q.iter(world) {
            let trigger_u8 = match i.trigger {
                InteractTrigger::OnPress => 0u8,
                InteractTrigger::OnRelease => 1u8,
                InteractTrigger::OnHold => 2u8,
            };
            ia_map.insert(
                name.0.clone(),
                (
                    i.range,
                    i.prompt.clone(),
                    trigger_u8,
                    i.hold_duration,
                    i.enabled,
                ),
            );
        }
        INTERACTABLE_SNAPSHOT.with(|s| *s.borrow_mut() = ia_map);
    }
    {
        use bsengine_core::ScreenShake;
        let mut ss_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &ScreenShake)>();
        for (name, ss) in q.iter(world) {
            ss_map.insert(
                name.0.clone(),
                (ss.trauma, ss.amplitude, ss.decay_rate, ss.frequency),
            );
        }
        SCREEN_SHAKE_SNAPSHOT.with(|s| *s.borrow_mut() = ss_map);
    }
    {
        use bsengine_core::{Footstep, SurfaceKind};
        let mut fs_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Footstep)>();
        for (name, f) in q.iter(world) {
            let surface_u8 = match &f.surface {
                SurfaceKind::Concrete => 0u8,
                SurfaceKind::Grass => 1u8,
                SurfaceKind::Wood => 2u8,
                SurfaceKind::Metal => 3u8,
                SurfaceKind::Sand => 4u8,
                SurfaceKind::Water => 5u8,
                SurfaceKind::Gravel => 6u8,
                SurfaceKind::Custom(_) => 7u8,
            };
            fs_map.insert(
                name.0.clone(),
                (
                    f.step_interval,
                    f.distance_accumulated,
                    f.volume,
                    f.audio_prefix.clone(),
                    surface_u8,
                    f.min_speed,
                    f.enabled,
                ),
            );
        }
        FOOTSTEP_SNAPSHOT.with(|s| *s.borrow_mut() = fs_map);
    }
    {
        use bsengine_core::Wind;
        let mut wind_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Wind)>();
        for (name, w) in q.iter(world) {
            wind_map.insert(
                name.0.clone(),
                (
                    w.velocity.x,
                    w.velocity.y,
                    w.velocity.z,
                    w.turbulence,
                    w.turbulence_frequency,
                    w.radius,
                ),
            );
        }
        WIND_SNAPSHOT.with(|s| *s.borrow_mut() = wind_map);
    }
    {
        use bsengine_core::Dialogue;
        let mut dlg_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Dialogue)>();
        for (name, d) in q.iter(world) {
            let (speaker, text) = d
                .current_line()
                .map(|l| (l.speaker.clone(), l.text.clone()))
                .unwrap_or_default();
            dlg_map.insert(
                name.0.clone(),
                (
                    d.current_index as u32,
                    d.lines.len() as u32,
                    d.looping,
                    d.enabled,
                    d.is_finished(),
                    speaker,
                    text,
                ),
            );
        }
        DIALOGUE_SNAPSHOT.with(|s| *s.borrow_mut() = dlg_map);
    }
    {
        use bsengine_core::Dissolve;
        let mut diss_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Dissolve)>();
        for (name, d) in q.iter(world) {
            diss_map.insert(
                name.0.clone(),
                (
                    d.progress,
                    d.edge_width,
                    d.edge_color[0],
                    d.edge_color[1],
                    d.edge_color[2],
                    d.edge_color[3],
                    d.noise_scale,
                    d.enabled,
                ),
            );
        }
        DISSOLVE_SNAPSHOT.with(|s| *s.borrow_mut() = diss_map);
    }
    {
        use bsengine_core::Emissive;
        let mut em_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Emissive)>();
        for (name, e) in q.iter(world) {
            em_map.insert(
                name.0.clone(),
                (
                    e.color[0],
                    e.color[1],
                    e.color[2],
                    e.color[3],
                    e.intensity,
                    e.contributes_to_bloom,
                    e.enabled,
                ),
            );
        }
        EMISSIVE_SNAPSHOT.with(|s| *s.borrow_mut() = em_map);
    }
    {
        use bsengine_core::GridSnap;
        let mut gs_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &GridSnap)>();
        for (name, g) in q.iter(world) {
            gs_map.insert(
                name.0.clone(),
                (
                    g.cell_size.x,
                    g.cell_size.y,
                    g.cell_size.z,
                    g.offset.x,
                    g.offset.y,
                    g.offset.z,
                    g.enabled,
                ),
            );
        }
        GRID_SNAP_SNAPSHOT.with(|s| *s.borrow_mut() = gs_map);
    }
    {
        use bsengine_core::{Crosshair, CrosshairStyle};
        let mut ch_map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Crosshair)>();
        for (name, c) in q.iter(world) {
            let style_u8 = match c.style {
                CrosshairStyle::Cross => 0u8,
                CrosshairStyle::Circle => 1u8,
                CrosshairStyle::CrossCircle => 2u8,
                CrosshairStyle::Dot => 3u8,
            };
            ch_map.insert(
                name.0.clone(),
                (
                    style_u8,
                    c.color[0],
                    c.color[1],
                    c.color[2],
                    c.color[3],
                    c.size,
                    c.thickness,
                    c.gap,
                    c.spread,
                    c.max_spread,
                    c.spread_decay,
                    c.enabled,
                ),
            );
        }
        CROSSHAIR_SNAPSHOT.with(|s| *s.borrow_mut() = ch_map);
    }
    {
        use bsengine_core::Bloom;
        let mut bl_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Bloom)>();
        for (_, name, b) in q.iter(world) {
            bl_map.insert(
                name.0.clone(),
                (b.intensity, b.threshold, b.radius, b.softness, b.enabled),
            );
        }
        BLOOM_SNAPSHOT.with(|s| *s.borrow_mut() = bl_map);
    }
    {
        use bsengine_core::{Tint, TintMode};
        let mut tint_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Tint)>();
        for (_, name, t) in q.iter(world) {
            let mode_u32 = match t.mode {
                TintMode::Constant => 0u32,
                TintMode::Fading => 1u32,
                TintMode::Pulsing => 2u32,
            };
            tint_map.insert(
                name.0.clone(),
                (
                    t.color.x,
                    t.color.y,
                    t.color.z,
                    t.intensity,
                    mode_u32,
                    t.fade_rate,
                    t.pulse_speed,
                    t.pulse_phase,
                    t.peak_intensity,
                    t.enabled,
                ),
            );
        }
        TINT_SNAPSHOT.with(|s| *s.borrow_mut() = tint_map);
    }
    {
        use bsengine_core::ChromaticAberration;
        let mut ca_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ChromaticAberration)>();
        for (_, name, ca) in q.iter(world) {
            ca_map.insert(name.0.clone(), (ca.intensity, ca.enabled));
        }
        CHROM_AB_SNAPSHOT.with(|s| *s.borrow_mut() = ca_map);
    }
    {
        use bsengine_core::ColorGrading;
        let mut cg_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ColorGrading)>();
        for (_, name, cg) in q.iter(world) {
            cg_map.insert(
                name.0.clone(),
                (
                    cg.lut_path.clone().unwrap_or_default(),
                    cg.exposure,
                    cg.contrast,
                    cg.saturation,
                    cg.hue_shift,
                    cg.brightness,
                    cg.enabled,
                ),
            );
        }
        COLOR_GRADING_SNAPSHOT.with(|s| *s.borrow_mut() = cg_map);
    }
    {
        use bsengine_core::Absorption;
        let mut ab_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Absorption)>();
        for (_, name, ab) in q.iter(world) {
            ab_map.insert(
                name.0.clone(),
                (
                    ab.fraction,
                    ab.pool,
                    ab.max_pool,
                    ab.absorbed_total,
                    ab.just_depleted,
                    ab.enabled,
                ),
            );
        }
        ABSORPTION_SNAPSHOT.with(|s| *s.borrow_mut() = ab_map);
    }
    {
        use bsengine_core::AmbientOcclusion;
        let mut ao_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &AmbientOcclusion)>();
        for (_, name, ao) in q.iter(world) {
            ao_map.insert(
                name.0.clone(),
                (
                    ao.radius,
                    ao.bias,
                    ao.intensity,
                    ao.sample_count,
                    ao.enabled,
                ),
            );
        }
        AMBIENT_OCCLUSION_SNAPSHOT.with(|s| *s.borrow_mut() = ao_map);
    }
    {
        use bsengine_core::DepthOfField;
        let mut dof_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &DepthOfField)>();
        for (_, name, dof) in q.iter(world) {
            dof_map.insert(
                name.0.clone(),
                (
                    dof.focal_distance,
                    dof.focal_range,
                    dof.max_blur,
                    dof.bokeh_scale,
                    dof.enabled,
                ),
            );
        }
        DEPTH_OF_FIELD_SNAPSHOT.with(|s| *s.borrow_mut() = dof_map);
    }
    {
        use bsengine_core::MotionBlur;
        let mut mb_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &MotionBlur)>();
        for (_, name, mb) in q.iter(world) {
            mb_map.insert(
                name.0.clone(),
                (mb.shutter_angle, mb.sample_count, mb.enabled),
            );
        }
        MOTION_BLUR_SNAPSHOT.with(|s| *s.borrow_mut() = mb_map);
    }
    {
        use bsengine_core::{ToneMap, ToneMappingMode};
        let mut tm_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ToneMap)>();
        for (_, name, tm) in q.iter(world) {
            let mode_u32 = match tm.mode {
                ToneMappingMode::None => 0u32,
                ToneMappingMode::Reinhard => 1,
                ToneMappingMode::ReinhardLuminance => 2,
                ToneMappingMode::Aces => 3,
                ToneMappingMode::Filmic => 4,
            };
            tm_map.insert(name.0.clone(), (mode_u32, tm.exposure, tm.enabled));
        }
        TONE_MAP_SNAPSHOT.with(|s| *s.borrow_mut() = tm_map);
    }
    {
        use bsengine_core::Vignette;
        let mut vg_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Vignette)>();
        for (_, name, vg) in q.iter(world) {
            vg_map.insert(
                name.0.clone(),
                (
                    vg.intensity,
                    vg.smoothness,
                    vg.color[0],
                    vg.color[1],
                    vg.color[2],
                    vg.enabled,
                ),
            );
        }
        VIGNETTE_SNAPSHOT.with(|s| *s.borrow_mut() = vg_map);
    }
    {
        use bsengine_core::{Fog, FogMode};
        let mut fog_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Fog)>();
        for (_, name, fog) in q.iter(world) {
            let mode_u32 = match fog.mode {
                FogMode::Linear => 0u32,
                FogMode::Exponential => 1u32,
                FogMode::ExponentialSquared => 2u32,
            };
            fog_map.insert(
                name.0.clone(),
                (
                    fog.color.r,
                    fog.color.g,
                    fog.color.b,
                    fog.color.a,
                    fog.density,
                    fog.start_distance,
                    fog.end_distance,
                    mode_u32,
                    fog.enabled,
                ),
            );
        }
        FOG_SNAPSHOT.with(|s| *s.borrow_mut() = fog_map);
    }
    {
        use bsengine_core::Spring;
        let mut sp_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Spring)>();
        for (_, name, sp) in q.iter(world) {
            let target_name = ENTITY_NAME_MAP.with(|m| {
                m.borrow()
                    .get(&sp.target.to_bits())
                    .cloned()
                    .unwrap_or_default()
            });
            let break_ext = sp.break_extension.unwrap_or(-1.0);
            sp_map.insert(
                name.0.clone(),
                (
                    target_name,
                    sp.rest_length,
                    sp.stiffness,
                    sp.damping,
                    break_ext,
                    sp.enabled,
                ),
            );
        }
        SPRING_SNAPSHOT.with(|s| *s.borrow_mut() = sp_map);
    }
    {
        use bsengine_core::{EasingFn, RepeatMode, Tween, TweenTarget};
        let mut tw_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Tween)>();
        for (_, name, tw) in q.iter(world) {
            let target_type = match tw.target {
                TweenTarget::Translation { .. } => 0u32,
                TweenTarget::Rotation { .. } => 1u32,
                TweenTarget::Scale { .. } => 2u32,
            };
            let easing_u32 = match tw.easing {
                EasingFn::Linear => 0u32,
                EasingFn::EaseInQuad => 1u32,
                EasingFn::EaseOutQuad => 2u32,
                EasingFn::EaseInOutQuad => 3u32,
            };
            let repeat_u32 = match tw.repeat {
                RepeatMode::Once => 0u32,
                RepeatMode::Loop => 1u32,
                RepeatMode::PingPong => 2u32,
            };
            tw_map.insert(
                name.0.clone(),
                (
                    target_type,
                    tw.duration,
                    easing_u32,
                    repeat_u32,
                    tw.elapsed,
                    tw.finished,
                    tw.reversed,
                ),
            );
        }
        TWEEN_SNAPSHOT.with(|s| *s.borrow_mut() = tw_map);
    }
    {
        use bsengine_core::Buoyancy;
        let mut b_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Buoyancy)>();
        for (_, name, b) in q.iter(world) {
            b_map.insert(
                name.0.clone(),
                (
                    b.fluid_density,
                    b.volume,
                    b.linear_drag,
                    b.angular_drag,
                    b.surface_y,
                ),
            );
        }
        BUOYANCY_SNAPSHOT.with(|s| *s.borrow_mut() = b_map);
    }
    {
        use bsengine_core::Follow;
        let mut f_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Follow)>();
        for (_, name, f) in q.iter(world) {
            let target_name = ENTITY_NAME_MAP.with(|m| {
                m.borrow()
                    .get(&f.target.to_bits())
                    .cloned()
                    .unwrap_or_default()
            });
            f_map.insert(
                name.0.clone(),
                (target_name, f.offset.x, f.offset.y, f.offset.z, f.speed),
            );
        }
        FOLLOW_SNAPSHOT.with(|s| *s.borrow_mut() = f_map);
    }
    {
        use bsengine_core::LookAt;
        let mut la_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &LookAt)>();
        for (_, name, la) in q.iter(world) {
            let target_name = ENTITY_NAME_MAP.with(|m| {
                m.borrow()
                    .get(&la.target.to_bits())
                    .cloned()
                    .unwrap_or_default()
            });
            la_map.insert(name.0.clone(), (target_name, la.up.x, la.up.y, la.up.z));
        }
        LOOK_AT_SNAPSHOT.with(|s| *s.borrow_mut() = la_map);
    }
    {
        use bsengine_core::{Billboard, BillboardMode};
        let mut bb_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Billboard)>();
        for (_, name, bb) in q.iter(world) {
            let mode = match bb.mode {
                BillboardMode::Full => 0u32,
                BillboardMode::Vertical => 1u32,
            };
            bb_map.insert(name.0.clone(), mode);
        }
        BILLBOARD_SNAPSHOT.with(|s| *s.borrow_mut() = bb_map);
    }
    {
        use bsengine_core::{Outline, OutlineMode};
        let mut ol_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Outline)>();
        for (_, name, ol) in q.iter(world) {
            let mode = match ol.mode {
                OutlineMode::Outer => 0u32,
                OutlineMode::Inner => 1u32,
                OutlineMode::Center => 2u32,
            };
            ol_map.insert(
                name.0.clone(),
                (
                    ol.color.r, ol.color.g, ol.color.b, ol.color.a, ol.width, mode, ol.visible,
                ),
            );
        }
        OUTLINE_SNAPSHOT.with(|s| *s.borrow_mut() = ol_map);
    }
    {
        use bsengine_core::ZIndex;
        let mut zi_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ZIndex)>();
        for (_, name, zi) in q.iter(world) {
            zi_map.insert(name.0.clone(), zi.value());
        }
        Z_INDEX_SNAPSHOT.with(|s| *s.borrow_mut() = zi_map);
    }
    {
        use bsengine_core::Layer;
        let mut l_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Layer)>();
        for (_, name, l) in q.iter(world) {
            l_map.insert(name.0.clone(), l.bits());
        }
        LAYER_SNAPSHOT.with(|s| *s.borrow_mut() = l_map);
    }
    {
        use bsengine_core::{Anchor, AnchorPreset};
        let mut a_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Anchor)>();
        for (_, name, a) in q.iter(world) {
            let (preset_u32, norm_x, norm_y) = match a.preset {
                AnchorPreset::Center => (0u32, 0.5f32, 0.5f32),
                AnchorPreset::TopLeft => (1, 0.0, 1.0),
                AnchorPreset::TopCenter => (2, 0.5, 1.0),
                AnchorPreset::TopRight => (3, 1.0, 1.0),
                AnchorPreset::MiddleLeft => (4, 0.0, 0.5),
                AnchorPreset::MiddleRight => (5, 1.0, 0.5),
                AnchorPreset::BottomLeft => (6, 0.0, 0.0),
                AnchorPreset::BottomCenter => (7, 0.5, 0.0),
                AnchorPreset::BottomRight => (8, 1.0, 0.0),
                AnchorPreset::Custom(v) => (9, v.x, v.y),
            };
            a_map.insert(
                name.0.clone(),
                (preset_u32, norm_x, norm_y, a.offset.x, a.offset.y),
            );
        }
        ANCHOR_SNAPSHOT.with(|s| *s.borrow_mut() = a_map);
    }
    {
        use bsengine_core::Trigger;
        let mut t_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Trigger)>();
        for (_, name, t) in q.iter(world) {
            t_map.insert(name.0.clone(), (t.layer_mask, t.enabled));
        }
        TRIGGER_SNAPSHOT.with(|s| *s.borrow_mut() = t_map);
    }
    {
        use bsengine_core::{Damage, DamageType};
        let mut d_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Damage)>();
        for (_, name, d) in q.iter(world) {
            let (type_u32, custom_id) = match d.damage_type {
                DamageType::Physical => (0u32, 0u32),
                DamageType::Fire => (1, 0),
                DamageType::Ice => (2, 0),
                DamageType::Lightning => (3, 0),
                DamageType::Poison => (4, 0),
                DamageType::Custom(id) => (5, id),
            };
            d_map.insert(
                name.0.clone(),
                (d.amount, type_u32, custom_id, d.multiplier, d.piercing),
            );
        }
        DAMAGE_SNAPSHOT.with(|s| *s.borrow_mut() = d_map);
    }
    {
        use bsengine_core::SpawnPoint;
        let mut sp_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &SpawnPoint)>();
        for (_, name, sp) in q.iter(world) {
            let team_u32 = sp.team.unwrap_or(u32::MAX);
            sp_map.insert(name.0.clone(), (sp.tag.clone(), team_u32, sp.enabled));
        }
        SPAWN_POINT_SNAPSHOT.with(|s| *s.borrow_mut() = sp_map);
    }
    {
        use bsengine_core::{EffectKind, StatusEffect};
        let mut se_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &StatusEffect)>();
        for (_, name, se) in q.iter(world) {
            let (kind_u32, custom_id) = match se.kind {
                EffectKind::StatMultiplier => (0u32, 0u32),
                EffectKind::DamageOverTime => (1, 0),
                EffectKind::Immobilize => (2, 0),
                EffectKind::Silence => (3, 0),
                EffectKind::Custom(id) => (4, id),
            };
            se_map.insert(
                name.0.clone(),
                (
                    se.id.clone(),
                    kind_u32,
                    custom_id,
                    se.value,
                    se.duration,
                    se.ticks_every_frame,
                    se.enabled,
                ),
            );
        }
        STATUS_EFFECT_SNAPSHOT.with(|s| *s.borrow_mut() = se_map);
    }
    {
        use bsengine_core::Ability;
        let mut ab_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ability)>();
        for (_, name, ab) in q.iter(world) {
            ab_map.insert(
                name.0.clone(),
                (
                    ab.name.clone(),
                    ab.cooldown,
                    ab.cooldown_remaining,
                    ab.max_charges,
                    ab.charges,
                    ab.charge_regen_time,
                    ab.charge_regen_accumulated,
                    ab.enabled,
                ),
            );
        }
        ABILITY_SNAPSHOT.with(|s| *s.borrow_mut() = ab_map);
    }
    {
        use bsengine_core::Alarm;
        let mut al_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Alarm)>();
        for (_, name, al) in q.iter(world) {
            al_map.insert(
                name.0.clone(),
                (
                    al.alert_duration,
                    al.timer,
                    al.detection_radius,
                    al.just_triggered,
                    al.just_calmed,
                    al.enabled,
                ),
            );
        }
        ALARM_SNAPSHOT.with(|s| *s.borrow_mut() = al_map);
    }
    {
        use bsengine_core::Amplify;
        let mut amp_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Amplify)>();
        for (_, name, amp) in q.iter(world) {
            amp_map.insert(
                name.0.clone(),
                (
                    amp.duration,
                    amp.timer,
                    amp.power_multiplier,
                    amp.just_amplified,
                    amp.just_faded,
                    amp.enabled,
                ),
            );
        }
        AMPLIFY_SNAPSHOT.with(|s| *s.borrow_mut() = amp_map);
    }
    {
        use bsengine_core::Barrier;
        let mut bar_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Barrier)>();
        for (_, name, bar) in q.iter(world) {
            bar_map.insert(
                name.0.clone(),
                (
                    bar.capacity,
                    bar.current,
                    bar.regen_rate,
                    bar.regen_delay,
                    bar.regen_timer,
                    bar.just_broken,
                    bar.just_restored,
                    bar.enabled,
                ),
            );
        }
        BARRIER_SNAPSHOT.with(|s| *s.borrow_mut() = bar_map);
    }
    {
        use bsengine_core::Beacon;
        let mut bec_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Beacon)>();
        for (_, name, bec) in q.iter(world) {
            bec_map.insert(
                name.0.clone(),
                (
                    bec.priority,
                    bec.broadcast_radius,
                    bec.duration,
                    bec.timer,
                    bec.lit,
                    bec.just_lit,
                    bec.just_extinguished,
                    bec.enabled,
                ),
            );
        }
        BEACON_SNAPSHOT.with(|s| *s.borrow_mut() = bec_map);
    }
    {
        use bsengine_core::ShieldBreak;
        let mut sb_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &ShieldBreak)>();
        for (_, name, sb) in q.iter(world) {
            sb_map.insert(
                name.0.clone(),
                (
                    sb.duration,
                    sb.timer,
                    sb.reduction_fraction,
                    sb.just_broken,
                    sb.just_recovered,
                    sb.enabled,
                ),
            );
        }
        SHIELD_BREAK_SNAPSHOT.with(|s| *s.borrow_mut() = sb_map);
    }
    {
        use bsengine_core::Root;
        let mut root_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Root)>();
        for (_, name, root) in q.iter(world) {
            root_map.insert(
                name.0.clone(),
                (
                    root.duration,
                    root.timer,
                    root.allows_rotation,
                    root.allows_attack,
                    root.just_rooted,
                    root.just_freed,
                    root.enabled,
                ),
            );
        }
        ROOT_SNAPSHOT.with(|s| *s.borrow_mut() = root_map);
    }
    {
        use bsengine_core::Slow;
        let mut sl_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Slow)>();
        for (_, name, sl) in q.iter(world) {
            sl_map.insert(
                name.0.clone(),
                (
                    sl.reduction,
                    sl.duration,
                    sl.timer,
                    sl.just_slowed,
                    sl.just_recovered,
                    sl.enabled,
                ),
            );
        }
        SLOW_SNAPSHOT.with(|s| *s.borrow_mut() = sl_map);
    }
    {
        use bsengine_core::{Stun, StunSeverity};
        let mut st_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Stun)>();
        for (_, name, st) in q.iter(world) {
            let sev = match st.severity {
                StunSeverity::Light => 0u32,
                StunSeverity::Heavy => 1u32,
                StunSeverity::Knockdown => 2u32,
            };
            st_map.insert(
                name.0.clone(),
                (
                    sev,
                    st.timer,
                    st.just_stunned,
                    st.just_recovered,
                    st.enabled,
                ),
            );
        }
        STUN_SNAPSHOT.with(|s| *s.borrow_mut() = st_map);
    }
    {
        use bsengine_core::Burn;
        let mut bn_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Burn)>();
        for (_, name, bn) in q.iter(world) {
            bn_map.insert(
                name.0.clone(),
                (
                    bn.burn_rate,
                    bn.stacks,
                    bn.max_stacks,
                    bn.remaining,
                    bn.duration,
                    bn.intensity,
                    bn.just_ignited,
                    bn.just_extinguished,
                    bn.ignitable,
                    bn.enabled,
                ),
            );
        }
        BURN_SNAPSHOT.with(|s| *s.borrow_mut() = bn_map);
    }
    {
        use bsengine_core::Bleed;
        let mut bl_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Bleed)>();
        for (_, name, bl) in q.iter(world) {
            bl_map.insert(
                name.0.clone(),
                (
                    bl.stacks,
                    bl.max_stacks,
                    bl.damage_per_stack_per_tick,
                    bl.tick_interval,
                    bl.tick_timer,
                    bl.duration,
                    bl.duration_timer,
                    bl.heal_reduction,
                    bl.just_applied,
                    bl.just_cleared,
                    bl.enabled,
                ),
            );
        }
        BLEED_SNAPSHOT.with(|s| *s.borrow_mut() = bl_map);
    }
    {
        use bsengine_core::Poison;
        let mut po_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Poison)>();
        for (_, name, po) in q.iter(world) {
            po_map.insert(
                name.0.clone(),
                (
                    po.stacks,
                    po.max_stacks,
                    po.damage_per_stack_per_tick,
                    po.base_tick_interval,
                    po.min_tick_interval,
                    po.tick_timer,
                    po.duration,
                    po.duration_timer,
                    po.virulent,
                    po.just_poisoned,
                    po.just_cured,
                    po.enabled,
                ),
            );
        }
        POISON_SNAPSHOT.with(|s| *s.borrow_mut() = po_map);
    }
    {
        use bsengine_core::{Freeze, FreezeState};
        let mut fr_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Freeze)>();
        for (_, name, fr) in q.iter(world) {
            let state_u32 = match fr.state {
                FreezeState::Normal => 0u32,
                FreezeState::Chilled => 1u32,
                FreezeState::Frozen => 2u32,
            };
            fr_map.insert(
                name.0.clone(),
                (
                    state_u32,
                    fr.cold_buildup,
                    fr.chill_threshold,
                    fr.freeze_threshold,
                    fr.cold_decay_rate,
                    fr.chill_slow,
                    fr.frozen_duration,
                    fr.frozen_timer,
                    fr.just_frozen,
                    fr.just_thawed,
                    fr.immune,
                    fr.enabled,
                ),
            );
        }
        FREEZE_SNAPSHOT.with(|s| *s.borrow_mut() = fr_map);
    }
    {
        use bsengine_core::Blind;
        let mut bl_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Blind)>();
        for (_, name, bl) in q.iter(world) {
            bl_map.insert(
                name.0.clone(),
                (
                    bl.duration,
                    bl.timer,
                    bl.range_limit,
                    bl.aim_deviation_rad,
                    bl.just_blinded,
                    bl.just_unblinded,
                    bl.enabled,
                ),
            );
        }
        BLIND_SNAPSHOT.with(|s| *s.borrow_mut() = bl_map);
    }
    {
        use bsengine_core::Charm;
        let mut ch_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Charm)>();
        for (_, name, ch) in q.iter(world) {
            ch_map.insert(
                name.0.clone(),
                (
                    ch.duration,
                    ch.timer,
                    ch.just_charmed,
                    ch.just_uncharmed,
                    ch.enabled,
                ),
            );
        }
        CHARM_SNAPSHOT.with(|s| *s.borrow_mut() = ch_map);
    }
    {
        use bsengine_core::Confuse;
        let mut co_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Confuse)>();
        for (_, name, co) in q.iter(world) {
            co_map.insert(
                name.0.clone(),
                (
                    co.duration,
                    co.timer,
                    co.chance,
                    co.just_confused,
                    co.just_unconfused,
                    co.enabled,
                ),
            );
        }
        CONFUSE_SNAPSHOT.with(|s| *s.borrow_mut() = co_map);
    }
    {
        use bsengine_core::Cripple;
        let mut cr_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Cripple)>();
        for (_, name, cr) in q.iter(world) {
            cr_map.insert(
                name.0.clone(),
                (
                    cr.duration,
                    cr.timer,
                    cr.speed_fraction,
                    cr.prevents_jump,
                    cr.just_crippled,
                    cr.just_recovered,
                    cr.enabled,
                ),
            );
        }
        CRIPPLE_SNAPSHOT.with(|s| *s.borrow_mut() = cr_map);
    }
    {
        use bsengine_core::Daze;
        let mut dz_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Daze)>();
        for (_, name, dz) in q.iter(world) {
            dz_map.insert(
                name.0.clone(),
                (
                    dz.duration,
                    dz.timer,
                    dz.slow_fraction,
                    dz.aim_deviation_rad,
                    dz.just_dazed,
                    dz.just_undazed,
                    dz.enabled,
                ),
            );
        }
        DAZE_SNAPSHOT.with(|s| *s.borrow_mut() = dz_map);
    }
    {
        use bsengine_core::Disarm;
        let mut di_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Disarm)>();
        for (_, name, di) in q.iter(world) {
            di_map.insert(
                name.0.clone(),
                (
                    di.duration,
                    di.timer,
                    di.just_disarmed,
                    di.just_rearmed,
                    di.enabled,
                ),
            );
        }
        DISARM_SNAPSHOT.with(|s| *s.borrow_mut() = di_map);
    }
    {
        use bsengine_core::Concuss;
        let mut cn_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Concuss)>();
        for (_, name, cn) in q.iter(world) {
            cn_map.insert(
                name.0.clone(),
                (
                    cn.duration,
                    cn.timer,
                    cn.aim_deviation_rad,
                    cn.ability_suppress_chance,
                    cn.just_concussed,
                    cn.just_cleared,
                    cn.enabled,
                ),
            );
        }
        CONCUSS_SNAPSHOT.with(|s| *s.borrow_mut() = cn_map);
    }
    {
        use bsengine_core::Corrosion;
        let mut co_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Corrosion)>();
        for (_, name, co) in q.iter(world) {
            co_map.insert(
                name.0.clone(),
                (
                    co.stacks,
                    co.max_stacks,
                    co.decay_rate,
                    co.armor_reduction_per_stack,
                    co.just_corroded,
                    co.just_cleared,
                    co.enabled,
                ),
            );
        }
        CORROSION_SNAPSHOT.with(|s| *s.borrow_mut() = co_map);
    }
    {
        use bsengine_core::Curse;
        let mut cu_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Curse)>();
        for (_, name, cu) in q.iter(world) {
            cu_map.insert(
                name.0.clone(),
                (
                    cu.kind as u32,
                    cu.strength,
                    cu.duration,
                    cu.timer,
                    cu.just_cursed,
                    cu.just_lifted,
                    cu.enabled,
                ),
            );
        }
        CURSE_SNAPSHOT.with(|s| *s.borrow_mut() = cu_map);
    }
    {
        use bsengine_core::Dread;
        let mut dr_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Dread)>();
        for (_, name, dr) in q.iter(world) {
            dr_map.insert(
                name.0.clone(),
                (
                    dr.radius,
                    dr.pulse_interval,
                    dr.pulse_timer,
                    dr.buildup_per_pulse,
                    dr.just_pulsed,
                    dr.enabled,
                ),
            );
        }
        DREAD_SNAPSHOT.with(|s| *s.borrow_mut() = dr_map);
    }
    {
        use bsengine_core::Doom;
        let mut dm_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Doom)>();
        for (_, name, dm) in q.iter(world) {
            dm_map.insert(
                name.0.clone(),
                (
                    dm.active,
                    dm.countdown,
                    dm.max_countdown,
                    dm.just_doomed,
                    dm.just_expired,
                    dm.enabled,
                ),
            );
        }
        DOOM_SNAPSHOT.with(|s| *s.borrow_mut() = dm_map);
    }
    {
        use bsengine_core::Demoralize;
        let mut de_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Demoralize)>();
        for (_, name, de) in q.iter(world) {
            de_map.insert(
                name.0.clone(),
                (
                    de.duration,
                    de.timer,
                    de.damage_fraction,
                    de.flee_chance,
                    de.just_demoralized,
                    de.just_recovered,
                    de.enabled,
                ),
            );
        }
        DEMORALIZE_SNAPSHOT.with(|s| *s.borrow_mut() = de_map);
    }
    {
        use bsengine_core::{Dodge, DodgePhase};
        let mut dg_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Dodge)>();
        for (_, name, dg) in q.iter(world) {
            let phase_u32 = match dg.phase {
                DodgePhase::Idle => 0u32,
                DodgePhase::Rolling => 1u32,
                DodgePhase::Cooldown => 2u32,
            };
            dg_map.insert(
                name.0.clone(),
                (
                    phase_u32,
                    dg.direction.x,
                    dg.direction.y,
                    dg.direction.z,
                    dg.speed,
                    dg.duration,
                    dg.timer,
                    dg.invincible,
                    dg.cooldown,
                    dg.wants_dodge,
                    dg.allow_airborne,
                    dg.chain_count,
                    dg.max_chain,
                    dg.enabled,
                ),
            );
        }
        DODGE_SNAPSHOT.with(|s| *s.borrow_mut() = dg_map);
    }
    {
        use bsengine_core::Drain;
        let mut dr_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Drain)>();
        for (_, name, dr) in q.iter(world) {
            dr_map.insert(
                name.0.clone(),
                (
                    dr.rate,
                    dr.duration,
                    dr.timer,
                    dr.just_drained,
                    dr.just_expired,
                    dr.enabled,
                ),
            );
        }
        DRAIN_SNAPSHOT.with(|s| *s.borrow_mut() = dr_map);
    }
    {
        use bsengine_core::Empower;
        let mut em_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Empower)>();
        for (_, name, em) in q.iter(world) {
            em_map.insert(
                name.0.clone(),
                (
                    em.duration,
                    em.timer,
                    em.potency_multiplier,
                    em.just_empowered,
                    em.just_faded,
                    em.enabled,
                ),
            );
        }
        EMPOWER_SNAPSHOT.with(|s| *s.borrow_mut() = em_map);
    }
    {
        use bsengine_core::Enervate;
        let mut en_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Enervate)>();
        for (_, name, en) in q.iter(world) {
            en_map.insert(
                name.0.clone(),
                (
                    en.duration,
                    en.timer,
                    en.regen_fraction,
                    en.max_pool_fraction,
                    en.just_enervated,
                    en.just_restored,
                    en.enabled,
                ),
            );
        }
        ENERVATE_SNAPSHOT.with(|s| *s.borrow_mut() = en_map);
    }
    {
        use bsengine_core::Entangle;
        let mut et_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Entangle)>();
        for (_, name, et) in q.iter(world) {
            et_map.insert(
                name.0.clone(),
                (
                    et.duration,
                    et.timer,
                    et.just_entangled,
                    et.just_unentangled,
                    et.enabled,
                ),
            );
        }
        ENTANGLE_SNAPSHOT.with(|s| *s.borrow_mut() = et_map);
    }
    {
        use bsengine_core::Expose;
        let mut ex_map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Expose)>();
        for (_, name, ex) in q.iter(world) {
            ex_map.insert(
                name.0.clone(),
                (
                    ex.duration,
                    ex.timer,
                    ex.damage_multiplier,
                    ex.just_exposed,
                    ex.just_recovered,
                    ex.enabled,
                ),
            );
        }
        EXPOSE_SNAPSHOT.with(|s| *s.borrow_mut() = ex_map);
    }
    {
        use bsengine_core::Exhaustion;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Exhaustion)>();
        for (_, name, ex) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ex.level,
                    ex.recovery_rate,
                    ex.threshold,
                    ex.penalty_speed,
                    ex.penalty_regen,
                    ex.just_exhausted,
                    ex.just_recovered,
                    ex.enabled,
                ),
            );
        }
        EXHAUSTION_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Fear, FearState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Fear)>();
        for (_, name, fe) in q.iter(world) {
            let state_u32 = match fe.state {
                FearState::Calm => 0u32,
                FearState::Frightened => 1u32,
                FearState::Fleeing => 2u32,
            };
            map.insert(
                name.0.clone(),
                (
                    state_u32,
                    fe.duration,
                    fe.timer,
                    fe.flee_speed_multiplier,
                    fe.just_feared,
                    fe.just_calmed,
                    fe.enabled,
                ),
            );
        }
        FEAR_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Fracture;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Fracture)>();
        for (_, name, fr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    fr.duration,
                    fr.timer,
                    fr.damage_amplification,
                    fr.move_speed_penalty,
                    fr.just_fractured,
                    fr.just_healed,
                    fr.enabled,
                ),
            );
        }
        FRACTURE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Frostbite;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Frostbite)>();
        for (_, name, fb) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    fb.duration,
                    fb.timer,
                    fb.cold_damage_per_second,
                    fb.action_speed_fraction,
                    fb.just_frostbitten,
                    fb.just_thawed,
                    fb.enabled,
                ),
            );
        }
        FROSTBITE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Fury;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Fury)>();
        for (_, name, fu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    fu.fury_factor,
                    fu.max_speed_bonus,
                    fu.just_peaked,
                    fu.enabled,
                ),
            );
        }
        FURY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Galvanize;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Galvanize)>();
        for (_, name, gv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    gv.duration,
                    gv.timer,
                    gv.speed_multiplier,
                    gv.just_galvanized,
                    gv.just_worn_off,
                    gv.enabled,
                ),
            );
        }
        GALVANIZE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Haste;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Haste)>();
        for (_, name, hs) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    hs.effective_multiplier(),
                    hs.stack_count() as u32,
                    hs.max_stacks as u32,
                    hs.enabled,
                ),
            );
        }
        HASTE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Havoc;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Havoc)>();
        for (_, name, hv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    hv.duration,
                    hv.timer,
                    hv.stray_chance,
                    hv.damage_multiplier,
                    hv.just_entered,
                    hv.just_exited,
                    hv.enabled,
                ),
            );
        }
        HAVOC_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Haze;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Haze)>();
        for (_, name, hz) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    hz.duration,
                    hz.timer,
                    hz.detection_range_fraction,
                    hz.just_hazed,
                    hz.just_cleared,
                    hz.enabled,
                ),
            );
        }
        HAZE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Heat, ThermalState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Heat)>();
        for (_, name, ht) in q.iter(world) {
            let state_u32 = match ht.state {
                ThermalState::Normal => 0u32,
                ThermalState::Overheated => 1u32,
                ThermalState::Frozen => 2u32,
            };
            map.insert(
                name.0.clone(),
                (
                    ht.temperature,
                    ht.resting_temp,
                    ht.heat_threshold,
                    ht.cold_threshold,
                    ht.decay_rate,
                    ht.resistance,
                    state_u32,
                    ht.enabled,
                ),
            );
        }
        HEAT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Hex;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Hex)>();
        for (_, name, hx) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    hx.stacks,
                    hx.max_stacks,
                    hx.duration,
                    hx.timer,
                    hx.reduction_per_stack,
                    hx.just_applied,
                    hx.just_expired,
                    hx.enabled,
                ),
            );
        }
        HEX_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Hobble;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Hobble)>();
        for (_, name, ho) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ho.duration,
                    ho.timer,
                    ho.speed_fraction,
                    ho.prevents_dash,
                    ho.just_hobbled,
                    ho.just_recovered,
                    ho.enabled,
                ),
            );
        }
        HOBBLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ignite;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ignite)>();
        for (_, name, ig) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ig.stacks,
                    ig.threshold,
                    ig.decay_rate,
                    ig.just_ignited,
                    ig.just_extinguished,
                    ig.enabled,
                ),
            );
        }
        IGNITE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Imbue;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Imbue)>();
        for (_, name, im) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    im.charged,
                    im.bonus_damage,
                    im.just_charged,
                    im.just_consumed,
                    im.enabled,
                ),
            );
        }
        IMBUE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Immune;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Immune)>();
        for (_, name, im) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (im.damage_type_mask, im.effect_type_mask, im.enabled),
            );
        }
        IMMUNE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Impact;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Impact)>();
        for (_, name, ip) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ip.force,
                    ip.just_impacted,
                    ip.impact_count,
                    ip.normal.x,
                    ip.normal.y,
                    ip.normal.z,
                    ip.enabled,
                ),
            );
        }
        IMPACT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Intercept;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Intercept)>();
        for (_, name, ic) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ic.duration,
                    ic.timer,
                    ic.radius,
                    ic.damage_reduction,
                    ic.just_activated,
                    ic.just_deactivated,
                    ic.enabled,
                ),
            );
        }
        INTERCEPT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Interrupt;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Interrupt)>();
        for (_, name, ir) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ir.threshold,
                    ir.resistance,
                    ir.just_interrupted,
                    ir.interrupt_count,
                    ir.enabled,
                ),
            );
        }
        INTERRUPT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    COMMAND_BUFFER.with(|c| c.borrow_mut().clear());

    if let Some(mut rt) = world.get_non_send_resource_mut::<ScriptRuntimeResource>() {
        // Dispatch collision events to JS before update
        if collision_json != "[]" {
            let call = format!("Bsengine._runCollisions({collision_json});");
            if let Err(e) = rt.0.exec_source(&call, "<run_collisions>") {
                tracing::error!("[scripting] _runCollisions error: {e}");
            }
        }

        let entities_json = serde_json::to_string(&scripted).unwrap_or_else(|_| "[]".to_string());
        let call = format!("Bsengine._runAll({entities_json});");
        if let Err(e) = rt.0.exec_source(&call, "<run_scripts>") {
            tracing::error!("[scripting] _runAll error: {e}");
        }
    }

    // Clear physics pointer — must happen after all V8 execution is complete.
    PHYSICS_WORLD_PTR.with(|p| *p.borrow_mut() = std::ptr::null());

    let commands: Vec<ScriptCommand> = COMMAND_BUFFER.with(|c| c.borrow().clone());
    for cmd in commands {
        match cmd {
            ScriptCommand::SetTransform { name, x, y, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation = Vec3::new(x, y, z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotation {
                name,
                rx,
                ry,
                rz,
                rw,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.rotation = Quat::from_xyzw(rx, ry, rz, rw);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEuler {
                name,
                pitch_deg,
                yaw_deg,
                roll_deg,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.rotation = Quat::from_euler(
                            EulerRot::YXZ,
                            yaw_deg.to_radians(),
                            pitch_deg.to_radians(),
                            roll_deg.to_radians(),
                        );
                        break;
                    }
                }
            }
            ScriptCommand::SetScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale = Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPosition { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation += Vec3::new(dx, dy, dz);
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionLocal { name, dx, dy, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let rot = t.rotation;
                        t.translation += rot.mul_vec3(Vec3::new(dx, dy, dz));
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionX { name, x } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.x = x;
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionY { name, y } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.y = y;
                        break;
                    }
                }
            }
            ScriptCommand::SetPositionZ { name, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.z = z;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionX { name, dx } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.x += dx;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionY { name, dy } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.y += dy;
                        break;
                    }
                }
            }
            ScriptCommand::AddPositionZ { name, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.translation.z += dz;
                        break;
                    }
                }
            }
            ScriptCommand::SetEmissive { name, r, g, b } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.emissive = Vec3::new(r, g, b);
                    } else {
                        world.entity_mut(e).insert(Material {
                            emissive: Vec3::new(r, g, b),
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetColor { name, r, g, b } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.base_color = Vec3::new(r, g, b);
                    } else {
                        world.entity_mut(e).insert(Material {
                            base_color: Vec3::new(r, g, b),
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetMetallic { name, value } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.metallic = value;
                    } else {
                        world.entity_mut(e).insert(Material {
                            metallic: value,
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetRoughness { name, value } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mat) = world.get_mut::<Material>(e) {
                        mat.roughness = value;
                    } else {
                        world.entity_mut(e).insert(Material {
                            roughness: value,
                            ..Default::default()
                        });
                    }
                }
            }
            ScriptCommand::SetPointLightColor { name, r, g, b } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.color = glam::Vec3::new(r, g, b);
                    }
                }
            }
            ScriptCommand::SetPointLightIntensity { name, value } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.intensity = value;
                    }
                }
            }
            ScriptCommand::SetPointLightRange { name, value } => {
                use bsengine_core::PointLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<PointLight>(e) {
                        light.range = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightColor { name, r, g, b } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.color = glam::Vec3::new(r, g, b);
                    }
                }
            }
            ScriptCommand::SetSpotLightIntensity { name, value } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.intensity = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightRange { name, value } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.range = value;
                    }
                }
            }
            ScriptCommand::SetSpotLightInnerAngle { name, deg } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.inner_angle = deg.to_radians();
                    }
                }
            }
            ScriptCommand::SetSpotLightOuterAngle { name, deg } => {
                use bsengine_core::SpotLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<SpotLight>(e) {
                        light.outer_angle = deg.to_radians();
                    }
                }
            }
            ScriptCommand::SetDirectionalLightColor { name, r, g, b } => {
                use bsengine_core::DirectionalLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<DirectionalLight>(e) {
                        light.color = glam::Vec3::new(r, g, b);
                    }
                }
            }
            ScriptCommand::SetDirectionalLightAmbient { name, r, g, b } => {
                use bsengine_core::DirectionalLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<DirectionalLight>(e) {
                        light.ambient = glam::Vec3::new(r, g, b);
                    }
                }
            }
            ScriptCommand::SetDirectionalLightDirection { name, x, y, z } => {
                use bsengine_core::DirectionalLight;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut light) = world.get_mut::<DirectionalLight>(e) {
                        light.direction = glam::Vec3::new(x, y, z).normalize();
                    }
                }
            }
            ScriptCommand::SetCameraFov { name, deg } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.fov_y_radians = deg.to_radians();
                    }
                }
            }
            ScriptCommand::SetCameraNear { name, value } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.near = value;
                    }
                }
            }
            ScriptCommand::SetCameraFar { name, value } => {
                use bsengine_core::Camera;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cam) = world.get_mut::<Camera>(e) {
                        cam.far = value;
                    }
                }
            }
            ScriptCommand::SetDamping { name, value } => {
                use bsengine_core::Damping;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damping>(e) {
                        d.linear = value;
                    }
                }
            }
            ScriptCommand::DamageEntity { name, amount } => {
                use bsengine_core::Health;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut h) = world.get_mut::<Health>(e) {
                        h.damage(amount);
                    }
                }
            }
            ScriptCommand::HealEntity { name, amount } => {
                use bsengine_core::Health;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut h) = world.get_mut::<Health>(e) {
                        h.heal(amount);
                    }
                }
            }
            ScriptCommand::SetHealth { name, value } => {
                use bsengine_core::Health;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut h) = world.get_mut::<Health>(e) {
                        h.current = value.clamp(0.0, h.max);
                    }
                }
            }
            ScriptCommand::SetMaxHealth { name, value } => {
                use bsengine_core::Health;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut h) = world.get_mut::<Health>(e) {
                        h.max = value.max(0.0);
                        h.current = h.current.min(h.max);
                    }
                }
            }
            ScriptCommand::PlayAnimation { name, clip } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.clip = clip;
                        ap.time = 0.0;
                        ap.playing = true;
                    }
                }
            }
            ScriptCommand::PauseAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.pause();
                    }
                }
            }
            ScriptCommand::ResumeAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.play();
                    }
                }
            }
            ScriptCommand::ResetAnimation { name } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.reset();
                    }
                }
            }
            ScriptCommand::SetAnimationSpeed { name, speed } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.speed = speed;
                    }
                }
            }
            ScriptCommand::SetAnimationLooping { name, looping } => {
                use bsengine_core::AnimationPlayer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ap) = world.get_mut::<AnimationPlayer>(e) {
                        ap.looping = looping;
                    }
                }
            }
            ScriptCommand::SetLifetime { name, seconds } => {
                use bsengine_core::Lifetime;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lt) = world.get_mut::<Lifetime>(e) {
                        lt.remaining = seconds.max(0.0);
                    }
                }
            }
            ScriptCommand::SpendStamina { name, cost } => {
                use bsengine_core::Stamina;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stamina>(e) {
                        st.spend(cost);
                    }
                }
            }
            ScriptCommand::RestoreStamina { name, amount } => {
                use bsengine_core::Stamina;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stamina>(e) {
                        st.restore(amount);
                    }
                }
            }
            ScriptCommand::SetMaxStamina { name, value } => {
                use bsengine_core::Stamina;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stamina>(e) {
                        st.max = value.max(0.0);
                        st.current = st.current.min(st.max);
                    }
                }
            }
            ScriptCommand::SpendMana { name, cost } => {
                use bsengine_core::Mana;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mana>(e) {
                        mn.spend(cost);
                    }
                }
            }
            ScriptCommand::RestoreMana { name, amount } => {
                use bsengine_core::Mana;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mana>(e) {
                        mn.restore(amount);
                    }
                }
            }
            ScriptCommand::SetMaxMana { name, value } => {
                use bsengine_core::Mana;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mana>(e) {
                        mn.max = value.max(0.0);
                        mn.current = mn.current.min(mn.max);
                    }
                }
            }
            ScriptCommand::SetMoveSpeedBase { name, value } => {
                use bsengine_core::MoveSpeed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ms) = world.get_mut::<MoveSpeed>(e) {
                        ms.base = value.max(0.0);
                    }
                }
            }
            ScriptCommand::AddMoveSpeedFlat { name, amount } => {
                use bsengine_core::MoveSpeed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ms) = world.get_mut::<MoveSpeed>(e) {
                        ms.add_flat(amount);
                    }
                }
            }
            ScriptCommand::ScaleMoveSpeed { name, factor } => {
                use bsengine_core::MoveSpeed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ms) = world.get_mut::<MoveSpeed>(e) {
                        ms.scale(factor);
                    }
                }
            }
            ScriptCommand::DamageShield { name, amount } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.absorb(amount);
                    }
                }
            }
            ScriptCommand::RestoreShield { name, amount } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.current = (sh.current + amount.max(0.0)).min(sh.max);
                    }
                }
            }
            ScriptCommand::SetMaxShield { name, value } => {
                use bsengine_core::Shield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<Shield>(e) {
                        sh.max = value.max(0.0);
                        sh.current = sh.current.min(sh.max);
                    }
                }
            }
            ScriptCommand::AddXp { name, amount } => {
                use bsengine_core::Experience;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut xp) = world.get_mut::<Experience>(e) {
                        xp.add_xp(amount);
                    }
                }
            }
            ScriptCommand::LevelUp { name } => {
                use bsengine_core::Level;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lvl) = world.get_mut::<Level>(e) {
                        lvl.level_up();
                    }
                }
            }
            ScriptCommand::Prestige { name } => {
                use bsengine_core::Level;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lvl) = world.get_mut::<Level>(e) {
                        lvl.prestige();
                    }
                }
            }
            ScriptCommand::StartCooldown { name } => {
                use bsengine_core::Cooldown;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cd) = world.get_mut::<Cooldown>(e) {
                        cd.start();
                    }
                }
            }
            ScriptCommand::SetCooldownDuration { name, seconds } => {
                use bsengine_core::Cooldown;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cd) = world.get_mut::<Cooldown>(e) {
                        cd.duration = seconds.max(0.0);
                    }
                }
            }
            ScriptCommand::ResetTimer { name } => {
                use bsengine_core::Timer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Timer>(e) {
                        t.reset();
                    }
                }
            }
            ScriptCommand::FireAmmo { name } => {
                use bsengine_core::Ammo;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Ammo>(e) {
                        a.fire();
                    }
                }
            }
            ScriptCommand::ReloadAmmo { name } => {
                use bsengine_core::Ammo;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Ammo>(e) {
                        a.reload();
                    }
                }
            }
            ScriptCommand::AddAmmoReserve { name, amount } => {
                use bsengine_core::Ammo;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Ammo>(e) {
                        a.add_reserve(amount);
                    }
                }
            }
            ScriptCommand::SetAmmoEnabled { name, enabled } => {
                use bsengine_core::Ammo;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Ammo>(e) {
                        a.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRegenRate { name, rate } => {
                use bsengine_core::Regen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut r) = world.get_mut::<Regen>(e) {
                        r.rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetRegenDelay { name, seconds } => {
                use bsengine_core::Regen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut r) = world.get_mut::<Regen>(e) {
                        r.delay_after_damage = seconds.max(0.0);
                    }
                }
            }
            ScriptCommand::SetRegenEnabled { name, enabled } => {
                use bsengine_core::Regen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut r) = world.get_mut::<Regen>(e) {
                        r.enabled = enabled;
                    }
                }
            }
            ScriptCommand::NotifyRegenDamage { name } => {
                use bsengine_core::Regen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut r) = world.get_mut::<Regen>(e) {
                        r.notify_damage();
                    }
                }
            }
            ScriptCommand::Refuel { name, amount } => {
                use bsengine_core::Fuel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Fuel>(e) {
                        f.refuel(amount);
                    }
                }
            }
            ScriptCommand::SetMaxFuel { name, value } => {
                use bsengine_core::Fuel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Fuel>(e) {
                        f.max_fuel = value.max(0.0);
                        f.fuel = f.fuel.min(f.max_fuel);
                    }
                }
            }
            ScriptCommand::SetFuelEnabled { name, enabled } => {
                use bsengine_core::Fuel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Fuel>(e) {
                        f.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginCharge { name } => {
                use bsengine_core::Charge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Charge>(e) {
                        c.begin();
                    }
                }
            }
            ScriptCommand::ReleaseCharge { name } => {
                use bsengine_core::Charge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Charge>(e) {
                        c.release();
                    }
                }
            }
            ScriptCommand::CancelCharge { name } => {
                use bsengine_core::Charge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Charge>(e) {
                        c.cancel();
                    }
                }
            }
            ScriptCommand::SetChargeEnabled { name, enabled } => {
                use bsengine_core::Charge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Charge>(e) {
                        c.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetChargeRate { name, rate } => {
                use bsengine_core::Charge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Charge>(e) {
                        c.charge_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::RepairArmor { name, amount } => {
                use bsengine_core::Armor;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Armor>(e) {
                        a.repair(amount);
                    }
                }
            }
            ScriptCommand::SetArmorEnabled { name, enabled } => {
                use bsengine_core::Armor;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Armor>(e) {
                        a.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetArmorFlat { name, value } => {
                use bsengine_core::Armor;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Armor>(e) {
                        a.flat_reduction = value.max(0.0);
                    }
                }
            }
            ScriptCommand::SetArmorPercent { name, value } => {
                use bsengine_core::Armor;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Armor>(e) {
                        a.percent_reduction = value.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::PressJump { name } => {
                use bsengine_core::Jump;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut j) = world.get_mut::<Jump>(e) {
                        j.press();
                    }
                }
            }
            ScriptCommand::ReleaseJump { name } => {
                use bsengine_core::Jump;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut j) = world.get_mut::<Jump>(e) {
                        j.release();
                    }
                }
            }
            ScriptCommand::SetJumpEnabled { name, enabled } => {
                use bsengine_core::Jump;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut j) = world.get_mut::<Jump>(e) {
                        j.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetJumpImpulse { name, impulse } => {
                use bsengine_core::Jump;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut j) = world.get_mut::<Jump>(e) {
                        j.impulse = impulse;
                    }
                }
            }
            ScriptCommand::SetMaxJumps { name, max } => {
                use bsengine_core::Jump;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut j) = world.get_mut::<Jump>(e) {
                        j.max_jumps = max;
                    }
                }
            }
            ScriptCommand::BeginSprint { name } => {
                use bsengine_core::Sprint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Sprint>(e) {
                        sp.begin();
                    }
                }
            }
            ScriptCommand::EndSprint { name } => {
                use bsengine_core::Sprint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Sprint>(e) {
                        sp.end();
                    }
                }
            }
            ScriptCommand::SetSprintEnabled { name, enabled } => {
                use bsengine_core::Sprint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Sprint>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetSprintMultiplier { name, multiplier } => {
                use bsengine_core::Sprint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Sprint>(e) {
                        sp.speed_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::TriggerDash { name, dx, dy, dz } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.trigger(glam::Vec3::new(dx, dy, dz));
                    }
                }
            }
            ScriptCommand::SetDashEnabled { name, enabled } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetDashSpeed { name, speed } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDashDuration { name, duration } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDashCooldown { name, cooldown } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.cooldown = cooldown.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMaxDashCharges { name, max } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.max_charges = max.max(1);
                    }
                }
            }
            ScriptCommand::SetDashInvincible { name, enabled } => {
                use bsengine_core::Dash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dash>(e) {
                        d.invincible_during_dash = enabled;
                    }
                }
            }
            ScriptCommand::SetNavDestination { name, x, y, z } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.destination = Some(glam::Vec3::new(x, y, z));
                    }
                }
            }
            ScriptCommand::ClearNavDestination { name } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.clear_destination();
                    }
                }
            }
            ScriptCommand::SetNavSpeed { name, speed } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavAngularSpeed { name, speed } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.angular_speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavStoppingDistance { name, distance } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.stopping_distance = distance.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNavEnabled { name, enabled } => {
                use bsengine_core::NavMeshAgent;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<NavMeshAgent>(e) {
                        a.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyKnockbackDir {
                name,
                dx,
                dy,
                dz,
                force,
            } => {
                use bsengine_core::Knockback;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world
                        .entity_mut(e)
                        .insert(Knockback::from_direction(Vec3::new(dx, dy, dz), force));
                }
            }
            ScriptCommand::ApplyKnockbackFromPoint {
                name,
                ox,
                oy,
                oz,
                force,
            } => {
                use bsengine_core::Knockback;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world
                        .entity_mut(e)
                        .insert(Knockback::from_point(Vec3::new(ox, oy, oz), force));
                }
            }
            ScriptCommand::SetKnockbackEnabled { name, enabled } => {
                use bsengine_core::Knockback;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut k) = world.get_mut::<Knockback>(e) {
                        k.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetKnockbackVerticalBoost { name, boost } => {
                use bsengine_core::Knockback;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut k) = world.get_mut::<Knockback>(e) {
                        k.vertical_boost = boost;
                    }
                }
            }
            ScriptCommand::SetKnockbackHits { name, hits } => {
                use bsengine_core::Knockback;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut k) = world.get_mut::<Knockback>(e) {
                        k.hits_remaining = hits;
                    }
                }
            }
            ScriptCommand::SetProjectileSpeed { name, speed } => {
                use bsengine_core::Projectile;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut p) = world.get_mut::<Projectile>(e) {
                        p.speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetProjectileGravityScale { name, scale } => {
                use bsengine_core::Projectile;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut p) = world.get_mut::<Projectile>(e) {
                        p.gravity_scale = scale.max(0.0);
                    }
                }
            }
            ScriptCommand::SetProjectilePiercing { name, count } => {
                use bsengine_core::Projectile;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut p) = world.get_mut::<Projectile>(e) {
                        p.piercing = count;
                    }
                }
            }
            ScriptCommand::SetProjectileRange { name, range } => {
                use bsengine_core::Projectile;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut p) = world.get_mut::<Projectile>(e) {
                        p.range = range.max(0.0);
                    }
                }
            }
            ScriptCommand::FireGrapple { name } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.fire();
                    }
                }
            }
            ScriptCommand::RetractGrapple { name } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.retract();
                    }
                }
            }
            ScriptCommand::ResetGrapple { name } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.reset();
                    }
                }
            }
            ScriptCommand::SetGrappleMaxRange { name, range } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.max_range = range.max(0.0);
                    }
                }
            }
            ScriptCommand::SetGrappleHookSpeed { name, speed } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.hook_speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetGrapplePullForce { name, force } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.pull_force = force.max(0.0);
                    }
                }
            }
            ScriptCommand::SetGrappleEnabled { name, enabled } => {
                use bsengine_core::Grapple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<Grapple>(e) {
                        g.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetInteractableRange { name, range } => {
                use bsengine_core::Interactable;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut i) = world.get_mut::<Interactable>(e) {
                        i.range = range.max(0.0);
                    }
                }
            }
            ScriptCommand::SetInteractablePrompt { name, prompt } => {
                use bsengine_core::Interactable;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut i) = world.get_mut::<Interactable>(e) {
                        i.prompt = prompt;
                    }
                }
            }
            ScriptCommand::SetInteractableTrigger { name, trigger } => {
                use bsengine_core::{InteractTrigger, Interactable};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut i) = world.get_mut::<Interactable>(e) {
                        i.trigger = match trigger {
                            1 => InteractTrigger::OnRelease,
                            2 => InteractTrigger::OnHold,
                            _ => InteractTrigger::OnPress,
                        };
                    }
                }
            }
            ScriptCommand::SetInteractableHoldDuration { name, duration } => {
                use bsengine_core::Interactable;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut i) = world.get_mut::<Interactable>(e) {
                        i.hold_duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetInteractableEnabled { name, enabled } => {
                use bsengine_core::Interactable;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut i) = world.get_mut::<Interactable>(e) {
                        i.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddScreenShakeTrauma { name, amount } => {
                use bsengine_core::ScreenShake;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<ScreenShake>(e) {
                        ss.trauma = (ss.trauma + amount.max(0.0)).min(1.0);
                    }
                }
            }
            ScriptCommand::SetScreenShakeTrauma { name, trauma } => {
                use bsengine_core::ScreenShake;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<ScreenShake>(e) {
                        ss.trauma = trauma.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetScreenShakeAmplitude { name, amplitude } => {
                use bsengine_core::ScreenShake;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<ScreenShake>(e) {
                        ss.amplitude = amplitude.max(0.0);
                    }
                }
            }
            ScriptCommand::SetScreenShakeDecayRate { name, rate } => {
                use bsengine_core::ScreenShake;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<ScreenShake>(e) {
                        ss.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetScreenShakeFrequency { name, frequency } => {
                use bsengine_core::ScreenShake;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<ScreenShake>(e) {
                        ss.frequency = frequency.max(0.0);
                    }
                }
            }
            ScriptCommand::SetFootstepStepInterval { name, interval } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.step_interval = interval.max(0.0);
                    }
                }
            }
            ScriptCommand::SetFootstepVolume { name, volume } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.volume = volume.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetFootstepAudioPrefix { name, prefix } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.audio_prefix = prefix;
                    }
                }
            }
            ScriptCommand::SetFootstepSurface { name, surface } => {
                use bsengine_core::{Footstep, SurfaceKind};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.surface = match surface {
                            0 => SurfaceKind::Concrete,
                            1 => SurfaceKind::Grass,
                            2 => SurfaceKind::Wood,
                            3 => SurfaceKind::Metal,
                            4 => SurfaceKind::Sand,
                            5 => SurfaceKind::Water,
                            6 => SurfaceKind::Gravel,
                            _ => SurfaceKind::Custom(String::new()),
                        };
                    }
                }
            }
            ScriptCommand::SetFootstepMinSpeed { name, speed } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.min_speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetFootstepEnabled { name, enabled } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ResetFootstep { name } => {
                use bsengine_core::Footstep;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Footstep>(e) {
                        f.reset();
                    }
                }
            }
            ScriptCommand::SetWindVelocity { name, vx, vy, vz } => {
                use bsengine_core::Wind;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut w) = world.get_mut::<Wind>(e) {
                        w.velocity = Vec3::new(vx, vy, vz);
                    }
                }
            }
            ScriptCommand::SetWindTurbulence { name, turbulence } => {
                use bsengine_core::Wind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut w) = world.get_mut::<Wind>(e) {
                        w.turbulence = turbulence.max(0.0);
                    }
                }
            }
            ScriptCommand::SetWindTurbulenceFrequency { name, frequency } => {
                use bsengine_core::Wind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut w) = world.get_mut::<Wind>(e) {
                        w.turbulence_frequency = frequency.max(0.0);
                    }
                }
            }
            ScriptCommand::SetWindRadius { name, radius } => {
                use bsengine_core::Wind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut w) = world.get_mut::<Wind>(e) {
                        w.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::AdvanceDialogue { name } => {
                use bsengine_core::Dialogue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dialogue>(e) {
                        d.advance();
                    }
                }
            }
            ScriptCommand::ResetDialogue { name } => {
                use bsengine_core::Dialogue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dialogue>(e) {
                        d.reset();
                    }
                }
            }
            ScriptCommand::SetDialogueEnabled { name, enabled } => {
                use bsengine_core::Dialogue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dialogue>(e) {
                        d.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetDialogueLooping { name, looping } => {
                use bsengine_core::Dialogue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dialogue>(e) {
                        d.looping = looping;
                    }
                }
            }
            ScriptCommand::SetDialogueCurrentIndex { name, index } => {
                use bsengine_core::Dialogue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dialogue>(e) {
                        d.current_index = (index as usize).min(d.lines.len());
                    }
                }
            }
            ScriptCommand::SetDissolveProgress { name, progress } => {
                use bsengine_core::Dissolve;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dissolve>(e) {
                        d.progress = progress.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetDissolveEdgeWidth { name, width } => {
                use bsengine_core::Dissolve;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dissolve>(e) {
                        d.edge_width = width.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDissolveEdgeColor { name, r, g, b, a } => {
                use bsengine_core::Dissolve;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dissolve>(e) {
                        d.edge_color = [
                            r.clamp(0.0, 1.0),
                            g.clamp(0.0, 1.0),
                            b.clamp(0.0, 1.0),
                            a.clamp(0.0, 1.0),
                        ];
                    }
                }
            }
            ScriptCommand::SetDissolveNoiseScale { name, scale } => {
                use bsengine_core::Dissolve;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dissolve>(e) {
                        d.noise_scale = scale.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDissolveEnabled { name, enabled } => {
                use bsengine_core::Dissolve;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Dissolve>(e) {
                        d.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetEmissiveColor { name, r, g, b, a } => {
                use bsengine_core::Emissive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Emissive>(e) {
                        em.color = [r, g, b, a];
                    }
                }
            }
            ScriptCommand::SetEmissiveIntensity { name, intensity } => {
                use bsengine_core::Emissive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Emissive>(e) {
                        em.intensity = intensity.max(0.0);
                    }
                }
            }
            ScriptCommand::SetEmissiveContributesToBloom { name, value } => {
                use bsengine_core::Emissive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Emissive>(e) {
                        em.contributes_to_bloom = value;
                    }
                }
            }
            ScriptCommand::SetEmissiveEnabled { name, enabled } => {
                use bsengine_core::Emissive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Emissive>(e) {
                        em.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetGridSnapCellSize { name, vx, vy, vz } => {
                use bsengine_core::GridSnap;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<GridSnap>(e) {
                        g.cell_size = Vec3::new(vx, vy, vz).max(Vec3::ZERO);
                    }
                }
            }
            ScriptCommand::SetGridSnapOffset { name, x, y, z } => {
                use bsengine_core::GridSnap;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<GridSnap>(e) {
                        g.offset = Vec3::new(x, y, z);
                    }
                }
            }
            ScriptCommand::SetGridSnapEnabled { name, enabled } => {
                use bsengine_core::GridSnap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut g) = world.get_mut::<GridSnap>(e) {
                        g.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetCrosshairStyle { name, style } => {
                use bsengine_core::{Crosshair, CrosshairStyle};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.style = match style {
                            1 => CrosshairStyle::Circle,
                            2 => CrosshairStyle::CrossCircle,
                            3 => CrosshairStyle::Dot,
                            _ => CrosshairStyle::Cross,
                        };
                    }
                }
            }
            ScriptCommand::SetCrosshairColor { name, r, g, b, a } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.color = [r, g, b, a];
                    }
                }
            }
            ScriptCommand::SetCrosshairSize { name, size } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.size = size.max(0.0);
                    }
                }
            }
            ScriptCommand::SetCrosshairThickness { name, thickness } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.thickness = thickness.max(0.0);
                    }
                }
            }
            ScriptCommand::SetCrosshairGap { name, gap } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.gap = gap.max(0.0);
                    }
                }
            }
            ScriptCommand::SetCrosshairSpread { name, spread } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.spread = spread.clamp(0.0, c.max_spread);
                    }
                }
            }
            ScriptCommand::SetCrosshairMaxSpread { name, max_spread } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.max_spread = max_spread.max(0.0);
                    }
                }
            }
            ScriptCommand::SetCrosshairSpreadDecay { name, decay } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.spread_decay = decay.max(0.0);
                    }
                }
            }
            ScriptCommand::SetCrosshairEnabled { name, enabled } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddCrosshairSpread { name, amount } => {
                use bsengine_core::Crosshair;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut c) = world.get_mut::<Crosshair>(e) {
                        c.add_spread(amount);
                    }
                }
            }
            ScriptCommand::SetBloomIntensity { name, intensity } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.intensity = intensity.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomThreshold { name, threshold } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.threshold = threshold.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomRadius { name, radius } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBloomSoftness { name, softness } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.softness = softness.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetBloomEnabled { name, enabled } => {
                use bsengine_core::Bloom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Bloom>(e) {
                        b.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetTintColor { name, r, g, b } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.color = Vec3::new(r, g, b);
                    }
                }
            }
            ScriptCommand::SetTintIntensity { name, intensity } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.intensity = intensity.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetTintMode { name, mode } => {
                use bsengine_core::{Tint, TintMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.mode = match mode {
                            1 => TintMode::Fading,
                            2 => TintMode::Pulsing,
                            _ => TintMode::Constant,
                        };
                    }
                }
            }
            ScriptCommand::SetTintFadeRate { name, rate } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.fade_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetTintPulseSpeed { name, speed } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.pulse_speed = speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetTintPeakIntensity { name, peak } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.peak_intensity = peak.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetTintEnabled { name, enabled } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetTint {
                name,
                r,
                g,
                b,
                intensity,
            } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.set(Vec3::new(r, g, b), intensity);
                    }
                }
            }
            ScriptCommand::ClearTint { name } => {
                use bsengine_core::Tint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Tint>(e) {
                        t.clear();
                    }
                }
            }
            ScriptCommand::SetChromAbIntensity { name, intensity } => {
                use bsengine_core::ChromaticAberration;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ca) = world.get_mut::<ChromaticAberration>(e) {
                        ca.intensity = intensity.max(0.0);
                    }
                }
            }
            ScriptCommand::SetChromAbEnabled { name, enabled } => {
                use bsengine_core::ChromaticAberration;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ca) = world.get_mut::<ChromaticAberration>(e) {
                        ca.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetColorGradingLutPath { name, path } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.lut_path = if path.is_empty() { None } else { Some(path) };
                    }
                }
            }
            ScriptCommand::SetColorGradingExposure { name, exposure } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.exposure = exposure;
                    }
                }
            }
            ScriptCommand::SetColorGradingContrast { name, contrast } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.contrast = contrast.clamp(-1.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetColorGradingSaturation { name, saturation } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.saturation = saturation.max(0.0);
                    }
                }
            }
            ScriptCommand::SetColorGradingHueShift { name, hue_shift } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.hue_shift = hue_shift.clamp(-180.0, 180.0);
                    }
                }
            }
            ScriptCommand::SetColorGradingBrightness { name, brightness } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.brightness = brightness;
                    }
                }
            }
            ScriptCommand::SetColorGradingEnabled { name, enabled } => {
                use bsengine_core::ColorGrading;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cg) = world.get_mut::<ColorGrading>(e) {
                        cg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetAoRadius { name, radius } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAoBias { name, bias } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.bias = bias.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAoIntensity { name, intensity } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.intensity = intensity.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetAoSampleCount { name, count } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.sample_count = count.max(1);
                    }
                }
            }
            ScriptCommand::SetAoEnabled { name, enabled } => {
                use bsengine_core::AmbientOcclusion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ao) = world.get_mut::<AmbientOcclusion>(e) {
                        ao.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetDofFocalDistance { name, distance } => {
                use bsengine_core::DepthOfField;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dof) = world.get_mut::<DepthOfField>(e) {
                        dof.focal_distance = distance.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDofFocalRange { name, range } => {
                use bsengine_core::DepthOfField;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dof) = world.get_mut::<DepthOfField>(e) {
                        dof.focal_range = range.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDofMaxBlur { name, max_blur } => {
                use bsengine_core::DepthOfField;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dof) = world.get_mut::<DepthOfField>(e) {
                        dof.max_blur = max_blur.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDofBokehScale { name, scale } => {
                use bsengine_core::DepthOfField;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dof) = world.get_mut::<DepthOfField>(e) {
                        dof.bokeh_scale = scale.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDofEnabled { name, enabled } => {
                use bsengine_core::DepthOfField;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dof) = world.get_mut::<DepthOfField>(e) {
                        dof.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetMotionBlurShutterAngle { name, angle } => {
                use bsengine_core::MotionBlur;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mb) = world.get_mut::<MotionBlur>(e) {
                        mb.shutter_angle = angle.clamp(0.0, 360.0);
                    }
                }
            }
            ScriptCommand::SetMotionBlurSampleCount { name, count } => {
                use bsengine_core::MotionBlur;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mb) = world.get_mut::<MotionBlur>(e) {
                        mb.sample_count = count.max(1);
                    }
                }
            }
            ScriptCommand::SetMotionBlurEnabled { name, enabled } => {
                use bsengine_core::MotionBlur;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mb) = world.get_mut::<MotionBlur>(e) {
                        mb.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetToneMapMode { name, mode } => {
                use bsengine_core::{ToneMap, ToneMappingMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.mode = match mode {
                            0 => ToneMappingMode::None,
                            1 => ToneMappingMode::Reinhard,
                            2 => ToneMappingMode::ReinhardLuminance,
                            3 => ToneMappingMode::Aces,
                            4 => ToneMappingMode::Filmic,
                            _ => ToneMappingMode::Aces,
                        };
                    }
                }
            }
            ScriptCommand::SetToneMapExposure { name, exposure } => {
                use bsengine_core::ToneMap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.exposure = exposure;
                    }
                }
            }
            ScriptCommand::SetToneMapEnabled { name, enabled } => {
                use bsengine_core::ToneMap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tm) = world.get_mut::<ToneMap>(e) {
                        tm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetAbsorptionFraction { name, fraction } => {
                use bsengine_core::Absorption;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Absorption>(e) {
                        ab.fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetAbsorptionPool { name, pool } => {
                use bsengine_core::Absorption;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Absorption>(e) {
                        ab.pool = pool.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAbsorptionMaxPool { name, max_pool } => {
                use bsengine_core::Absorption;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Absorption>(e) {
                        ab.max_pool = max_pool.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAbsorptionEnabled { name, enabled } => {
                use bsengine_core::Absorption;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Absorption>(e) {
                        ab.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetVignetteIntensity { name, intensity } => {
                use bsengine_core::Vignette;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut vg) = world.get_mut::<Vignette>(e) {
                        vg.intensity = intensity.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetVignetteSmoothness { name, smoothness } => {
                use bsengine_core::Vignette;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut vg) = world.get_mut::<Vignette>(e) {
                        vg.smoothness = smoothness.max(0.0);
                    }
                }
            }
            ScriptCommand::SetVignetteColor { name, r, g, b } => {
                use bsengine_core::Vignette;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut vg) = world.get_mut::<Vignette>(e) {
                        vg.color = [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
                    }
                }
            }
            ScriptCommand::SetVignetteEnabled { name, enabled } => {
                use bsengine_core::Vignette;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut vg) = world.get_mut::<Vignette>(e) {
                        vg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetFogColor { name, r, g, b, a } => {
                use bsengine_core::{Color, Fog};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.color = Color::rgba(
                            r.clamp(0.0, 1.0),
                            g.clamp(0.0, 1.0),
                            b.clamp(0.0, 1.0),
                            a.clamp(0.0, 1.0),
                        );
                    }
                }
            }
            ScriptCommand::SetFogDensity { name, density } => {
                use bsengine_core::Fog;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.density = density.max(0.0);
                    }
                }
            }
            ScriptCommand::SetFogStartDistance { name, start } => {
                use bsengine_core::Fog;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.start_distance = start.max(0.0);
                    }
                }
            }
            ScriptCommand::SetFogEndDistance { name, end } => {
                use bsengine_core::Fog;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.end_distance = end.max(fog.start_distance);
                    }
                }
            }
            ScriptCommand::SetFogMode { name, mode } => {
                use bsengine_core::{Fog, FogMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.mode = match mode {
                            0 => FogMode::Linear,
                            2 => FogMode::ExponentialSquared,
                            _ => FogMode::Exponential,
                        };
                    }
                }
            }
            ScriptCommand::SetFogEnabled { name, enabled } => {
                use bsengine_core::Fog;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fog) = world.get_mut::<Fog>(e) {
                        fog.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetSpringTarget { name, target } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == target).map(|(e, _)| e)
                };
                if let (Some(e), Some(te)) = (entity, target_entity) {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.target = te;
                    }
                }
            }
            ScriptCommand::SetSpringRestLength { name, length } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.rest_length = length.max(0.0);
                    }
                }
            }
            ScriptCommand::SetSpringStiffness { name, stiffness } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.stiffness = stiffness.max(0.0);
                    }
                }
            }
            ScriptCommand::SetSpringDamping { name, damping } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.damping = damping.max(0.0);
                    }
                }
            }
            ScriptCommand::SetSpringBreakExtension { name, ext } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.break_extension = if ext >= 0.0 { Some(ext) } else { None };
                    }
                }
            }
            ScriptCommand::SetSpringEnabled { name, enabled } => {
                use bsengine_core::Spring;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<Spring>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetTweenDuration { name, duration } => {
                use bsengine_core::Tween;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetTweenEasing { name, easing } => {
                use bsengine_core::{EasingFn, Tween};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.easing = match easing {
                            1 => EasingFn::EaseInQuad,
                            2 => EasingFn::EaseOutQuad,
                            3 => EasingFn::EaseInOutQuad,
                            _ => EasingFn::Linear,
                        };
                    }
                }
            }
            ScriptCommand::SetTweenRepeat { name, repeat } => {
                use bsengine_core::{RepeatMode, Tween};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.repeat = match repeat {
                            1 => RepeatMode::Loop,
                            2 => RepeatMode::PingPong,
                            _ => RepeatMode::Once,
                        };
                    }
                }
            }
            ScriptCommand::SetTweenElapsed { name, elapsed } => {
                use bsengine_core::Tween;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tw) = world.get_mut::<Tween>(e) {
                        tw.elapsed = elapsed.clamp(0.0, tw.duration);
                        tw.finished = false;
                    }
                }
            }
            ScriptCommand::SetBuoyancyFluidDensity { name, density } => {
                use bsengine_core::Buoyancy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Buoyancy>(e) {
                        b.fluid_density = density.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBuoyancyVolume { name, volume } => {
                use bsengine_core::Buoyancy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Buoyancy>(e) {
                        b.volume = volume.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBuoyancyLinearDrag { name, drag } => {
                use bsengine_core::Buoyancy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Buoyancy>(e) {
                        b.linear_drag = drag.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBuoyancyAngularDrag { name, drag } => {
                use bsengine_core::Buoyancy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Buoyancy>(e) {
                        b.angular_drag = drag.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBuoyancySurfaceY { name, y } => {
                use bsengine_core::Buoyancy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut b) = world.get_mut::<Buoyancy>(e) {
                        b.surface_y = y;
                    }
                }
            }
            ScriptCommand::SetFollowTarget { name, target } => {
                use bsengine_core::Follow;
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == target).map(|(e, _)| e)
                };
                if let Some(te) = target_entity {
                    let entity = {
                        let mut q = world.query::<(Entity, &Name)>();
                        q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                    };
                    if let Some(e) = entity {
                        if let Some(mut f) = world.get_mut::<Follow>(e) {
                            f.target = te;
                        }
                    }
                }
            }
            ScriptCommand::SetFollowOffset { name, x, y, z } => {
                use bsengine_core::Follow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Follow>(e) {
                        f.offset = Vec3::new(x, y, z);
                    }
                }
            }
            ScriptCommand::SetFollowSpeed { name, speed } => {
                use bsengine_core::Follow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut f) = world.get_mut::<Follow>(e) {
                        f.speed = speed;
                    }
                }
            }
            ScriptCommand::SetLookAtTarget { name, target } => {
                use bsengine_core::LookAt;
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == target).map(|(e, _)| e)
                };
                if let Some(te) = target_entity {
                    let entity = {
                        let mut q = world.query::<(Entity, &Name)>();
                        q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                    };
                    if let Some(e) = entity {
                        if let Some(mut la) = world.get_mut::<LookAt>(e) {
                            la.target = te;
                        }
                    }
                }
            }
            ScriptCommand::SetLookAtUp { name, x, y, z } => {
                use bsengine_core::LookAt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<LookAt>(e) {
                        la.up = Vec3::new(x, y, z);
                    }
                }
            }
            ScriptCommand::SetBillboardMode { name, mode } => {
                use bsengine_core::{Billboard, BillboardMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bb) = world.get_mut::<Billboard>(e) {
                        bb.mode = if mode == 1 {
                            BillboardMode::Vertical
                        } else {
                            BillboardMode::Full
                        };
                    }
                }
            }
            ScriptCommand::SetOutlineColor { name, r, g, b, a } => {
                use bsengine_core::{Color, Outline};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outline>(e) {
                        ol.color = Color { r, g, b, a };
                    }
                }
            }
            ScriptCommand::SetOutlineWidth { name, width } => {
                use bsengine_core::Outline;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outline>(e) {
                        ol.width = width.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOutlineMode { name, mode } => {
                use bsengine_core::{Outline, OutlineMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outline>(e) {
                        ol.mode = match mode {
                            1 => OutlineMode::Inner,
                            2 => OutlineMode::Center,
                            _ => OutlineMode::Outer,
                        };
                    }
                }
            }
            ScriptCommand::SetOutlineVisible { name, visible } => {
                use bsengine_core::Outline;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outline>(e) {
                        ol.visible = visible;
                    }
                }
            }
            ScriptCommand::SetZIndex { name, index } => {
                use bsengine_core::ZIndex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut zi) = world.get_mut::<ZIndex>(e) {
                        *zi = ZIndex::new(index);
                    }
                }
            }
            ScriptCommand::SetLayer { name, bits } => {
                use bsengine_core::Layer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut l) = world.get_mut::<Layer>(e) {
                        *l = Layer::new(bits);
                    }
                }
            }
            ScriptCommand::SetAnchorPreset { name, preset } => {
                use bsengine_core::{Anchor, AnchorPreset};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Anchor>(e) {
                        a.preset = match preset {
                            1 => AnchorPreset::TopLeft,
                            2 => AnchorPreset::TopCenter,
                            3 => AnchorPreset::TopRight,
                            4 => AnchorPreset::MiddleLeft,
                            5 => AnchorPreset::MiddleRight,
                            6 => AnchorPreset::BottomLeft,
                            7 => AnchorPreset::BottomCenter,
                            8 => AnchorPreset::BottomRight,
                            _ => AnchorPreset::Center,
                        };
                    }
                }
            }
            ScriptCommand::SetAnchorCustom { name, x, y } => {
                use bsengine_core::{Anchor, AnchorPreset};
                use glam::Vec2;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Anchor>(e) {
                        a.preset = AnchorPreset::Custom(Vec2::new(x, y));
                    }
                }
            }
            ScriptCommand::SetAnchorOffset { name, x, y } => {
                use bsengine_core::Anchor;
                use glam::Vec2;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut a) = world.get_mut::<Anchor>(e) {
                        a.offset = Vec2::new(x, y);
                    }
                }
            }
            ScriptCommand::SetTriggerLayerMask { name, mask } => {
                use bsengine_core::Trigger;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Trigger>(e) {
                        t.layer_mask = mask;
                    }
                }
            }
            ScriptCommand::SetTriggerEnabled { name, enabled } => {
                use bsengine_core::Trigger;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Trigger>(e) {
                        t.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetDamageAmount { name, amount } => {
                use bsengine_core::Damage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damage>(e) {
                        d.amount = amount.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDamageType { name, damage_type } => {
                use bsengine_core::{Damage, DamageType};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damage>(e) {
                        d.damage_type = match damage_type {
                            1 => DamageType::Fire,
                            2 => DamageType::Ice,
                            3 => DamageType::Lightning,
                            4 => DamageType::Poison,
                            _ => DamageType::Physical,
                        };
                    }
                }
            }
            ScriptCommand::SetDamageTypeCustom { name, id } => {
                use bsengine_core::{Damage, DamageType};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damage>(e) {
                        d.damage_type = DamageType::Custom(id);
                    }
                }
            }
            ScriptCommand::SetDamageMultiplier { name, multiplier } => {
                use bsengine_core::Damage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damage>(e) {
                        d.multiplier = multiplier.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDamagePiercing { name, piercing } => {
                use bsengine_core::Damage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut d) = world.get_mut::<Damage>(e) {
                        d.piercing = piercing;
                    }
                }
            }
            ScriptCommand::SetSpawnPointTag { name, tag } => {
                use bsengine_core::SpawnPoint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<SpawnPoint>(e) {
                        sp.tag = tag;
                    }
                }
            }
            ScriptCommand::SetSpawnPointTeam { name, team } => {
                use bsengine_core::SpawnPoint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<SpawnPoint>(e) {
                        sp.team = if team == u32::MAX { None } else { Some(team) };
                    }
                }
            }
            ScriptCommand::ClearSpawnPointTeam { name } => {
                use bsengine_core::SpawnPoint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<SpawnPoint>(e) {
                        sp.team = None;
                    }
                }
            }
            ScriptCommand::SetSpawnPointEnabled { name, enabled } => {
                use bsengine_core::SpawnPoint;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<SpawnPoint>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetStatusEffectId { name, id } => {
                use bsengine_core::StatusEffect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.id = id;
                    }
                }
            }
            ScriptCommand::SetStatusEffectKind { name, kind } => {
                use bsengine_core::{EffectKind, StatusEffect};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.kind = match kind {
                            0 => EffectKind::StatMultiplier,
                            1 => EffectKind::DamageOverTime,
                            2 => EffectKind::Immobilize,
                            3 => EffectKind::Silence,
                            _ => EffectKind::StatMultiplier,
                        };
                    }
                }
            }
            ScriptCommand::SetStatusEffectKindCustom { name, custom_id } => {
                use bsengine_core::{EffectKind, StatusEffect};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.kind = EffectKind::Custom(custom_id);
                    }
                }
            }
            ScriptCommand::SetStatusEffectValue { name, value } => {
                use bsengine_core::StatusEffect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.value = value;
                    }
                }
            }
            ScriptCommand::SetStatusEffectDuration { name, duration } => {
                use bsengine_core::StatusEffect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.duration = duration;
                    }
                }
            }
            ScriptCommand::SetStatusEffectTicksEveryFrame { name, ticks } => {
                use bsengine_core::StatusEffect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.ticks_every_frame = ticks;
                    }
                }
            }
            ScriptCommand::SetStatusEffectEnabled { name, enabled } => {
                use bsengine_core::StatusEffect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<StatusEffect>(e) {
                        se.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetAbilityName { name, ability_name } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.name = ability_name;
                    }
                }
            }
            ScriptCommand::SetAbilityCooldown { name, cooldown } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.cooldown = cooldown.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAbilityCooldownRemaining { name, remaining } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.cooldown_remaining = remaining.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAbilityMaxCharges { name, max_charges } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.max_charges = max_charges;
                    }
                }
            }
            ScriptCommand::SetAbilityCharges { name, charges } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.charges = charges;
                    }
                }
            }
            ScriptCommand::SetAbilityChargeRegenTime { name, regen_time } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.charge_regen_time = regen_time.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAbilityEnabled { name, enabled } => {
                use bsengine_core::Ability;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ab) = world.get_mut::<Ability>(e) {
                        ab.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetAlarmAlertDuration { name, duration } => {
                use bsengine_core::Alarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut al) = world.get_mut::<Alarm>(e) {
                        al.alert_duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAlarmDetectionRadius { name, radius } => {
                use bsengine_core::Alarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut al) = world.get_mut::<Alarm>(e) {
                        al.detection_radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetAlarmEnabled { name, enabled } => {
                use bsengine_core::Alarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut al) = world.get_mut::<Alarm>(e) {
                        al.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerAlarm { name } => {
                use bsengine_core::Alarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut al) = world.get_mut::<Alarm>(e) {
                        al.trigger();
                    }
                }
            }
            ScriptCommand::CalmAlarm { name } => {
                use bsengine_core::Alarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut al) = world.get_mut::<Alarm>(e) {
                        al.calm();
                    }
                }
            }
            ScriptCommand::ApplyAmplify { name, duration } => {
                use bsengine_core::Amplify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut amp) = world.get_mut::<Amplify>(e) {
                        amp.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearAmplify { name } => {
                use bsengine_core::Amplify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut amp) = world.get_mut::<Amplify>(e) {
                        amp.clear();
                    }
                }
            }
            ScriptCommand::SetAmplifyDuration { name, duration } => {
                use bsengine_core::Amplify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut amp) = world.get_mut::<Amplify>(e) {
                        amp.duration = duration;
                    }
                }
            }
            ScriptCommand::SetAmplifyPowerMultiplier { name, multiplier } => {
                use bsengine_core::Amplify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut amp) = world.get_mut::<Amplify>(e) {
                        amp.power_multiplier = multiplier.max(1.0);
                    }
                }
            }
            ScriptCommand::SetAmplifyEnabled { name, enabled } => {
                use bsengine_core::Amplify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut amp) = world.get_mut::<Amplify>(e) {
                        amp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetBarrierCapacity { name, capacity } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.capacity = capacity.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBarrierCurrent { name, current } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.current = current.clamp(0.0, bar.capacity);
                    }
                }
            }
            ScriptCommand::SetBarrierRegenRate { name, rate } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.regen_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBarrierRegenDelay { name, delay } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.regen_delay = delay.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBarrierEnabled { name, enabled } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.enabled = enabled;
                    }
                }
            }
            ScriptCommand::RestoreBarrier { name } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.restore();
                    }
                }
            }
            ScriptCommand::DrainBarrier { name } => {
                use bsengine_core::Barrier;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bar) = world.get_mut::<Barrier>(e) {
                        bar.drain();
                    }
                }
            }
            ScriptCommand::SetBeaconPriority { name, priority } => {
                use bsengine_core::Beacon;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bec) = world.get_mut::<Beacon>(e) {
                        bec.priority = priority;
                    }
                }
            }
            ScriptCommand::SetBeaconBroadcastRadius { name, radius } => {
                use bsengine_core::Beacon;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bec) = world.get_mut::<Beacon>(e) {
                        bec.broadcast_radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBeaconEnabled { name, enabled } => {
                use bsengine_core::Beacon;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bec) = world.get_mut::<Beacon>(e) {
                        bec.enabled = enabled;
                    }
                }
            }
            ScriptCommand::LightBeacon { name, duration } => {
                use bsengine_core::Beacon;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bec) = world.get_mut::<Beacon>(e) {
                        bec.light(duration);
                    }
                }
            }
            ScriptCommand::ExtinguishBeacon { name } => {
                use bsengine_core::Beacon;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bec) = world.get_mut::<Beacon>(e) {
                        bec.extinguish();
                    }
                }
            }
            ScriptCommand::ApplyShieldBreak { name, duration } => {
                use bsengine_core::ShieldBreak;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sb) = world.get_mut::<ShieldBreak>(e) {
                        sb.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearShieldBreak { name } => {
                use bsengine_core::ShieldBreak;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sb) = world.get_mut::<ShieldBreak>(e) {
                        sb.clear();
                    }
                }
            }
            ScriptCommand::SetShieldBreakDuration { name, duration } => {
                use bsengine_core::ShieldBreak;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sb) = world.get_mut::<ShieldBreak>(e) {
                        sb.duration = duration;
                    }
                }
            }
            ScriptCommand::SetShieldBreakReduction { name, reduction } => {
                use bsengine_core::ShieldBreak;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sb) = world.get_mut::<ShieldBreak>(e) {
                        sb.reduction_fraction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetShieldBreakEnabled { name, enabled } => {
                use bsengine_core::ShieldBreak;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sb) = world.get_mut::<ShieldBreak>(e) {
                        sb.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyRoot { name, duration } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearRoot { name } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.clear();
                    }
                }
            }
            ScriptCommand::SetRootDuration { name, duration } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetRootAllowsRotation { name, allows } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.allows_rotation = allows;
                    }
                }
            }
            ScriptCommand::SetRootAllowsAttack { name, allows } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.allows_attack = allows;
                    }
                }
            }
            ScriptCommand::SetRootEnabled { name, enabled } => {
                use bsengine_core::Root;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut root) = world.get_mut::<Root>(e) {
                        root.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySlow {
                name,
                reduction,
                duration,
            } => {
                use bsengine_core::Slow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<Slow>(e) {
                        sl.apply(reduction, duration);
                    }
                }
            }
            ScriptCommand::ClearSlow { name } => {
                use bsengine_core::Slow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<Slow>(e) {
                        sl.clear();
                    }
                }
            }
            ScriptCommand::SetSlowReduction { name, reduction } => {
                use bsengine_core::Slow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<Slow>(e) {
                        sl.reduction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetSlowDuration { name, duration } => {
                use bsengine_core::Slow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<Slow>(e) {
                        sl.duration = duration.max(0.0);
                    }
                }
            }
            ScriptCommand::SetSlowEnabled { name, enabled } => {
                use bsengine_core::Slow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<Slow>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyStun {
                name,
                duration,
                severity,
            } => {
                use bsengine_core::{Stun, StunSeverity};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stun>(e) {
                        let sev = match severity {
                            1 => StunSeverity::Heavy,
                            2 => StunSeverity::Knockdown,
                            _ => StunSeverity::Light,
                        };
                        st.apply(duration, sev);
                    }
                }
            }
            ScriptCommand::ClearStun { name } => {
                use bsengine_core::Stun;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stun>(e) {
                        st.clear();
                    }
                }
            }
            ScriptCommand::SetStunEnabled { name, enabled } => {
                use bsengine_core::Stun;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<Stun>(e) {
                        st.enabled = enabled;
                    }
                }
            }
            ScriptCommand::IgniteBurn { name } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.ignite();
                    }
                }
            }
            ScriptCommand::ExtinguishBurn { name } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.extinguish();
                    }
                }
            }
            ScriptCommand::SetBurnRate { name, rate } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.burn_rate = rate;
                    }
                }
            }
            ScriptCommand::SetBurnMaxStacks { name, max_stacks } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.max_stacks = max_stacks.max(1);
                    }
                }
            }
            ScriptCommand::SetBurnDuration { name, duration } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.duration = duration;
                    }
                }
            }
            ScriptCommand::SetBurnEnabled { name, enabled } => {
                use bsengine_core::Burn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bn) = world.get_mut::<Burn>(e) {
                        bn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyBleed { name } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.apply();
                    }
                }
            }
            ScriptCommand::ClearBleed { name } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.clear();
                    }
                }
            }
            ScriptCommand::SetBleedMaxStacks { name, max_stacks } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.max_stacks = max_stacks.max(1);
                    }
                }
            }
            ScriptCommand::SetBleedDamagePerStack { name, damage } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.damage_per_stack_per_tick = damage;
                    }
                }
            }
            ScriptCommand::SetBleedDuration { name, duration } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.duration = duration;
                    }
                }
            }
            ScriptCommand::SetBleedHealReduction { name, reduction } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.heal_reduction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetBleedEnabled { name, enabled } => {
                use bsengine_core::Bleed;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Bleed>(e) {
                        bl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyPoison { name } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.apply();
                    }
                }
            }
            ScriptCommand::ClearPoison { name } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.clear();
                    }
                }
            }
            ScriptCommand::SetPoisonMaxStacks { name, max_stacks } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.max_stacks = max_stacks.max(1);
                    }
                }
            }
            ScriptCommand::SetPoisonDamagePerStack { name, damage } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.damage_per_stack_per_tick = damage;
                    }
                }
            }
            ScriptCommand::SetPoisonDuration { name, duration } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.duration = duration;
                    }
                }
            }
            ScriptCommand::SetPoisonEnabled { name, enabled } => {
                use bsengine_core::Poison;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poison>(e) {
                        po.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyCold { name, amount } => {
                use bsengine_core::Freeze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Freeze>(e) {
                        fr.apply_cold(amount);
                    }
                }
            }
            ScriptCommand::ThawFreeze { name } => {
                use bsengine_core::Freeze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Freeze>(e) {
                        fr.thaw();
                    }
                }
            }
            ScriptCommand::SetFreezeChillSlow { name, slow } => {
                use bsengine_core::Freeze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Freeze>(e) {
                        fr.chill_slow = slow.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetFreezeFrozenDuration { name, duration } => {
                use bsengine_core::Freeze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Freeze>(e) {
                        fr.frozen_duration = duration;
                    }
                }
            }
            ScriptCommand::SetFreezeEnabled { name, enabled } => {
                use bsengine_core::Freeze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Freeze>(e) {
                        fr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyBlind { name, duration } => {
                use bsengine_core::Blind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Blind>(e) {
                        bl.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearBlind { name } => {
                use bsengine_core::Blind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Blind>(e) {
                        bl.clear();
                    }
                }
            }
            ScriptCommand::SetBlindRangeLimit { name, limit } => {
                use bsengine_core::Blind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Blind>(e) {
                        bl.range_limit = limit.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBlindAimDeviation { name, deviation } => {
                use bsengine_core::Blind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Blind>(e) {
                        bl.aim_deviation_rad = deviation.max(0.0);
                    }
                }
            }
            ScriptCommand::SetBlindEnabled { name, enabled } => {
                use bsengine_core::Blind;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut bl) = world.get_mut::<Blind>(e) {
                        bl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyCharm {
                name,
                source_name,
                duration,
            } => {
                use bsengine_core::Charm;
                let source_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world)
                        .find(|(_, n)| n.0 == source_name)
                        .map(|(e, _)| e)
                };
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(src), Some(tgt)) = (source_entity, target_entity) {
                    if let Some(mut ch) = world.get_mut::<Charm>(tgt) {
                        ch.apply(src, duration);
                    }
                }
            }
            ScriptCommand::ClearCharm { name } => {
                use bsengine_core::Charm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ch) = world.get_mut::<Charm>(e) {
                        ch.clear();
                    }
                }
            }
            ScriptCommand::SetCharmEnabled { name, enabled } => {
                use bsengine_core::Charm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ch) = world.get_mut::<Charm>(e) {
                        ch.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyConfuse { name, duration } => {
                use bsengine_core::Confuse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Confuse>(e) {
                        co.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearConfuse { name } => {
                use bsengine_core::Confuse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Confuse>(e) {
                        co.clear();
                    }
                }
            }
            ScriptCommand::SetConfuseChance { name, chance } => {
                use bsengine_core::Confuse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Confuse>(e) {
                        co.chance = chance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetConfuseEnabled { name, enabled } => {
                use bsengine_core::Confuse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Confuse>(e) {
                        co.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyCripple { name, duration } => {
                use bsengine_core::Cripple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cr) = world.get_mut::<Cripple>(e) {
                        cr.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearCripple { name } => {
                use bsengine_core::Cripple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cr) = world.get_mut::<Cripple>(e) {
                        cr.clear();
                    }
                }
            }
            ScriptCommand::SetCrippleSpeedFraction { name, fraction } => {
                use bsengine_core::Cripple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cr) = world.get_mut::<Cripple>(e) {
                        cr.speed_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetCripplePreventsJump { name, prevents } => {
                use bsengine_core::Cripple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cr) = world.get_mut::<Cripple>(e) {
                        cr.prevents_jump = prevents;
                    }
                }
            }
            ScriptCommand::SetCrippleEnabled { name, enabled } => {
                use bsengine_core::Cripple;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cr) = world.get_mut::<Cripple>(e) {
                        cr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyDaze { name, duration } => {
                use bsengine_core::Daze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dz) = world.get_mut::<Daze>(e) {
                        dz.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearDaze { name } => {
                use bsengine_core::Daze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dz) = world.get_mut::<Daze>(e) {
                        dz.clear();
                    }
                }
            }
            ScriptCommand::SetDazeSlowFraction { name, fraction } => {
                use bsengine_core::Daze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dz) = world.get_mut::<Daze>(e) {
                        dz.slow_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetDazeAimDeviation { name, deviation } => {
                use bsengine_core::Daze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dz) = world.get_mut::<Daze>(e) {
                        dz.aim_deviation_rad = deviation.max(0.0);
                    }
                }
            }
            ScriptCommand::SetDazeEnabled { name, enabled } => {
                use bsengine_core::Daze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dz) = world.get_mut::<Daze>(e) {
                        dz.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyDisarm { name, duration } => {
                use bsengine_core::Disarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut di) = world.get_mut::<Disarm>(e) {
                        di.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearDisarm { name } => {
                use bsengine_core::Disarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut di) = world.get_mut::<Disarm>(e) {
                        di.clear();
                    }
                }
            }
            ScriptCommand::SetDisarmEnabled { name, enabled } => {
                use bsengine_core::Disarm;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut di) = world.get_mut::<Disarm>(e) {
                        di.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyConcuss { name, duration } => {
                use bsengine_core::Concuss;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cn) = world.get_mut::<Concuss>(e) {
                        cn.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearConcuss { name } => {
                use bsengine_core::Concuss;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cn) = world.get_mut::<Concuss>(e) {
                        cn.clear();
                    }
                }
            }
            ScriptCommand::SetConcussAimDeviation { name, deviation } => {
                use bsengine_core::Concuss;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cn) = world.get_mut::<Concuss>(e) {
                        cn.aim_deviation_rad = deviation;
                    }
                }
            }
            ScriptCommand::SetConcussSuppressChance { name, chance } => {
                use bsengine_core::Concuss;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cn) = world.get_mut::<Concuss>(e) {
                        cn.ability_suppress_chance = chance;
                    }
                }
            }
            ScriptCommand::SetConcussEnabled { name, enabled } => {
                use bsengine_core::Concuss;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cn) = world.get_mut::<Concuss>(e) {
                        cn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyCorrosion { name, amount } => {
                use bsengine_core::Corrosion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Corrosion>(e) {
                        co.apply(amount);
                    }
                }
            }
            ScriptCommand::SetCorrosionDecayRate { name, rate } => {
                use bsengine_core::Corrosion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Corrosion>(e) {
                        co.decay_rate = rate;
                    }
                }
            }
            ScriptCommand::SetCorrosionArmorReduction { name, reduction } => {
                use bsengine_core::Corrosion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Corrosion>(e) {
                        co.armor_reduction_per_stack = reduction;
                    }
                }
            }
            ScriptCommand::SetCorrosionEnabled { name, enabled } => {
                use bsengine_core::Corrosion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut co) = world.get_mut::<Corrosion>(e) {
                        co.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyCurse {
                name,
                kind,
                strength,
                duration,
            } => {
                use bsengine_core::{Curse, CurseKind};
                let curse_kind = match kind {
                    0 => CurseKind::DamageDown,
                    1 => CurseKind::SpeedDown,
                    2 => CurseKind::ArmorDown,
                    3 => CurseKind::DamageTakenUp,
                    _ => CurseKind::Custom,
                };
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cu) = world.get_mut::<Curse>(e) {
                        cu.apply(curse_kind, strength, duration);
                    }
                }
            }
            ScriptCommand::ClearCurse { name } => {
                use bsengine_core::Curse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cu) = world.get_mut::<Curse>(e) {
                        cu.clear();
                    }
                }
            }
            ScriptCommand::SetCurseEnabled { name, enabled } => {
                use bsengine_core::Curse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut cu) = world.get_mut::<Curse>(e) {
                        cu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetDreadRadius { name, radius } => {
                use bsengine_core::Dread;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Dread>(e) {
                        dr.radius = radius;
                    }
                }
            }
            ScriptCommand::SetDreadPulseInterval { name, interval } => {
                use bsengine_core::Dread;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Dread>(e) {
                        dr.pulse_interval = interval;
                    }
                }
            }
            ScriptCommand::SetDreadBuildupPerPulse { name, buildup } => {
                use bsengine_core::Dread;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Dread>(e) {
                        dr.buildup_per_pulse = buildup;
                    }
                }
            }
            ScriptCommand::SetDreadEnabled { name, enabled } => {
                use bsengine_core::Dread;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Dread>(e) {
                        dr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyDoom { name, duration } => {
                use bsengine_core::Doom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dm) = world.get_mut::<Doom>(e) {
                        dm.doom(duration);
                    }
                }
            }
            ScriptCommand::CleanseDoom { name } => {
                use bsengine_core::Doom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dm) = world.get_mut::<Doom>(e) {
                        dm.cleanse();
                    }
                }
            }
            ScriptCommand::SetDoomEnabled { name, enabled } => {
                use bsengine_core::Doom;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dm) = world.get_mut::<Doom>(e) {
                        dm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyDemoralize { name, duration } => {
                use bsengine_core::Demoralize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut de) = world.get_mut::<Demoralize>(e) {
                        de.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearDemoralize { name } => {
                use bsengine_core::Demoralize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut de) = world.get_mut::<Demoralize>(e) {
                        de.clear();
                    }
                }
            }
            ScriptCommand::SetDemoralizeDamageFraction { name, fraction } => {
                use bsengine_core::Demoralize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut de) = world.get_mut::<Demoralize>(e) {
                        de.damage_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetDemoralizeFleeChance { name, chance } => {
                use bsengine_core::Demoralize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut de) = world.get_mut::<Demoralize>(e) {
                        de.flee_chance = chance;
                    }
                }
            }
            ScriptCommand::SetDemoralizeEnabled { name, enabled } => {
                use bsengine_core::Demoralize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut de) = world.get_mut::<Demoralize>(e) {
                        de.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerDodge { name, dx, dy, dz } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.start(Vec3::new(dx, dy, dz));
                    }
                }
            }
            ScriptCommand::SetDodgeSpeed { name, speed } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.speed = speed;
                    }
                }
            }
            ScriptCommand::SetDodgeDuration { name, duration } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.duration = duration;
                    }
                }
            }
            ScriptCommand::SetDodgeCooldown { name, cooldown } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.cooldown = cooldown;
                    }
                }
            }
            ScriptCommand::SetDodgeAllowAirborne { name, allow } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.allow_airborne = allow;
                    }
                }
            }
            ScriptCommand::SetDodgeMaxChain { name, max_chain } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.max_chain = max_chain;
                    }
                }
            }
            ScriptCommand::SetDodgeEnabled { name, enabled } => {
                use bsengine_core::Dodge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dg) = world.get_mut::<Dodge>(e) {
                        dg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyDrain {
                name,
                rate,
                duration,
            } => {
                use bsengine_core::Drain;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Drain>(e) {
                        dr.apply(rate, duration);
                    }
                }
            }
            ScriptCommand::ClearDrain { name } => {
                use bsengine_core::Drain;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Drain>(e) {
                        dr.clear();
                    }
                }
            }
            ScriptCommand::SetDrainRate { name, rate } => {
                use bsengine_core::Drain;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Drain>(e) {
                        dr.rate = rate;
                    }
                }
            }
            ScriptCommand::SetDrainEnabled { name, enabled } => {
                use bsengine_core::Drain;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut dr) = world.get_mut::<Drain>(e) {
                        dr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyEmpower { name, duration } => {
                use bsengine_core::Empower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Empower>(e) {
                        em.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearEmpower { name } => {
                use bsengine_core::Empower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Empower>(e) {
                        em.clear();
                    }
                }
            }
            ScriptCommand::SetEmpowerPotencyMultiplier { name, multiplier } => {
                use bsengine_core::Empower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Empower>(e) {
                        em.potency_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetEmpowerEnabled { name, enabled } => {
                use bsengine_core::Empower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut em) = world.get_mut::<Empower>(e) {
                        em.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyEnervate { name, duration } => {
                use bsengine_core::Enervate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut en) = world.get_mut::<Enervate>(e) {
                        en.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearEnervate { name } => {
                use bsengine_core::Enervate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut en) = world.get_mut::<Enervate>(e) {
                        en.clear();
                    }
                }
            }
            ScriptCommand::SetEnervateRegenFraction { name, fraction } => {
                use bsengine_core::Enervate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut en) = world.get_mut::<Enervate>(e) {
                        en.regen_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetEnervateMaxPoolFraction { name, fraction } => {
                use bsengine_core::Enervate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut en) = world.get_mut::<Enervate>(e) {
                        en.max_pool_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetEnervateEnabled { name, enabled } => {
                use bsengine_core::Enervate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut en) = world.get_mut::<Enervate>(e) {
                        en.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyEntangle { name, duration } => {
                use bsengine_core::Entangle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut et) = world.get_mut::<Entangle>(e) {
                        et.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearEntangle { name } => {
                use bsengine_core::Entangle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut et) = world.get_mut::<Entangle>(e) {
                        et.clear();
                    }
                }
            }
            ScriptCommand::SetEntangleEnabled { name, enabled } => {
                use bsengine_core::Entangle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut et) = world.get_mut::<Entangle>(e) {
                        et.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyExpose { name, duration } => {
                use bsengine_core::Expose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Expose>(e) {
                        ex.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearExpose { name } => {
                use bsengine_core::Expose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Expose>(e) {
                        ex.clear();
                    }
                }
            }
            ScriptCommand::SetExposeDamageMultiplier { name, multiplier } => {
                use bsengine_core::Expose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Expose>(e) {
                        ex.damage_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetExposeEnabled { name, enabled } => {
                use bsengine_core::Expose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Expose>(e) {
                        ex.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddExhaustion { name, amount } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.add(amount);
                    }
                }
            }
            ScriptCommand::ClearExhaustion { name } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.clear();
                    }
                }
            }
            ScriptCommand::SetExhaustionRecoveryRate { name, rate } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.recovery_rate = rate;
                    }
                }
            }
            ScriptCommand::SetExhaustionThreshold { name, threshold } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetExhaustionPenaltySpeed { name, penalty } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.penalty_speed = penalty;
                    }
                }
            }
            ScriptCommand::SetExhaustionPenaltyRegen { name, penalty } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.penalty_regen = penalty;
                    }
                }
            }
            ScriptCommand::SetExhaustionEnabled { name, enabled } => {
                use bsengine_core::Exhaustion;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ex) = world.get_mut::<Exhaustion>(e) {
                        ex.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyFear {
                name,
                source_name,
                duration,
            } => {
                use bsengine_core::Fear;
                let source_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world)
                        .find(|(_, n)| n.0 == source_name)
                        .map(|(e, _)| e)
                };
                let target_entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(src), Some(tgt)) = (source_entity, target_entity) {
                    if let Some(mut fe) = world.get_mut::<Fear>(tgt) {
                        fe.apply(src, duration);
                    }
                }
            }
            ScriptCommand::ClearFear { name } => {
                use bsengine_core::Fear;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fe) = world.get_mut::<Fear>(e) {
                        fe.clear();
                    }
                }
            }
            ScriptCommand::SetFearFleeSpeedMultiplier { name, multiplier } => {
                use bsengine_core::Fear;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fe) = world.get_mut::<Fear>(e) {
                        fe.flee_speed_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetFearEnabled { name, enabled } => {
                use bsengine_core::Fear;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fe) = world.get_mut::<Fear>(e) {
                        fe.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyFracture { name, duration } => {
                use bsengine_core::Fracture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Fracture>(e) {
                        fr.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearFracture { name } => {
                use bsengine_core::Fracture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Fracture>(e) {
                        fr.clear();
                    }
                }
            }
            ScriptCommand::SetFractureDamageAmplification {
                name,
                amplification,
            } => {
                use bsengine_core::Fracture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Fracture>(e) {
                        fr.damage_amplification = amplification;
                    }
                }
            }
            ScriptCommand::SetFractureMoveSpeedPenalty { name, penalty } => {
                use bsengine_core::Fracture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Fracture>(e) {
                        fr.move_speed_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetFractureEnabled { name, enabled } => {
                use bsengine_core::Fracture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fr) = world.get_mut::<Fracture>(e) {
                        fr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyFrostbite { name, duration } => {
                use bsengine_core::Frostbite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fb) = world.get_mut::<Frostbite>(e) {
                        fb.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearFrostbite { name } => {
                use bsengine_core::Frostbite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fb) = world.get_mut::<Frostbite>(e) {
                        fb.clear();
                    }
                }
            }
            ScriptCommand::SetFrostbiteColdDamagePerSecond { name, damage } => {
                use bsengine_core::Frostbite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fb) = world.get_mut::<Frostbite>(e) {
                        fb.cold_damage_per_second = damage;
                    }
                }
            }
            ScriptCommand::SetFrostbiteActionSpeedFraction { name, fraction } => {
                use bsengine_core::Frostbite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fb) = world.get_mut::<Frostbite>(e) {
                        fb.action_speed_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetFrostbiteEnabled { name, enabled } => {
                use bsengine_core::Frostbite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fb) = world.get_mut::<Frostbite>(e) {
                        fb.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetFuryEnabled { name, enabled } => {
                use bsengine_core::Fury;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut fu) = world.get_mut::<Fury>(e) {
                        fu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyGalvanize { name, duration } => {
                use bsengine_core::Galvanize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut gv) = world.get_mut::<Galvanize>(e) {
                        gv.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearGalvanize { name } => {
                use bsengine_core::Galvanize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut gv) = world.get_mut::<Galvanize>(e) {
                        gv.clear();
                    }
                }
            }
            ScriptCommand::SetGalvanizeSpeedMultiplier { name, multiplier } => {
                use bsengine_core::Galvanize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut gv) = world.get_mut::<Galvanize>(e) {
                        gv.speed_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetGalvanizeEnabled { name, enabled } => {
                use bsengine_core::Galvanize;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut gv) = world.get_mut::<Galvanize>(e) {
                        gv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyHaste {
                name,
                multiplier,
                duration,
            } => {
                use bsengine_core::Haste;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hs) = world.get_mut::<Haste>(e) {
                        hs.apply(multiplier, duration);
                    }
                }
            }
            ScriptCommand::ClearHaste { name } => {
                use bsengine_core::Haste;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hs) = world.get_mut::<Haste>(e) {
                        hs.clear();
                    }
                }
            }
            ScriptCommand::SetHasteMaxStacks { name, max_stacks } => {
                use bsengine_core::Haste;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hs) = world.get_mut::<Haste>(e) {
                        hs.max_stacks = max_stacks as usize;
                    }
                }
            }
            ScriptCommand::SetHasteEnabled { name, enabled } => {
                use bsengine_core::Haste;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hs) = world.get_mut::<Haste>(e) {
                        hs.enabled = enabled;
                    }
                }
            }
            ScriptCommand::CallHavoc { name, duration } => {
                use bsengine_core::Havoc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hv) = world.get_mut::<Havoc>(e) {
                        hv.call(duration);
                    }
                }
            }
            ScriptCommand::QuellHavoc { name } => {
                use bsengine_core::Havoc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hv) = world.get_mut::<Havoc>(e) {
                        hv.quell();
                    }
                }
            }
            ScriptCommand::SetHavocStrayChance { name, chance } => {
                use bsengine_core::Havoc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hv) = world.get_mut::<Havoc>(e) {
                        hv.stray_chance = chance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetHavocDamageMultiplier { name, multiplier } => {
                use bsengine_core::Havoc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hv) = world.get_mut::<Havoc>(e) {
                        hv.damage_multiplier = multiplier.max(1.0);
                    }
                }
            }
            ScriptCommand::SetHavocEnabled { name, enabled } => {
                use bsengine_core::Havoc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hv) = world.get_mut::<Havoc>(e) {
                        hv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyHaze { name, duration } => {
                use bsengine_core::Haze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hz) = world.get_mut::<Haze>(e) {
                        hz.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearHaze { name } => {
                use bsengine_core::Haze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hz) = world.get_mut::<Haze>(e) {
                        hz.clear();
                    }
                }
            }
            ScriptCommand::SetHazeDetectionRangeFraction { name, fraction } => {
                use bsengine_core::Haze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hz) = world.get_mut::<Haze>(e) {
                        hz.detection_range_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetHazeEnabled { name, enabled } => {
                use bsengine_core::Haze;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hz) = world.get_mut::<Haze>(e) {
                        hz.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyHeat { name, amount } => {
                use bsengine_core::{Heat, ThermalState};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ht) = world.get_mut::<Heat>(e) {
                        ht.temperature += amount * ht.resistance;
                        ht.state = if ht.temperature >= ht.heat_threshold {
                            ThermalState::Overheated
                        } else if ht.temperature <= ht.cold_threshold {
                            ThermalState::Frozen
                        } else {
                            ThermalState::Normal
                        };
                    }
                }
            }
            ScriptCommand::HeatApplyCold { name, amount } => {
                use bsengine_core::{Heat, ThermalState};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ht) = world.get_mut::<Heat>(e) {
                        ht.temperature -= amount * ht.resistance;
                        ht.state = if ht.temperature >= ht.heat_threshold {
                            ThermalState::Overheated
                        } else if ht.temperature <= ht.cold_threshold {
                            ThermalState::Frozen
                        } else {
                            ThermalState::Normal
                        };
                    }
                }
            }
            ScriptCommand::SetHeatTemperature { name, temperature } => {
                use bsengine_core::{Heat, ThermalState};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ht) = world.get_mut::<Heat>(e) {
                        ht.temperature = temperature;
                        ht.state = if ht.temperature >= ht.heat_threshold {
                            ThermalState::Overheated
                        } else if ht.temperature <= ht.cold_threshold {
                            ThermalState::Frozen
                        } else {
                            ThermalState::Normal
                        };
                    }
                }
            }
            ScriptCommand::SetHeatEnabled { name, enabled } => {
                use bsengine_core::Heat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ht) = world.get_mut::<Heat>(e) {
                        ht.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyHex { name, duration } => {
                use bsengine_core::Hex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hx) = world.get_mut::<Hex>(e) {
                        hx.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearHex { name } => {
                use bsengine_core::Hex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hx) = world.get_mut::<Hex>(e) {
                        hx.clear();
                    }
                }
            }
            ScriptCommand::SetHexReductionPerStack { name, reduction } => {
                use bsengine_core::Hex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hx) = world.get_mut::<Hex>(e) {
                        hx.reduction_per_stack = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetHexEnabled { name, enabled } => {
                use bsengine_core::Hex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut hx) = world.get_mut::<Hex>(e) {
                        hx.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyHobble { name, duration } => {
                use bsengine_core::Hobble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ho) = world.get_mut::<Hobble>(e) {
                        ho.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearHobble { name } => {
                use bsengine_core::Hobble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ho) = world.get_mut::<Hobble>(e) {
                        ho.clear();
                    }
                }
            }
            ScriptCommand::SetHobbleSpeedFraction { name, fraction } => {
                use bsengine_core::Hobble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ho) = world.get_mut::<Hobble>(e) {
                        ho.speed_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetHobblePreventsDash { name, prevents } => {
                use bsengine_core::Hobble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ho) = world.get_mut::<Hobble>(e) {
                        ho.prevents_dash = prevents;
                    }
                }
            }
            ScriptCommand::SetHobbleEnabled { name, enabled } => {
                use bsengine_core::Hobble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ho) = world.get_mut::<Hobble>(e) {
                        ho.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddIgniteStacks { name, amount } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.add_stacks(amount);
                    }
                }
            }
            ScriptCommand::RemoveIgniteStacks { name, amount } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.remove_stacks(amount);
                    }
                }
            }
            ScriptCommand::ExtinguishIgnite { name } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.extinguish();
                    }
                }
            }
            ScriptCommand::SetIgniteThreshold { name, threshold } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.threshold = threshold.max(0.0);
                    }
                }
            }
            ScriptCommand::SetIgniteDecayRate { name, rate } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetIgniteEnabled { name, enabled } => {
                use bsengine_core::Ignite;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ig) = world.get_mut::<Ignite>(e) {
                        ig.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyImbue { name, bonus_damage } => {
                use bsengine_core::Imbue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Imbue>(e) {
                        im.imbue(bonus_damage);
                    }
                }
            }
            ScriptCommand::ConsumeImbue { name } => {
                use bsengine_core::Imbue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Imbue>(e) {
                        im.consume();
                    }
                }
            }
            ScriptCommand::SetImbueEnabled { name, enabled } => {
                use bsengine_core::Imbue;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Imbue>(e) {
                        im.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddDamageImmunity { name, bit } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.add_damage_immunity(bit);
                    }
                }
            }
            ScriptCommand::RemoveDamageImmunity { name, bit } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.remove_damage_immunity(bit);
                    }
                }
            }
            ScriptCommand::AddEffectImmunity { name, bit } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.add_effect_immunity(bit);
                    }
                }
            }
            ScriptCommand::RemoveEffectImmunity { name, bit } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.remove_effect_immunity(bit);
                    }
                }
            }
            ScriptCommand::ClearAllImmunities { name } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.clear_all();
                    }
                }
            }
            ScriptCommand::SetImmuneEnabled { name, enabled } => {
                use bsengine_core::Immune;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut im) = world.get_mut::<Immune>(e) {
                        im.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetImpactEnabled { name, enabled } => {
                use bsengine_core::Impact;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ip) = world.get_mut::<Impact>(e) {
                        ip.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetImpactMinForce { name, min_force } => {
                use bsengine_core::Impact;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ip) = world.get_mut::<Impact>(e) {
                        ip.min_force = min_force.max(0.0);
                    }
                }
            }
            ScriptCommand::ResetImpactCount { name } => {
                use bsengine_core::Impact;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ip) = world.get_mut::<Impact>(e) {
                        ip.impact_count = 0;
                    }
                }
            }
            ScriptCommand::ActivateIntercept { name, duration } => {
                use bsengine_core::Intercept;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ic) = world.get_mut::<Intercept>(e) {
                        ic.activate(duration);
                    }
                }
            }
            ScriptCommand::DeactivateIntercept { name } => {
                use bsengine_core::Intercept;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ic) = world.get_mut::<Intercept>(e) {
                        ic.deactivate();
                    }
                }
            }
            ScriptCommand::SetInterceptRadius { name, radius } => {
                use bsengine_core::Intercept;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ic) = world.get_mut::<Intercept>(e) {
                        ic.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetInterceptDamageReduction { name, reduction } => {
                use bsengine_core::Intercept;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ic) = world.get_mut::<Intercept>(e) {
                        ic.damage_reduction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetInterceptEnabled { name, enabled } => {
                use bsengine_core::Intercept;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ic) = world.get_mut::<Intercept>(e) {
                        ic.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ForceInterrupt { name } => {
                use bsengine_core::Interrupt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ir) = world.get_mut::<Interrupt>(e) {
                        ir.force_interrupt();
                    }
                }
            }
            ScriptCommand::ResetInterrupt { name } => {
                use bsengine_core::Interrupt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ir) = world.get_mut::<Interrupt>(e) {
                        ir.reset();
                    }
                }
            }
            ScriptCommand::SetInterruptThreshold { name, threshold } => {
                use bsengine_core::Interrupt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ir) = world.get_mut::<Interrupt>(e) {
                        ir.threshold = threshold.max(0.0);
                    }
                }
            }
            ScriptCommand::SetInterruptResistance { name, resistance } => {
                use bsengine_core::Interrupt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ir) = world.get_mut::<Interrupt>(e) {
                        ir.resistance = resistance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetInterruptEnabled { name, enabled } => {
                use bsengine_core::Interrupt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ir) = world.get_mut::<Interrupt>(e) {
                        ir.enabled = enabled;
                    }
                }
            }
            ScriptCommand::Spawn(params) => {
                spawn_entity(world, params);
            }
            ScriptCommand::Destroy { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    world.despawn(e);
                }
            }
            ScriptCommand::PlaySound {
                id,
                path,
                volume,
                loop_,
            } => {
                let project_dir = world
                    .get_resource::<ProjectDir>()
                    .map(|pd| pd.0.clone())
                    .unwrap_or_default();
                let full_path = if project_dir.is_empty() {
                    path.clone()
                } else {
                    format!("{}/{}", project_dir, path)
                };
                match kira::sound::static_sound::StaticSoundData::from_file(&full_path) {
                    Ok(data) => {
                        use kira::Decibels;
                        let volume_db = 20.0_f32 * volume.max(1e-10_f32).log10();
                        let data = data.volume(Decibels(volume_db));
                        let data = if loop_ { data.loop_region(..) } else { data };
                        if let Some(mut audio) = world.get_resource_mut::<AudioWorld>() {
                            if let Some(handle) = audio.play(data) {
                                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>()
                                {
                                    handles.0.insert(id, handle);
                                }
                            }
                        }
                    }
                    Err(e) => tracing::warn!("[audio] failed to load {full_path}: {e}"),
                }
            }
            ScriptCommand::StopSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(mut handle) = handles.0.remove(&id) {
                        use kira::Tween;
                        let _ = handle.stop(Tween::default());
                    }
                }
            }
            ScriptCommand::PauseSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::Tween;
                        let _ = handle.pause(Tween::default());
                    }
                }
            }
            ScriptCommand::ResumeSound { id } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::Tween;
                        let _ = handle.resume(Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundVolume { id, db } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{Decibels, Tween};
                        let _ = handle.set_volume(Decibels(db), Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundPanning { id, panning } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{Panning, Tween};
                        let _ = handle.set_panning(Panning(panning), Tween::default());
                    }
                }
            }
            ScriptCommand::SetSoundPlaybackRate { id, rate } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        use kira::{PlaybackRate, Tween};
                        let _ =
                            handle.set_playback_rate(PlaybackRate(rate as f64), Tween::default());
                    }
                }
            }
            ScriptCommand::SeekSound { id, position } => {
                if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
                    if let Some(handle) = handles.0.get_mut(&id) {
                        handle.seek_to(position);
                    }
                }
            }
            ScriptCommand::SetHudText { id, text } => {
                if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
                    hud.0.insert(id, text);
                }
            }
            ScriptCommand::ClearHudText { id } => {
                if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
                    hud.0.remove(&id);
                }
            }
            ScriptCommand::LoadScene { path } => {
                world.insert_resource(PendingSceneLoad { path });
            }
            ScriptCommand::SetVisible { name, visible } => {
                let mut q = world.query::<(&Name, &mut Visible)>();
                for (n, mut v) in q.iter_mut(world) {
                    if n.0 == name {
                        v.is_visible = visible;
                        break;
                    }
                }
            }
            ScriptCommand::SetSkybox { path } => {
                let project_dir = world
                    .get_resource::<ProjectDir>()
                    .map(|pd| pd.0.clone())
                    .unwrap_or_default();
                let full_path = if project_dir.is_empty() {
                    path
                } else {
                    format!("{}/{}", project_dir, path)
                };
                world.insert_resource(SkyboxPath(Some(full_path)));
            }
            ScriptCommand::SetParent { child, parent } => {
                let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                let mut child_entity = None;
                let mut parent_entity = None;
                for (e, n) in q.iter(world) {
                    if n.0 == child {
                        child_entity = Some(e);
                    } else if n.0 == parent {
                        parent_entity = Some(e);
                    }
                }
                if let (Some(ce), Some(pe)) = (child_entity, parent_entity) {
                    world.entity_mut(ce).insert(Parent(pe));
                }
            }
            ScriptCommand::ClearParent { child } => {
                let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                let child_entity = q.iter(world).find(|(_, n)| n.0 == child).map(|(e, _)| e);
                if let Some(ce) = child_entity {
                    world.entity_mut(ce).remove::<Parent>();
                }
            }
            ScriptCommand::SetCursorVisible { visible } => {
                if let Some(mut cfg) = world.get_resource_mut::<CursorConfig>() {
                    cfg.visible = visible;
                } else {
                    world.insert_resource(CursorConfig {
                        visible,
                        locked: false,
                    });
                }
            }
            ScriptCommand::SetCursorLocked { locked } => {
                if let Some(mut cfg) = world.get_resource_mut::<CursorConfig>() {
                    cfg.locked = locked;
                } else {
                    world.insert_resource(CursorConfig {
                        visible: true,
                        locked,
                    });
                }
            }
            ScriptCommand::AddImpulse { name, fx, fy, fz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_impulse(e, Vec3::new(fx, fy, fz));
                }
            }
            ScriptCommand::AddImpulseAtPoint {
                name,
                fx,
                fy,
                fz,
                px,
                py,
                pz,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_impulse_at_point(e, Vec3::new(fx, fy, fz), Vec3::new(px, py, pz));
                }
            }
            ScriptCommand::AddForce { name, fx, fy, fz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_force(e, Vec3::new(fx, fy, fz));
                }
            }
            ScriptCommand::AddForceAtPoint {
                name,
                fx,
                fy,
                fz,
                px,
                py,
                pz,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_force_at_point(e, Vec3::new(fx, fy, fz), Vec3::new(px, py, pz));
                }
            }
            ScriptCommand::SetVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_linvel(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetVelocityX { name, vx } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(vx, cur.y, cur.z));
                }
            }
            ScriptCommand::SetVelocityY { name, vy } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(cur.x, vy, cur.z));
                }
            }
            ScriptCommand::SetVelocityZ { name, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, Vec3::new(cur.x, cur.y, vz));
                }
            }
            ScriptCommand::SetGravity { magnitude } => {
                if let Some(mut pw) = world.get_resource_mut::<PhysicsWorld>() {
                    pw.set_gravity(magnitude);
                }
            }
            ScriptCommand::SetAngularVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_angvel(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetAngularVelocityX { name, vx } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(vx, cur.y, cur.z));
                }
            }
            ScriptCommand::SetAngularVelocityY { name, vy } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(cur.x, vy, cur.z));
                }
            }
            ScriptCommand::SetAngularVelocityZ { name, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, Vec3::new(cur.x, cur.y, vz));
                }
            }
            ScriptCommand::AddVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_linvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_linvel(e, cur + Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddAngularVelocity { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    let cur = pw.get_angvel(e).unwrap_or(Vec3::ZERO);
                    pw.set_angvel(e, cur + Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddAngularImpulse { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.apply_torque_impulse(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::AddTorque { name, vx, vy, vz } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.add_torque(e, Vec3::new(vx, vy, vz));
                }
            }
            ScriptCommand::SetCCDEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_ccd_enabled(e, enabled);
                }
            }
            ScriptCommand::SetLinearDamping { name, damping } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_linear_damping(e, damping);
                }
            }
            ScriptCommand::SetAngularDamping { name, damping } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_angular_damping(e, damping);
                }
            }
            ScriptCommand::SetMass { name, mass } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_mass(e, mass);
                }
            }
            ScriptCommand::LockRotation {
                name,
                lock_x,
                lock_y,
                lock_z,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.lock_rotations(e, lock_x, lock_y, lock_z);
                }
            }
            ScriptCommand::LockTranslation {
                name,
                lock_x,
                lock_y,
                lock_z,
            } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.lock_translations(e, lock_x, lock_y, lock_z);
                }
            }
            ScriptCommand::WakeUp { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.wake_up(e);
                }
            }
            ScriptCommand::PutToSleep { name } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.put_to_sleep(e);
                }
            }
            ScriptCommand::RotateBy {
                name,
                rx,
                ry,
                rz,
                rw,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_xyzw(rx, ry, rz, rw).normalize();
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::RotateAroundAxis {
                name,
                ax,
                ay,
                az,
                angle_deg,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let axis = Vec3::new(ax, ay, az);
                        if axis.length_squared() > 1e-10 {
                            let delta =
                                Quat::from_axis_angle(axis.normalize(), angle_deg.to_radians());
                            t.rotation = (t.rotation * delta).normalize();
                        }
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEuler {
                name,
                pitch,
                yaw,
                roll,
            } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(
                            glam::EulerRot::XYZ,
                            pitch.to_radians(),
                            yaw.to_radians(),
                            roll.to_radians(),
                        );
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), 0.0, 0.0);
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, deg.to_radians(), 0.0);
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::AddRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let delta = Quat::from_euler(EulerRot::XYZ, 0.0, 0.0, deg.to_radians());
                        t.rotation = (t.rotation * delta).normalize();
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleX { name, x } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.x = x;
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleY { name, y } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.y = y;
                        break;
                    }
                }
            }
            ScriptCommand::SetScaleZ { name, z } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.z = z;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleX { name, dx } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.x += dx;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleY { name, dy } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.y += dy;
                        break;
                    }
                }
            }
            ScriptCommand::AddScaleZ { name, dz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale.z += dz;
                        break;
                    }
                }
            }
            ScriptCommand::AddScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale += Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerX { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (_, y, z) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, deg.to_radians(), y, z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerY { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, _, z) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, deg.to_radians(), z);
                        break;
                    }
                }
            }
            ScriptCommand::SetRotationEulerZ { name, deg } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        let (x, y, _) = t.rotation.to_euler(EulerRot::XYZ);
                        t.rotation = Quat::from_euler(EulerRot::XYZ, x, y, deg.to_radians());
                        break;
                    }
                }
            }
            ScriptCommand::MultiplyScale { name, sx, sy, sz } => {
                let mut q = world.query::<(&Name, &mut Transform)>();
                for (n, mut t) in q.iter_mut(world) {
                    if n.0 == name {
                        t.scale *= Vec3::new(sx, sy, sz);
                        break;
                    }
                }
            }
            ScriptCommand::AddTag { name, label } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tag) = world.get_mut::<Tag>(e) {
                        tag.add(label);
                    }
                }
            }
            ScriptCommand::RemoveTag { name, label } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tag) = world.get_mut::<Tag>(e) {
                        tag.remove(&label);
                    }
                }
            }
            ScriptCommand::SetKinematic { name, kinematic } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_body_type(e, kinematic);
                }
            }
            ScriptCommand::SetGravityScale { name, scale } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_gravity_scale(e, scale);
                }
            }
            ScriptCommand::SetColliderSensor { name, sensor } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_collider_sensor(e, sensor);
                }
            }
            ScriptCommand::SetRestitution { name, restitution } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_restitution(e, restitution);
                }
            }
            ScriptCommand::SetFriction { name, friction } => {
                let entity = {
                    let mut q = world.query::<(bevy_ecs::prelude::Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let (Some(e), Some(mut pw)) = (entity, world.get_resource_mut::<PhysicsWorld>())
                {
                    pw.set_friction(e, friction);
                }
            }
        }
    }
}

fn spawn_entity(world: &mut World, params: SpawnParams) {
    let prim = match params.primitive.as_str() {
        "Sphere" => Primitive::Sphere,
        "Plane" => Primitive::Plane,
        "Capsule" => Primitive::Capsule,
        _ => Primitive::Cube,
    };

    let transform = Transform {
        translation: Vec3::new(params.x, params.y, params.z),
        rotation: Quat::from_xyzw(params.rx, params.ry, params.rz, params.rw).normalize(),
        scale: Vec3::new(params.sx, params.sy, params.sz),
    };

    let mut cmd = world.spawn((
        Name(params.name.clone()),
        transform,
        GlobalTransform::default(),
        PrimitiveMesh(prim),
    ));

    let has_color = params.color.is_some() || params.emissive.is_some();
    if has_color {
        cmd.insert(Material {
            base_color: params.color.map(Vec3::from).unwrap_or(Vec3::ONE),
            emissive: params.emissive.map(Vec3::from).unwrap_or(Vec3::ZERO),
            ..Default::default()
        });
    }

    if let Some(script) = params.script {
        cmd.insert(ScriptPath(script));
    }
}

#[cfg(test)]
mod tests {
    use super::{ScriptRuntimeResource, ScriptingPlugin};
    use bsengine_app::new_app;

    #[test]
    fn scripting_plugin_registers_runtime() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());
        assert!(app
            .world()
            .get_non_send_resource::<ScriptRuntimeResource>()
            .is_some());
    }

    #[test]
    fn scripting_plugin_runtime_can_eval() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin::default());

        let result = app
            .world_mut()
            .get_non_send_resource_mut::<ScriptRuntimeResource>()
            .expect("ScriptRuntimeResource not found")
            .0
            .eval("40 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }
}
