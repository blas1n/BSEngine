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
use bsengine_core::{Time, Transform};
use bsengine_ecs::{Commands, Component, Entity, Query, Res, ResMut, Without};
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
            // Alongside `update_skinned_meshes`, and for the same reason: the
            // sheet should settle after whatever moved the entity this frame,
            // not a frame behind it.
            .add_systems(PostUpdate, simulate_cloth);
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

fn simulate_cloth(
    time: Res<Time>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    queue: Option<Res<GpuQueueResource>>,
    mut query: Query<(&Cloth, &Transform, &mut ClothSim)>,
) {
    let dt = time.delta_seconds;
    for (cloth, transform, mut sim) in query.iter_mut() {
        // Gravity is authored in world space, but the sheet is simulated in the
        // entity's local space (its vertices are what the mesh buffer holds), so
        // the entity's rotation has to come out of it. Without this a curtain
        // rotated flat onto its side falls sideways along its own surface.
        //
        // Scale is deliberately not undone: a non-uniformly scaled cloth
        // simulates in its own space and is stretched on the way to the screen,
        // which is what every other component on the entity does too.
        let local_gravity = transform.rotation.0.inverse() * Vec3::from(cloth.gravity);

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
