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
use bsengine_ecs::{Changed, Commands, Component, Entity, Query, Res, ResMut, With, Without};
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
    /// The `(columns, rows, spacing)` this sheet was actually built from.
    ///
    /// Compared against the component every time it is written, so that editing
    /// the sheet's size rebuilds it while editing its stiffness does not. The
    /// other five parameters are read afresh each step and need no record.
    built_from: (u32, u32, f32),
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
            // Chained so a cloth refused on one frame and fixed on the next is
            // let back in before `generate_cloth` looks, rather than a frame
            // later.
            .add_systems(Update, (regenerate_resized_cloth, generate_cloth).chain())
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
        let mut links = cloth_solver::links_from_indices(&positions, &indices);
        // Built whatever `bending_stiffness` currently is, so that raising it on
        // a live cloth takes effect immediately -- the solver skips bend links
        // while it is zero, which costs a branch rather than a rebuild.
        links.extend(cloth_solver::bend_links(
            &positions,
            cloth.columns,
            cloth.rows,
        ));
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
                built_from: (cloth.columns, cloth.rows, cloth.spacing),
                uploaded: false,
            },
        ));
    }
}

/// Rebuilds a sheet whose size was edited, and lets a refused one back in once
/// its size makes sense.
///
/// Five of a [`Cloth`]'s parameters are read afresh every step, so editing them
/// takes effect on the next frame with nothing to do here. The three that
/// describe geometry cannot: they decide how many vertices there are, and the
/// mesh, the links and the solver's arrays are all built from them once. Before
/// this, changing the size of a cloth in the Inspector did *nothing* -- which is
/// worse than it sounds, because the other five fields do work, so the sheet
/// looks like it is responding right up until it is asked to resize.
///
/// The sheet snaps back to flat when it rebuilds. There is no honest way around
/// that: a different vertex count has no correspondence with the drape it had.
/// Unity, Unreal and Godot all reset a cloth when its mesh changes too.
fn regenerate_resized_cloth(
    mut commands: Commands,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut query: Query<(Entity, &Cloth, Option<&mut ClothSim>), Changed<Cloth>>,
    rejected: Query<(), With<ClothRejected>>,
) {
    for (entity, cloth, sim) in query.iter_mut() {
        let wanted = (cloth.columns, cloth.rows, cloth.spacing);

        let Some(mut sim) = sim else {
            // No sheet yet. Either it is still waiting for a registry, or it was
            // refused -- and a refusal has to be reversible now that the size
            // can be edited: typing "1" on the way to "12" must not leave the
            // cloth dead for the rest of the session.
            //
            // The new size is not checked here. `generate_cloth` runs next and
            // is the one place that decides what can be simulated; re-deciding
            // it here would be the same rule written twice, and each edit
            // earning one warning is the right amount of feedback anyway.
            if rejected.get(entity).is_ok() {
                commands.entity(entity).remove::<ClothRejected>();
            }
            continue;
        };

        if sim.built_from == wanted {
            // The usual case by far: some other field was edited, or the
            // component was written back unchanged.
            continue;
        }

        let Some((vertices, indices)) =
            cloth_solver::grid(cloth.columns, cloth.rows, cloth.spacing)
        else {
            // Keep the sheet that is already there rather than deleting it. An
            // Inspector edit passes through whatever the author has typed so
            // far, and a cloth that vanishes at "1" and never returns is a
            // worse answer than one that waits for the second digit.
            warn!(
                "[cloth] a cloth of {} x {} vertices at spacing {} cannot be simulated; \
                 keeping the {} x {} sheet it already has",
                cloth.columns, cloth.rows, cloth.spacing, sim.built_from.0, sim.built_from.1
            );
            continue;
        };

        let Some(registry) = mesh_registry.as_mut() else {
            continue;
        };

        // `replace`, not `register`: the id is what `MeshRenderer` holds and the
        // registry never frees, so registering again would both leak the old
        // mesh and leave this entity drawing it.
        if !registry.replace(sim.mesh_id, &vertices, &indices) {
            warn!(
                "[cloth] mesh {} vanished from the registry; leaving the sheet as it was",
                sim.mesh_id
            );
            continue;
        }

        let positions: Vec<Vec3> = vertices.iter().map(|v| Vec3::from(v.position)).collect();
        let mut links = cloth_solver::links_from_indices(&positions, &indices);
        links.extend(cloth_solver::bend_links(
            &positions,
            cloth.columns,
            cloth.rows,
        ));
        sim.previous = positions.clone();
        sim.positions = positions;
        sim.links = links;
        sim.vertices = vertices;
        sim.indices = indices;
        sim.built_from = wanted;
    }
}

/// The tunables this frame's step should read, with the one rule the solver
/// cannot check for itself applied.
///
/// ⚠️ `self_collision_distance` is clamped below `spacing`. At or above it every
/// vertex is permanently inside its neighbours and the sheet inflates rather
/// than draping -- Unity documents the same rule for the same setting. Clamped
/// rather than refused, because the sensible thing to do with "too much
/// thickness" is as much as will work, and the warning says so.
fn step_params(cloth: &Cloth) -> cloth_solver::StepParams {
    let limit = cloth.spacing * SELF_COLLISION_LIMIT;
    let mut self_collision_distance = cloth.self_collision_distance;
    if self_collision_distance > limit {
        warn!(
            "[cloth] a self-collision distance of {} is not smaller than the {}              spacing between vertices; using {limit}",
            cloth.self_collision_distance, cloth.spacing
        );
        self_collision_distance = limit;
    }
    cloth_solver::StepParams {
        iterations: cloth.iterations,
        stiffness: cloth.stiffness,
        bending_stiffness: cloth.bending_stiffness,
        damping: cloth.damping,
        self_collision_distance,
    }
}

/// The largest fraction of a sheet's vertex spacing its self-collision distance
/// may take. Short of the full spacing, so a pair at exactly its rest length is
/// never also touching.
const SELF_COLLISION_LIMIT: f32 = 0.9;

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
            &step_params(cloth),
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
    use bevy_ecs::change_detection::DetectChangesMut;
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
            bending_stiffness: 0.0,
            self_collision_distance: 0.0,
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
    /// One headless device and queue for the whole test binary.
    ///
    /// ⚠️ Shared rather than built per test, and that is not tidiness. Each
    /// `request_device` holds a real GPU allocation for the life of the process,
    /// and this crate's suite already builds two per terrain test; going from
    /// 6 cloth tests to 13 was enough to turn the next request into
    /// `RequestDeviceError(OutOfMemory)` on Windows CI -- which fails the
    /// *terrain* tests, since they are the ones that happen to ask last.
    ///
    /// Sharing is safe here because nothing shares state across it: each test
    /// still gets its own `GpuMeshRegistry`, and a registry owns its buffers.
    fn shared_gpu() -> (std::sync::Arc<wgpu::Device>, std::sync::Arc<wgpu::Queue>) {
        static GPU: std::sync::OnceLock<(
            std::sync::Arc<wgpu::Device>,
            std::sync::Arc<wgpu::Queue>,
        )> = std::sync::OnceLock::new();
        GPU.get_or_init(|| {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..Default::default()
            });
            let adapter =
                pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
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
            (std::sync::Arc::new(device), std::sync::Arc::new(queue))
        })
        .clone()
    }

    fn test_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(crate::TimePlugin);
        app.add_plugins(ClothPlugin);

        let (device, queue) = shared_gpu();
        app.insert_resource(GpuMeshRegistry::new(device));
        app.insert_resource(GpuQueueResource(queue));
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
    fn a_stiffer_cloth_drapes_less_sharply_over_an_edge() {
        // ⚠️ The only test that drives `Cloth::bending_stiffness` through the
        // component rather than calling the solver directly, and it exists
        // because two mutations proved it was needed: deleting the bend-link
        // generation in `generate_cloth`, and passing 0 for the authored
        // stiffness, both left the whole suite green.
        //
        // A sheet dropped over a narrow pedestal. Both end up with their centre
        // on it; what bending decides is the corners, which fold straight down
        // over the edge when the fabric is limp and are carried further out when
        // it is not. Measured: -2.157 against -1.901.
        let drape = |bending: f32| {
            let mut app = test_app();
            app.add_plugins(bsengine_physics::PhysicsPlugin);
            app.world_mut().spawn((
                bsengine_core::Transform::default(),
                bsengine_physics::PhysicsInput {
                    position: Vec3::new(CLOTH_AT.x + 0.75, CLOTH_AT.y - 2.0, CLOTH_AT.z + 0.75)
                        .into(),
                    rotation: Quat::IDENTITY.into(),
                },
                bsengine_physics::RigidBody::fixed(),
                bsengine_physics::Collider::cuboid(0.35, 0.5, 0.35),
            ));
            let entity = spawn(
                &mut app,
                Cloth {
                    columns: 6,
                    rows: 6,
                    spacing: 0.3,
                    pinned: Vec::new(),
                    bending_stiffness: bending,
                    ..curtain()
                },
                Quat::IDENTITY,
            );
            for _ in 0..300 {
                app.update();
            }
            let sim = app.world().get::<ClothSim>(entity).unwrap();
            let corners = [0usize, 5, 30, 35];
            (
                corners.iter().map(|&i| sim.positions[i].y).sum::<f32>() / 4.0,
                sim.positions[21].y,
            )
        };

        let (limp_corners, limp_centre) = drape(0.0);
        let (stiff_corners, stiff_centre) = drape(1.0);
        // The fixture only means anything if both sheets actually landed on the
        // pedestal rather than missing it or sliding off.
        for (label, centre) in [("limp", limp_centre), ("stiff", stiff_centre)] {
            assert!(
                (centre - (-1.49)).abs() < 0.2,
                "the {label} sheet should be resting on the pedestal, its centre                  is at {centre}"
            );
        }
        assert!(
            stiff_corners > limp_corners + 0.1,
            "a stiffer sheet's corners must not fold as far over the edge:              stiff {stiff_corners}, limp {limp_corners}"
        );
    }

    /// Lets a sheet fall for a second so it has a drape to lose.
    fn settle(app: &mut bevy_app::App) {
        for _ in 0..60 {
            app.update();
        }
    }

    #[test]
    fn resizing_a_cloth_rebuilds_its_sheet_in_place() {
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();
        let before = app.world().get::<MeshRenderer>(entity).unwrap().mesh_id;
        assert_eq!(
            app.world().get::<ClothSim>(entity).unwrap().positions.len(),
            16
        );

        app.world_mut().get_mut::<Cloth>(entity).unwrap().columns = 7;
        app.update();

        let sim = app.world().get::<ClothSim>(entity).unwrap();
        assert_eq!(sim.positions.len(), 7 * 4, "the sheet has to be rebuilt");
        assert_eq!(sim.vertices.len(), 7 * 4, "and so does its vertex buffer");
        // ⚠️ The same mesh id, reused through `replace`. Registering a second
        // mesh would leak the first -- the registry never frees -- and leave
        // `MeshRenderer` pointing at geometry nothing simulates.
        assert_eq!(
            app.world().get::<MeshRenderer>(entity).unwrap().mesh_id,
            before,
            "the entity must keep drawing the mesh it already had"
        );
        assert!(
            app.world()
                .resource::<GpuMeshRegistry>()
                .get(before)
                .is_some(),
            "and that mesh must still be registered"
        );
        // ⚠️ And the two ids must still agree. Registering a fresh mesh instead
        // of replacing leaves `MeshRenderer` on the old one while the solver
        // uploads into the new -- the entity then draws a sheet that never
        // moves, and the assertions above cannot tell, since each is separately
        // satisfied.
        assert_eq!(
            app.world().get::<ClothSim>(entity).unwrap().mesh_id,
            app.world().get::<MeshRenderer>(entity).unwrap().mesh_id,
            "the simulated mesh and the drawn mesh have to be the same one"
        );
    }

    #[test]
    fn a_cloth_written_every_frame_keeps_simulating() {
        // ⚠️ How the Inspector actually behaves: it writes the whole component
        // back, so `Changed<Cloth>` fires on a cloth nobody is editing. If the
        // rebuild does not record what it built, every one of those writes looks
        // like a resize and the sheet is reset to flat forever -- a curtain that
        // freezes the moment it is selected, and only then.
        // Written back by a system, which is what the Inspector is. It writes
        // the whole component every frame whether or not anything differs, and
        // `set_changed` is that without pretending a value moved.
        fn rewrite_every_cloth(mut cloths: Query<&mut Cloth>) {
            for mut cloth in cloths.iter_mut() {
                cloth.set_changed();
            }
        }

        let mut app = test_app();
        app.add_systems(Update, rewrite_every_cloth.before(regenerate_resized_cloth));
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        // ⚠️ The sheet has to exist *before* the resize. Setting `columns`
        // first means `generate_cloth` simply builds the new size on the first
        // frame and `regenerate_resized_cloth` never runs -- counted, it
        // rebuilt exactly 0 times, and this test passed anyway while measuring
        // nothing it was written to measure.
        app.update();
        app.world_mut().get_mut::<Cloth>(entity).unwrap().columns = 6;
        for _ in 0..60 {
            app.update();
        }

        let sim = app.world().get::<ClothSim>(entity).unwrap();
        assert_eq!(sim.positions.len(), 6 * 4, "the resize still has to happen");
        assert!(
            sim.positions.iter().any(|p| p.y < -0.05),
            "and the sheet has to have kept falling rather than being rebuilt              flat every frame: {:?}",
            sim.positions
        );
    }

    #[test]
    fn editing_a_cloths_other_settings_does_not_reset_its_drape() {
        // ⚠️ The assertion a naive version fails. Rebuilding on any write to
        // `Cloth` is one line shorter and looks correct -- until someone nudges
        // the damping on a settled curtain and it snaps back to a flat plane.
        // The Inspector writes the whole component, so "some field changed" is
        // not the same question as "the geometry changed".
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        settle(&mut app);
        let draped = app
            .world()
            .get::<ClothSim>(entity)
            .unwrap()
            .positions
            .clone();
        assert!(
            draped.iter().any(|p| p.y < -0.05),
            "the fixture needs a sheet that has actually fallen: {draped:?}"
        );

        app.world_mut().get_mut::<Cloth>(entity).unwrap().damping = 0.5;
        app.update();

        let now = &app.world().get::<ClothSim>(entity).unwrap().positions;
        for (i, (a, b)) in now.iter().zip(&draped).enumerate() {
            assert!(
                (*a - *b).length() < 0.05,
                "vertex {i} jumped when an unrelated field was edited: {a:?} \
                 from {b:?}"
            );
        }
    }

    #[test]
    fn a_size_that_cannot_be_simulated_keeps_the_sheet_already_there() {
        // Half-typed values reach the component on their way to the real one.
        // Deleting the sheet at "1" and never bringing it back would make the
        // Inspector unusable for exactly the field this feature is about.
        let mut app = test_app();
        let entity = spawn(&mut app, curtain(), Quat::IDENTITY);
        app.update();

        app.world_mut().get_mut::<Cloth>(entity).unwrap().columns = 1;
        app.update();
        assert_eq!(
            app.world().get::<ClothSim>(entity).unwrap().positions.len(),
            16,
            "the 4x4 sheet must still be there"
        );

        // And the second digit takes.
        app.world_mut().get_mut::<Cloth>(entity).unwrap().columns = 12;
        app.update();
        assert_eq!(
            app.world().get::<ClothSim>(entity).unwrap().positions.len(),
            12 * 4
        );
    }

    #[test]
    fn a_refused_cloth_comes_back_once_its_size_makes_sense() {
        // `ClothRejected` was permanent, which was fine when the size could only
        // be set in a scene file. With it editable, a cloth that spent one frame
        // at an impossible size would have stayed dead for the session.
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
        assert!(app.world().get::<MeshRenderer>(entity).is_none());

        app.world_mut().get_mut::<Cloth>(entity).unwrap().columns = 5;
        app.update();

        assert!(
            app.world().get::<MeshRenderer>(entity).is_some(),
            "a cloth whose size was corrected has to be given its sheet"
        );
        assert_eq!(
            app.world().get::<ClothSim>(entity).unwrap().positions.len(),
            5 * 4
        );
    }

    #[test]
    fn a_self_collision_distance_is_clamped_below_the_vertex_spacing() {
        // ⚠️ At or above the spacing, every vertex is permanently inside its
        // own neighbours and the sheet inflates instead of draping. Unity
        // documents the same rule; the solver cannot check it, because it is
        // handed links and not a grid.
        let over = Cloth {
            spacing: 0.5,
            self_collision_distance: 2.0,
            ..curtain()
        };
        let clamped = step_params(&over).self_collision_distance;
        assert!(
            clamped < over.spacing,
            "must end up under the {} spacing, got {clamped}",
            over.spacing
        );

        // And a distance that already fits is passed through untouched -- a
        // clamp that always clamps would satisfy the assertion above.
        let fits = Cloth {
            spacing: 0.5,
            self_collision_distance: 0.2,
            ..curtain()
        };
        assert_eq!(step_params(&fits).self_collision_distance, 0.2);
        // The rest of the settings travel unchanged.
        assert_eq!(step_params(&fits).stiffness, fits.stiffness);
        assert_eq!(step_params(&fits).iterations, fits.iterations);
    }

    #[test]
    fn a_cloth_hanging_from_its_own_middle_does_not_pass_through_itself() {
        // ⚠️ The only test that drives `self_collision_distance` through the
        // component, and it needs an arrangement where the sheet meets itself.
        // Pinning the middle row does it with no collider at all: everything
        // above the pins and everything below swings down about the same line,
        // so the two halves end up hanging in the same place.
        //
        // An earlier fixture draped the sheet over a narrow rail, which never
        // folded -- collision against the world is frictionless, so the sheet
        // simply slid off. Closest pair came back as exactly the vertex spacing
        // both with self-collision and without, which is the sheet reporting
        // that nothing had folded at all.
        let fold = |distance: f32| {
            let mut app = test_app();
            let entity = spawn(
                &mut app,
                Cloth {
                    columns: 6,
                    rows: 6,
                    spacing: 0.3,
                    // The middle row, and nothing else.
                    pinned: (12..18).collect(),
                    self_collision_distance: distance,
                    ..curtain()
                },
                Quat::IDENTITY,
            );
            for _ in 0..300 {
                app.update();
            }
            let sim = app.world().get::<ClothSim>(entity).unwrap();
            let mut closest = f32::MAX;
            for i in 0..sim.positions.len() {
                for j in i + 1..sim.positions.len() {
                    closest = closest.min((sim.positions[j] - sim.positions[i]).length());
                }
            }
            closest
        };

        let through = fold(0.0);
        let apart = fold(0.2);
        assert!(
            through < 0.15,
            "the fixture only means something if the halves meet without              self-collision: closest pair {through}"
        );
        assert!(
            apart > 0.15,
            "with a 0.2 distance no two vertices may come that close:              closest pair {apart}"
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
