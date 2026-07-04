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
    INTERCEPT_SNAPSHOT, INTERRUPT_SNAPSHOT, INVINCIBLE_SNAPSHOT, ISOLATE_SNAPSHOT, JEER_SNAPSHOT,
    JETPACK_SNAPSHOT, JOLT_SNAPSHOT, JOSTLE_SNAPSHOT, JUKE_SNAPSHOT, JUMP_SNAPSHOT,
    KEY_JUST_PRESSED_SNAPSHOT, KEY_JUST_RELEASED_SNAPSHOT, KEY_SNAPSHOT, KNEEL_SNAPSHOT,
    KNIT_SNAPSHOT, KNOCKBACK_SNAPSHOT, LACERATE_SNAPSHOT, LADEN_SNAPSHOT, LAMENT_SNAPSHOT,
    LANCE_SNAPSHOT, LAPSE_SNAPSHOT, LASH_SNAPSHOT, LATCH_SNAPSHOT, LAYER_SNAPSHOT, LEDGE_SNAPSHOT,
    LEECH_SNAPSHOT, LEVEL_SNAPSHOT, LIFETIME_SNAPSHOT, LINEAR_DAMPING_SNAPSHOT, LOOK_AT_SNAPSHOT,
    LUNGE_SNAPSHOT, LURE_SNAPSHOT, LURK_SNAPSHOT, MAGNET_SNAPSHOT, MAIM_SNAPSHOT, MALICE_SNAPSHOT,
    MANA_SNAPSHOT, MARK_SNAPSHOT, MASS_SNAPSHOT, MATERIAL_COLOR_SNAPSHOT,
    MATERIAL_EMISSIVE_SNAPSHOT, MATERIAL_METALLIC_SNAPSHOT, MATERIAL_ROUGHNESS_SNAPSHOT,
    MELEE_SNAPSHOT, MEND_SNAPSHOT, MERGE_SNAPSHOT, MESH_SNAPSHOT, MINIMAP_SNAPSHOT,
    MIRAGE_SNAPSHOT, MOMENTUM_SNAPSHOT, MORALE_SNAPSHOT, MORPH_SNAPSHOT, MOTION_BLUR_SNAPSHOT,
    MOUNT_SNAPSHOT, MOUSE_DELTA_SNAPSHOT, MOUSE_JUST_PRESSED_SNAPSHOT,
    MOUSE_JUST_RELEASED_SNAPSHOT, MOUSE_POS_SNAPSHOT, MOUSE_PRESSED_SNAPSHOT, MOVE_SPEED_SNAPSHOT,
    MUFFLE_SNAPSHOT, NAV_SNAPSHOT, NETWORK_ID_SNAPSHOT, NIMBLE_SNAPSHOT, NOTICE_SNAPSHOT,
    NOURISH_SNAPSHOT, NOVA_SNAPSHOT, NPC_SNAPSHOT, NULLIFY_SNAPSHOT, NUMB_SNAPSHOT,
    OBSTACLE_SNAPSHOT, OMEN_SNAPSHOT, ORBIT_SNAPSHOT, ORDEAL_SNAPSHOT, OSCILLATE_SNAPSHOT,
    OUTLAST_SNAPSHOT, OUTLINE_SNAPSHOT, OVERFLOW_SNAPSHOT, OVERHEAT_SNAPSHOT, OVERLOAD_SNAPSHOT,
    OVERPOWER_SNAPSHOT, OVERSHIELD_SNAPSHOT, PARENT_SNAPSHOT, PARRY_SNAPSHOT, PATIENCE_SNAPSHOT,
    PETRIFY_SNAPSHOT, PHASE_SNAPSHOT, PHYSICS_WORLD_PTR, PIERCE_SNAPSHOT, PIN_SNAPSHOT,
    PLEA_SNAPSHOT, PLOY_SNAPSHOT, PLUCK_SNAPSHOT, POISE_SNAPSHOT, POISON_SNAPSHOT, POUNCE_SNAPSHOT,
    PROJECTILE_SNAPSHOT, PRONE_SNAPSHOT, PROTECT_SNAPSHOT, PROUD_SNAPSHOT, PROVOKE_SNAPSHOT,
    PROWL_SNAPSHOT, PULSE_SNAPSHOT, QUEST_SNAPSHOT, RADAR_SNAPSHOT, RAGE_SNAPSHOT, RALLY_SNAPSHOT,
    RAMPAGE_SNAPSHOT, RAVAGE_SNAPSHOT, REAVE_SNAPSHOT, REBOUND_SNAPSHOT, RECHARGE_SNAPSHOT,
    RECKLESS_SNAPSHOT, RECLUSE_SNAPSHOT, RECOIL_SNAPSHOT, REFLECT_SNAPSHOT, REFLEX_SNAPSHOT,
    REGEN_SNAPSHOT, REPEL_SNAPSHOT, REPOSE_SNAPSHOT, RESPAWN_SNAPSHOT, RESTITUTION_SNAPSHOT,
    RETALIATE_SNAPSHOT, REVEAL_SNAPSHOT, REVENGE_SNAPSHOT, REVIVE_SNAPSHOT, RICOCHET_SNAPSHOT,
    RIFLE_SNAPSHOT, ROOT_SNAPSHOT, ROT_SNAPSHOT, ROUT_SNAPSHOT, RUPTURE_SNAPSHOT, SCALD_SNAPSHOT,
    SCAN_SNAPSHOT, SCAR_SNAPSHOT, SCATTER_SNAPSHOT, SCOPE_SNAPSHOT, SCORCH_SNAPSHOT,
    SCREEN_SHAKE_SNAPSHOT, SCREEN_SIZE_SNAPSHOT, SHEAR_SNAPSHOT, SHIELD_BREAK_SNAPSHOT,
    SHIELD_SNAPSHOT, SHOCK_SNAPSHOT, SHRIVEL_SNAPSHOT, SHROUD_SNAPSHOT, SHUNT_SNAPSHOT,
    SILENCE_SNAPSHOT, SIPHON_SNAPSHOT, SLAM_SNAPSHOT, SLAY_SNAPSHOT, SLEEP_SNAPSHOT,
    SLIDE_SNAPSHOT, SLIME_SNAPSHOT, SLINK_SNAPSHOT, SLOW_MO_SNAPSHOT, SLOW_SNAPSHOT,
    SMOKE_SNAPSHOT, SNARE_SNAPSHOT, SOAK_SNAPSHOT, SOUND_POSITION_SNAPSHOT, SOUND_STATE_SNAPSHOT,
    SPAWN_POINT_SNAPSHOT, SPIKE_SNAPSHOT, SPLINTER_SNAPSHOT, SPRING_SNAPSHOT, SPRINT_SNAPSHOT,
    STAGGER_SNAPSHOT, STAKE_SNAPSHOT, STALK_SNAPSHOT, STAMINA_SNAPSHOT, STANCE_SNAPSHOT,
    STATUS_EFFECT_SNAPSHOT, STAT_SNAPSHOT, STEALTH_SNAPSHOT, STOMP_SNAPSHOT, STRIDE_SNAPSHOT,
    STRIFE_SNAPSHOT, STUMBLE_SNAPSHOT, STUN_SNAPSHOT, SULK_SNAPSHOT, SUNDER_SNAPSHOT,
    SUPPRESS_SNAPSHOT, SURGE_SNAPSHOT, SURROUND_SNAPSHOT, SURVIVE_SNAPSHOT, SWIM_SNAPSHOT,
    TAG_SNAPSHOT, TAINT_SNAPSHOT, TALLY_SNAPSHOT, TALON_SNAPSHOT, TAPER_SNAPSHOT, TAUNT_SNAPSHOT,
    THAW_SNAPSHOT, TIMER_SNAPSHOT, TIME_DELTA_SNAPSHOT, TIME_ELAPSED_SNAPSHOT, TINT_SNAPSHOT,
    TONE_MAP_SNAPSHOT, TRANSFORM_SNAPSHOT, TRIGGER_SNAPSHOT, TWEEN_SNAPSHOT, VELOCITY_SNAPSHOT,
    VIGNETTE_SNAPSHOT, VISIBLE_SNAPSHOT, WIND_SNAPSHOT, WORLD_TRANSFORM_SNAPSHOT, Z_INDEX_SNAPSHOT,
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
        use bsengine_core::Scald;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scald)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sc.heat_stacks,
                    sc.max_stacks,
                    sc.amplify_per_stack,
                    sc.stack_duration,
                    sc.just_scalded,
                    sc.just_cooled,
                    sc.enabled,
                ),
            );
        }
        SCALD_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Scan;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scan)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (sc.radius, sc.interval, sc.timer, sc.just_pulsed, sc.enabled),
            );
        }
        SCAN_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Scar;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scar)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sc.scars,
                    sc.max_scars,
                    sc.regen_penalty_per_scar,
                    sc.just_scarred,
                    sc.just_cleansed,
                    sc.enabled,
                ),
            );
        }
        SCAR_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Scatter;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scatter)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sc.duration,
                    sc.timer,
                    sc.spread_multiplier,
                    sc.extra_pellets,
                    sc.just_scattered,
                    sc.just_cleared,
                    sc.enabled,
                ),
            );
        }
        SCATTER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Scope;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scope)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sc.active,
                    sc.accuracy_bonus,
                    sc.range_bonus,
                    sc.move_speed_penalty,
                    sc.just_scoped,
                    sc.just_unscoped,
                    sc.enabled,
                ),
            );
        }
        SCOPE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Scorch;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Scorch)>();
        for (name, sc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sc.duration,
                    sc.timer,
                    sc.fire_amplify,
                    sc.dot_rate,
                    sc.just_scorched,
                    sc.just_healed,
                    sc.enabled,
                ),
            );
        }
        SCORCH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Shear;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shear)>();
        for (name, sh) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (sh.armor_penetration, sh.flat_penetration, sh.enabled),
            );
        }
        SHEAR_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Shock;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shock)>();
        for (name, sh) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sh.duration,
                    sh.timer,
                    sh.damage_per_second,
                    sh.interrupt_chance,
                    sh.just_shocked,
                    sh.just_discharged,
                    sh.enabled,
                ),
            );
        }
        SHOCK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Shrivel;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shrivel)>();
        for (name, sh) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sh.shrivel_fraction,
                    sh.shrivel_rate,
                    sh.recovery_rate,
                    sh.shrivel_factor,
                    sh.shriveled,
                    sh.just_afflicted,
                    sh.just_recovered,
                    sh.enabled,
                ),
            );
        }
        SHRIVEL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Shroud;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shroud)>();
        for (name, sh) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sh.charges,
                    sh.save_health_fraction,
                    sh.cooldown,
                    sh.cooldown_timer,
                    sh.just_saved,
                    sh.just_exhausted,
                    sh.enabled,
                ),
            );
        }
        SHROUD_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Shunt;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Shunt)>();
        for (name, sh) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sh.shunt_resistance,
                    sh.last_shunt_magnitude,
                    sh.shunts_received,
                    sh.cooldown_timer,
                    sh.cooldown,
                    sh.just_shunted,
                    sh.enabled,
                ),
            );
        }
        SHUNT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Silence;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Silence)>();
        for (name, si) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    si.duration,
                    si.timer,
                    si.just_silenced,
                    si.just_unsilenced,
                    si.enabled,
                ),
            );
        }
        SILENCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Siphon;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Siphon)>();
        for (name, si) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    si.duration,
                    si.timer,
                    si.drain_per_second,
                    si.return_fraction,
                    si.just_started,
                    si.just_ended,
                    si.enabled,
                ),
            );
        }
        SIPHON_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Slam;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Slam)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.phase as u32,
                    sl.slam_speed,
                    sl.impact_radius,
                    sl.impact_force,
                    sl.min_height,
                    sl.launch_height,
                    sl.recovery_time,
                    sl.recovery_timer,
                    sl.cooldown,
                    sl.cooldown_timer,
                    sl.wants_slam,
                    sl.enabled,
                ),
            );
        }
        SLAM_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Slay;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Slay)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.kill_count,
                    sl.threshold,
                    sl.trigger_count,
                    sl.just_triggered,
                    sl.enabled,
                ),
            );
        }
        SLAY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Slide;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Slide)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.phase as u32,
                    sl.direction.x,
                    sl.direction.y,
                    sl.direction.z,
                    sl.duration,
                    sl.brake_start,
                    sl.elapsed,
                    sl.slide_speed,
                    sl.wants_slide,
                    sl.crouch_scale,
                    sl.enabled,
                ),
            );
        }
        SLIDE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Slime;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Slime)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.slime_timer,
                    sl.slow_factor,
                    sl.slimed,
                    sl.just_slimed,
                    sl.just_cleansed,
                    sl.enabled,
                ),
            );
        }
        SLIME_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Slink;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Slink)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.active,
                    sl.speed_reduction,
                    sl.noise_reduction,
                    sl.just_engaged,
                    sl.just_disengaged,
                    sl.enabled,
                ),
            );
        }
        SLINK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::SlowMo;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &SlowMo)>();
        for (name, sm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sm.target_scale,
                    sm.current_scale,
                    sm.blend_speed,
                    sm.max_duration.unwrap_or(0.0),
                    sm.elapsed,
                    sm.charge,
                    sm.drain_rate,
                    sm.source as u32,
                    sm.enabled,
                ),
            );
        }
        SLOW_MO_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Smoke;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Smoke)>();
        for (name, sm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sm.style as u32,
                    sm.rate,
                    sm.color[0],
                    sm.color[1],
                    sm.color[2],
                    sm.color[3],
                    sm.particle_speed,
                    sm.spread_rate,
                    sm.particle_lifetime,
                    sm.offset.x,
                    sm.offset.y,
                    sm.offset.z,
                    sm.burst_duration.unwrap_or(0.0),
                    sm.elapsed,
                    sm.enabled,
                ),
            );
        }
        SMOKE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Snare;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Snare)>();
        for (name, sn) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sn.duration,
                    sn.timer,
                    sn.slow_fraction,
                    sn.escape_chance,
                    sn.just_snared,
                    sn.just_escaped,
                    sn.enabled,
                ),
            );
        }
        SNARE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Soak;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Soak)>();
        for (name, so) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    so.soak_level,
                    so.decay_rate,
                    so.fire_resistance,
                    so.lightning_amplify,
                    so.just_soaked,
                    so.just_dried,
                    so.enabled,
                ),
            );
        }
        SOAK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Spike;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Spike)>();
        for (name, sp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sp.duration,
                    sp.timer,
                    sp.damage,
                    sp.push_force,
                    sp.just_extended,
                    sp.just_retracted,
                    sp.enabled,
                ),
            );
        }
        SPIKE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Splinter;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Splinter)>();
        for (name, sp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sp.threshold,
                    sp.radius,
                    sp.damage_fraction,
                    sp.cooldown,
                    sp.cooldown_timer,
                    sp.just_splintered,
                    sp.enabled,
                ),
            );
        }
        SPLINTER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stagger;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stagger)>();
        for (name, sg) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sg.phase as u32,
                    sg.stagger_threshold,
                    sg.stagger_duration,
                    sg.stagger_timer,
                    sg.recovery_duration,
                    sg.recovery_timer,
                    sg.stagger_count,
                    sg.resist,
                    sg.just_staggered,
                    sg.enabled,
                ),
            );
        }
        STAGGER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stake;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stake)>();
        for (name, sk) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sk.active,
                    sk.hold_timer,
                    sk.min_hold,
                    sk.damage_bonus,
                    sk.just_staked,
                    sk.just_broke,
                    sk.enabled,
                ),
            );
        }
        STAKE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stalk;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stalk)>();
        for (name, st) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    st.active,
                    st.damage_multiplier,
                    st.just_began,
                    st.just_consumed,
                    st.enabled,
                ),
            );
        }
        STALK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stance;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stance)>();
        for (name, sa) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sa.current.clone() as u32,
                    sa.offense_bonus,
                    sa.defense_reduction,
                    sa.just_changed,
                    sa.enabled,
                ),
            );
        }
        STANCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stat;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stat)>();
        for (name, ss) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ss.base,
                    ss.bonus,
                    ss.multiplier,
                    ss.min.unwrap_or(0.0),
                    ss.max.unwrap_or(0.0),
                ),
            );
        }
        STAT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stealth;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stealth)>();
        for (name, se) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    se.base_visibility,
                    se.visibility_modifier,
                    se.noise_level,
                    se.noise_decay_rate,
                    se.sneaking,
                    se.sneak_visibility_scale,
                    se.enabled,
                ),
            );
        }
        STEALTH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stomp;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stomp)>();
        for (name, sm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sm.magnitude,
                    sm.impact_radius,
                    sm.damage_per_unit,
                    sm.just_stomped,
                    sm.enabled,
                ),
            );
        }
        STOMP_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stride;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stride)>();
        for (name, sd) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sd.stride_count,
                    sd.max_strides,
                    sd.speed_bonus,
                    sd.just_peaked,
                    sd.just_broke,
                    sd.enabled,
                ),
            );
        }
        STRIDE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Strife;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Strife)>();
        for (name, sf) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sf.strife,
                    sf.max_strife,
                    sf.gain_per_hit,
                    sf.decay_rate,
                    sf.just_peaked,
                    sf.enabled,
                ),
            );
        }
        STRIFE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Stumble;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Stumble)>();
        for (name, su) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    su.stumble_timer,
                    su.stumble_duration,
                    su.vulnerability_factor,
                    su.move_penalty,
                    su.stumble_count,
                    su.stumbling,
                    su.just_stumbled,
                    su.just_recovered,
                    su.enabled,
                ),
            );
        }
        STUMBLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Sulk;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Sulk)>();
        for (name, sl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sl.sulk_depth,
                    sl.sulk_rate,
                    sl.recovery_rate,
                    sl.support_penalty,
                    sl.sulking,
                    sl.just_sulked,
                    sl.just_snapped_out,
                    sl.enabled,
                ),
            );
        }
        SULK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Sunder;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Sunder)>();
        for (name, sd) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sd.shards,
                    sd.max_shards,
                    sd.damage_reduction_per_shard,
                    sd.just_sundered,
                    sd.enabled,
                ),
            );
        }
        SUNDER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Suppress;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Suppress)>();
        for (name, sp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sp.duration,
                    sp.timer,
                    sp.potency_fraction,
                    sp.blocks_ultimates,
                    sp.just_suppressed,
                    sp.just_lifted,
                    sp.enabled,
                ),
            );
        }
        SUPPRESS_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Surge;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Surge)>();
        for (name, sg) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sg.duration,
                    sg.timer,
                    sg.multiplier,
                    sg.just_surged,
                    sg.just_expired,
                    sg.enabled,
                ),
            );
        }
        SURGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Surround;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Surround)>();
        for (name, sr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sr.adjacent_count,
                    sr.encircle_threshold,
                    sr.defense_bonus,
                    sr.just_encircled,
                    sr.just_cleared,
                    sr.enabled,
                ),
            );
        }
        SURROUND_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Survive;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Survive)>();
        for (name, sv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (sv.charges, sv.max_charges, sv.just_survived, sv.enabled),
            );
        }
        SURVIVE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Swim;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Swim)>();
        for (name, sw) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    sw.state as u32,
                    sw.swim_speed,
                    sw.dive_speed,
                    sw.ascent_speed,
                    sw.breath_remaining,
                    sw.max_breath,
                    sw.breath_drain_rate,
                    sw.breath_regen_rate,
                    sw.depth,
                    sw.submerge_depth,
                    sw.wants_dive,
                    sw.wants_surface,
                    sw.enabled,
                ),
            );
        }
        SWIM_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Taint;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Taint)>();
        for (name, ta) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ta.duration,
                    ta.timer,
                    ta.healing_reduction,
                    ta.just_tainted,
                    ta.just_cleansed,
                    ta.enabled,
                ),
            );
        }
        TAINT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Tally;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Tally)>();
        for (name, tl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    tl.count,
                    tl.goal,
                    tl.just_incremented,
                    tl.just_completed,
                    tl.just_reset,
                    tl.enabled,
                ),
            );
        }
        TALLY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Talon;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Talon)>();
        for (name, tn) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    tn.active,
                    tn.grip_bonus,
                    tn.slip_resistance,
                    tn.just_gripped,
                    tn.just_released,
                    tn.enabled,
                ),
            );
        }
        TALON_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Taper;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Taper)>();
        for (name, tp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    tp.elapsed_time,
                    tp.max_reduction_time,
                    tp.damage_reduction,
                    tp.in_combat,
                    tp.just_peaked,
                    tp.enabled,
                ),
            );
        }
        TAPER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Taunt;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Taunt)>();
        for (name, tt) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    tt.is_active,
                    tt.radius,
                    tt.threat_boost,
                    tt.duration,
                    tt.timer,
                    tt.just_activated,
                    tt.just_expired,
                    tt.enabled,
                ),
            );
        }
        TAUNT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Thaw;
        let mut map = std::collections::HashMap::new();
        let mut q = world.query::<(&Name, &Thaw)>();
        for (name, th) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    th.thaw_fraction,
                    th.thaw_rate,
                    th.freeze_penalty,
                    th.just_thawed,
                    th.just_frozen,
                    th.enabled,
                ),
            );
        }
        THAW_SNAPSHOT.with(|s| *s.borrow_mut() = map);
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
    {
        use bsengine_core::Invincible;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Invincible)>();
        for (_, name, inv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    inv.stacks,
                    inv.timer,
                    inv.flash_interval,
                    inv.flash_visible,
                    inv.just_became_invincible,
                    inv.just_lost_invincibility,
                    inv.enabled,
                ),
            );
        }
        INVINCIBLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Isolate;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Isolate)>();
        for (_, name, iso) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    iso.duration,
                    iso.timer,
                    iso.buff_reduction,
                    iso.debuff_reduction,
                    iso.just_began,
                    iso.just_ended,
                    iso.enabled,
                ),
            );
        }
        ISOLATE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Jeer;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Jeer)>();
        for (_, name, jr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    jr.duration,
                    jr.timer,
                    jr.aim_penalty_rad,
                    jr.damage_fraction,
                    jr.just_jeered,
                    jr.just_rallied,
                    jr.enabled,
                ),
            );
        }
        JEER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Jetpack, JetpackState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Jetpack)>();
        for (_, name, jp) in q.iter(world) {
            let state_u32 = match jp.state {
                JetpackState::Idle => 0u32,
                JetpackState::Thrusting => 1u32,
                JetpackState::Depleted => 2u32,
            };
            map.insert(
                name.0.clone(),
                (
                    state_u32,
                    jp.thrust_direction.x,
                    jp.thrust_direction.y,
                    jp.thrust_direction.z,
                    jp.thrust_force,
                    jp.fuel,
                    jp.max_fuel,
                    jp.fuel_drain_rate,
                    jp.fuel_regen_rate,
                    jp.wants_thrust,
                    jp.regen_in_air,
                    jp.enabled,
                ),
            );
        }
        JETPACK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Jolt;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Jolt)>();
        for (_, name, jt) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    jt.duration,
                    jt.timer,
                    jt.chain_chance,
                    jt.chain_fraction,
                    jt.just_jolted,
                    jt.just_expired,
                    jt.enabled,
                ),
            );
        }
        JOLT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Jostle;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Jostle)>();
        for (_, name, jo) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    jo.accumulated,
                    jo.threshold,
                    jo.decay_rate,
                    jo.just_destabilized,
                    jo.enabled,
                ),
            );
        }
        JOSTLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Juke;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Juke)>();
        for (_, name, jk) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (jk.charges, jk.max_charges, jk.just_juked, jk.enabled),
            );
        }
        JUKE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Kneel;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Kneel)>();
        for (_, name, kn) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    kn.duration,
                    kn.timer,
                    kn.speed_fraction,
                    kn.just_kneeled,
                    kn.just_risen,
                    kn.enabled,
                ),
            );
        }
        KNEEL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Knit;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Knit)>();
        for (_, name, kt) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    kt.duration,
                    kt.timer,
                    kt.heal_rate,
                    kt.interruption_threshold,
                    kt.just_began,
                    kt.just_completed,
                    kt.just_interrupted,
                    kt.enabled,
                ),
            );
        }
        KNIT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lacerate;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lacerate)>();
        for (_, name, lc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lc.stacks,
                    lc.max_stacks,
                    lc.damage_per_stack_per_second,
                    lc.duration,
                    lc.timer,
                    lc.just_lacerated,
                    lc.just_closed,
                    lc.enabled,
                ),
            );
        }
        LACERATE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Laden;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Laden)>();
        for (_, name, ld) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (ld.current_load, ld.max_load, ld.speed_penalty, ld.enabled),
            );
        }
        LADEN_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lament;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lament)>();
        for (_, name, lm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lm.intensity,
                    lm.decay_rate,
                    lm.damage_penalty,
                    lm.speed_penalty,
                    lm.just_lamented,
                    lm.just_recovered,
                    lm.enabled,
                ),
            );
        }
        LAMENT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lance;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lance)>();
        for (_, name, la) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    la.duration,
                    la.timer,
                    la.base_damage,
                    la.speed_scale,
                    la.speed_threshold,
                    la.just_struck,
                    la.just_ended,
                    la.enabled,
                ),
            );
        }
        LANCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lapse;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lapse)>();
        for (_, name, lp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lp.lapsing,
                    lp.interval_timer,
                    lp.duration_timer,
                    lp.interval,
                    lp.lapse_duration,
                    lp.just_lapsed,
                    lp.just_focused,
                    lp.enabled,
                ),
            );
        }
        LAPSE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lash;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lash)>();
        for (_, name, la) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    la.pull_force,
                    la.damage,
                    la.duration,
                    la.timer,
                    la.just_connected,
                    la.just_released,
                    la.enabled,
                ),
            );
        }
        LASH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Latch;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Latch)>();
        for (_, name, la) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    la.active,
                    la.timer,
                    la.damage_per_second,
                    la.just_latched,
                    la.just_released,
                    la.enabled,
                ),
            );
        }
        LATCH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ledge;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ledge)>();
        for (_, name, le) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    le.phase as u32,
                    le.hang_position.x,
                    le.hang_position.y,
                    le.hang_position.z,
                    le.climb_duration,
                    le.climb_timer,
                    le.detection_range,
                    le.can_grab,
                    le.enabled,
                ),
            );
        }
        LEDGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Leech;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Leech)>();
        for (_, name, le) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    le.fraction,
                    le.flat,
                    le.last_leeched,
                    le.total_leeched,
                    le.just_leeched,
                    le.enabled,
                ),
            );
        }
        LEECH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lunge;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lunge)>();
        for (_, name, lu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lu.phase as u32,
                    lu.direction.x,
                    lu.direction.y,
                    lu.direction.z,
                    lu.target_point.x,
                    lu.target_point.y,
                    lu.target_point.z,
                    lu.speed,
                    lu.range,
                    lu.traveled,
                    lu.recovery_time,
                    lu.recovery_timer,
                    lu.cooldown,
                    lu.cooldown_timer,
                    lu.ground_only,
                    lu.just_lunged,
                    lu.hit_registered,
                    lu.enabled,
                ),
            );
        }
        LUNGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lure;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lure)>();
        for (_, name, lu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lu.state as u32,
                    lu.position.x,
                    lu.position.y,
                    lu.position.z,
                    lu.radius,
                    lu.strength,
                    lu.duration,
                    lu.timer,
                    lu.just_activated,
                    lu.just_expired,
                    lu.enabled,
                ),
            );
        }
        LURE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Lurk;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Lurk)>();
        for (_, name, lu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    lu.detection_range_fraction,
                    lu.ambush_multiplier,
                    lu.lurking,
                    lu.just_lurked,
                    lu.just_struck,
                    lu.enabled,
                ),
            );
        }
        LURK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Magnet;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Magnet)>();
        for (_, name, mg) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mg.mode as u32,
                    mg.radius,
                    mg.strength,
                    mg.falloff,
                    mg.affects_projectiles,
                    mg.affects_entities,
                    mg.enabled,
                ),
            );
        }
        MAGNET_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Maim;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Maim)>();
        for (_, name, ma) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ma.stacks,
                    ma.max_stacks,
                    ma.speed_fraction_per_stack,
                    ma.bleed_per_stack_per_second,
                    ma.just_maimed,
                    ma.just_healed,
                    ma.enabled,
                ),
            );
        }
        MAIM_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Malice;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Malice)>();
        for (_, name, ml) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ml.stacks,
                    ml.max_stacks,
                    ml.damage_amplify_per_stack,
                    ml.decay_interval,
                    ml.decay_timer,
                    ml.just_stacked,
                    ml.just_cleared,
                    ml.enabled,
                ),
            );
        }
        MALICE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Mark;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Mark)>();
        for (_, name, mk) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mk.marks.len() as u32,
                    mk.total_damage_bonus(),
                    mk.just_marked,
                    mk.just_unmarked,
                    mk.enabled,
                ),
            );
        }
        MARK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Melee;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Melee)>();
        for (_, name, me) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    me.phase as u32,
                    me.attack_direction.x,
                    me.attack_direction.y,
                    me.attack_direction.z,
                    me.reach,
                    me.arc_angle,
                    me.windup_time,
                    me.active_time,
                    me.recovery_time,
                    me.timer,
                    me.hit_count,
                    me.max_hits,
                    me.combo_step,
                    me.combo_buffered,
                    me.can_cancel_recovery,
                    me.enabled,
                ),
            );
        }
        MELEE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Mend;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Mend)>();
        for (_, name, mn) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (mn.mend_pool, mn.rate, mn.just_depleted, mn.enabled),
            );
        }
        MEND_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Merge;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Merge)>();
        for (_, name, mg) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mg.can_merge,
                    mg.merge_weight,
                    mg.max_weight,
                    mg.just_merged,
                    mg.enabled,
                ),
            );
        }
        MERGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Mesh;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Mesh)>();
        for (_, name, me) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    me.path.clone(),
                    me.submesh_index as u32,
                    me.cast_shadow,
                    me.receive_shadow,
                ),
            );
        }
        MESH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Minimap;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Minimap)>();
        for (_, name, mm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mm.icon.clone(),
                    mm.color[0],
                    mm.color[1],
                    mm.color[2],
                    mm.color[3],
                    mm.size,
                    mm.category.clone(),
                    mm.rotate_with_entity,
                    mm.clamp_to_edge,
                    mm.enabled,
                ),
            );
        }
        MINIMAP_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Mirage;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Mirage)>();
        for (_, name, mi) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mi.duration,
                    mi.timer,
                    mi.misdirect_chance,
                    mi.just_created,
                    mi.just_faded,
                    mi.enabled,
                ),
            );
        }
        MIRAGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Momentum;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Momentum)>();
        for (_, name, mo) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mo.current.x,
                    mo.current.y,
                    mo.current.z,
                    mo.damping,
                    mo.max_speed,
                    mo.enabled,
                ),
            );
        }
        MOMENTUM_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Morale;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Morale)>();
        for (_, name, mo) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mo.morale,
                    mo.decay_rate,
                    mo.damage_bonus,
                    mo.speed_bonus,
                    mo.just_peaked,
                    mo.just_broke,
                    mo.enabled,
                ),
            );
        }
        MORALE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Morph;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Morph)>();
        for (_, name, mo) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mo.form,
                    mo.target_form,
                    mo.morph_time,
                    mo.morph_timer,
                    mo.is_morphing,
                    mo.just_started,
                    mo.just_finished,
                    mo.enabled,
                ),
            );
        }
        MORPH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Mount;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Mount)>();
        for (_, name, mt) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mt.rider_count() as u32,
                    mt.max_riders as u32,
                    mt.speed_scale,
                    mt.forced_dismount_damage.unwrap_or(-1.0),
                    mt.enabled,
                ),
            );
        }
        MOUNT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Muffle;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Muffle)>();
        for (_, name, mu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    mu.duration,
                    mu.timer,
                    mu.sound_radius_fraction,
                    mu.just_muffled,
                    mu.just_unmuffled,
                    mu.enabled,
                ),
            );
        }
        MUFFLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{NetworkAuthority, NetworkId};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &NetworkId)>();
        for (_, name, nid) in q.iter(world) {
            let (auth_kind, peer_id_str) = match nid.authority {
                NetworkAuthority::Server => (0u32, String::new()),
                NetworkAuthority::Client { peer_id } => (1u32, peer_id.to_string()),
                NetworkAuthority::Local => (2u32, String::new()),
            };
            map.insert(name.0.clone(), (nid.id.to_string(), auth_kind, peer_id_str));
        }
        NETWORK_ID_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Nimble;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Nimble)>();
        for (_, name, ni) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ni.duration,
                    ni.timer,
                    ni.dodge_chance,
                    ni.speed_bonus_fraction,
                    ni.just_quickened,
                    ni.just_faded,
                    ni.enabled,
                ),
            );
        }
        NIMBLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Notice, NoticeState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Notice)>();
        for (_, name, no) in q.iter(world) {
            let state_u32 = match no.state {
                NoticeState::Unaware => 0u32,
                NoticeState::Alert => 1u32,
                NoticeState::Alarmed => 2u32,
                NoticeState::Searching => 3u32,
            };
            map.insert(
                name.0.clone(),
                (
                    state_u32,
                    no.suspicion,
                    no.suspicion_decay_rate,
                    no.alert_threshold,
                    no.alarm_threshold,
                    no.last_known_position.x,
                    no.last_known_position.y,
                    no.last_known_position.z,
                    no.investigate_timer,
                    no.max_investigate_time,
                    no.has_last_known,
                    no.enabled,
                ),
            );
        }
        NOTICE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Nourish;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Nourish)>();
        for (_, name, no) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    no.satiety,
                    no.decay_rate,
                    no.regen_scale,
                    no.just_starved,
                    no.enabled,
                ),
            );
        }
        NOURISH_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Nova;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Nova)>();
        for (_, name, nv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    nv.charge_time,
                    nv.charge_timer,
                    nv.radius,
                    nv.damage,
                    nv.just_primed,
                    nv.just_discharged,
                    nv.enabled,
                ),
            );
        }
        NOVA_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Npc, NpcRole, NpcState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Npc)>();
        for (_, name, np) in q.iter(world) {
            let role_u32 = match np.role {
                NpcRole::Civilian => 0u32,
                NpcRole::Guard => 1u32,
                NpcRole::Creature => 2u32,
                NpcRole::Vendor => 3u32,
                NpcRole::Scripted => 4u32,
            };
            let state_u32 = match np.state {
                NpcState::Idle => 0u32,
                NpcState::Patrolling => 1u32,
                NpcState::Investigating => 2u32,
                NpcState::Alerted => 3u32,
                NpcState::Engaging => 4u32,
                NpcState::Fleeing => 5u32,
                NpcState::Interacting => 6u32,
                NpcState::Dead => 7u32,
            };
            let template_id = np.template_id.clone().unwrap_or_default();
            map.insert(
                name.0.clone(),
                (
                    role_u32,
                    state_u32,
                    np.display_name.clone(),
                    template_id,
                    np.faction_id,
                    np.alert,
                    np.alert_decay,
                    np.enabled,
                ),
            );
        }
        NPC_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Nullify;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Nullify)>();
        for (_, name, nu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    nu.duration,
                    nu.timer,
                    nu.blocks_buffs,
                    nu.blocks_debuffs,
                    nu.just_activated,
                    nu.just_expired,
                    nu.enabled,
                ),
            );
        }
        NULLIFY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Numb;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Numb)>();
        for (_, name, nu) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    nu.duration,
                    nu.timer,
                    nu.damage_fraction,
                    nu.just_numbed,
                    nu.just_worn_off,
                    nu.enabled,
                ),
            );
        }
        NUMB_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Obstacle, ObstacleShape};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Obstacle)>();
        for (_, name, ob) in q.iter(world) {
            let (kind, p1, p2, p3) = match &ob.shape {
                ObstacleShape::Circle { radius } => (0u32, *radius, 0.0f32, 0.0f32),
                ObstacleShape::Box { half_x, half_z } => (1u32, *half_x, *half_z, 0.0f32),
                ObstacleShape::Capsule { radius, height } => (2u32, *radius, *height, 0.0f32),
            };
            let bounding = ob.bounding_radius();
            map.insert(
                name.0.clone(),
                (
                    kind,
                    p1,
                    p2,
                    p3,
                    ob.dynamic,
                    ob.carve_depth,
                    bounding,
                    ob.enabled,
                ),
            );
        }
        OBSTACLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Omen;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Omen)>();
        for (_, name, om) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    om.stacks,
                    om.max_stacks,
                    om.damage_multiplier_per_stack,
                    om.just_stacked,
                    om.just_consumed,
                    om.enabled,
                ),
            );
        }
        OMEN_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Orbit;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Orbit)>();
        for (_, name, or) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    or.radius,
                    or.speed,
                    or.angle,
                    or.axis.x,
                    or.axis.y,
                    or.axis.z,
                    or.altitude,
                    or.enabled,
                ),
            );
        }
        ORBIT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ordeal;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ordeal)>();
        for (_, name, od) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    od.duration,
                    od.timer,
                    od.just_began,
                    od.just_endured,
                    od.just_failed,
                    od.enabled,
                ),
            );
        }
        ORDEAL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Oscillate, OscillateAxis};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Oscillate)>();
        for (_, name, os) in q.iter(world) {
            let axis_kind = match os.axis {
                OscillateAxis::Translation => 0u32,
                OscillateAxis::Rotation => 1u32,
            };
            let scalar_offset = if os.enabled {
                (os.phase + os.phase_offset).sin() * os.amplitude
            } else {
                0.0
            };
            map.insert(
                name.0.clone(),
                (
                    axis_kind,
                    os.direction.x,
                    os.direction.y,
                    os.direction.z,
                    os.amplitude,
                    os.frequency,
                    os.phase,
                    os.phase_offset,
                    scalar_offset,
                    os.enabled,
                ),
            );
        }
        OSCILLATE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Outlast;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Outlast)>();
        for (_, name, ol) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ol.combat_time,
                    ol.max_bonus_time,
                    ol.defense_bonus,
                    ol.in_combat,
                    ol.just_peaked,
                    ol.enabled,
                ),
            );
        }
        OUTLAST_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Overflow;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Overflow)>();
        for (_, name, of) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    of.current,
                    of.max_pool,
                    of.decay_rate,
                    of.just_gained,
                    of.just_depleted,
                    of.enabled,
                ),
            );
        }
        OVERFLOW_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Overheat, OverheatState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Overheat)>();
        for (_, name, oh) in q.iter(world) {
            let state_u32 = match oh.state {
                OverheatState::Normal => 0u32,
                OverheatState::Warning => 1u32,
                OverheatState::Overheated => 2u32,
                OverheatState::Cooling => 3u32,
            };
            map.insert(
                name.0.clone(),
                (
                    state_u32,
                    oh.heat,
                    oh.max_heat,
                    oh.warn_threshold,
                    oh.cool_threshold,
                    oh.cool_rate,
                    oh.forced_cool_rate,
                    oh.just_overheated,
                    oh.just_cooled,
                    oh.enabled,
                ),
            );
        }
        OVERHEAT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Overload;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Overload)>();
        for (_, name, ol) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ol.duration,
                    ol.timer,
                    ol.cost_multiplier,
                    ol.just_overloaded,
                    ol.just_recovered,
                    ol.enabled,
                ),
            );
        }
        OVERLOAD_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Overpower;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Overpower)>();
        for (_, name, op) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    op.duration,
                    op.timer,
                    op.armor_penetration,
                    op.just_overpowered,
                    op.just_faded,
                    op.enabled,
                ),
            );
        }
        OVERPOWER_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Overshield;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Overshield)>();
        for (_, name, os) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    os.current,
                    os.max_overshield,
                    os.decay_rate,
                    os.just_granted,
                    os.just_depleted,
                    os.enabled,
                ),
            );
        }
        OVERSHIELD_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Parry, ParryState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Parry)>();
        for (_, name, pa) in q.iter(world) {
            let state_kind: u32 = match pa.state {
                ParryState::Idle => 0,
                ParryState::Active => 1,
                ParryState::Success => 2,
                ParryState::Missed => 3,
            };
            map.insert(
                name.0.clone(),
                (
                    state_kind,
                    pa.startup_duration,
                    pa.active_duration,
                    pa.recovery_duration,
                    pa.timer,
                    pa.parry_count,
                    pa.just_opened,
                    pa.just_succeeded,
                    pa.just_missed,
                    pa.just_finished,
                    pa.enabled,
                ),
            );
        }
        PARRY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Patience;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Patience)>();
        for (_, name, pa) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pa.patience_level,
                    pa.max_patience,
                    pa.patience_bonus,
                    pa.just_primed,
                    pa.just_spent,
                    pa.enabled,
                ),
            );
        }
        PATIENCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Petrify;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Petrify)>();
        for (_, name, pe) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pe.duration,
                    pe.timer,
                    pe.armor_bonus,
                    pe.just_petrified,
                    pe.just_unpetrified,
                    pe.enabled,
                ),
            );
        }
        PETRIFY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Phase;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Phase)>();
        for (_, name, ph) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ph.is_phased,
                    ph.duration,
                    ph.timer,
                    ph.cooldown,
                    ph.cooldown_timer,
                    ph.just_phased,
                    ph.just_unphased,
                    ph.enabled,
                ),
            );
        }
        PHASE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Pierce;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Pierce)>();
        for (_, name, pi) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pi.max_pierce,
                    pi.pierce_chance,
                    pi.pierced_this_attack,
                    pi.just_pierced,
                    pi.enabled,
                ),
            );
        }
        PIERCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Pin;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Pin)>();
        for (_, name, pi) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pi.active,
                    pi.timer,
                    pi.duration,
                    pi.knockback_immune,
                    pi.just_pinned,
                    pi.just_freed,
                    pi.enabled,
                ),
            );
        }
        PIN_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Plea;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Plea)>();
        for (_, name, pl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pl.duration,
                    pl.timer,
                    pl.avoidance_chance,
                    pl.just_began,
                    pl.just_ended,
                    pl.enabled,
                ),
            );
        }
        PLEA_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ploy;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ploy)>();
        for (_, name, pl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pl.active,
                    pl.timer,
                    pl.just_began,
                    pl.just_ended,
                    pl.enabled,
                ),
            );
        }
        PLOY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Pluck;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Pluck)>();
        for (_, name, pl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pl.hp_threshold,
                    pl.crit_bonus,
                    pl.pluck_active,
                    pl.just_triggered,
                    pl.just_recovered,
                    pl.enabled,
                ),
            );
        }
        PLUCK_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Poise;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Poise)>();
        for (_, name, po) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    po.current,
                    po.max,
                    po.regen_rate,
                    po.broken,
                    po.just_broken,
                    po.just_restored,
                    po.enabled,
                ),
            );
        }
        POISE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Pounce;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Pounce)>();
        for (_, name, po) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    po.duration,
                    po.timer,
                    po.damage,
                    po.knockdown_duration,
                    po.min_range,
                    po.max_range,
                    po.just_leaped,
                    po.just_landed,
                    po.enabled,
                ),
            );
        }
        POUNCE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Prone;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Prone)>();
        for (_, name, pr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pr.is_prone,
                    pr.stand_up_duration,
                    pr.stand_up_timer,
                    pr.movement_penalty,
                    pr.attack_penalty,
                    pr.just_fell_prone,
                    pr.just_stood_up,
                    pr.enabled,
                ),
            );
        }
        PRONE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Protect;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Protect)>();
        for (_, name, pr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pr.duration,
                    pr.timer,
                    pr.guard_radius,
                    pr.redirect_fraction,
                    pr.just_began,
                    pr.just_ended,
                    pr.enabled,
                ),
            );
        }
        PROTECT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Proud;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Proud)>();
        for (_, name, pr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pr.hp_threshold,
                    pr.damage_bonus,
                    pr.prideful,
                    pr.just_humbled,
                    pr.just_restored,
                    pr.enabled,
                ),
            );
        }
        PROUD_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Provoke;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Provoke)>();
        for (_, name, pr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pr.duration,
                    pr.timer,
                    pr.aggro_multiplier,
                    pr.radius,
                    pr.just_provoked,
                    pr.just_expired,
                    pr.enabled,
                ),
            );
        }
        PROVOKE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Prowl;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Prowl)>();
        for (_, name, pr) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    pr.duration,
                    pr.timer,
                    pr.speed_bonus_fraction,
                    pr.ambush_damage_multiplier,
                    pr.ambush_consumed,
                    pr.just_prowling,
                    pr.just_faded,
                    pr.enabled,
                ),
            );
        }
        PROWL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Pulse, PulseMode};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Pulse)>();
        for (_, name, pu) in q.iter(world) {
            let mode_kind: u32 = match pu.mode {
                PulseMode::Oneshot => 0,
                PulseMode::Repeating => 1,
            };
            map.insert(
                name.0.clone(),
                (
                    mode_kind,
                    pu.is_active,
                    pu.radius,
                    pu.max_radius,
                    pu.interval,
                    pu.timer,
                    pu.falloff,
                    pu.pulse_count,
                    pu.just_pulsed,
                    pu.enabled,
                ),
            );
        }
        PULSE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Quest, QuestState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Quest)>();
        for (_, name, qu) in q.iter(world) {
            let state: u32 = match qu.state {
                QuestState::Active => 0,
                QuestState::ReadyToComplete => 1,
                QuestState::Completed => 2,
                QuestState::Abandoned => 3,
            };
            map.insert(name.0.clone(), (state, qu.xp_reward, qu.enabled));
        }
        QUEST_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Radar;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Radar)>();
        for (_, name, rd) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (rd.range, rd.scan_interval, rd.scan_timer, rd.enabled),
            );
        }
        RADAR_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Rage, RagePhase};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rage)>();
        for (_, name, rg) in q.iter(world) {
            let phase: u32 = match rg.phase {
                RagePhase::Calm => 0,
                RagePhase::Building => 1,
                RagePhase::Raging => 2,
                RagePhase::Cooling => 3,
            };
            map.insert(
                name.0.clone(),
                (
                    phase,
                    rg.rage,
                    rg.max_rage,
                    rg.rage_per_damage,
                    rg.activation_threshold,
                    rg.damage_multiplier,
                    rg.defense_multiplier,
                    rg.just_entered_rage,
                    rg.just_left_rage,
                    rg.enabled,
                ),
            );
        }
        RAGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rally;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rally)>();
        for (_, name, ra) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ra.duration,
                    ra.timer,
                    ra.aura_radius,
                    ra.speed_bonus_fraction,
                    ra.damage_bonus_fraction,
                    ra.just_rallied,
                    ra.just_ended,
                    ra.enabled,
                ),
            );
        }
        RALLY_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rampage;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rampage)>();
        for (_, name, rm) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rm.stacks,
                    rm.max_stacks,
                    rm.damage_per_stack,
                    rm.speed_per_stack,
                    rm.decay_interval,
                    rm.decay_timer,
                    rm.just_stacked,
                    rm.just_ended,
                    rm.enabled,
                ),
            );
        }
        RAMPAGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ravage;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ravage)>();
        for (_, name, rv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rv.active,
                    rv.timer,
                    rv.damage_bonus,
                    rv.attack_speed_bonus,
                    rv.just_triggered,
                    rv.just_expired,
                    rv.enabled,
                ),
            );
        }
        RAVAGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Reave;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Reave)>();
        for (_, name, re) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    re.duration,
                    re.timer,
                    re.leech_fraction,
                    re.just_reaving,
                    re.just_faded,
                    re.enabled,
                ),
            );
        }
        REAVE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rebound;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rebound)>();
        for (_, name, rb) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rb.rebound_coefficient,
                    rb.min_speed,
                    rb.last_rebound_speed,
                    rb.just_rebounded,
                    rb.enabled,
                ),
            );
        }
        REBOUND_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Recharge;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Recharge)>();
        for (_, name, rc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rc.current,
                    rc.max,
                    rc.rate,
                    rc.just_recharged,
                    rc.just_depleted,
                    rc.enabled,
                ),
            );
        }
        RECHARGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Reckless;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Reckless)>();
        for (_, name, rk) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rk.duration,
                    rk.timer,
                    rk.damage_bonus,
                    rk.defense_penalty,
                    rk.just_entered,
                    rk.just_exited,
                    rk.enabled,
                ),
            );
        }
        RECKLESS_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Recluse;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Recluse)>();
        for (_, name, rl) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rl.is_alone,
                    rl.damage_bonus,
                    rl.defense_bonus,
                    rl.just_became_alone,
                    rl.just_joined_group,
                    rl.enabled,
                ),
            );
        }
        RECLUSE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Recoil;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Recoil)>();
        for (_, name, rc) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rc.kick_force,
                    rc.angular_kick,
                    rc.recovery_speed,
                    rc.yaw_fraction,
                    rc.max_position_offset,
                    rc.max_angular_offset,
                    rc.enabled,
                ),
            );
        }
        RECOIL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Reflect;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Reflect)>();
        for (_, name, rf) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rf.is_active,
                    rf.damage_multiplier,
                    rf.window_duration,
                    rf.window_timer,
                    rf.just_activated,
                    rf.just_reflected,
                    rf.just_closed,
                    rf.enabled,
                ),
            );
        }
        REFLECT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Reflex;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Reflex)>();
        for (_, name, rf) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rf.timer,
                    rf.just_triggered,
                    rf.just_evaded,
                    rf.just_missed,
                    rf.enabled,
                ),
            );
        }
        REFLEX_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Repel;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Repel)>();
        for (_, name, rp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rp.duration,
                    rp.timer,
                    rp.push_force,
                    rp.radius,
                    rp.just_activated,
                    rp.just_deactivated,
                    rp.enabled,
                ),
            );
        }
        REPEL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Repose;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Repose)>();
        for (_, name, rp) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rp.active,
                    rp.timer,
                    rp.regen_multiplier,
                    rp.just_began,
                    rp.just_ended,
                    rp.enabled,
                ),
            );
        }
        REPOSE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Respawn, RespawnState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Respawn)>();
        for (_, name, rs) in q.iter(world) {
            let state: u32 = match rs.state {
                RespawnState::Alive => 0,
                RespawnState::Pending => 1,
                RespawnState::Ready => 2,
                RespawnState::Forbidden => 3,
            };
            map.insert(
                name.0.clone(),
                (
                    state,
                    rs.delay,
                    rs.delay_timer,
                    rs.respawn_count,
                    rs.enabled,
                ),
            );
        }
        RESPAWN_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Retaliate;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Retaliate)>();
        for (_, name, rt) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rt.multiplier,
                    rt.max_charges,
                    rt.charges,
                    rt.just_charged,
                    rt.just_consumed,
                    rt.enabled,
                ),
            );
        }
        RETALIATE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Revenge;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Revenge)>();
        for (_, name, rv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rv.duration,
                    rv.timer,
                    rv.revenge_multiplier,
                    rv.trigger_fraction,
                    rv.triggered,
                    rv.just_triggered,
                    rv.just_ended,
                    rv.enabled,
                ),
            );
        }
        REVENGE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Reveal;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Reveal)>();
        for (_, name, rv) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    rv.duration,
                    rv.timer,
                    rv.radius,
                    rv.just_activated,
                    rv.just_expired,
                    rv.enabled,
                ),
            );
        }
        REVEAL_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::{Revive, ReviveState};
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Revive)>();
        for (_, name, rv) in q.iter(world) {
            let state: u32 = match rv.state {
                ReviveState::Alive => 0,
                ReviveState::Downed => 1,
                ReviveState::Reviving => 2,
                ReviveState::Dead => 3,
            };
            map.insert(
                name.0.clone(),
                (
                    state,
                    rv.down_duration,
                    rv.down_timer,
                    rv.revive_duration,
                    rv.revive_progress,
                    rv.revives_remaining,
                    rv.just_downed,
                    rv.just_revived,
                    rv.just_died,
                    rv.enabled,
                ),
            );
        }
        REVIVE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Ricochet;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Ricochet)>();
        for (_, name, ri) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ri.max_bounces,
                    ri.bounces_remaining,
                    ri.energy_retention,
                    ri.min_dot,
                    ri.just_bounced,
                    ri.enabled,
                ),
            );
        }
        RICOCHET_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rifle;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rifle)>();
        for (_, name, ri) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ri.min_range,
                    ri.peak_range,
                    ri.damage_bonus,
                    ri.point_blank_penalty,
                    ri.enabled,
                ),
            );
        }
        RIFLE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rot;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rot)>();
        for (_, name, ro) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ro.active,
                    ro.decay_rate,
                    ro.total_decayed,
                    ro.decay_cap,
                    ro.just_began,
                    ro.just_capped,
                    ro.enabled,
                ),
            );
        }
        ROT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rout;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rout)>();
        for (_, name, ro) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ro.duration,
                    ro.timer,
                    ro.flee_speed_multiplier,
                    ro.just_routed,
                    ro.just_recovered,
                    ro.enabled,
                ),
            );
        }
        ROUT_SNAPSHOT.with(|s| *s.borrow_mut() = map);
    }
    {
        use bsengine_core::Rupture;
        let mut map = HashMap::new();
        let mut q = world.query::<(Entity, &Name, &Rupture)>();
        for (_, name, ru) in q.iter(world) {
            map.insert(
                name.0.clone(),
                (
                    ru.stacks,
                    ru.max_stacks,
                    ru.damage_per_stack,
                    ru.just_maxed,
                    ru.enabled,
                ),
            );
        }
        RUPTURE_SNAPSHOT.with(|s| *s.borrow_mut() = map);
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
            ScriptCommand::GrantInvincible { name, duration } => {
                use bsengine_core::Invincible;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut inv) = world.get_mut::<Invincible>(e) {
                        inv.grant(duration);
                    }
                }
            }
            ScriptCommand::RevokeInvincible { name } => {
                use bsengine_core::Invincible;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut inv) = world.get_mut::<Invincible>(e) {
                        inv.revoke();
                    }
                }
            }
            ScriptCommand::SetInvincibleFlashInterval { name, interval } => {
                use bsengine_core::Invincible;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut inv) = world.get_mut::<Invincible>(e) {
                        inv.flash_interval = interval.max(0.0);
                    }
                }
            }
            ScriptCommand::SetInvincibleEnabled { name, enabled } => {
                use bsengine_core::Invincible;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut inv) = world.get_mut::<Invincible>(e) {
                        inv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SecludeIsolate { name, duration } => {
                use bsengine_core::Isolate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut iso) = world.get_mut::<Isolate>(e) {
                        iso.seclude(duration);
                    }
                }
            }
            ScriptCommand::RejoinIsolate { name } => {
                use bsengine_core::Isolate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut iso) = world.get_mut::<Isolate>(e) {
                        iso.rejoin();
                    }
                }
            }
            ScriptCommand::SetIsolateBuffReduction { name, reduction } => {
                use bsengine_core::Isolate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut iso) = world.get_mut::<Isolate>(e) {
                        iso.buff_reduction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetIsolateDebuffReduction { name, reduction } => {
                use bsengine_core::Isolate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut iso) = world.get_mut::<Isolate>(e) {
                        iso.debuff_reduction = reduction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetIsolateEnabled { name, enabled } => {
                use bsengine_core::Isolate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut iso) = world.get_mut::<Isolate>(e) {
                        iso.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyJeer { name, duration } => {
                use bsengine_core::Jeer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jr) = world.get_mut::<Jeer>(e) {
                        jr.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearJeer { name } => {
                use bsengine_core::Jeer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jr) = world.get_mut::<Jeer>(e) {
                        jr.clear();
                    }
                }
            }
            ScriptCommand::SetJeerAimPenalty { name, penalty_rad } => {
                use bsengine_core::Jeer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jr) = world.get_mut::<Jeer>(e) {
                        jr.aim_penalty_rad = penalty_rad.max(0.0);
                    }
                }
            }
            ScriptCommand::SetJeerDamageFraction { name, fraction } => {
                use bsengine_core::Jeer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jr) = world.get_mut::<Jeer>(e) {
                        jr.damage_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetJeerEnabled { name, enabled } => {
                use bsengine_core::Jeer;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jr) = world.get_mut::<Jeer>(e) {
                        jr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetJetpackWantsThrust { name, wants } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.wants_thrust = wants;
                    }
                }
            }
            ScriptCommand::RefuelJetpack { name } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.refuel();
                    }
                }
            }
            ScriptCommand::SetJetpackThrustForce { name, force } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.thrust_force = force.max(0.0);
                    }
                }
            }
            ScriptCommand::SetJetpackFuelDrainRate { name, rate } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.fuel_drain_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetJetpackFuelRegenRate { name, rate } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.fuel_regen_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetJetpackRegenInAir { name, regen_in_air } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.regen_in_air = regen_in_air;
                    }
                }
            }
            ScriptCommand::SetJetpackEnabled { name, enabled } => {
                use bsengine_core::Jetpack;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jp) = world.get_mut::<Jetpack>(e) {
                        jp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyJolt { name, duration } => {
                use bsengine_core::Jolt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jt) = world.get_mut::<Jolt>(e) {
                        jt.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearJolt { name } => {
                use bsengine_core::Jolt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jt) = world.get_mut::<Jolt>(e) {
                        jt.clear();
                    }
                }
            }
            ScriptCommand::SetJoltChainChance { name, chance } => {
                use bsengine_core::Jolt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jt) = world.get_mut::<Jolt>(e) {
                        jt.chain_chance = chance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetJoltChainFraction { name, fraction } => {
                use bsengine_core::Jolt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jt) = world.get_mut::<Jolt>(e) {
                        jt.chain_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetJoltEnabled { name, enabled } => {
                use bsengine_core::Jolt;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jt) = world.get_mut::<Jolt>(e) {
                        jt.enabled = enabled;
                    }
                }
            }
            ScriptCommand::JostleEntity { name, amount } => {
                use bsengine_core::Jostle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jo) = world.get_mut::<Jostle>(e) {
                        jo.jostle(amount);
                    }
                }
            }
            ScriptCommand::SetJostleThreshold { name, threshold } => {
                use bsengine_core::Jostle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jo) = world.get_mut::<Jostle>(e) {
                        jo.threshold = threshold.max(0.001);
                    }
                }
            }
            ScriptCommand::SetJostleDecayRate { name, rate } => {
                use bsengine_core::Jostle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jo) = world.get_mut::<Jostle>(e) {
                        jo.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetJostleEnabled { name, enabled } => {
                use bsengine_core::Jostle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jo) = world.get_mut::<Jostle>(e) {
                        jo.enabled = enabled;
                    }
                }
            }
            ScriptCommand::PrimeJuke { name } => {
                use bsengine_core::Juke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jk) = world.get_mut::<Juke>(e) {
                        jk.prime();
                    }
                }
            }
            ScriptCommand::ConsumeJuke { name } => {
                use bsengine_core::Juke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jk) = world.get_mut::<Juke>(e) {
                        jk.consume();
                    }
                }
            }
            ScriptCommand::SetJukeMaxCharges { name, max_charges } => {
                use bsengine_core::Juke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jk) = world.get_mut::<Juke>(e) {
                        jk.max_charges = max_charges.max(1);
                    }
                }
            }
            ScriptCommand::SetJukeEnabled { name, enabled } => {
                use bsengine_core::Juke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut jk) = world.get_mut::<Juke>(e) {
                        jk.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyKneel { name, duration } => {
                use bsengine_core::Kneel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kn) = world.get_mut::<Kneel>(e) {
                        kn.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearKneel { name } => {
                use bsengine_core::Kneel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kn) = world.get_mut::<Kneel>(e) {
                        kn.clear();
                    }
                }
            }
            ScriptCommand::SetKneelSpeedFraction { name, fraction } => {
                use bsengine_core::Kneel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kn) = world.get_mut::<Kneel>(e) {
                        kn.speed_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetKneelEnabled { name, enabled } => {
                use bsengine_core::Kneel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kn) = world.get_mut::<Kneel>(e) {
                        kn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginKnit { name, duration } => {
                use bsengine_core::Knit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kt) = world.get_mut::<Knit>(e) {
                        kt.begin(duration);
                    }
                }
            }
            ScriptCommand::InterruptKnit { name, damage } => {
                use bsengine_core::Knit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kt) = world.get_mut::<Knit>(e) {
                        kt.interrupt_if(damage);
                    }
                }
            }
            ScriptCommand::SetKnitHealRate { name, rate } => {
                use bsengine_core::Knit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kt) = world.get_mut::<Knit>(e) {
                        kt.heal_rate = rate;
                    }
                }
            }
            ScriptCommand::SetKnitInterruptionThreshold { name, threshold } => {
                use bsengine_core::Knit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kt) = world.get_mut::<Knit>(e) {
                        kt.interruption_threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetKnitEnabled { name, enabled } => {
                use bsengine_core::Knit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut kt) = world.get_mut::<Knit>(e) {
                        kt.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyLacerate { name, duration } => {
                use bsengine_core::Lacerate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lc) = world.get_mut::<Lacerate>(e) {
                        lc.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearLacerate { name } => {
                use bsengine_core::Lacerate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lc) = world.get_mut::<Lacerate>(e) {
                        lc.clear();
                    }
                }
            }
            ScriptCommand::SetLacerateMaxStacks { name, max_stacks } => {
                use bsengine_core::Lacerate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lc) = world.get_mut::<Lacerate>(e) {
                        lc.max_stacks = max_stacks.max(1);
                    }
                }
            }
            ScriptCommand::SetLacerateDamagePerStack { name, dps } => {
                use bsengine_core::Lacerate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lc) = world.get_mut::<Lacerate>(e) {
                        lc.damage_per_stack_per_second = dps;
                    }
                }
            }
            ScriptCommand::SetLacerateEnabled { name, enabled } => {
                use bsengine_core::Lacerate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lc) = world.get_mut::<Lacerate>(e) {
                        lc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddLaden { name, amount } => {
                use bsengine_core::Laden;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ld) = world.get_mut::<Laden>(e) {
                        ld.add_load(amount);
                    }
                }
            }
            ScriptCommand::RemoveLaden { name, amount } => {
                use bsengine_core::Laden;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ld) = world.get_mut::<Laden>(e) {
                        ld.remove_load(amount);
                    }
                }
            }
            ScriptCommand::SetLadenMaxLoad { name, max_load } => {
                use bsengine_core::Laden;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ld) = world.get_mut::<Laden>(e) {
                        ld.max_load = max_load;
                    }
                }
            }
            ScriptCommand::SetLadenSpeedPenalty { name, penalty } => {
                use bsengine_core::Laden;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ld) = world.get_mut::<Laden>(e) {
                        ld.speed_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetLadenEnabled { name, enabled } => {
                use bsengine_core::Laden;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ld) = world.get_mut::<Laden>(e) {
                        ld.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyLament { name, intensity } => {
                use bsengine_core::Lament;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lm) = world.get_mut::<Lament>(e) {
                        lm.apply(intensity);
                    }
                }
            }
            ScriptCommand::SetLamentDecayRate { name, rate } => {
                use bsengine_core::Lament;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lm) = world.get_mut::<Lament>(e) {
                        lm.decay_rate = rate;
                    }
                }
            }
            ScriptCommand::SetLamentDamagePenalty { name, penalty } => {
                use bsengine_core::Lament;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lm) = world.get_mut::<Lament>(e) {
                        lm.damage_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetLamentSpeedPenalty { name, penalty } => {
                use bsengine_core::Lament;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lm) = world.get_mut::<Lament>(e) {
                        lm.speed_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetLamentEnabled { name, enabled } => {
                use bsengine_core::Lament;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lm) = world.get_mut::<Lament>(e) {
                        lm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ThrustLance {
                name,
                current_speed,
                duration,
            } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.thrust(current_speed, duration);
                    }
                }
            }
            ScriptCommand::RetractLance { name } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.retract();
                    }
                }
            }
            ScriptCommand::SetLanceBaseDamage { name, damage } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.base_damage = damage;
                    }
                }
            }
            ScriptCommand::SetLanceSpeedScale { name, scale } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.speed_scale = scale;
                    }
                }
            }
            ScriptCommand::SetLanceSpeedThreshold { name, threshold } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.speed_threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetLanceEnabled { name, enabled } => {
                use bsengine_core::Lance;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lance>(e) {
                        la.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ResetLapse { name } => {
                use bsengine_core::Lapse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lp) = world.get_mut::<Lapse>(e) {
                        lp.reset();
                    }
                }
            }
            ScriptCommand::SetLapseInterval { name, interval } => {
                use bsengine_core::Lapse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lp) = world.get_mut::<Lapse>(e) {
                        lp.interval = interval;
                    }
                }
            }
            ScriptCommand::SetLapseDuration { name, duration } => {
                use bsengine_core::Lapse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lp) = world.get_mut::<Lapse>(e) {
                        lp.lapse_duration = duration;
                    }
                }
            }
            ScriptCommand::SetLapseEnabled { name, enabled } => {
                use bsengine_core::Lapse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lp) = world.get_mut::<Lapse>(e) {
                        lp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ConnectLash { name, duration } => {
                use bsengine_core::Lash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lash>(e) {
                        la.connect(duration);
                    }
                }
            }
            ScriptCommand::ReleaseLash { name } => {
                use bsengine_core::Lash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lash>(e) {
                        la.release();
                    }
                }
            }
            ScriptCommand::SetLashPullForce { name, force } => {
                use bsengine_core::Lash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lash>(e) {
                        la.pull_force = force;
                    }
                }
            }
            ScriptCommand::SetLashDamage { name, damage } => {
                use bsengine_core::Lash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lash>(e) {
                        la.damage = damage;
                    }
                }
            }
            ScriptCommand::SetLashEnabled { name, enabled } => {
                use bsengine_core::Lash;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Lash>(e) {
                        la.enabled = enabled;
                    }
                }
            }
            ScriptCommand::LatchEntity { name, duration } => {
                use bsengine_core::Latch;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Latch>(e) {
                        la.latch(duration);
                    }
                }
            }
            ScriptCommand::ReleaseLatch { name } => {
                use bsengine_core::Latch;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Latch>(e) {
                        la.release();
                    }
                }
            }
            ScriptCommand::SetLatchDamagePerSecond { name, dps } => {
                use bsengine_core::Latch;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Latch>(e) {
                        la.damage_per_second = dps;
                    }
                }
            }
            ScriptCommand::SetLatchEnabled { name, enabled } => {
                use bsengine_core::Latch;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut la) = world.get_mut::<Latch>(e) {
                        la.enabled = enabled;
                    }
                }
            }
            ScriptCommand::GrabLedge {
                name,
                hang_x,
                hang_y,
                hang_z,
                normal_x,
                normal_y,
                normal_z,
            } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.grab(
                            Vec3::new(hang_x, hang_y, hang_z),
                            Vec3::new(normal_x, normal_y, normal_z),
                        );
                    }
                }
            }
            ScriptCommand::ClimbLedge { name } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.climb_up();
                    }
                }
            }
            ScriptCommand::DropLedge { name } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.drop();
                    }
                }
            }
            ScriptCommand::ReleaseLedge { name } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.release();
                    }
                }
            }
            ScriptCommand::SetLedgeClimbDuration { name, duration } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.climb_duration = duration;
                    }
                }
            }
            ScriptCommand::SetLedgeDetectionRange { name, range } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.detection_range = range;
                    }
                }
            }
            ScriptCommand::SetLedgeEnabled { name, enabled } => {
                use bsengine_core::Ledge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Ledge>(e) {
                        le.enabled = enabled;
                    }
                }
            }
            ScriptCommand::NotifyLeechHit { name, damage } => {
                use bsengine_core::Leech;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Leech>(e) {
                        le.notify_hit(damage);
                    }
                }
            }
            ScriptCommand::SetLeechFraction { name, fraction } => {
                use bsengine_core::Leech;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Leech>(e) {
                        le.fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetLeechFlat { name, flat } => {
                use bsengine_core::Leech;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Leech>(e) {
                        le.flat = flat;
                    }
                }
            }
            ScriptCommand::SetLeechEnabled { name, enabled } => {
                use bsengine_core::Leech;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut le) = world.get_mut::<Leech>(e) {
                        le.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginLunge {
                name,
                target_x,
                target_y,
                target_z,
            } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.begin(Vec3::ZERO, Vec3::new(target_x, target_y, target_z), true);
                    }
                }
            }
            ScriptCommand::SetLungeSpeed { name, speed } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.speed = speed;
                    }
                }
            }
            ScriptCommand::SetLungeRange { name, range } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.range = range;
                    }
                }
            }
            ScriptCommand::SetLungeRecoveryTime { name, time } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.recovery_time = time;
                    }
                }
            }
            ScriptCommand::SetLungeCooldown { name, cooldown } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.cooldown = cooldown;
                    }
                }
            }
            ScriptCommand::SetLungeEnabled { name, enabled } => {
                use bsengine_core::Lunge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lunge>(e) {
                        lu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::DeployLure {
                name,
                pos_x,
                pos_y,
                pos_z,
            } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.deploy(Vec3::new(pos_x, pos_y, pos_z));
                    }
                }
            }
            ScriptCommand::DeactivateLure { name } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.deactivate();
                    }
                }
            }
            ScriptCommand::ResetLure { name } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.reset();
                    }
                }
            }
            ScriptCommand::SetLureRadius { name, radius } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.radius = radius;
                    }
                }
            }
            ScriptCommand::SetLureStrength { name, strength } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.strength = strength;
                    }
                }
            }
            ScriptCommand::SetLureDuration { name, duration } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.duration = duration;
                    }
                }
            }
            ScriptCommand::SetLureEnabled { name, enabled } => {
                use bsengine_core::Lure;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lure>(e) {
                        lu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::EnterLurk { name } => {
                use bsengine_core::Lurk;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lurk>(e) {
                        lu.enter();
                    }
                }
            }
            ScriptCommand::ExitLurk { name } => {
                use bsengine_core::Lurk;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lurk>(e) {
                        lu.exit();
                    }
                }
            }
            ScriptCommand::SetLurkDetectionRangeFraction { name, fraction } => {
                use bsengine_core::Lurk;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lurk>(e) {
                        lu.detection_range_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetLurkAmbushMultiplier { name, multiplier } => {
                use bsengine_core::Lurk;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lurk>(e) {
                        lu.ambush_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetLurkEnabled { name, enabled } => {
                use bsengine_core::Lurk;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut lu) = world.get_mut::<Lurk>(e) {
                        lu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetMagnetMode { name, mode_u32 } => {
                use bsengine_core::{Magnet, MagnetMode};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.mode = if mode_u32 == 0 {
                            MagnetMode::Attract
                        } else {
                            MagnetMode::Repel
                        };
                    }
                }
            }
            ScriptCommand::SetMagnetRadius { name, radius } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.radius = radius;
                    }
                }
            }
            ScriptCommand::SetMagnetStrength { name, strength } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.strength = strength;
                    }
                }
            }
            ScriptCommand::SetMagnetFalloff { name, falloff } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.falloff = falloff;
                    }
                }
            }
            ScriptCommand::SetMagnetAffectsProjectiles { name, affects } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.affects_projectiles = affects;
                    }
                }
            }
            ScriptCommand::SetMagnetAffectsEntities { name, affects } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.affects_entities = affects;
                    }
                }
            }
            ScriptCommand::SetMagnetEnabled { name, enabled } => {
                use bsengine_core::Magnet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Magnet>(e) {
                        mg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyMaim { name, amount } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.apply(amount);
                    }
                }
            }
            ScriptCommand::HealMaim { name, amount } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.heal(amount);
                    }
                }
            }
            ScriptCommand::SetMaimMaxStacks { name, max_stacks } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.max_stacks = max_stacks;
                    }
                }
            }
            ScriptCommand::SetMaimSpeedFractionPerStack { name, fraction } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.speed_fraction_per_stack = fraction;
                    }
                }
            }
            ScriptCommand::SetMaimBleedPerStackPerSecond { name, bleed } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.bleed_per_stack_per_second = bleed;
                    }
                }
            }
            ScriptCommand::SetMaimEnabled { name, enabled } => {
                use bsengine_core::Maim;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ma) = world.get_mut::<Maim>(e) {
                        ma.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddMaliceStack { name } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.add_stack();
                    }
                }
            }
            ScriptCommand::ClearMalice { name } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.clear_all();
                    }
                }
            }
            ScriptCommand::SetMaliceMaxStacks { name, max_stacks } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.max_stacks = max_stacks;
                    }
                }
            }
            ScriptCommand::SetMaliceDamageAmplifyPerStack { name, amplify } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.damage_amplify_per_stack = amplify;
                    }
                }
            }
            ScriptCommand::SetMaliceDecayInterval { name, interval } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.decay_interval = interval;
                    }
                }
            }
            ScriptCommand::SetMaliceEnabled { name, enabled } => {
                use bsengine_core::Malice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ml) = world.get_mut::<Malice>(e) {
                        ml.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyMark {
                name,
                kind,
                bonus,
                duration,
            } => {
                use bsengine_core::{Mark, MarkEntry};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mk) = world.get_mut::<Mark>(e) {
                        mk.apply(MarkEntry::new(kind, bonus, duration));
                    }
                }
            }
            ScriptCommand::ClearMarks { name } => {
                use bsengine_core::Mark;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mk) = world.get_mut::<Mark>(e) {
                        mk.clear();
                    }
                }
            }
            ScriptCommand::SetMarkEnabled { name, enabled } => {
                use bsengine_core::Mark;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mk) = world.get_mut::<Mark>(e) {
                        mk.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginMelee {
                name,
                dir_x,
                dir_y,
                dir_z,
            } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.begin(Vec3::new(dir_x, dir_y, dir_z));
                    }
                }
            }
            ScriptCommand::SetMeleeReach { name, reach } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.reach = reach;
                    }
                }
            }
            ScriptCommand::SetMeleeArcAngle { name, angle } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.arc_angle = angle;
                    }
                }
            }
            ScriptCommand::SetMeleeWindupTime { name, time } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.windup_time = time;
                    }
                }
            }
            ScriptCommand::SetMeleeActiveTime { name, time } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.active_time = time;
                    }
                }
            }
            ScriptCommand::SetMeleeRecoveryTime { name, time } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.recovery_time = time;
                    }
                }
            }
            ScriptCommand::SetMeleeMaxHits { name, max_hits } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.max_hits = max_hits;
                    }
                }
            }
            ScriptCommand::SetMeleeEnabled { name, enabled } => {
                use bsengine_core::Melee;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Melee>(e) {
                        me.enabled = enabled;
                    }
                }
            }
            ScriptCommand::MendHealth { name, amount } => {
                use bsengine_core::Mend;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mend>(e) {
                        mn.mend(amount);
                    }
                }
            }
            ScriptCommand::SetMendRate { name, rate } => {
                use bsengine_core::Mend;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mend>(e) {
                        mn.rate = rate;
                    }
                }
            }
            ScriptCommand::SetMendEnabled { name, enabled } => {
                use bsengine_core::Mend;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mn) = world.get_mut::<Mend>(e) {
                        mn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::MergeWith { name, amount } => {
                use bsengine_core::Merge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Merge>(e) {
                        mg.merge_with(amount);
                    }
                }
            }
            ScriptCommand::SetMergeCanMerge { name, can_merge } => {
                use bsengine_core::Merge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Merge>(e) {
                        mg.can_merge = can_merge;
                    }
                }
            }
            ScriptCommand::SetMergeMaxWeight { name, max_weight } => {
                use bsengine_core::Merge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Merge>(e) {
                        mg.max_weight = max_weight;
                    }
                }
            }
            ScriptCommand::SetMergeEnabled { name, enabled } => {
                use bsengine_core::Merge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mg) = world.get_mut::<Merge>(e) {
                        mg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetMeshPath { name, path } => {
                use bsengine_core::Mesh;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Mesh>(e) {
                        me.path = path;
                    }
                }
            }
            ScriptCommand::SetMeshSubmeshIndex { name, index } => {
                use bsengine_core::Mesh;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Mesh>(e) {
                        me.submesh_index = index as usize;
                    }
                }
            }
            ScriptCommand::SetMeshCastShadow { name, cast } => {
                use bsengine_core::Mesh;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Mesh>(e) {
                        me.cast_shadow = cast;
                    }
                }
            }
            ScriptCommand::SetMeshReceiveShadow { name, receive } => {
                use bsengine_core::Mesh;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut me) = world.get_mut::<Mesh>(e) {
                        me.receive_shadow = receive;
                    }
                }
            }
            ScriptCommand::SetMinimapIcon { name, icon } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.icon = icon;
                    }
                }
            }
            ScriptCommand::SetMinimapColor { name, r, g, b, a } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.color = [
                            r.clamp(0.0, 1.0),
                            g.clamp(0.0, 1.0),
                            b.clamp(0.0, 1.0),
                            a.clamp(0.0, 1.0),
                        ];
                    }
                }
            }
            ScriptCommand::SetMinimapSize { name, size } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.size = size.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMinimapCategory { name, category } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.category = category;
                    }
                }
            }
            ScriptCommand::SetMinimapRotateWithEntity { name, rotate } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.rotate_with_entity = rotate;
                    }
                }
            }
            ScriptCommand::SetMinimapClampToEdge { name, clamp } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.clamp_to_edge = clamp;
                    }
                }
            }
            ScriptCommand::SetMinimapEnabled { name, enabled } => {
                use bsengine_core::Minimap;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mm) = world.get_mut::<Minimap>(e) {
                        mm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ProjectMirage { name, duration } => {
                use bsengine_core::Mirage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mi) = world.get_mut::<Mirage>(e) {
                        mi.project(duration);
                    }
                }
            }
            ScriptCommand::DispelMirage { name } => {
                use bsengine_core::Mirage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mi) = world.get_mut::<Mirage>(e) {
                        mi.dispel();
                    }
                }
            }
            ScriptCommand::SetMirageMisdirectChance { name, chance } => {
                use bsengine_core::Mirage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mi) = world.get_mut::<Mirage>(e) {
                        mi.misdirect_chance = chance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetMirageEnabled { name, enabled } => {
                use bsengine_core::Mirage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mi) = world.get_mut::<Mirage>(e) {
                        mi.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddMomentum { name, x, y, z } => {
                use bsengine_core::Momentum;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Momentum>(e) {
                        mo.add(Vec3::new(x, y, z));
                    }
                }
            }
            ScriptCommand::StopMomentum { name } => {
                use bsengine_core::Momentum;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Momentum>(e) {
                        mo.stop();
                    }
                }
            }
            ScriptCommand::SetMomentumDamping { name, damping } => {
                use bsengine_core::Momentum;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Momentum>(e) {
                        mo.damping = damping.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetMomentumMaxSpeed { name, max_speed } => {
                use bsengine_core::Momentum;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Momentum>(e) {
                        mo.max_speed = max_speed.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMomentumEnabled { name, enabled } => {
                use bsengine_core::Momentum;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Momentum>(e) {
                        mo.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BoostMorale { name, amount } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.boost(amount);
                    }
                }
            }
            ScriptCommand::DropMorale { name, amount } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.drop(amount);
                    }
                }
            }
            ScriptCommand::SetMoraleDecayRate { name, rate } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMoraleDamageBonus { name, bonus } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.damage_bonus = bonus.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMoraleSpeedBonus { name, bonus } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.speed_bonus = bonus.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMoraleEnabled { name, enabled } => {
                use bsengine_core::Morale;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morale>(e) {
                        mo.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginMorph { name, target } => {
                use bsengine_core::Morph;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morph>(e) {
                        mo.begin(target);
                    }
                }
            }
            ScriptCommand::CancelMorph { name } => {
                use bsengine_core::Morph;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morph>(e) {
                        mo.cancel();
                    }
                }
            }
            ScriptCommand::InstantMorph { name, form } => {
                use bsengine_core::Morph;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morph>(e) {
                        mo.instant(form);
                    }
                }
            }
            ScriptCommand::SetMorphTime { name, time } => {
                use bsengine_core::Morph;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morph>(e) {
                        mo.morph_time = time.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMorphEnabled { name, enabled } => {
                use bsengine_core::Morph;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mo) = world.get_mut::<Morph>(e) {
                        mo.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetMountSpeedScale { name, scale } => {
                use bsengine_core::Mount;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mt) = world.get_mut::<Mount>(e) {
                        mt.speed_scale = scale.max(0.0);
                    }
                }
            }
            ScriptCommand::SetMountMaxRiders { name, max_riders } => {
                use bsengine_core::Mount;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mt) = world.get_mut::<Mount>(e) {
                        mt.max_riders = (max_riders.min(255)) as u8;
                    }
                }
            }
            ScriptCommand::SetMountForcedDismountDamage { name, damage } => {
                use bsengine_core::Mount;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mt) = world.get_mut::<Mount>(e) {
                        mt.forced_dismount_damage = if damage < 0.0 { None } else { Some(damage) };
                    }
                }
            }
            ScriptCommand::SetMountEnabled { name, enabled } => {
                use bsengine_core::Mount;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mt) = world.get_mut::<Mount>(e) {
                        mt.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyMuffle { name, duration } => {
                use bsengine_core::Muffle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mu) = world.get_mut::<Muffle>(e) {
                        mu.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearMuffle { name } => {
                use bsengine_core::Muffle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mu) = world.get_mut::<Muffle>(e) {
                        mu.clear();
                    }
                }
            }
            ScriptCommand::SetMuffleSoundRadiusFraction { name, fraction } => {
                use bsengine_core::Muffle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mu) = world.get_mut::<Muffle>(e) {
                        mu.sound_radius_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetMuffleEnabled { name, enabled } => {
                use bsengine_core::Muffle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut mu) = world.get_mut::<Muffle>(e) {
                        mu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyNimble { name, duration } => {
                use bsengine_core::Nimble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ni) = world.get_mut::<Nimble>(e) {
                        ni.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearNimble { name } => {
                use bsengine_core::Nimble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ni) = world.get_mut::<Nimble>(e) {
                        ni.clear();
                    }
                }
            }
            ScriptCommand::SetNimbleDodgeChance { name, chance } => {
                use bsengine_core::Nimble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ni) = world.get_mut::<Nimble>(e) {
                        ni.dodge_chance = chance.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetNimbleSpeedBonusFraction { name, fraction } => {
                use bsengine_core::Nimble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ni) = world.get_mut::<Nimble>(e) {
                        ni.speed_bonus_fraction = fraction.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNimbleEnabled { name, enabled } => {
                use bsengine_core::Nimble;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ni) = world.get_mut::<Nimble>(e) {
                        ni.enabled = enabled;
                    }
                }
            }
            ScriptCommand::RaiseNotice {
                name,
                amount,
                x,
                y,
                z,
            } => {
                use bsengine_core::Notice;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Notice>(e) {
                        no.raise(amount, Vec3::new(x, y, z));
                    }
                }
            }
            ScriptCommand::LoseSight { name } => {
                use bsengine_core::Notice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Notice>(e) {
                        no.lose_sight();
                    }
                }
            }
            ScriptCommand::ResetNotice { name } => {
                use bsengine_core::Notice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Notice>(e) {
                        no.reset();
                    }
                }
            }
            ScriptCommand::SetNoticeSuspicionDecayRate { name, rate } => {
                use bsengine_core::Notice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Notice>(e) {
                        no.suspicion_decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNoticeEnabled { name, enabled } => {
                use bsengine_core::Notice;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Notice>(e) {
                        no.enabled = enabled;
                    }
                }
            }
            ScriptCommand::FeedNourish { name, amount } => {
                use bsengine_core::Nourish;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Nourish>(e) {
                        no.feed(amount);
                    }
                }
            }
            ScriptCommand::SetNourishDecayRate { name, rate } => {
                use bsengine_core::Nourish;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Nourish>(e) {
                        no.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNourishRegenScale { name, scale } => {
                use bsengine_core::Nourish;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Nourish>(e) {
                        no.regen_scale = scale.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNourishEnabled { name, enabled } => {
                use bsengine_core::Nourish;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut no) = world.get_mut::<Nourish>(e) {
                        no.enabled = enabled;
                    }
                }
            }
            ScriptCommand::PrimeNova { name } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.prime();
                    }
                }
            }
            ScriptCommand::CancelNova { name } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.cancel();
                    }
                }
            }
            ScriptCommand::SetNovaChargeTime { name, time } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.charge_time = time.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNovaRadius { name, radius } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNovaDamage { name, damage } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.damage = damage.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNovaEnabled { name, enabled } => {
                use bsengine_core::Nova;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nv) = world.get_mut::<Nova>(e) {
                        nv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetNpcRole { name, role } => {
                use bsengine_core::{Npc, NpcRole};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.role = match role {
                            1 => NpcRole::Guard,
                            2 => NpcRole::Creature,
                            3 => NpcRole::Vendor,
                            4 => NpcRole::Scripted,
                            _ => NpcRole::Civilian,
                        };
                    }
                }
            }
            ScriptCommand::SetNpcState { name, state } => {
                use bsengine_core::{Npc, NpcState};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.state = match state {
                            1 => NpcState::Patrolling,
                            2 => NpcState::Investigating,
                            3 => NpcState::Alerted,
                            4 => NpcState::Engaging,
                            5 => NpcState::Fleeing,
                            6 => NpcState::Interacting,
                            7 => NpcState::Dead,
                            _ => NpcState::Idle,
                        };
                    }
                }
            }
            ScriptCommand::SetNpcDisplayName { name, display_name } => {
                use bsengine_core::Npc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.display_name = display_name;
                    }
                }
            }
            ScriptCommand::SetNpcFactionId { name, faction_id } => {
                use bsengine_core::Npc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.faction_id = faction_id;
                    }
                }
            }
            ScriptCommand::RaiseNpcAlert { name, amount } => {
                use bsengine_core::Npc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.raise_alert(amount);
                    }
                }
            }
            ScriptCommand::SetNpcAlertDecay { name, rate } => {
                use bsengine_core::Npc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.alert_decay = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetNpcEnabled { name, enabled } => {
                use bsengine_core::Npc;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut np) = world.get_mut::<Npc>(e) {
                        np.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyNullify { name, duration } => {
                use bsengine_core::Nullify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Nullify>(e) {
                        nu.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearNullify { name } => {
                use bsengine_core::Nullify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Nullify>(e) {
                        nu.clear();
                    }
                }
            }
            ScriptCommand::SetNullifyBlocksBuffs { name, blocks } => {
                use bsengine_core::Nullify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Nullify>(e) {
                        nu.blocks_buffs = blocks;
                    }
                }
            }
            ScriptCommand::SetNullifyBlocksDebuffs { name, blocks } => {
                use bsengine_core::Nullify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Nullify>(e) {
                        nu.blocks_debuffs = blocks;
                    }
                }
            }
            ScriptCommand::SetNullifyEnabled { name, enabled } => {
                use bsengine_core::Nullify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Nullify>(e) {
                        nu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyNumb { name, duration } => {
                use bsengine_core::Numb;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Numb>(e) {
                        nu.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearNumb { name } => {
                use bsengine_core::Numb;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Numb>(e) {
                        nu.clear();
                    }
                }
            }
            ScriptCommand::SetNumbDamageFraction { name, fraction } => {
                use bsengine_core::Numb;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Numb>(e) {
                        nu.damage_fraction = fraction.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetNumbEnabled { name, enabled } => {
                use bsengine_core::Numb;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut nu) = world.get_mut::<Numb>(e) {
                        nu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetObstacleCircle { name, radius } => {
                use bsengine_core::{Obstacle, ObstacleShape};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.shape = ObstacleShape::Circle {
                            radius: radius.max(0.0),
                        };
                    }
                }
            }
            ScriptCommand::SetObstacleBox {
                name,
                half_x,
                half_z,
            } => {
                use bsengine_core::{Obstacle, ObstacleShape};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.shape = ObstacleShape::Box {
                            half_x: half_x.max(0.0),
                            half_z: half_z.max(0.0),
                        };
                    }
                }
            }
            ScriptCommand::SetObstacleCapsule {
                name,
                radius,
                height,
            } => {
                use bsengine_core::{Obstacle, ObstacleShape};
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.shape = ObstacleShape::Capsule {
                            radius: radius.max(0.0),
                            height: height.max(0.0),
                        };
                    }
                }
            }
            ScriptCommand::SetObstacleDynamic { name, dynamic } => {
                use bsengine_core::Obstacle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.dynamic = dynamic;
                    }
                }
            }
            ScriptCommand::SetObstacleCarveDepth { name, depth } => {
                use bsengine_core::Obstacle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.carve_depth = depth.max(0.0);
                    }
                }
            }
            ScriptCommand::SetObstacleEnabled { name, enabled } => {
                use bsengine_core::Obstacle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ob) = world.get_mut::<Obstacle>(e) {
                        ob.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddOmenStack { name } => {
                use bsengine_core::Omen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut om) = world.get_mut::<Omen>(e) {
                        om.add_stack();
                    }
                }
            }
            ScriptCommand::ConsumeOmen { name } => {
                use bsengine_core::Omen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut om) = world.get_mut::<Omen>(e) {
                        om.consume();
                    }
                }
            }
            ScriptCommand::SetOmenMaxStacks { name, max_stacks } => {
                use bsengine_core::Omen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut om) = world.get_mut::<Omen>(e) {
                        om.max_stacks = max_stacks.max(1);
                    }
                }
            }
            ScriptCommand::SetOmenDamageMultiplierPerStack { name, multiplier } => {
                use bsengine_core::Omen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut om) = world.get_mut::<Omen>(e) {
                        om.damage_multiplier_per_stack = multiplier.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOmenEnabled { name, enabled } => {
                use bsengine_core::Omen;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut om) = world.get_mut::<Omen>(e) {
                        om.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetOrbitRadius { name, radius } => {
                use bsengine_core::Orbit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.radius = radius.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOrbitSpeed { name, speed } => {
                use bsengine_core::Orbit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.speed = speed;
                    }
                }
            }
            ScriptCommand::SetOrbitAngle { name, angle } => {
                use bsengine_core::Orbit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.angle = angle;
                    }
                }
            }
            ScriptCommand::SetOrbitAxis { name, x, y, z } => {
                use bsengine_core::Orbit;
                use glam::Vec3;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.axis = Vec3::new(x, y, z).normalize_or_zero();
                    }
                }
            }
            ScriptCommand::SetOrbitAltitude { name, altitude } => {
                use bsengine_core::Orbit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.altitude = altitude;
                    }
                }
            }
            ScriptCommand::SetOrbitEnabled { name, enabled } => {
                use bsengine_core::Orbit;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut or) = world.get_mut::<Orbit>(e) {
                        or.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginOrdeal { name, duration } => {
                use bsengine_core::Ordeal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut od) = world.get_mut::<Ordeal>(e) {
                        od.begin(duration);
                    }
                }
            }
            ScriptCommand::FailOrdeal { name } => {
                use bsengine_core::Ordeal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut od) = world.get_mut::<Ordeal>(e) {
                        od.fail();
                    }
                }
            }
            ScriptCommand::ResetOrdeal { name } => {
                use bsengine_core::Ordeal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut od) = world.get_mut::<Ordeal>(e) {
                        od.reset();
                    }
                }
            }
            ScriptCommand::SetOrdealEnabled { name, enabled } => {
                use bsengine_core::Ordeal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut od) = world.get_mut::<Ordeal>(e) {
                        od.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetOscillateAmplitude { name, amplitude } => {
                use bsengine_core::Oscillate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Oscillate>(e) {
                        os.amplitude = amplitude.abs();
                    }
                }
            }
            ScriptCommand::SetOscillateFrequency { name, frequency } => {
                use bsengine_core::Oscillate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Oscillate>(e) {
                        os.frequency = frequency.abs();
                    }
                }
            }
            ScriptCommand::SetOscillatePhaseOffset { name, offset } => {
                use bsengine_core::Oscillate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Oscillate>(e) {
                        os.phase_offset = offset;
                    }
                }
            }
            ScriptCommand::SetOscillateEnabled { name, enabled } => {
                use bsengine_core::Oscillate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Oscillate>(e) {
                        os.enabled = enabled;
                    }
                }
            }
            ScriptCommand::EnterOutlastCombat { name } => {
                use bsengine_core::Outlast;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outlast>(e) {
                        ol.enter_combat();
                    }
                }
            }
            ScriptCommand::ExitOutlastCombat { name } => {
                use bsengine_core::Outlast;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outlast>(e) {
                        ol.exit_combat();
                    }
                }
            }
            ScriptCommand::SetOutlastMaxBonusTime { name, time } => {
                use bsengine_core::Outlast;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outlast>(e) {
                        ol.max_bonus_time = time.max(1.0);
                    }
                }
            }
            ScriptCommand::SetOutlastDefenseBonus { name, bonus } => {
                use bsengine_core::Outlast;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outlast>(e) {
                        ol.defense_bonus = bonus.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetOutlastEnabled { name, enabled } => {
                use bsengine_core::Outlast;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Outlast>(e) {
                        ol.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddOverflow { name, amount } => {
                use bsengine_core::Overflow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut of) = world.get_mut::<Overflow>(e) {
                        of.add(amount);
                    }
                }
            }
            ScriptCommand::SetOverflowMaxPool { name, max_pool } => {
                use bsengine_core::Overflow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut of) = world.get_mut::<Overflow>(e) {
                        of.max_pool = max_pool.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOverflowDecayRate { name, rate } => {
                use bsengine_core::Overflow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut of) = world.get_mut::<Overflow>(e) {
                        of.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOverflowEnabled { name, enabled } => {
                use bsengine_core::Overflow;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut of) = world.get_mut::<Overflow>(e) {
                        of.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddOverheat { name, amount } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.add_heat(amount);
                    }
                }
            }
            ScriptCommand::SetOverheatMaxHeat { name, max_heat } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.max_heat = max_heat.max(1.0);
                    }
                }
            }
            ScriptCommand::SetOverheatCoolRate { name, rate } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.cool_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOverheatForcedCoolRate { name, rate } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.forced_cool_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOverheatWarnThreshold { name, threshold } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.warn_threshold = threshold.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetOverheatCoolThreshold { name, threshold } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.cool_threshold = threshold.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetOverheatEnabled { name, enabled } => {
                use bsengine_core::Overheat;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut oh) = world.get_mut::<Overheat>(e) {
                        oh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyOverload { name, duration } => {
                use bsengine_core::Overload;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Overload>(e) {
                        ol.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearOverload { name } => {
                use bsengine_core::Overload;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Overload>(e) {
                        ol.clear();
                    }
                }
            }
            ScriptCommand::SetOverloadCostMultiplier { name, multiplier } => {
                use bsengine_core::Overload;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Overload>(e) {
                        ol.cost_multiplier = multiplier.max(1.0);
                    }
                }
            }
            ScriptCommand::SetOverloadEnabled { name, enabled } => {
                use bsengine_core::Overload;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ol) = world.get_mut::<Overload>(e) {
                        ol.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyOverpower { name, duration } => {
                use bsengine_core::Overpower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut op) = world.get_mut::<Overpower>(e) {
                        op.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearOverpower { name } => {
                use bsengine_core::Overpower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut op) = world.get_mut::<Overpower>(e) {
                        op.clear();
                    }
                }
            }
            ScriptCommand::SetOverpowerArmorPenetration { name, penetration } => {
                use bsengine_core::Overpower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut op) = world.get_mut::<Overpower>(e) {
                        op.armor_penetration = penetration.clamp(0.0, 1.0);
                    }
                }
            }
            ScriptCommand::SetOverpowerEnabled { name, enabled } => {
                use bsengine_core::Overpower;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut op) = world.get_mut::<Overpower>(e) {
                        op.enabled = enabled;
                    }
                }
            }
            ScriptCommand::GrantOvershield { name, amount } => {
                use bsengine_core::Overshield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Overshield>(e) {
                        os.grant(amount);
                    }
                }
            }
            ScriptCommand::SetOvershieldMax {
                name,
                max_overshield,
            } => {
                use bsengine_core::Overshield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Overshield>(e) {
                        os.max_overshield = max_overshield.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOvershieldDecayRate { name, rate } => {
                use bsengine_core::Overshield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Overshield>(e) {
                        os.decay_rate = rate.max(0.0);
                    }
                }
            }
            ScriptCommand::SetOvershieldEnabled { name, enabled } => {
                use bsengine_core::Overshield;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut os) = world.get_mut::<Overshield>(e) {
                        os.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginParry { name } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.begin();
                    }
                }
            }
            ScriptCommand::NotifyParryHit { name } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.notify_hit();
                    }
                }
            }
            ScriptCommand::SetParryStartupDuration { name, duration } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.startup_duration = duration;
                    }
                }
            }
            ScriptCommand::SetParryActiveDuration { name, duration } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.active_duration = duration;
                    }
                }
            }
            ScriptCommand::SetParryRecoveryDuration { name, duration } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.recovery_duration = duration;
                    }
                }
            }
            ScriptCommand::SetParryEnabled { name, enabled } => {
                use bsengine_core::Parry;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Parry>(e) {
                        pa.enabled = enabled;
                    }
                }
            }
            ScriptCommand::PatienceOnAttack { name } => {
                use bsengine_core::Patience;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Patience>(e) {
                        pa.on_attack();
                    }
                }
            }
            ScriptCommand::SetPatienceMaxPatience { name, max_patience } => {
                use bsengine_core::Patience;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Patience>(e) {
                        pa.max_patience = max_patience;
                    }
                }
            }
            ScriptCommand::SetPatienceBonus { name, bonus } => {
                use bsengine_core::Patience;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Patience>(e) {
                        pa.patience_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetPatienceEnabled { name, enabled } => {
                use bsengine_core::Patience;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pa) = world.get_mut::<Patience>(e) {
                        pa.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyPetrify { name, duration } => {
                use bsengine_core::Petrify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pe) = world.get_mut::<Petrify>(e) {
                        pe.apply(duration);
                    }
                }
            }
            ScriptCommand::ShatterPetrify { name } => {
                use bsengine_core::Petrify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pe) = world.get_mut::<Petrify>(e) {
                        pe.shatter();
                    }
                }
            }
            ScriptCommand::SetPetrifyArmorBonus { name, bonus } => {
                use bsengine_core::Petrify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pe) = world.get_mut::<Petrify>(e) {
                        pe.armor_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetPetrifyEnabled { name, enabled } => {
                use bsengine_core::Petrify;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pe) = world.get_mut::<Petrify>(e) {
                        pe.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivatePhase { name } => {
                use bsengine_core::Phase;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ph) = world.get_mut::<Phase>(e) {
                        ph.activate();
                    }
                }
            }
            ScriptCommand::CancelPhase { name } => {
                use bsengine_core::Phase;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ph) = world.get_mut::<Phase>(e) {
                        ph.cancel();
                    }
                }
            }
            ScriptCommand::SetPhaseDuration { name, duration } => {
                use bsengine_core::Phase;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ph) = world.get_mut::<Phase>(e) {
                        ph.duration = duration;
                    }
                }
            }
            ScriptCommand::SetPhaseCooldown { name, cooldown } => {
                use bsengine_core::Phase;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ph) = world.get_mut::<Phase>(e) {
                        ph.cooldown = cooldown;
                    }
                }
            }
            ScriptCommand::SetPhaseEnabled { name, enabled } => {
                use bsengine_core::Phase;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ph) = world.get_mut::<Phase>(e) {
                        ph.enabled = enabled;
                    }
                }
            }
            ScriptCommand::PierceBeginAttack { name } => {
                use bsengine_core::Pierce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pierce>(e) {
                        pi.begin_attack();
                    }
                }
            }
            ScriptCommand::SetPierceMax { name, max_pierce } => {
                use bsengine_core::Pierce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pierce>(e) {
                        pi.max_pierce = max_pierce;
                    }
                }
            }
            ScriptCommand::SetPierceChance { name, chance } => {
                use bsengine_core::Pierce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pierce>(e) {
                        pi.pierce_chance = chance;
                    }
                }
            }
            ScriptCommand::SetPierceEnabled { name, enabled } => {
                use bsengine_core::Pierce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pierce>(e) {
                        pi.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyPin { name, duration } => {
                use bsengine_core::Pin;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pin>(e) {
                        pi.pin(duration);
                    }
                }
            }
            ScriptCommand::FreePin { name } => {
                use bsengine_core::Pin;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pin>(e) {
                        pi.free();
                    }
                }
            }
            ScriptCommand::SetPinKnockbackImmune { name, immune } => {
                use bsengine_core::Pin;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pin>(e) {
                        pi.knockback_immune = immune;
                    }
                }
            }
            ScriptCommand::SetPinEnabled { name, enabled } => {
                use bsengine_core::Pin;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pi) = world.get_mut::<Pin>(e) {
                        pi.enabled = enabled;
                    }
                }
            }
            ScriptCommand::Plead { name, duration } => {
                use bsengine_core::Plea;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Plea>(e) {
                        pl.plead(duration);
                    }
                }
            }
            ScriptCommand::SilencePlea { name } => {
                use bsengine_core::Plea;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Plea>(e) {
                        pl.silence();
                    }
                }
            }
            ScriptCommand::SetPleaAvoidanceChance { name, chance } => {
                use bsengine_core::Plea;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Plea>(e) {
                        pl.avoidance_chance = chance;
                    }
                }
            }
            ScriptCommand::SetPleaEnabled { name, enabled } => {
                use bsengine_core::Plea;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Plea>(e) {
                        pl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::Feign { name, duration } => {
                use bsengine_core::Ploy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Ploy>(e) {
                        pl.feign(duration);
                    }
                }
            }
            ScriptCommand::DropPloy { name } => {
                use bsengine_core::Ploy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Ploy>(e) {
                        pl.drop_ploy();
                    }
                }
            }
            ScriptCommand::SetPloyEnabled { name, enabled } => {
                use bsengine_core::Ploy;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Ploy>(e) {
                        pl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::UpdatePluck { name, hp, max_hp } => {
                use bsengine_core::Pluck;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Pluck>(e) {
                        pl.update(hp, max_hp);
                    }
                }
            }
            ScriptCommand::SetPluckHpThreshold { name, threshold } => {
                use bsengine_core::Pluck;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Pluck>(e) {
                        pl.hp_threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetPluckCritBonus { name, bonus } => {
                use bsengine_core::Pluck;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Pluck>(e) {
                        pl.crit_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetPluckEnabled { name, enabled } => {
                use bsengine_core::Pluck;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pl) = world.get_mut::<Pluck>(e) {
                        pl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::DamagePoise { name, amount } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.damage(amount);
                    }
                }
            }
            ScriptCommand::BreakPoise { name } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.break_now();
                    }
                }
            }
            ScriptCommand::RestorePoise { name, amount } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.restore(amount);
                    }
                }
            }
            ScriptCommand::SetPoiseMax { name, max } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.max = max;
                    }
                }
            }
            ScriptCommand::SetPoiseRegenRate { name, rate } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.regen_rate = rate;
                    }
                }
            }
            ScriptCommand::SetPoiseEnabled { name, enabled } => {
                use bsengine_core::Poise;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Poise>(e) {
                        po.enabled = enabled;
                    }
                }
            }
            ScriptCommand::PounceLeap { name, duration } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.leap(duration);
                    }
                }
            }
            ScriptCommand::PounceLand { name } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.land();
                    }
                }
            }
            ScriptCommand::SetPounceDamage { name, damage } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.damage = damage;
                    }
                }
            }
            ScriptCommand::SetPounceKnockdownDuration { name, duration } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.knockdown_duration = duration;
                    }
                }
            }
            ScriptCommand::SetPounceMinRange { name, range } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.min_range = range;
                    }
                }
            }
            ScriptCommand::SetPounceMaxRange { name, range } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.max_range = range;
                    }
                }
            }
            ScriptCommand::SetPounceEnabled { name, enabled } => {
                use bsengine_core::Pounce;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut po) = world.get_mut::<Pounce>(e) {
                        po.enabled = enabled;
                    }
                }
            }
            ScriptCommand::FallProne { name } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.fall();
                    }
                }
            }
            ScriptCommand::BeginStandUp { name } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.begin_stand_up();
                    }
                }
            }
            ScriptCommand::ForceStandUp { name } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.force_stand_up();
                    }
                }
            }
            ScriptCommand::SetProneStandUpDuration { name, duration } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.stand_up_duration = duration;
                    }
                }
            }
            ScriptCommand::SetProneMovementPenalty { name, penalty } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.movement_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetProneAttackPenalty { name, penalty } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.attack_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetProneEnabled { name, enabled } => {
                use bsengine_core::Prone;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prone>(e) {
                        pr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::Guard { name, duration } => {
                use bsengine_core::Protect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Protect>(e) {
                        pr.guard(duration);
                    }
                }
            }
            ScriptCommand::ProtectStandDown { name } => {
                use bsengine_core::Protect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Protect>(e) {
                        pr.stand_down();
                    }
                }
            }
            ScriptCommand::SetProtectGuardRadius { name, radius } => {
                use bsengine_core::Protect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Protect>(e) {
                        pr.guard_radius = radius;
                    }
                }
            }
            ScriptCommand::SetProtectRedirectFraction { name, fraction } => {
                use bsengine_core::Protect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Protect>(e) {
                        pr.redirect_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetProtectEnabled { name, enabled } => {
                use bsengine_core::Protect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Protect>(e) {
                        pr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::UpdateProud { name, hp, max_hp } => {
                use bsengine_core::Proud;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Proud>(e) {
                        pr.update(hp, max_hp);
                    }
                }
            }
            ScriptCommand::SetProudHpThreshold { name, threshold } => {
                use bsengine_core::Proud;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Proud>(e) {
                        pr.hp_threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetProudDamageBonus { name, bonus } => {
                use bsengine_core::Proud;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Proud>(e) {
                        pr.damage_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetProudEnabled { name, enabled } => {
                use bsengine_core::Proud;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Proud>(e) {
                        pr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivateProvoke { name, duration } => {
                use bsengine_core::Provoke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Provoke>(e) {
                        pr.activate(duration);
                    }
                }
            }
            ScriptCommand::DeactivateProvoke { name } => {
                use bsengine_core::Provoke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Provoke>(e) {
                        pr.deactivate();
                    }
                }
            }
            ScriptCommand::SetProvokeAggroMultiplier { name, multiplier } => {
                use bsengine_core::Provoke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Provoke>(e) {
                        pr.aggro_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetProvokeRadius { name, radius } => {
                use bsengine_core::Provoke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Provoke>(e) {
                        pr.radius = radius;
                    }
                }
            }
            ScriptCommand::SetProvokeEnabled { name, enabled } => {
                use bsengine_core::Provoke;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Provoke>(e) {
                        pr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyProwl { name, duration } => {
                use bsengine_core::Prowl;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prowl>(e) {
                        pr.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearProwl { name } => {
                use bsengine_core::Prowl;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prowl>(e) {
                        pr.clear();
                    }
                }
            }
            ScriptCommand::SetProwlSpeedBonus { name, bonus } => {
                use bsengine_core::Prowl;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prowl>(e) {
                        pr.speed_bonus_fraction = bonus;
                    }
                }
            }
            ScriptCommand::SetProwlAmbushMultiplier { name, multiplier } => {
                use bsengine_core::Prowl;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prowl>(e) {
                        pr.ambush_damage_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetProwlEnabled { name, enabled } => {
                use bsengine_core::Prowl;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pr) = world.get_mut::<Prowl>(e) {
                        pr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivatePulse { name } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.activate();
                    }
                }
            }
            ScriptCommand::DeactivatePulse { name } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.deactivate();
                    }
                }
            }
            ScriptCommand::ResetPulse { name } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.reset();
                    }
                }
            }
            ScriptCommand::SetPulseRadius { name, radius } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.radius = radius;
                    }
                }
            }
            ScriptCommand::SetPulseInterval { name, interval } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.interval = interval;
                    }
                }
            }
            ScriptCommand::SetPulseFalloff { name, falloff } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.falloff = falloff;
                    }
                }
            }
            ScriptCommand::SetPulseEnabled { name, enabled } => {
                use bsengine_core::Pulse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut pu) = world.get_mut::<Pulse>(e) {
                        pu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetQuestXpReward { name, xp_reward } => {
                use bsengine_core::Quest;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut qu) = world.get_mut::<Quest>(e) {
                        qu.xp_reward = xp_reward;
                    }
                }
            }
            ScriptCommand::SetQuestEnabled { name, enabled } => {
                use bsengine_core::Quest;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut qu) = world.get_mut::<Quest>(e) {
                        qu.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRadarRange { name, range } => {
                use bsengine_core::Radar;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rd) = world.get_mut::<Radar>(e) {
                        rd.range = range;
                    }
                }
            }
            ScriptCommand::SetRadarScanInterval { name, interval } => {
                use bsengine_core::Radar;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rd) = world.get_mut::<Radar>(e) {
                        rd.scan_interval = interval;
                    }
                }
            }
            ScriptCommand::SetRadarEnabled { name, enabled } => {
                use bsengine_core::Radar;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rd) = world.get_mut::<Radar>(e) {
                        rd.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetMaxRage { name, max_rage } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.max_rage = max_rage;
                    }
                }
            }
            ScriptCommand::SetRagePerDamage {
                name,
                rage_per_damage,
            } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.rage_per_damage = rage_per_damage;
                    }
                }
            }
            ScriptCommand::SetRageActivationThreshold { name, threshold } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.activation_threshold = threshold;
                    }
                }
            }
            ScriptCommand::SetRageDamageMultiplier { name, multiplier } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.damage_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetRageDefenseMultiplier { name, multiplier } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.defense_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetRageEnabled { name, enabled } => {
                use bsengine_core::Rage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rg) = world.get_mut::<Rage>(e) {
                        rg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::CallRally { name, duration } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.call(duration);
                    }
                }
            }
            ScriptCommand::DismissRally { name } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.dismiss();
                    }
                }
            }
            ScriptCommand::SetRallyAuraRadius { name, radius } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.aura_radius = radius;
                    }
                }
            }
            ScriptCommand::SetRallySpeedBonusFraction { name, fraction } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.speed_bonus_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetRallyDamageBonusFraction { name, fraction } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.damage_bonus_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetRallyEnabled { name, enabled } => {
                use bsengine_core::Rally;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ra) = world.get_mut::<Rally>(e) {
                        ra.enabled = enabled;
                    }
                }
            }
            ScriptCommand::RampageKill { name } => {
                use bsengine_core::Rampage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rm) = world.get_mut::<Rampage>(e) {
                        rm.kill();
                    }
                }
            }
            ScriptCommand::SetRampageDamagePerStack {
                name,
                damage_per_stack,
            } => {
                use bsengine_core::Rampage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rm) = world.get_mut::<Rampage>(e) {
                        rm.damage_per_stack = damage_per_stack;
                    }
                }
            }
            ScriptCommand::SetRampageSpeedPerStack {
                name,
                speed_per_stack,
            } => {
                use bsengine_core::Rampage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rm) = world.get_mut::<Rampage>(e) {
                        rm.speed_per_stack = speed_per_stack;
                    }
                }
            }
            ScriptCommand::SetRampageDecayInterval { name, interval } => {
                use bsengine_core::Rampage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rm) = world.get_mut::<Rampage>(e) {
                        rm.decay_interval = interval;
                    }
                }
            }
            ScriptCommand::SetRampageEnabled { name, enabled } => {
                use bsengine_core::Rampage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rm) = world.get_mut::<Rampage>(e) {
                        rm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerRavage { name, duration } => {
                use bsengine_core::Ravage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rv) = world.get_mut::<Ravage>(e) {
                        rv.trigger(duration);
                    }
                }
            }
            ScriptCommand::SetRavageDamageBonus { name, bonus } => {
                use bsengine_core::Ravage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rv) = world.get_mut::<Ravage>(e) {
                        rv.damage_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRavageAttackSpeedBonus { name, bonus } => {
                use bsengine_core::Ravage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rv) = world.get_mut::<Ravage>(e) {
                        rv.attack_speed_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRavageEnabled { name, enabled } => {
                use bsengine_core::Ravage;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rv) = world.get_mut::<Ravage>(e) {
                        rv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyReave { name, duration } => {
                use bsengine_core::Reave;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut re) = world.get_mut::<Reave>(e) {
                        re.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearReave { name } => {
                use bsengine_core::Reave;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut re) = world.get_mut::<Reave>(e) {
                        re.clear();
                    }
                }
            }
            ScriptCommand::SetReaveLechFraction {
                name,
                leech_fraction,
            } => {
                use bsengine_core::Reave;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut re) = world.get_mut::<Reave>(e) {
                        re.leech_fraction = leech_fraction;
                    }
                }
            }
            ScriptCommand::SetReaveEnabled { name, enabled } => {
                use bsengine_core::Reave;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut re) = world.get_mut::<Reave>(e) {
                        re.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetReboundCoefficient { name, coefficient } => {
                use bsengine_core::Rebound;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rb) = world.get_mut::<Rebound>(e) {
                        rb.rebound_coefficient = coefficient;
                    }
                }
            }
            ScriptCommand::SetReboundMinSpeed { name, min_speed } => {
                use bsengine_core::Rebound;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rb) = world.get_mut::<Rebound>(e) {
                        rb.min_speed = min_speed;
                    }
                }
            }
            ScriptCommand::SetReboundEnabled { name, enabled } => {
                use bsengine_core::Rebound;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rb) = world.get_mut::<Rebound>(e) {
                        rb.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRechargeRate { name, rate } => {
                use bsengine_core::Recharge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rch) = world.get_mut::<Recharge>(e) {
                        rch.rate = rate;
                    }
                }
            }
            ScriptCommand::SetRechargeEnabled { name, enabled } => {
                use bsengine_core::Recharge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rch) = world.get_mut::<Recharge>(e) {
                        rch.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ChargeReckless { name, duration } => {
                use bsengine_core::Reckless;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rk) = world.get_mut::<Reckless>(e) {
                        rk.charge(duration);
                    }
                }
            }
            ScriptCommand::SnapOutReckless { name } => {
                use bsengine_core::Reckless;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rk) = world.get_mut::<Reckless>(e) {
                        rk.snap_out();
                    }
                }
            }
            ScriptCommand::SetRecklessDamageBonus { name, bonus } => {
                use bsengine_core::Reckless;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rk) = world.get_mut::<Reckless>(e) {
                        rk.damage_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRecklessDefensePenalty { name, penalty } => {
                use bsengine_core::Reckless;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rk) = world.get_mut::<Reckless>(e) {
                        rk.defense_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetRecklessEnabled { name, enabled } => {
                use bsengine_core::Reckless;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rk) = world.get_mut::<Reckless>(e) {
                        rk.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRecluseAlone { name, alone } => {
                use bsengine_core::Recluse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rl) = world.get_mut::<Recluse>(e) {
                        rl.is_alone = alone;
                    }
                }
            }
            ScriptCommand::SetRecluseDamageBonus { name, bonus } => {
                use bsengine_core::Recluse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rl) = world.get_mut::<Recluse>(e) {
                        rl.damage_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRecluseDefenseBonus { name, bonus } => {
                use bsengine_core::Recluse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rl) = world.get_mut::<Recluse>(e) {
                        rl.defense_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRecluseEnabled { name, enabled } => {
                use bsengine_core::Recluse;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rl) = world.get_mut::<Recluse>(e) {
                        rl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::KickRecoil { name } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.kick();
                    }
                }
            }
            ScriptCommand::ResetRecoil { name } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.reset();
                    }
                }
            }
            ScriptCommand::SetRecoilKickForce { name, force } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.kick_force = force;
                    }
                }
            }
            ScriptCommand::SetRecoilAngularKick { name, angular_kick } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.angular_kick = angular_kick;
                    }
                }
            }
            ScriptCommand::SetRecoilRecoverySpeed { name, speed } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.recovery_speed = speed;
                    }
                }
            }
            ScriptCommand::SetRecoilEnabled { name, enabled } => {
                use bsengine_core::Recoil;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rco) = world.get_mut::<Recoil>(e) {
                        rco.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivateReflect { name } => {
                use bsengine_core::Reflect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfl) = world.get_mut::<Reflect>(e) {
                        rfl.activate();
                    }
                }
            }
            ScriptCommand::DeactivateReflect { name } => {
                use bsengine_core::Reflect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfl) = world.get_mut::<Reflect>(e) {
                        rfl.deactivate();
                    }
                }
            }
            ScriptCommand::SetReflectDamageMultiplier { name, multiplier } => {
                use bsengine_core::Reflect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfl) = world.get_mut::<Reflect>(e) {
                        rfl.damage_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetReflectWindowDuration { name, duration } => {
                use bsengine_core::Reflect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfl) = world.get_mut::<Reflect>(e) {
                        rfl.window_duration = duration;
                    }
                }
            }
            ScriptCommand::SetReflectEnabled { name, enabled } => {
                use bsengine_core::Reflect;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfl) = world.get_mut::<Reflect>(e) {
                        rfl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerReflex { name, duration } => {
                use bsengine_core::Reflex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfx) = world.get_mut::<Reflex>(e) {
                        rfx.trigger(duration);
                    }
                }
            }
            ScriptCommand::EvadeReflex { name } => {
                use bsengine_core::Reflex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfx) = world.get_mut::<Reflex>(e) {
                        rfx.evade();
                    }
                }
            }
            ScriptCommand::SetReflexEnabled { name, enabled } => {
                use bsengine_core::Reflex;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rfx) = world.get_mut::<Reflex>(e) {
                        rfx.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyRepel { name, duration } => {
                use bsengine_core::Repel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rp) = world.get_mut::<Repel>(e) {
                        rp.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearRepel { name } => {
                use bsengine_core::Repel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rp) = world.get_mut::<Repel>(e) {
                        rp.clear();
                    }
                }
            }
            ScriptCommand::SetRepelPushForce { name, force } => {
                use bsengine_core::Repel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rp) = world.get_mut::<Repel>(e) {
                        rp.push_force = force;
                    }
                }
            }
            ScriptCommand::SetRepelRadius { name, radius } => {
                use bsengine_core::Repel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rp) = world.get_mut::<Repel>(e) {
                        rp.radius = radius;
                    }
                }
            }
            ScriptCommand::SetRepelEnabled { name, enabled } => {
                use bsengine_core::Repel;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rp) = world.get_mut::<Repel>(e) {
                        rp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::RestRepose { name, duration } => {
                use bsengine_core::Repose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rpo) = world.get_mut::<Repose>(e) {
                        rpo.rest(duration);
                    }
                }
            }
            ScriptCommand::RouseRepose { name } => {
                use bsengine_core::Repose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rpo) = world.get_mut::<Repose>(e) {
                        rpo.rouse();
                    }
                }
            }
            ScriptCommand::SetReposeRegenMultiplier { name, multiplier } => {
                use bsengine_core::Repose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rpo) = world.get_mut::<Repose>(e) {
                        rpo.regen_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetReposeEnabled { name, enabled } => {
                use bsengine_core::Repose;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rpo) = world.get_mut::<Repose>(e) {
                        rpo.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRespawnDelay { name, delay } => {
                use bsengine_core::Respawn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rs) = world.get_mut::<Respawn>(e) {
                        rs.delay = delay;
                    }
                }
            }
            ScriptCommand::SetRespawnEnabled { name, enabled } => {
                use bsengine_core::Respawn;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rs) = world.get_mut::<Respawn>(e) {
                        rs.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ChargeRetaliate { name } => {
                use bsengine_core::Retaliate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ret) = world.get_mut::<Retaliate>(e) {
                        ret.charge();
                    }
                }
            }
            ScriptCommand::ClearRetaliate { name } => {
                use bsengine_core::Retaliate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ret) = world.get_mut::<Retaliate>(e) {
                        ret.clear();
                    }
                }
            }
            ScriptCommand::SetRetaliateMultiplier { name, multiplier } => {
                use bsengine_core::Retaliate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ret) = world.get_mut::<Retaliate>(e) {
                        ret.multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetRetaliateEnabled { name, enabled } => {
                use bsengine_core::Retaliate;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ret) = world.get_mut::<Retaliate>(e) {
                        ret.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ResetRevenge { name } => {
                use bsengine_core::Revenge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvn) = world.get_mut::<Revenge>(e) {
                        rvn.reset();
                    }
                }
            }
            ScriptCommand::SetRevengeMultiplier { name, multiplier } => {
                use bsengine_core::Revenge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvn) = world.get_mut::<Revenge>(e) {
                        rvn.revenge_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetRevengeTriggerFraction { name, fraction } => {
                use bsengine_core::Revenge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvn) = world.get_mut::<Revenge>(e) {
                        rvn.trigger_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetRevengeEnabled { name, enabled } => {
                use bsengine_core::Revenge;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvn) = world.get_mut::<Revenge>(e) {
                        rvn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivateReveal { name, duration } => {
                use bsengine_core::Reveal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvl) = world.get_mut::<Reveal>(e) {
                        rvl.activate(duration);
                    }
                }
            }
            ScriptCommand::DeactivateReveal { name } => {
                use bsengine_core::Reveal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvl) = world.get_mut::<Reveal>(e) {
                        rvl.deactivate();
                    }
                }
            }
            ScriptCommand::SetRevealRadius { name, radius } => {
                use bsengine_core::Reveal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvl) = world.get_mut::<Reveal>(e) {
                        rvl.radius = radius;
                    }
                }
            }
            ScriptCommand::SetRevealEnabled { name, enabled } => {
                use bsengine_core::Reveal;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvl) = world.get_mut::<Reveal>(e) {
                        rvl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TakeDownRevive { name } => {
                use bsengine_core::Revive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvi) = world.get_mut::<Revive>(e) {
                        rvi.take_down();
                    }
                }
            }
            ScriptCommand::BeginRevive { name } => {
                use bsengine_core::Revive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvi) = world.get_mut::<Revive>(e) {
                        rvi.begin_revive();
                    }
                }
            }
            ScriptCommand::CancelRevive { name } => {
                use bsengine_core::Revive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvi) = world.get_mut::<Revive>(e) {
                        rvi.cancel_revive();
                    }
                }
            }
            ScriptCommand::SetReviveEnabled { name, enabled } => {
                use bsengine_core::Revive;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rvi) = world.get_mut::<Revive>(e) {
                        rvi.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ResetRicochet { name } => {
                use bsengine_core::Ricochet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ric) = world.get_mut::<Ricochet>(e) {
                        ric.reset();
                    }
                }
            }
            ScriptCommand::SetRicochetEnergyRetention { name, retention } => {
                use bsengine_core::Ricochet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ric) = world.get_mut::<Ricochet>(e) {
                        ric.energy_retention = retention;
                    }
                }
            }
            ScriptCommand::SetRicochetMinDot { name, min_dot } => {
                use bsengine_core::Ricochet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ric) = world.get_mut::<Ricochet>(e) {
                        ric.min_dot = min_dot;
                    }
                }
            }
            ScriptCommand::SetRicochetEnabled { name, enabled } => {
                use bsengine_core::Ricochet;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ric) = world.get_mut::<Ricochet>(e) {
                        ric.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetRifleMinRange { name, range } => {
                use bsengine_core::Rifle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rif) = world.get_mut::<Rifle>(e) {
                        rif.min_range = range;
                    }
                }
            }
            ScriptCommand::SetRiflePeakRange { name, range } => {
                use bsengine_core::Rifle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rif) = world.get_mut::<Rifle>(e) {
                        rif.peak_range = range;
                    }
                }
            }
            ScriptCommand::SetRifleDamageBonus { name, bonus } => {
                use bsengine_core::Rifle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rif) = world.get_mut::<Rifle>(e) {
                        rif.damage_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetRiflePointBlankPenalty { name, penalty } => {
                use bsengine_core::Rifle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rif) = world.get_mut::<Rifle>(e) {
                        rif.point_blank_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetRifleEnabled { name, enabled } => {
                use bsengine_core::Rifle;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rif) = world.get_mut::<Rifle>(e) {
                        rif.enabled = enabled;
                    }
                }
            }
            ScriptCommand::InfectRot { name } => {
                use bsengine_core::Rot;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rot) = world.get_mut::<Rot>(e) {
                        rot.infect();
                    }
                }
            }
            ScriptCommand::CleanseRot { name } => {
                use bsengine_core::Rot;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rot) = world.get_mut::<Rot>(e) {
                        rot.cleanse();
                    }
                }
            }
            ScriptCommand::RestoreRot { name } => {
                use bsengine_core::Rot;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rot) = world.get_mut::<Rot>(e) {
                        rot.restore();
                    }
                }
            }
            ScriptCommand::SetRotDecayRate { name, rate } => {
                use bsengine_core::Rot;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rot) = world.get_mut::<Rot>(e) {
                        rot.decay_rate = rate;
                    }
                }
            }
            ScriptCommand::SetRotEnabled { name, enabled } => {
                use bsengine_core::Rot;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rot) = world.get_mut::<Rot>(e) {
                        rot.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyRout { name, duration } => {
                use bsengine_core::Rout;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rout) = world.get_mut::<Rout>(e) {
                        rout.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearRout { name } => {
                use bsengine_core::Rout;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rout) = world.get_mut::<Rout>(e) {
                        rout.clear();
                    }
                }
            }
            ScriptCommand::SetRoutFleeSpeedMultiplier { name, multiplier } => {
                use bsengine_core::Rout;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rout) = world.get_mut::<Rout>(e) {
                        rout.flee_speed_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetRoutEnabled { name, enabled } => {
                use bsengine_core::Rout;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rout) = world.get_mut::<Rout>(e) {
                        rout.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyRupture { name, count } => {
                use bsengine_core::Rupture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rup) = world.get_mut::<Rupture>(e) {
                        rup.apply(count);
                    }
                }
            }
            ScriptCommand::CleanseRupture { name, count } => {
                use bsengine_core::Rupture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rup) = world.get_mut::<Rupture>(e) {
                        rup.cleanse(count);
                    }
                }
            }
            ScriptCommand::SetRuptureDamagePerStack {
                name,
                damage_per_stack,
            } => {
                use bsengine_core::Rupture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rup) = world.get_mut::<Rupture>(e) {
                        rup.damage_per_stack = damage_per_stack;
                    }
                }
            }
            ScriptCommand::SetRuptureEnabled { name, enabled } => {
                use bsengine_core::Rupture;
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut rup) = world.get_mut::<Rupture>(e) {
                        rup.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyScald { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scald>(e) {
                        sc.apply_stack(count);
                    }
                }
            }
            ScriptCommand::SetScaldAmplifyPerStack {
                name,
                amplify_per_stack,
            } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scald>(e) {
                        sc.amplify_per_stack = amplify_per_stack;
                    }
                }
            }
            ScriptCommand::SetScaldStackDuration {
                name,
                stack_duration,
            } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scald>(e) {
                        sc.stack_duration = stack_duration;
                    }
                }
            }
            ScriptCommand::SetScaldEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scald>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerScan { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scan>(e) {
                        sc.trigger();
                    }
                }
            }
            ScriptCommand::SetScanRadius { name, radius } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scan>(e) {
                        sc.set_radius(radius);
                    }
                }
            }
            ScriptCommand::SetScanInterval { name, interval } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scan>(e) {
                        sc.interval = interval;
                    }
                }
            }
            ScriptCommand::SetScanEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scan>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::InflictScar { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scar>(e) {
                        sc.inflict();
                    }
                }
            }
            ScriptCommand::CleanseScar { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scar>(e) {
                        sc.cleanse(count);
                    }
                }
            }
            ScriptCommand::SetScarRegenPenalty { name, penalty } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scar>(e) {
                        sc.regen_penalty_per_scar = penalty;
                    }
                }
            }
            ScriptCommand::SetScarEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scar>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyScatter { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scatter>(e) {
                        sc.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearScatter { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scatter>(e) {
                        sc.clear();
                    }
                }
            }
            ScriptCommand::SetScatterSpreadMultiplier { name, multiplier } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scatter>(e) {
                        sc.spread_multiplier = multiplier;
                    }
                }
            }
            ScriptCommand::SetScatterExtraPellets { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scatter>(e) {
                        sc.extra_pellets = count;
                    }
                }
            }
            ScriptCommand::SetScatterEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scatter>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ScopeIn { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.scope_in();
                    }
                }
            }
            ScriptCommand::ScopeOut { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.scope_out();
                    }
                }
            }
            ScriptCommand::SetScopeAccuracyBonus { name, bonus } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.accuracy_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetScopeRangeBonus { name, bonus } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.range_bonus = bonus;
                    }
                }
            }
            ScriptCommand::SetScopeMoveSpeedPenalty { name, penalty } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.move_speed_penalty = penalty;
                    }
                }
            }
            ScriptCommand::SetScopeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scope>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyScorch { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scorch>(e) {
                        sc.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearScorch { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scorch>(e) {
                        sc.clear();
                    }
                }
            }
            ScriptCommand::SetScorchFireAmplify { name, amplify } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scorch>(e) {
                        sc.fire_amplify = amplify;
                    }
                }
            }
            ScriptCommand::SetScorchDotRate { name, rate } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scorch>(e) {
                        sc.dot_rate = rate;
                    }
                }
            }
            ScriptCommand::SetScorchEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sc) = world.get_mut::<bsengine_core::Scorch>(e) {
                        sc.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetShearArmorPenetration { name, penetration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shear>(e) {
                        sh.armor_penetration = penetration;
                    }
                }
            }
            ScriptCommand::SetShearFlatPenetration { name, flat } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shear>(e) {
                        sh.flat_penetration = flat;
                    }
                }
            }
            ScriptCommand::SetShearEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shear>(e) {
                        sh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyShock { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shock>(e) {
                        sh.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearShock { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shock>(e) {
                        sh.clear();
                    }
                }
            }
            ScriptCommand::SetShockDamagePerSecond { name, damage } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shock>(e) {
                        sh.damage_per_second = damage;
                    }
                }
            }
            ScriptCommand::SetShockInterruptChance { name, chance } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shock>(e) {
                        sh.interrupt_chance = chance;
                    }
                }
            }
            ScriptCommand::SetShockEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shock>(e) {
                        sh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AfflictShrivel { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.afflict();
                    }
                }
            }
            ScriptCommand::CleanseShrivel { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.cleanse();
                    }
                }
            }
            ScriptCommand::SetShrivelRate { name, rate } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.shrivel_rate = rate;
                    }
                }
            }
            ScriptCommand::SetShrivelRecoveryRate { name, rate } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.recovery_rate = rate;
                    }
                }
            }
            ScriptCommand::SetShrivelFactor { name, factor } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.shrivel_factor = factor;
                    }
                }
            }
            ScriptCommand::SetShrivelEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shrivel>(e) {
                        sh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetShroudCharges { name, charges } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shroud>(e) {
                        sh.charges = charges;
                    }
                }
            }
            ScriptCommand::SetShroudSaveHealthFraction { name, fraction } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shroud>(e) {
                        sh.save_health_fraction = fraction;
                    }
                }
            }
            ScriptCommand::SetShroudCooldown { name, cooldown } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shroud>(e) {
                        sh.cooldown = cooldown;
                    }
                }
            }
            ScriptCommand::SetShroudEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shroud>(e) {
                        sh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TryShunt { name, magnitude } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shunt>(e) {
                        sh.try_shunt(magnitude);
                    }
                }
            }
            ScriptCommand::SetShuntResistance { name, resistance } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shunt>(e) {
                        sh.shunt_resistance = resistance;
                    }
                }
            }
            ScriptCommand::SetShuntCooldown { name, cooldown } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shunt>(e) {
                        sh.cooldown = cooldown;
                    }
                }
            }
            ScriptCommand::SetShuntEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sh) = world.get_mut::<bsengine_core::Shunt>(e) {
                        sh.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySilence { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Silence>(e) {
                        si.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearSilence { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Silence>(e) {
                        si.clear();
                    }
                }
            }
            ScriptCommand::SetSilenceEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Silence>(e) {
                        si.enabled = enabled;
                    }
                }
            }
            ScriptCommand::StartSiphon { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Siphon>(e) {
                        si.start(duration);
                    }
                }
            }
            ScriptCommand::StopSiphon { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Siphon>(e) {
                        si.stop();
                    }
                }
            }
            ScriptCommand::SetSiphonEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut si) = world.get_mut::<bsengine_core::Siphon>(e) {
                        si.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginSlam { name, height } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slam>(e) {
                        sl.begin(height);
                    }
                }
            }
            ScriptCommand::LandSlam { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slam>(e) {
                        sl.land();
                    }
                }
            }
            ScriptCommand::SetSlamEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slam>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::RegisterKill { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slay>(e) {
                        sl.register_kill();
                    }
                }
            }
            ScriptCommand::SetSlayEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slay>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::StartSlide {
                name,
                dir_x,
                dir_y,
                dir_z,
            } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slide>(e) {
                        sl.start(glam::Vec3::new(dir_x, dir_y, dir_z));
                    }
                }
            }
            ScriptCommand::CancelSlide { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slide>(e) {
                        sl.cancel();
                    }
                }
            }
            ScriptCommand::SetSlideEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slide>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySlime { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slime>(e) {
                        sl.apply_slime(duration);
                    }
                }
            }
            ScriptCommand::CleanseSlime { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slime>(e) {
                        sl.cleanse();
                    }
                }
            }
            ScriptCommand::SetSlimeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slime>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::EngageSlink { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slink>(e) {
                        sl.engage();
                    }
                }
            }
            ScriptCommand::DisengageSlink { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slink>(e) {
                        sl.disengage();
                    }
                }
            }
            ScriptCommand::SetSlinkEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Slink>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetSlowMoEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sm) = world.get_mut::<bsengine_core::SlowMo>(e) {
                        sm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetSmokeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sm) = world.get_mut::<bsengine_core::Smoke>(e) {
                        sm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySnare { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sn) = world.get_mut::<bsengine_core::Snare>(e) {
                        sn.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearSnare { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sn) = world.get_mut::<bsengine_core::Snare>(e) {
                        sn.clear();
                    }
                }
            }
            ScriptCommand::SetSnareEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sn) = world.get_mut::<bsengine_core::Snare>(e) {
                        sn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AbsorbSoak { name, amount } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut so) = world.get_mut::<bsengine_core::Soak>(e) {
                        so.absorb(amount);
                    }
                }
            }
            ScriptCommand::SetSoakEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut so) = world.get_mut::<bsengine_core::Soak>(e) {
                        so.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ExtendSpike { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Spike>(e) {
                        sp.extend(duration);
                    }
                }
            }
            ScriptCommand::RetractSpike { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Spike>(e) {
                        sp.retract();
                    }
                }
            }
            ScriptCommand::SetSpikeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Spike>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetSplinterEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Splinter>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyStagger { name, force } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Stagger>(e) {
                        sg.apply(force);
                    }
                }
            }
            ScriptCommand::ResetStagger { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Stagger>(e) {
                        sg.reset();
                    }
                }
            }
            ScriptCommand::SetStaggerEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Stagger>(e) {
                        sg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetStakeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sk) = world.get_mut::<bsengine_core::Stake>(e) {
                        sk.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginStalk { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<bsengine_core::Stalk>(e) {
                        st.begin();
                    }
                }
            }
            ScriptCommand::SetStalkEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut st) = world.get_mut::<bsengine_core::Stalk>(e) {
                        st.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetStance { name, kind } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sa) = world.get_mut::<bsengine_core::Stance>(e) {
                        let stance_kind = match kind {
                            1 => bsengine_core::StanceKind::Offensive,
                            2 => bsengine_core::StanceKind::Defensive,
                            _ => bsengine_core::StanceKind::Neutral,
                        };
                        sa.set_stance(stance_kind);
                    }
                }
            }
            ScriptCommand::SetStanceEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sa) = world.get_mut::<bsengine_core::Stance>(e) {
                        sa.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddStatBonus { name, amount } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<bsengine_core::Stat>(e) {
                        ss.add_bonus(amount);
                    }
                }
            }
            ScriptCommand::RemoveStatBonus { name, amount } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ss) = world.get_mut::<bsengine_core::Stat>(e) {
                        ss.remove_bonus(amount);
                    }
                }
            }
            ScriptCommand::StartSneaking { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<bsengine_core::Stealth>(e) {
                        se.start_sneaking();
                    }
                }
            }
            ScriptCommand::StopSneaking { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<bsengine_core::Stealth>(e) {
                        se.stop_sneaking();
                    }
                }
            }
            ScriptCommand::AddNoise { name, amount } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<bsengine_core::Stealth>(e) {
                        se.add_noise(amount);
                    }
                }
            }
            ScriptCommand::SetStealthEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut se) = world.get_mut::<bsengine_core::Stealth>(e) {
                        se.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerStomp { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sm) = world.get_mut::<bsengine_core::Stomp>(e) {
                        sm.trigger();
                    }
                }
            }
            ScriptCommand::SetStompEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sm) = world.get_mut::<bsengine_core::Stomp>(e) {
                        sm.enabled = enabled;
                    }
                }
            }
            ScriptCommand::StepStride { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Stride>(e) {
                        sd.step();
                    }
                }
            }
            ScriptCommand::BreakStride { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Stride>(e) {
                        sd.break_stride();
                    }
                }
            }
            ScriptCommand::SetStrideEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Stride>(e) {
                        sd.enabled = enabled;
                    }
                }
            }
            ScriptCommand::HitStrife { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sf) = world.get_mut::<bsengine_core::Strife>(e) {
                        sf.hit();
                    }
                }
            }
            ScriptCommand::SetStrifeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sf) = world.get_mut::<bsengine_core::Strife>(e) {
                        sf.enabled = enabled;
                    }
                }
            }
            ScriptCommand::TriggerStumble { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut su) = world.get_mut::<bsengine_core::Stumble>(e) {
                        su.trigger();
                    }
                }
            }
            ScriptCommand::SetStumbleEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut su) = world.get_mut::<bsengine_core::Stumble>(e) {
                        su.enabled = enabled;
                    }
                }
            }
            ScriptCommand::BeginSulk { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Sulk>(e) {
                        sl.begin_sulk();
                    }
                }
            }
            ScriptCommand::EndSulk { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Sulk>(e) {
                        sl.end_sulk();
                    }
                }
            }
            ScriptCommand::SetSulkEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sl) = world.get_mut::<bsengine_core::Sulk>(e) {
                        sl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySunder { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Sunder>(e) {
                        sd.apply(count);
                    }
                }
            }
            ScriptCommand::RepairSunder { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Sunder>(e) {
                        sd.repair(count);
                    }
                }
            }
            ScriptCommand::RepairAllSunder { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Sunder>(e) {
                        sd.repair_all();
                    }
                }
            }
            ScriptCommand::SetSunderEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sd) = world.get_mut::<bsengine_core::Sunder>(e) {
                        sd.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySuppress { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Suppress>(e) {
                        sp.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearSuppress { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Suppress>(e) {
                        sp.clear();
                    }
                }
            }
            ScriptCommand::SetSuppressEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sp) = world.get_mut::<bsengine_core::Suppress>(e) {
                        sp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplySurge { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Surge>(e) {
                        sg.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearSurge { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Surge>(e) {
                        sg.clear();
                    }
                }
            }
            ScriptCommand::SetSurgeEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sg) = world.get_mut::<bsengine_core::Surge>(e) {
                        sg.enabled = enabled;
                    }
                }
            }
            ScriptCommand::UpdateSurround { name, count } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sr) = world.get_mut::<bsengine_core::Surround>(e) {
                        sr.update(count);
                    }
                }
            }
            ScriptCommand::SetSurroundEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sr) = world.get_mut::<bsengine_core::Surround>(e) {
                        sr.enabled = enabled;
                    }
                }
            }
            ScriptCommand::AddSurviveCharge { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sv) = world.get_mut::<bsengine_core::Survive>(e) {
                        sv.add_charge();
                    }
                }
            }
            ScriptCommand::SetSurviveEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sv) = world.get_mut::<bsengine_core::Survive>(e) {
                        sv.enabled = enabled;
                    }
                }
            }
            ScriptCommand::EnterWater { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sw) = world.get_mut::<bsengine_core::Swim>(e) {
                        sw.enter_water();
                    }
                }
            }
            ScriptCommand::ExitWater { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sw) = world.get_mut::<bsengine_core::Swim>(e) {
                        sw.exit_water();
                    }
                }
            }
            ScriptCommand::SetWantsDive { name, wants } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sw) = world.get_mut::<bsengine_core::Swim>(e) {
                        sw.wants_dive = wants;
                    }
                }
            }
            ScriptCommand::SetWantsSurface { name, wants } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sw) = world.get_mut::<bsengine_core::Swim>(e) {
                        sw.wants_surface = wants;
                    }
                }
            }
            ScriptCommand::SetSwimEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut sw) = world.get_mut::<bsengine_core::Swim>(e) {
                        sw.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyTaint { name, duration } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ta) = world.get_mut::<bsengine_core::Taint>(e) {
                        ta.apply(duration);
                    }
                }
            }
            ScriptCommand::ClearTaint { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ta) = world.get_mut::<bsengine_core::Taint>(e) {
                        ta.clear();
                    }
                }
            }
            ScriptCommand::SetTaintEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut ta) = world.get_mut::<bsengine_core::Taint>(e) {
                        ta.enabled = enabled;
                    }
                }
            }
            ScriptCommand::IncrementTally { name, amount } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tl) = world.get_mut::<bsengine_core::Tally>(e) {
                        tl.increment(amount);
                    }
                }
            }
            ScriptCommand::ResetTally { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tl) = world.get_mut::<bsengine_core::Tally>(e) {
                        tl.reset();
                    }
                }
            }
            ScriptCommand::SetTallyEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tl) = world.get_mut::<bsengine_core::Tally>(e) {
                        tl.enabled = enabled;
                    }
                }
            }
            ScriptCommand::SetGripping { name, gripping } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tn) = world.get_mut::<bsengine_core::Talon>(e) {
                        tn.set_gripping(gripping);
                    }
                }
            }
            ScriptCommand::SetTalonEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tn) = world.get_mut::<bsengine_core::Talon>(e) {
                        tn.enabled = enabled;
                    }
                }
            }
            ScriptCommand::EngageTaper { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tp) = world.get_mut::<bsengine_core::Taper>(e) {
                        tp.engage();
                    }
                }
            }
            ScriptCommand::DisengageTaper { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tp) = world.get_mut::<bsengine_core::Taper>(e) {
                        tp.disengage();
                    }
                }
            }
            ScriptCommand::SetTaperEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tp) = world.get_mut::<bsengine_core::Taper>(e) {
                        tp.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ActivateTaunt { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tt) = world.get_mut::<bsengine_core::Taunt>(e) {
                        tt.activate();
                    }
                }
            }
            ScriptCommand::DeactivateTaunt { name } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tt) = world.get_mut::<bsengine_core::Taunt>(e) {
                        tt.deactivate();
                    }
                }
            }
            ScriptCommand::SetTauntEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut tt) = world.get_mut::<bsengine_core::Taunt>(e) {
                        tt.enabled = enabled;
                    }
                }
            }
            ScriptCommand::ApplyFreeze { name, intensity } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut th) = world.get_mut::<bsengine_core::Thaw>(e) {
                        th.apply_freeze(intensity);
                    }
                }
            }
            ScriptCommand::SetThawEnabled { name, enabled } => {
                let entity = {
                    let mut q = world.query::<(Entity, &Name)>();
                    q.iter(world).find(|(_, n)| n.0 == name).map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut th) = world.get_mut::<bsengine_core::Thaw>(e) {
                        th.enabled = enabled;
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
