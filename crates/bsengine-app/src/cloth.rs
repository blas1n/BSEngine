//! Simulated cloth: a [`Cloth`] entity gets a generated sheet mesh of its
//! own, and every frame that sheet is stepped by [`cloth_solver`] and
//! re-uploaded.
//!
//! The split mirrors `terrain`/`terrain_chunking`: the component is plain data
//! in `bsengine-scene`, the maths is ECS-free in
//! [`cloth_solver`](crate::cloth_solver), and this module is only the wiring --
//! generate once, then step and upload. The per-frame upload path itself is the
//! one CPU skinning already uses (`GpuMeshRegistry::update_vertices`), for the
//! same reason: the vertex count never changes, so the buffer is overwritten
//! rather than rebuilt.

use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::schedule::IntoSystemConfigs;
use bsengine_core::{GlobalTransform, Time, Transform};
use bsengine_ecs::{Commands, Component, Entity, Query, Res, ResMut, Without};
use bsengine_physics::PhysicsWorld;
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuQueueResource, Vertex};
use glam::Vec3;
use tracing::warn;

pub use bsengine_scene::Cloth;

use crate::cloth_solver::{self, Link};

/// A cloth's live simulation state: where its vertices are, where they were
/// last step, and which pairs of them are held at a distance.
///
/// Separate from [`Cloth`] and not reflected, matching `PendingTerrain`'s
/// precedent: none of this is meaningfully serializable scene state (it is
/// re-derived from the `Cloth` the moment the entity is spawned), and keeping
/// it off the authored component keeps that component's RON clean. Private for
/// the same reason -- nothing outside this module has a use for it.
#[derive(Component)]
struct ClothSim {
    /// The GPU mesh generated for this cloth, and the only one its deformed
    /// vertices are ever uploaded into. Generated per entity rather than shared,
    /// because `Primitive::Plane` hands every plane in the scene the same mesh
    /// id and deforming that would drag unrelated entities along.
    mesh_id: u64,
    /// Current vertex positions, in the entity's local space. The solver's
    /// working array.
    positions: Vec<Vec3>,
    /// Positions one step ago -- the velocity, in Verlet's sense.
    previous: Vec<Vec3>,
    /// Distance constraints, derived once from the generated grid.
    links: Vec<Link>,
    /// Staging copy of the mesh's vertices, reused every frame so the upload
    /// does not allocate. Positions and normals are overwritten; colour and UV
    /// stay as generated.
    vertices: Vec<Vertex>,
    /// The generated grid's triangles, kept for normal recomputation.
    indices: Vec<u32>,
    /// Whether the most recent step's vertices actually reached the GPU.
    ///
    /// Recorded because the upload is otherwise unobservable from outside:
    /// a mesh's vertex buffer is created `VERTEX | COPY_DST`, so nothing can
    /// read it back, and a deleted upload would leave this module's own state
    /// perfectly correct while the screen showed a sheet frozen flat. The same
    /// reason `AudioWorld` records the poses a device-less backend never plays.
    uploaded: bool,
}

/// Marks a [`Cloth`] whose description cannot be simulated, so the warning is
/// logged once instead of every frame.
///
/// A rejected cloth is left with no mesh at all rather than a flat placeholder:
/// an invisible entity sends the author back to the scene file, where a sheet
/// that silently refuses to move looks like a solver bug.
#[derive(Component)]
struct ClothRejected;

/// Generates each [`Cloth`]'s sheet mesh and steps it every frame.
pub struct ClothPlugin;

impl Plugin for ClothPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Cloth>()
            .add_systems(Update, generate_cloth)
            .add_systems(
                PostUpdate,
                // After the world transforms are propagated, because that is
                // what tells the sheet where it is -- and collision is resolved
                // against colliders that live only in world space, so a
                // frame-stale one resolves a cape against where its character
                // stood last frame. `render_frame` is private to
                // `bsengine-render` and cannot be ordered against from here, so
                // the upload lands on the same terms `update_skinned_meshes`
                // has always had.
                simulate_cloth.after(bsengine_core::propagate_global_transforms),
            );
    }
}

/// Cloth entities still waiting for a sheet: not yet simulated, and not already
/// refused. Named rather than written inline because the pair of filters is past
/// what clippy will read in a parameter list.
type PendingCloths<'w, 's> =
    Query<'w, 's, (Entity, &'static Cloth), (Without<ClothSim>, Without<ClothRejected>)>;

fn generate_cloth(
    mut commands: Commands,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    query: PendingCloths,
) {
    for (entity, cloth) in query.iter() {
        let Some((vertices, indices)) =
            cloth_solver::grid(cloth.columns, cloth.rows, cloth.spacing)
        else {
            warn!(
                "[cloth] a cloth of {} x {} vertices at spacing {} cannot be simulated; \
                 needs at least 2 x 2, a positive spacing, and at most {} vertices",
                cloth.columns,
                cloth.rows,
                cloth.spacing,
                cloth_solver::MAX_CLOTH_VERTICES
            );
            commands.entity(entity).insert(ClothRejected);
            continue;
        };

        // Waiting, not rejecting: a headless host has no registry at all and a
        // windowed one builds it when the window appears, so this is the same
        // poll-until-available the terrain and glTF loaders do.
        let Some(registry) = mesh_registry.as_mut() else {
            continue;
        };

        // A pin naming a vertex the sheet does not have simply never holds
        // anything, and the cloth then falls out of the world for a reason
        // nothing on screen explains -- worth a line in the log, since the
        // indices are hand-written arithmetic (`row * columns + column`).
        let out_of_range: Vec<u32> = cloth
            .pinned
            .iter()
            .copied()
            .filter(|&i| i >= cloth.columns * cloth.rows)
            .collect();
        if !out_of_range.is_empty() {
            warn!(
                "[cloth] pinned vertices {out_of_range:?} are outside a {} x {} sheet \
                 (highest index {}); they hold nothing",
                cloth.columns,
                cloth.rows,
                cloth.columns * cloth.rows - 1
            );
        }

        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let links = cloth_solver::links_from_indices(&positions, &indices);
        let mesh_id = registry.register(&vertices, &indices);
        commands.entity(entity).insert((
            MeshRenderer { mesh_id },
            ClothSim {
                mesh_id,
                previous: positions.clone(),
                positions,
                links,
                vertices,
                indices,
                uploaded: false,
            },
        ));
    }
}

/// How far into a collider a vertex can be and still be pushed back out.
///
/// `project_point`'s `max_dist` is measured to the *surface*, so a vertex deep
/// inside a large shape is not merely far from it, it is invisible to the query
/// and stays stuck there. Half a metre is well past what a frame can produce: a
/// sheet that has been falling for a second is doing about 10 m/s, which is
/// 16 cm in a 1/60 step.
const COLLISION_REACH: f32 = 0.5;

/// Moves each free vertex out of whatever it is inside, and off whatever it is
/// resting on by `thickness`.
///
/// Pinned vertices are left alone. A pin is the one promise this component
/// makes unconditionally, and a cloth pinned to a point inside a collider --
/// a cape fixed to a shoulder that has a capsule in it -- would otherwise tear
/// itself off its own anchor.
fn push_out_of_colliders(
    positions: &mut [Vec3],
    previous: &mut [Vec3],
    pinned: &[u32],
    model: &glam::Mat4,
    thickness: f32,
    physics: &PhysicsWorld,
) {
    if !thickness.is_finite() || thickness <= 0.0 {
        return;
    }
    let inverse = model.inverse();
    for i in 0..positions.len() {
        if pinned.contains(&(i as u32)) {
            continue;
        }
        let world = model.transform_point3(positions[i]);
        let Some(hit) = physics.project_point(world, thickness + COLLISION_REACH) else {
            continue;
        };

        // The direction out. For a vertex inside the shape that is *toward* the
        // nearest surface point; for one outside it is away from it. Getting
        // this backwards on either branch drags the sheet into the collider
        // instead of off it.
        let away = if hit.inside {
            hit.point - world
        } else {
            world - hit.point
        };
        let Some(outward) = away.try_normalize() else {
            // Exactly on the surface: no direction to leave along, and the next
            // step's gravity will produce one.
            continue;
        };
        if !hit.inside && away.length() >= thickness {
            // Outside and already clear. The common case, and the reason this
            // is a projection rather than a sweep.
            continue;
        }

        let target = inverse.transform_point3(hit.point + outward * thickness);
        let moved = target - positions[i];
        if moved.length_squared() <= f32::EPSILON {
            continue;
        }
        positions[i] = target;

        // Take the velocity *into* the surface away, and leave the rest. Zeroing
        // all of it would be infinite friction -- cloth would stick where it
        // landed instead of sliding off a slope -- and leaving all of it would
        // turn every push-out into a bounce.
        let normal = inverse.transform_vector3(outward).normalize_or_zero();
        let velocity = positions[i] - previous[i];
        let tangent = velocity - normal * velocity.dot(normal);
        previous[i] = positions[i] - tangent;
    }
}

fn simulate_cloth(
    time: Res<Time>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    queue: Option<Res<GpuQueueResource>>,
    physics: Option<Res<PhysicsWorld>>,
    mut query: Query<(&Cloth, &Transform, Option<&GlobalTransform>, &mut ClothSim)>,
) {
    let dt = time.delta_seconds;
    for (cloth, transform, global, mut sim) in query.iter_mut() {
        // The entity's *world* transform, preferring `GlobalTransform` exactly
        // as `render_frame` does. Not a refinement: a cape is parented to the
        // character wearing it, so its own `Transform` is an offset from a
        // shoulder and says nothing about where the cloth is in the world --
        // which is the only space the colliders it has to avoid live in.
        let model = global
            .map(|g| g.to_matrix())
            .unwrap_or_else(|| transform.to_matrix());
        let (_, rotation, _) = model.to_scale_rotation_translation();

        // Gravity is authored in world space, but the sheet is simulated in the
        // entity's local space (its vertices are what the mesh buffer holds), so
        // the entity's rotation has to come out of it. Without this a curtain
        // rotated flat onto its side falls sideways along its own surface.
        //
        // Scale is deliberately not undone: a non-uniformly scaled cloth
        // simulates in its own space and is stretched on the way to the screen,
        // which is what every other component on the entity does too.
        let local_gravity = rotation.inverse() * Vec3::from(cloth.gravity);

        let ClothSim {
            positions,
            previous,
            links,
            ..
        } = &mut *sim;
        cloth_solver::step(
            positions,
            previous,
            links,
            &cloth.pinned,
            local_gravity,
            dt,
            cloth.iterations,
            cloth.stiffness,
            cloth.damping,
        );

        // Collision last, after the constraints rather than inside them: a
        // constraint pass run afterwards would pull vertices straight back into
        // whatever they were just pushed out of, and visible interpenetration
        // reads far worse than the fraction of a percent of stretch this
        // leaves behind.
        if let Some(physics) = physics.as_ref() {
            push_out_of_colliders(
                positions,
                previous,
                &cloth.pinned,
                &model,
                cloth.collision_thickness,
                physics,
            );
        }

        // The deformed sheet is written back whether or not there is a GPU, so a
        // headless host still has the simulated positions to assert on -- the
        // same reason `AudioWorld` records poses a silent backend never plays.
        let ClothSim {
            positions,
            vertices,
            indices,
            ..
        } = &mut *sim;
        for (vertex, position) in vertices.iter_mut().zip(positions.iter()) {
            vertex.position = position.to_array();
        }
        cloth_solver::recompute_normals(vertices, indices);

        sim.uploaded = match (mesh_registry.as_mut(), queue.as_ref()) {
            (Some(registry), Some(queue)) => {
                registry.update_vertices(&queue.0, sim.mesh_id, &sim.vertices)
            }
            _ => false,
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;
    use glam::Quat;

    /// A curtain: 4 x 4 vertices, held along its first row.
    ///
    /// ⚠️ Not at the origin and not axis-aligned. A cloth at the origin with an
    /// identity rotation makes the local/world distinction unobservable -- the
    /// rotation could be dropped from `simulate_cloth` entirely and every
    /// assertion here would still pass.
    const CLOTH_AT: Vec3 = Vec3::new(3.0, 7.0, -2.0);

    fn curtain() -> Cloth {
        Cloth {
            columns: 4,
            rows: 4,
            spacing: 0.5,
            pinned: (0..4).collect(),
            gravity: [0.0, -9.81, 0.0],
            stiffness: 0.9,
            damping: 0.02,
            iterations: 8,
            collision_thickness: 0.01,
        }
    }

    /// An app with everything the cloth systems need: `ClothPlugin`, a fixed
    /// clock, and a real headless `GpuMeshRegistry` + `GpuQueueResource` built
    /// from *one* device -- `update_vertices` writes through the queue into a
    /// buffer the device made, so two devices would not be a slower version of
    /// this test, it would be a different and invalid one.
    ///
    /// Mirrors `terrain`'s `insert_headless_mesh_registry`, which explains why
    /// a window-less test has to build these itself.
    fn test_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(crate::TimePlugin);
        app.add_plugins(ClothPlugin);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("a headless adapter; the rest of this suite already requires one");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bsengine-app cloth test device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("headless device request");
        app.insert_resource(GpuMeshRegistry::new(std::sync::Arc::new(device)));
        app.insert_resource(GpuQueueResource(std::sync::Arc::new(queue)));
        // After the plugins, so `TimePlugin`'s own wall-clock `Time` does not
        // win: a headless frame takes well under a millisecond, and gravity
        // over a microsecond moves nothing measurable.
        app.insert_resource(Time::fixed(1.0 / 60.0));
        app
    }

    fn spawn(app: &mut bevy_app::App, cloth: Cloth, rotation: Quat) -> Entity {
        app.world_mut()
            .spawn((
                cloth,
                Transform {
                    position: CLOTH_AT.into(),
                    rotation: rotation.into(),
                    scale: Vec3::ONE.into(),
                },
            ))
            .id()
    }

    #[test]
    fn a_cloth_entity_gets_a_mesh_of_its_own() {
        let mut app = test_app();
        let a = spawn(&mut app, curtain(), Quat::IDENTITY);
        let b = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();

        let ids: Vec<u64> = [a, b]
            .iter()
            .map(|e| {
                app.world()
                    .get::<MeshRenderer>(*e)
                    .expect("a cloth must be drawable")
                    .mesh_id
            })
            .collect();
        for id in &ids {
            assert!(
                app.world().resource::<GpuMeshRegistry>().get(*id).is_some(),
                "mesh id {id} must be really registered, not a zero-valued default"
            );
        }
        // ⚠️ Two cloths, two meshes. Sharing one -- which is what reusing
        // `Primitive::Plane`'s cached id would do -- makes each sheet deform the
        // other, and a single-cloth test cannot see it.
        assert_ne!(ids[0], ids[1], "each cloth needs its own vertex buffer");
    }

    #[test]
    fn the_sheet_falls_and_the_pinned_row_does_not() {
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();
        let start = app
            .world()
            .get::<ClothSim>(entity)
            .expect("generated on the first frame")
            .positions
            .clone();
        for _ in 0..60 {
            app.update();
        }
        let now = &app.world().get::<ClothSim>(entity).unwrap().positions;

        for i in 0..4 {
            assert!(
                (now[i] - start[i]).length() < 1.0e-5,
                "pinned vertex {i} moved: {:?} from {:?}",
                now[i],
                start[i]
            );
        }
        for i in 12..16 {
            assert!(
                now[i].y < start[i].y - 0.05,
                "free vertex {i} should have fallen: {:?} from {:?}",
                now[i],
                start[i]
            );
        }
    }

    #[test]
    fn a_rotated_cloth_hangs_below_its_pin_in_the_world() {
        // ⚠️ The whole of why `simulate_cloth` un-rotates gravity, and the
        // fixture is built so the two answers point in *different* directions
        // rather than merely differing in size.
        //
        // The sheet is turned a quarter of a turn about +z, so its own plane
        // now contains world down, and it is held by one corner so it can
        // actually swing. Its far corner starts 1.5 units *above* the pin and
        // has to end up below it. Handed world gravity unchanged the sheet
        // would instead bend out of its own plane -- sideways, in the world --
        // and that corner would stay where it started.
        //
        // An earlier version of this test pinned the whole top row, and the
        // sheet then barely moved at all: a load lying in a triangulated
        // sheet's own plane is carried by its edges and diagonals, so all it
        // produces is a few millimetres of stretch. Correct physics, and
        // almost no signal.
        let mut app = test_app();
        let rotation = Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let entity = spawn(
            &mut app,
            Cloth {
                pinned: vec![0],
                ..curtain()
            },
            rotation,
        );
        app.update();

        let world = |app: &bevy_app::App, i: usize| -> Vec3 {
            CLOTH_AT + rotation * app.world().get::<ClothSim>(entity).unwrap().positions[i]
        };
        let pin = world(&app, 0);
        assert!(
            world(&app, 15).y > pin.y + 1.0,
            "the fixture is pointless unless the far corner starts above the \
             pin: corner {:?}, pin {pin:?}",
            world(&app, 15)
        );

        for _ in 0..120 {
            app.update();
        }
        assert!(
            (world(&app, 0) - pin).length() < 1.0e-5,
            "the pinned corner must not have moved: {:?}",
            world(&app, 0)
        );
        assert!(
            world(&app, 15).y < pin.y - 1.0,
            "the far corner should have swung below the pin: corner {:?}, \
             pin {pin:?}",
            world(&app, 15)
        );
    }

    #[test]
    fn the_uploaded_normals_follow_the_hanging_sheet() {
        // The mesh is what the renderer sees, and its normals are recomputed
        // rather than carried over from the flat grid. Without this the
        // simulation could be perfect and the curtain would still be lit as
        // though it were lying on the floor.
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();
        for _ in 0..60 {
            app.update();
        }
        let sim = app.world().get::<ClothSim>(entity).unwrap();
        let bottom = Vec3::from(sim.vertices[14].normal);
        assert!(
            (bottom.length() - 1.0).abs() < 1.0e-4,
            "normals must stay unit length, got {}",
            bottom.length()
        );
        assert!(
            bottom.y < 0.9,
            "a hanging sheet's normal must have tipped away from straight up, \
             got {bottom:?}"
        );
    }

    #[test]
    fn the_deformed_vertices_reach_the_gpu() {
        // ⚠️ The one piece of this feature nothing else can see. Every other
        // test here reads `ClothSim`, which stays perfectly correct with the
        // upload deleted -- the classic disconnected producer, indistinguishable
        // from a working one at the consumer.
        //
        // The registry and queue are real, so a `true` here also means wgpu
        // accepted exactly as many bytes as the buffer holds.
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();
        app.update();
        assert!(
            app.world().get::<ClothSim>(entity).unwrap().uploaded,
            "the simulated sheet must be uploaded into the mesh the renderer draws"
        );
    }

    /// Where the top face of [`floor`]'s slab sits.
    ///
    /// ⚠️ Not y = 0. A floor at the origin cannot tell "stopped on the surface"
    /// from "reset to zero", and the push-out writes an absolute position.
    const FLOOR_TOP: f32 = -1.25;

    /// A fixed slab wide enough that a falling sheet cannot miss it, with its
    /// top face at [`FLOOR_TOP`].
    /// ⚠️ Positioned through `PhysicsInput`, not `Transform`. `spawn_bodies`
    /// reads the former and defaults to the origin without it -- a floor
    /// authored by `Transform` alone lands at y = 0 with its top at +1, which
    /// looks exactly like a cloth that stopped too early.
    fn floor(app: &mut bevy_app::App) {
        app.world_mut().spawn((
            bsengine_core::Transform {
                position: Vec3::new(CLOTH_AT.x, FLOOR_TOP - 1.0, CLOTH_AT.z).into(),
                rotation: Quat::IDENTITY.into(),
                scale: Vec3::ONE.into(),
            },
            bsengine_physics::PhysicsInput {
                position: Vec3::new(CLOTH_AT.x, FLOOR_TOP - 1.0, CLOTH_AT.z).into(),
                rotation: Quat::IDENTITY.into(),
            },
            bsengine_physics::RigidBody::fixed(),
            bsengine_physics::Collider::cuboid(8.0, 1.0, 8.0),
        ));
    }

    /// `test_app` plus a real physics world, so the cloth has something to be
    /// pushed out of. Without `PhysicsPlugin` there is no `PhysicsWorld` and the
    /// collision pass does not run at all -- which is what keeps every test
    /// above measuring the solver alone.
    fn physical_app() -> bevy_app::App {
        let mut app = test_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        floor(&mut app);
        app
    }

    /// The world-space y of the sheet's lowest vertex.
    fn lowest(app: &bevy_app::App, entity: Entity) -> f32 {
        let sim = app.world().get::<ClothSim>(entity).unwrap();
        sim.positions
            .iter()
            .map(|p| CLOTH_AT.y + p.y)
            .fold(f32::MAX, f32::min)
    }

    /// An unpinned sheet, so gravity is free to take the whole thing down onto
    /// whatever is below it.
    fn loose() -> Cloth {
        Cloth {
            pinned: Vec::new(),
            ..curtain()
        }
    }

    #[test]
    fn a_falling_sheet_comes_to_rest_on_a_collider() {
        let mut app = physical_app();
        let entity = spawn(&mut app, loose(), Quat::IDENTITY);
        for _ in 0..240 {
            app.update();
        }
        let resting = lowest(&app, entity);
        assert!(
            resting > FLOOR_TOP - 1.0e-3,
            "the sheet went through the floor: lowest vertex at {resting}, \
             floor at {FLOOR_TOP}"
        );
        assert!(
            resting < FLOOR_TOP + 0.1,
            "and it has to actually reach the floor rather than hang in the \
             air: {resting}"
        );
    }

    #[test]
    fn collision_off_lets_the_sheet_through() {
        // ⚠️ The negative half, and the one that makes the positive mean
        // something. Without it, a sheet that stopped 1.25 below where it
        // started could be a sheet resting on a floor or a sheet that never
        // fell that far -- and the same test would pass either way.
        let mut app = physical_app();
        let entity = spawn(
            &mut app,
            Cloth {
                collision_thickness: 0.0,
                ..loose()
            },
            Quat::IDENTITY,
        );
        for _ in 0..240 {
            app.update();
        }
        assert!(
            lowest(&app, entity) < FLOOR_TOP - 1.0,
            "with collision off the sheet must keep falling, got {}",
            lowest(&app, entity)
        );
    }

    #[test]
    fn the_sheet_rests_its_own_thickness_above_the_surface() {
        // Proves the field is a distance and not a flag. A deliberately fat
        // sheet, because the 1cm default is inside the tolerance any
        // "did it stop" assertion needs.
        let mut app = physical_app();
        let entity = spawn(
            &mut app,
            Cloth {
                collision_thickness: 0.25,
                ..loose()
            },
            Quat::IDENTITY,
        );
        for _ in 0..240 {
            app.update();
        }
        let resting = lowest(&app, entity);
        assert!(
            (resting - (FLOOR_TOP + 0.25)).abs() < 0.05,
            "a 0.25-thick sheet should rest at {}, got {resting}",
            FLOOR_TOP + 0.25
        );
    }

    #[test]
    fn a_sheet_on_a_slope_slides_down_it() {
        // ⚠️ The friction choice, and the only thing that observes it. The
        // push-out takes the velocity *into* the surface away and leaves the
        // rest; zeroing all of it instead is one character's difference and
        // makes cloth stick wherever it lands, which every other collision test
        // here would still pass.
        //
        // Gravity keeps pulling either way, so this is not "does it move at
        // all" -- the threshold is set past what a sheet whose velocity is
        // reset every frame can creep.
        let mut app = test_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        // Tipped a quarter-radian about +z, so the surface normal leans toward
        // -x and downhill is -x.
        let tilt = Quat::from_rotation_z(0.45);
        app.world_mut().spawn((
            bsengine_core::Transform::default(),
            bsengine_physics::PhysicsInput {
                position: Vec3::new(CLOTH_AT.x, FLOOR_TOP - 1.0, CLOTH_AT.z).into(),
                rotation: tilt.into(),
            },
            bsengine_physics::RigidBody::fixed(),
            bsengine_physics::Collider::cuboid(8.0, 1.0, 8.0),
        ));

        let entity = spawn(&mut app, loose(), Quat::IDENTITY);
        app.update();
        let centre = |app: &bevy_app::App| -> f32 {
            let sim = app.world().get::<ClothSim>(entity).unwrap();
            sim.positions.iter().map(|p| p.x).sum::<f32>() / sim.positions.len() as f32
        };
        let start = centre(&app);
        for _ in 0..240 {
            app.update();
        }
        let slid = start - centre(&app);
        assert!(
            slid > 0.3,
            "the sheet should have slid downhill along -x, moved {slid}"
        );
    }

    #[test]
    fn a_sheet_hanging_near_a_surface_is_not_dragged_onto_it() {
        // ⚠️ `project_point` is asked for anything within half a metre, because
        // a vertex deep inside a shape is otherwise unreachable. That makes the
        // "already clear" check load-bearing rather than an optimisation:
        // without it every vertex within that reach is snapped to
        // surface + thickness, and a curtain hanging near a wall gets sucked
        // flat against it. Every other collision test here drops the sheet
        // *onto* something, so none of them can see it.
        let mut app = physical_app();
        // Pinned along the top row and short enough that it hangs with its
        // bottom edge a clear 0.2 above the floor, but well within the reach.
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.world_mut()
            .get_mut::<bsengine_core::Transform>(entity)
            .unwrap()
            .position = Vec3::new(CLOTH_AT.x, FLOOR_TOP + 1.7, CLOTH_AT.z).into();

        for _ in 0..240 {
            app.update();
        }
        let sim = app.world().get::<ClothSim>(entity).unwrap();
        let bottom = sim
            .positions
            .iter()
            .map(|p| FLOOR_TOP + 1.7 + p.y)
            .fold(f32::MAX, f32::min);
        assert!(
            bottom > FLOOR_TOP + 0.1,
            "the sheet hangs clear of the floor and must stay there, got \
             {bottom} against a floor at {FLOOR_TOP}"
        );
    }

    #[test]
    fn a_parented_cloth_collides_where_it_actually_is() {
        // ⚠️ The case cloth collision exists for: a cape is parented to the
        // character wearing it, so its own `Transform` is an offset from a
        // shoulder and says nothing about where it is in the world. Resolving
        // against that offset puts the cape's collisions wherever the offset
        // happens to point -- here, nowhere near the floor it is actually
        // resting on.
        //
        // Every other test in this file spawns an unparented cloth, where the
        // world transform and the local one are the same matrix, so none of
        // them can tell the two apart.
        let mut app = test_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_systems(PostUpdate, bsengine_core::propagate_global_transforms);
        // ⚠️ A *narrow* slab, directly under where the cape really hangs and
        // nowhere near the world origin its local transform claims. The wide
        // floor the other tests use reaches over the origin too, so a sheet
        // resolved in the wrong space still landed on it and this test passed
        // with the world transform thrown away.
        app.world_mut().spawn((
            bsengine_core::Transform::default(),
            bsengine_physics::PhysicsInput {
                position: Vec3::new(CLOTH_AT.x, FLOOR_TOP - 1.0, CLOTH_AT.z).into(),
                rotation: Quat::IDENTITY.into(),
            },
            bsengine_physics::RigidBody::fixed(),
            bsengine_physics::Collider::cuboid(2.0, 1.0, 2.0),
        ));

        // ⚠️ Both of these carry a `GlobalTransform` because
        // `propagate_global_transforms` only *writes into* one that already
        // exists -- it never inserts it. Scene-spawned entities get one from
        // `spawn_scene_entities`; a hand-built pair does not, and without it
        // this test silently exercises the local-transform fallback instead of
        // the path it exists to cover.
        let parent = app
            .world_mut()
            .spawn((
                bsengine_core::Transform {
                    position: Vec3::new(CLOTH_AT.x, CLOTH_AT.y, CLOTH_AT.z).into(),
                    rotation: Quat::IDENTITY.into(),
                    scale: Vec3::ONE.into(),
                },
                bsengine_core::GlobalTransform::default(),
            ))
            .id();
        // The child sits at the origin *of its parent*, so its own `Transform`
        // claims it is at the world origin -- far above and to the side of the
        // floor slab, and nowhere the sheet could rest.
        let child = app
            .world_mut()
            .spawn((
                loose(),
                bsengine_core::Transform::default(),
                bsengine_core::GlobalTransform::default(),
                bsengine_core::Parent(parent),
            ))
            .id();

        for _ in 0..240 {
            app.update();
        }
        let sim = app.world().get::<ClothSim>(child).unwrap();
        let lowest_world = sim
            .positions
            .iter()
            .map(|p| CLOTH_AT.y + p.y)
            .fold(f32::MAX, f32::min);
        assert!(
            lowest_world > FLOOR_TOP - 1.0e-2,
            "the cape has to rest on the floor it is really above, not on \
             where its local transform says it is: {lowest_world} against a \
             floor at {FLOOR_TOP}"
        );
    }

    #[test]
    fn a_pin_inside_a_collider_is_still_a_pin() {
        // A cape is pinned to a shoulder, and a shoulder has a capsule in it.
        // If the collision pass moved pinned vertices the cloth would tear
        // itself off its own anchor on the first frame -- and every other
        // assertion here uses a sheet whose pins are in open air, so none of
        // them would notice.
        let mut app = physical_app();
        let buried = Cloth {
            // Vertex 0 is at the entity's own origin, which this test puts
            // inside the floor slab.
            pinned: vec![0],
            ..curtain()
        };
        let entity = app
            .world_mut()
            .spawn((
                buried,
                bsengine_core::Transform {
                    position: Vec3::new(CLOTH_AT.x, FLOOR_TOP - 0.3, CLOTH_AT.z).into(),
                    rotation: Quat::IDENTITY.into(),
                    scale: Vec3::ONE.into(),
                },
            ))
            .id();
        for _ in 0..60 {
            app.update();
        }
        let now = app.world().get::<ClothSim>(entity).unwrap().positions[0];
        // ⚠️ Against the grid coordinate vertex 0 is *built* at, not against a
        // reading taken after the first frame. An earlier version of this test
        // sampled `positions[0]` once the app had stepped, which is already
        // after the push-out would have happened -- so it compared a moved
        // vertex against itself and a version that shoved pinned vertices
        // around passed it.
        assert!(
            now.length() < 1.0e-5,
            "a pinned vertex inside a collider must stay at the grid origin it \
             was built at, got {now:?}"
        );
    }

    #[test]
    fn a_cloth_with_no_edges_is_refused_once_rather_than_drawn() {
        let mut app = test_app();
        let entity = spawn(
            &mut app,
            Cloth {
                columns: 1,
                ..curtain()
            },
            Quat::IDENTITY,
        );
        app.update();
        app.update();
        assert!(
            app.world().get::<MeshRenderer>(entity).is_none(),
            "a cloth that cannot be simulated must not be given geometry"
        );
        assert!(
            app.world().get::<ClothRejected>(entity).is_some(),
            "and must be marked, so the warning is not repeated every frame"
        );
    }
}
