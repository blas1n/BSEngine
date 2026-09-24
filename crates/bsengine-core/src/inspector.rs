use bevy_ecs::prelude::Resource;
use bevy_reflect::Reflect;

/// Canonical set of recognized primitive-mesh kind strings, lowercase. This
/// is the string format used by `InspectorEntityInfo.primitive` and
/// `InspectorCmd::AttachPrimitiveMesh.primitive` below (the Inspector no
/// longer has a dedicated Mesh dropdown -- `PrimitiveMesh` is attached like
/// any other component, through the generic Add Component menu and
/// Reflected Fields list). `bsengine-editor`'s `primitive_to_str`/
/// `str_to_primitive` (the only place these strings convert to/from the
/// real `bsengine_scene::Primitive` enum) must stay in sync with this list
/// — see the doc comments there, and the round-trip test that enforces it.
pub const PRIMITIVE_KINDS: [&str; 5] = ["cube", "sphere", "plane", "capsule", "cylinder"];

/// Flattened, read-only snapshot of one ECS entity's editor-relevant
/// component data, rebuilt each frame for the Inspector/Hierarchy panels.
#[derive(Clone, Default)]
pub struct InspectorEntityInfo {
    /// Stable numeric identifier for this entity, as shown/referenced in the editor UI.
    pub id: u64,
    /// The entity's `Name` component value, if any.
    pub name: Option<String>,
    /// World-space position from `Transform`, if the entity has one.
    pub position: Option<[f32; 3]>,
    /// Euler rotation in degrees from `Transform`, if the entity has one.
    pub rotation: Option<[f32; 3]>,
    /// Scale from `Transform`, if the entity has one.
    pub scale: Option<[f32; 3]>,
    // light
    /// Kind of light attached ("point", "spot", "directional"), if any.
    pub light_type: Option<String>,
    /// Light color, if a light component is attached.
    pub light_color: Option<[f32; 3]>,
    /// Light intensity, if a light component is attached.
    pub light_intensity: Option<f32>,
    /// Light falloff range, if a point/spot light is attached.
    pub light_range: Option<f32>,
    /// Spot light inner cone angle in degrees, if a spot light is attached.
    pub spot_inner_angle: Option<f32>,
    /// Spot light outer cone angle in degrees, if a spot light is attached.
    pub spot_outer_angle: Option<f32>,
    // camera
    /// Vertical field of view in degrees, if a `Camera` is attached.
    pub camera_fov: Option<f32>,
    // material
    /// Base (albedo) color, if a `Material` is attached.
    pub material_base_color: Option<[f32; 3]>,
    /// Metallic factor, if a `Material` is attached.
    pub material_metallic: Option<f32>,
    /// Surface opacity, if a `Material` is attached. 1.0 is solid.
    pub material_opacity: Option<f32>,
    /// Roughness factor, if a `Material` is attached.
    pub material_roughness: Option<f32>,
    /// Emissive color, if a `Material` is attached.
    pub material_emissive: Option<[f32; 3]>,
    // hierarchy / tags / script / mesh
    /// Id of this entity's parent, if it has one.
    pub parent_id: Option<u64>,
    /// User-assigned tags on this entity.
    pub tags: Vec<String>,
    /// Path of the attached script asset, if any.
    pub script_path: Option<String>,
    /// Base color texture path, if a `TexturePath` is attached.
    pub texture_path: Option<String>,
    /// One of [`PRIMITIVE_KINDS`], or `None` if no `PrimitiveMesh` is
    /// attached. A plain `String`, not `bsengine_scene::Primitive`, because
    /// `bsengine-core` cannot depend on `bsengine-scene` (that crate already
    /// depends on `bsengine-core`). Mirrors this same struct's
    /// `light_type: Option<String>` convention.
    pub primitive: Option<String>,
    /// Whether the entity is currently visible in the scene.
    pub visible: bool,
    /// Whether the entity is currently selected in the editor.
    pub selected: bool,
    /// Whether this entity is a prefab instance root (has `PrefabInstance`).
    /// Drives the Hierarchy panel's "Apply to Prefab" context-menu entry.
    pub is_prefab_instance: bool,
    /// The entity's `ParticleEmitter`, as the Particles panel shows it, or
    /// `None` when it has none. Filled in by `bsengine-editor` after the
    /// rest of this snapshot, from the live emitter.
    pub particles: Option<ParticleSnapshot>,
}

/// What the Particles panel shows for one emitter: enough to tell a live
/// effect from a dead one and to reach the buttons that act on it. The
/// parameters themselves are edited in the Inspector, through reflection,
/// like any other component's.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ParticleSnapshot {
    /// Particles currently alive.
    pub alive: usize,
    /// Continuous emission rate, per second; zero for a burst-only effect.
    pub rate: f32,
    /// How many particles one burst emits.
    pub burst_count: u32,
    /// Whether continuous emission is on.
    pub enabled: bool,
}

/// How the editor previews particles while the game is stopped -- Unity's
/// Particle Effect overlay in the Scene view, Godot's editor-time emission.
///
/// Read by `ParticlePlugin`'s tick, and only in editor mode with the game
/// stopped: a running game is not a preview, and its particles play at the
/// game's own pace whatever this says. A plain `InspectorState` field
/// rather than an `InspectorCmd`, like `timeline_preview`, because it is
/// continuous per-frame state, not a one-shot action.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticlePreview {
    /// Freeze every emitter: no emission, no ageing, no motion.
    pub paused: bool,
    /// Multiplier on the preview's time step. `1.0` is real time; below it
    /// is the slow motion an effect is tuned in.
    pub speed: f32,
}

impl Default for ParticlePreview {
    fn default() -> Self {
        Self {
            paused: false,
            speed: 1.0,
        }
    }
}

/// One queued edit request from the editor UI, drained and applied to the
/// live ECS world by `apply_inspector_cmds` (`bsengine-editor`).
pub enum InspectorCmd {
    /// Set an entity's world-space position.
    SetPosition {
        /// Target entity id.
        id: u64,
        /// New X position.
        x: f32,
        /// New Y position.
        y: f32,
        /// New Z position.
        z: f32,
    },
    /// Set an entity's Euler rotation, in degrees.
    SetRotation {
        /// Target entity id.
        id: u64,
        /// New rotation about X, in degrees.
        rx: f32,
        /// New rotation about Y, in degrees.
        ry: f32,
        /// New rotation about Z, in degrees.
        rz: f32,
    },
    /// Set an entity's scale.
    SetScale {
        /// Target entity id.
        id: u64,
        /// New scale along X.
        sx: f32,
        /// New scale along Y.
        sy: f32,
        /// New scale along Z.
        sz: f32,
    },
    /// Spawn a new empty entity with the given name.
    SpawnEntity {
        /// Name to give the new entity.
        name: String,
    },
    /// Despawn an entity and its children.
    Despawn {
        /// Target entity id.
        id: u64,
    },
    /// Toggle whether an entity is rendered.
    SetVisible {
        /// Target entity id.
        id: u64,
        /// New visibility state.
        visible: bool,
    },
    /// Attach a default point light to an entity.
    AddPointLight {
        /// Target entity id.
        id: u64,
    },
    /// Attach a default camera to an entity.
    AddCamera {
        /// Target entity id.
        id: u64,
    },
    /// Replace the current editor selection.
    SetSelection {
        /// Ids of the entities to select.
        ids: Vec<u64>,
    },
    /// Clone an entity (and its component data) into a new entity.
    Duplicate {
        /// Id of the entity to duplicate.
        id: u64,
    },
    /// Save the current scene to its existing path.
    SaveScene,
    /// Reloads the current scene from `InspectorState.current_scene_path`,
    /// discarding all runtime changes (physics, script-mutated positions,
    /// ...) and respawning from the RON file's authored state — the
    /// Unity/Unreal-style "Stop resets the scene" behavior, triggered here
    /// by pressing Play again rather than by Stop itself (see the toolbar's
    /// play/stop button in bsengine-rhi-wgpu).
    ReloadScene,
    /// Replace the current scene by loading and parsing a `.ron` file at
    /// `path`. Mirrors `EditorCommand::LoadScene`, which already does the
    /// actual file read/parse/spawn — this variant only exists so UI code
    /// (the Asset Browser) can request it through the same `InspectorCmd`
    /// pipeline every other UI-driven command goes through.
    LoadScene {
        /// Path to the `.ron` scene file to load.
        path: String,
    },
    /// Record import settings in the sidecar beside an asset -- the
    /// Inspector's **Apply** on a selected texture or model. Applied by
    /// `bsengine-editor` through `bsengine_asset::identity::write_import_settings`,
    /// the same function the MCP `asset_import_settings` tool uses, so the
    /// two cannot disagree about minting or keeping the asset's identity.
    WriteImportSettings {
        /// The asset's path, as the Asset Browser hands it out.
        path: String,
        /// What to record. Must be the asset's own kind; the writer refuses
        /// the other and the error lands in `InspectorState::asset_import_error`.
        settings: crate::ImportSettings,
    },
    /// Queue one burst on an entity's `ParticleEmitter` -- the Particles
    /// panel's **Burst**, the same thing `Bsengine.particles.burst` does
    /// from a script.
    ParticleBurst {
        /// The emitter's entity id.
        id: u64,
    },
    /// Replay an effect from its start -- the Particles panel's **Restart**,
    /// Unity's and Godot's of the same name: every live particle is
    /// dropped, the fractional spawn carry is cleared, and a burst-only
    /// effect (rate zero) gets its burst again, since its "start" *is* the
    /// burst and a restart that left it empty would look like a delete.
    ParticleRestart {
        /// One emitter's entity id, or `None` for every emitter in the scene.
        id: Option<u64>,
    },
    /// Spawn a new named entity with a `GltfAsset { path }` component
    /// attached, so `bsengine-gltf`'s existing `load_gltf_assets` system
    /// (already registered in the editor app, already tested) picks it up
    /// and asynchronously replaces it with the loaded mesh's
    /// `MeshRenderer`/`Material`. Always spawns as a root entity.
    SpawnMeshAsset {
        /// Name to give the new entity.
        name: String,
        /// Path to the glTF asset to load.
        path: String,
    },
    /// Attach a reflected component to an entity by its type path.
    AttachComponentByType {
        /// Target entity id.
        id: u64,
        /// Fully qualified reflected type path of the component to attach.
        type_path: String,
    },
    /// Remove a reflected component from an entity by its type path.
    RemoveComponentByType {
        /// Target entity id.
        id: u64,
        /// Fully qualified reflected type path of the component to remove.
        type_path: String,
    },
    /// Rename an entity's `Name` component.
    RenameEntity {
        /// Target entity id.
        id: u64,
        /// New name.
        name: String,
    },
    /// Reparent an entity under another entity.
    SetParent {
        /// Target entity id.
        id: u64,
        /// Id of the new parent entity.
        parent_id: u64,
    },
    /// Remove an entity's parent, making it a root entity.
    RemoveParent {
        /// Target entity id.
        id: u64,
    },
    /// Add a tag to an entity.
    TagEntity {
        /// Target entity id.
        id: u64,
        /// Tag to add.
        tag: String,
    },
    /// Remove a tag from an entity.
    UntagEntity {
        /// Target entity id.
        id: u64,
        /// Tag to remove.
        tag: String,
    },
    /// Attach a script asset to an entity.
    AttachScript {
        /// Target entity id.
        id: u64,
        /// Path to the script asset.
        path: String,
    },
    /// Remove the attached script asset from an entity.
    DetachScript {
        /// Target entity id.
        id: u64,
    },
    /// Attach a primitive mesh to an entity.
    AttachPrimitiveMesh {
        /// Target entity id.
        id: u64,
        /// One of [`PRIMITIVE_KINDS`] — a plain `String`, not
        /// `bsengine_scene::Primitive`, for the same circular-dependency
        /// reason as `InspectorEntityInfo.primitive`. Parsed back into the
        /// real enum in `apply_inspector_cmds` (`bsengine-editor`), where
        /// `bsengine_scene` is already in scope.
        primitive: String,
    },
    /// Remove the attached primitive mesh from an entity.
    DetachPrimitiveMesh {
        /// Target entity id.
        id: u64,
    },
    /// Apply an edited clone of a reflected component's value back onto the
    /// real ECS component. `value` was originally cloned out by
    /// `populate_reflected_component_snapshot`, edited in place in the
    /// Inspector via `draw_reflect_ui`, then re-cloned here for the trip
    /// back through the command queue. Routed through the same
    /// `ReflectCommandQueueResource` `AttachComponentByType`/
    /// `RemoveComponentByType` already use, not the plain `EditorCommand`
    /// queue — see `apply_inspector_cmds`.
    ApplyReflectedComponent {
        /// Target entity id.
        id: u64,
        /// Fully qualified reflected type path of the component being updated.
        type_path: String,
        /// Edited component value to write back.
        value: Box<dyn bevy_reflect::Reflect>,
    },
    /// Instantiate a prefab at the given world position, optionally
    /// parented under an existing entity (by editor entity id). Routed
    /// through the dedicated `PrefabCommandQueueResource`/
    /// `process_prefab_commands` pipeline (`bsengine-editor`), not the
    /// plain `EditorCommand` queue, since `bsengine_scene::instantiate_prefab`
    /// needs `&mut World` -- see `PrefabInstantiateCommand`.
    InstantiatePrefab {
        /// Project-relative path to the prefab file.
        path: String,
        /// Explicit root name; `None` auto-generates one.
        name: Option<String>,
        /// World-space X position for the instantiated root.
        x: f32,
        /// World-space Y position for the instantiated root.
        y: f32,
        /// World-space Z position for the instantiated root.
        z: f32,
        /// Id of the entity to parent the instantiated root under, if any.
        parent_id: Option<u64>,
    },
    /// Extract an entity and its descendants from the live scene into a
    /// new `assets/prefabs/<name>.ron` file, via `save_entities_as_prefab`
    /// (`bsengine-editor`). Unlike `InstantiatePrefab`, this never needs
    /// `&mut World` -- it only reads the already-tracked entity snapshot
    /// and writes a file -- so it's handled directly inside
    /// `apply_inspector_cmds`, with no separate command queue/exclusive
    /// system needed.
    CreatePrefab {
        /// Root entity id; itself and all descendants are saved.
        entity_id: u64,
        /// Prefab file name, no extension or directory.
        name: String,
    },
    /// Push this prefab instance's field-level overrides back into its
    /// source file. Routed through `PrefabApplyCommandQueueResource` /
    /// `process_prefab_apply_commands` (`bsengine-editor`), the same queue
    /// the `apply_to_prefab` MCP tool already uses -- see
    /// `bsengine_editor::prefab_merge::apply_instance_to_prefab`'s doc
    /// comment for the actual logic.
    ApplyToPrefab {
        /// The prefab instance root entity id (must have a `PrefabInstance`).
        entity_id: u64,
    },
    /// Spawn a new terrain entity from a heightmap asset. UI-originated
    /// counterpart to the `terrain_write` MCP tool, which pushes
    /// `EditorCommand::SpawnTerrain` directly since it has no `InspectorCmd`
    /// indirection to go through. `apply_inspector_cmds` (`bsengine-editor`)
    /// forwards this straight onto the plain `EditorCommand` queue, same as
    /// `SpawnEntity` -- see `EditorCommand::SpawnTerrain`'s doc comment for
    /// what happens once it lands there.
    SpawnTerrain {
        /// Path to the heightmap asset to load.
        heightmap_path: String,
        /// Number of chunks along (x, z).
        chunk_count: (u32, u32),
        /// World-space size of one chunk along each axis.
        chunk_size: f32,
        /// Multiplier applied to the normalized heightmap sample.
        height_scale: f32,
        /// Diffuse texture for the low/flat splat layer (e.g. grass).
        layer0_texture_path: String,
        /// Diffuse texture for the steep-slope splat layer (e.g. rock).
        layer1_texture_path: String,
        /// Diffuse texture for the paint-only splat layer (e.g. dirt).
        layer2_texture_path: String,
        /// Diffuse texture for the high-altitude splat layer (e.g. snow).
        layer3_texture_path: String,
        /// Path to a splatmap image, or `None` to keep procedural splat
        /// generation. Mirrors `Terrain::splatmap_path`.
        splatmap_path: Option<String>,
    },
}

/// Whether the editor is showing the static scene or running gameplay.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum EditorPlayState {
    /// Gameplay systems are not running; the scene is being edited.
    #[default]
    Stopped,
    /// Gameplay systems are running as they would at runtime.
    Playing,
}

/// Which viewport manipulation gizmo is active for the selected entity.
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub enum GizmoMode {
    /// Drag handles move the entity along an axis.
    #[default]
    Translate,
    /// Drag rings rotate the entity about an axis.
    Rotate,
    /// Drag handles scale the entity along an axis, or drag the center
    /// handle to scale all three axes proportionally.
    Scale,
}

/// Which terrain-editing behavior a drag applies, and its parameters.
/// Orthogonal to `GizmoMode` -- the terrain brush is a distinct tool, not
/// a 4th gizmo mode, since it only makes sense while a `Terrain` entity is
/// selected and mutates terrain data rather than a transform.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub enum TerrainBrushKind {
    /// Raise (if `raise` is true) or lower the heightmap under the brush.
    Height {
        /// Whether the brush raises (`true`) or lowers (`false`) the terrain.
        raise: bool,
    },
    /// Paint the given splat layer (0-3) under the brush.
    Paint {
        /// Splat layer index (0-3) to paint.
        layer: u8,
    },
}

/// Settings for the terrain brush tool, active whenever
/// `InspectorState::terrain_brush_active` is true.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct TerrainBrushSettings {
    /// Which terrain-editing behavior is active.
    pub kind: TerrainBrushKind,
    /// World-space radius of the brush.
    pub radius: f32,
    /// 0-1 strength applied per frame the brush is held over a point.
    pub strength: f32,
}

impl Default for TerrainBrushSettings {
    fn default() -> Self {
        Self {
            kind: TerrainBrushKind::Height { raise: true },
            radius: 2.0,
            strength: 0.5,
        }
    }
}

/// One frame's worth of "the brush is being applied here" intent, written
/// by the viewport panel every frame a brush drag is active and read by
/// `bsengine-app`'s terrain-brush-apply system (the only place with the
/// `PhysicsWorld`/`GpuMeshRegistry`/`GpuTextureRegistry` access needed to
/// actually mutate anything) -- the same "UI writes a blackboard field, a
/// system elsewhere reads it" pattern `editor_view_proj` already uses.
#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct TerrainBrushStroke {
    /// The `Terrain` entity being edited, by id (matches
    /// `InspectorEntityInfo::id`/`InspectorCmd`'s existing `u64` entity-id
    /// convention, not a raw `bevy_ecs::Entity`, since `InspectorState`
    /// crosses the same crate boundary those already cross).
    pub terrain_entity_id: u64,
    /// World-space point the brush is centered on this frame (the picked
    /// point on the terrain surface).
    pub world_pos: [f32; 3],
}

/// The camera pose the Timeline panel wants previewed this frame.
///
/// Carried as a quaternion rather than the euler degrees
/// [`InspectorEntityInfo`] uses, because the value usually comes straight
/// from `crate::timeline::CameraPose::rotation()` and a round trip through
/// euler angles would be a lossy step with nothing to gain.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewCamera {
    /// World-space camera position.
    pub position: [f32; 3],
    /// World-space camera orientation, as `[x, y, z, w]`.
    pub rotation: [f32; 4],
    /// Vertical field of view in degrees, when the previewed shot names one.
    /// `None` leaves the editor camera's own field of view alone.
    pub fov_y_degrees: Option<f32>,
}

/// What the Timeline panel wants previewed this frame, or absent when the
/// panel's **Preview** toggle is off.
///
/// A plain [`InspectorState`] field rather than an [`InspectorCmd`] because
/// this is continuous per-frame state, not a one-shot undoable action -- the
/// same reasoning that put the terrain brush's drag deltas here. The panel
/// republishes it every frame it is previewing, so a stale request cannot
/// outlive the panel that made it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimelinePreview {
    /// Where the cutscene camera is at the playhead, if the timeline has a
    /// camera or shot track. Applied by overriding the editor's
    /// view-projection, so the scene's own camera entity is never touched.
    pub camera: Option<PreviewCamera>,
    /// `(entity name, clip name, time within the clip)` for every animation
    /// track whose most recent key is at or before the playhead.
    pub clips: Vec<(String, String, f32)>,
}

/// The import settings of the asset selected in the Asset Browser, as the
/// Inspector shows and edits them.
///
/// Populated by `bsengine-editor` (which can read the sidecar) whenever
/// [`InspectorState::selected_asset`] names a different asset than this
/// snapshot does, and again after a write lands; the panel only draws it.
/// `edit` is the Inspector's working copy -- Unity's importer model, where
/// changes sit in the panel until **Apply** writes them or **Revert** drops
/// them, rather than a write per checkbox click that would re-import the
/// asset four times for one four-field edit.
#[derive(Clone, Debug, PartialEq)]
pub struct AssetImportSnapshot {
    /// The asset this describes, exactly as `selected_asset` names it.
    pub path: String,
    /// What is in force for the asset: recorded settings or the defaults.
    pub settings: crate::ImportSettings,
    /// Whether `settings` came from the sidecar rather than being the
    /// kind's defaults. Shown so an author can tell "tuned to the defaults"
    /// from "never tuned".
    pub recorded: bool,
    /// The Inspector's working copy; differs from `settings` while an edit
    /// is pending.
    pub edit: crate::ImportSettings,
}

/// What references the asset selected in the Asset Browser, and what it
/// references -- Unreal's Reference Viewer and Godot's View Owners, as the
/// Inspector lists them.
///
/// Populated by `bsengine-editor`, which walks the project the way the
/// packager does (`bsengine_asset::cook`), whenever
/// [`InspectorState::selected_asset`] names an asset this snapshot does not
/// describe; the panel only draws it. Every click re-reads, because the
/// walk is the only thing that can notice a scene saved since the last one.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssetReferencesSnapshot {
    /// The asset this describes, exactly as `selected_asset` names it.
    pub path: String,
    /// Files that name this asset, project-relative and sorted;
    /// `"project.toml"` when the manifest does (the entry scene, or an
    /// `extra_assets` entry).
    pub referencers: Vec<String>,
    /// Assets this file names, sorted. Empty for a leaf such as a texture.
    pub dependencies: Vec<String>,
    /// Whether the walk from the entry scene reaches this asset at all.
    /// `false` is Godot's orphan resource: nothing names it, and a packaged
    /// build leaves it out.
    pub reached: bool,
    /// Why the walk could not run -- no `project.toml`, or one with no entry
    /// scene -- shown in place of the lists. Recorded in the snapshot rather
    /// than left as "nothing yet", so a project without a manifest is not
    /// walked again every frame.
    pub error: Option<String>,
}

/// Editor-side resource holding the current entity snapshot, selection,
/// pending edit commands, and all viewport/gizmo/camera UI state.
#[derive(Resource)]
pub struct InspectorState {
    /// Snapshot of every entity visible to the editor this frame.
    pub entities: Vec<InspectorEntityInfo>,
    /// Id of the currently selected entity, if any.
    pub selected_id: Option<u64>,
    /// Path of the asset selected in the Asset Browser, if any. One
    /// selection at a time, as in Unity's Project/Hierarchy pair: selecting
    /// an asset clears the entity selection ([`Self::select_asset`]) and
    /// selecting an entity clears this ([`Self::sync_selection`]), so the
    /// Inspector always knows which one it is showing.
    pub selected_asset: Option<String>,
    /// The selected asset's import settings, for the Inspector; see
    /// [`AssetImportSnapshot`]. `None` until `bsengine-editor` has read
    /// them, or when the asset has none.
    pub asset_import: Option<AssetImportSnapshot>,
    /// Why `asset_import` could not be read or written, for the Inspector
    /// to show instead of the fields: a broken sidecar, a write that failed.
    pub asset_import_error: Option<String>,
    /// What references the selected asset and what it references; see
    /// [`AssetReferencesSnapshot`]. `None` until `bsengine-editor` has
    /// walked the project for it.
    pub asset_references: Option<AssetReferencesSnapshot>,
    /// Edit commands queued by the UI this frame, drained by `apply_inspector_cmds`.
    pub cmd_queue: Vec<InspectorCmd>,
    /// Cloned reflected components currently attached to `selected_id`,
    /// repopulated every frame by `populate_reflected_component_snapshot`
    /// (bsengine-editor). The Inspector's "Reflected Fields" section edits
    /// these clones in place via `draw_reflect_ui`; on change, an edited
    /// clone is pushed back as `InspectorCmd::ApplyReflectedComponent`. Each
    /// entry is `(type_path, cloned value)` — `type_path` matches the
    /// format already used by `AttachComponentByType`/`RemoveComponentByType`
    /// (e.g. `"bsengine_core::camera::Camera"`).
    pub reflected_components: Vec<(String, Box<dyn bevy_reflect::Reflect>)>,
    /// World-space position of the selected entity, synced from it on
    /// selection change and read/written by the viewport's translate-gizmo
    /// drag handling (see `gizmo_drag_axis` below).
    pub edit_pos: [f32; 3],
    /// Euler rotation, in degrees, of the selected entity, synced from it on
    /// selection change and read/written by the viewport's rotate-gizmo
    /// drag handling (see `gizmo_rotate_axis` below).
    pub edit_rot: [f32; 3],
    /// Scale of the selected entity, synced from it on selection change and
    /// read/written by the viewport's scale-gizmo drag handling (see
    /// `gizmo_scale_axis` below). Defaults to `[1.0; 3]`, not `[0.0; 3]` --
    /// zero scale is a degenerate transform.
    pub edit_scale: [f32; 3],
    /// Live text in the Hierarchy panel's search box. Empty means "show the
    /// full tree"; non-empty switches Hierarchy to a flat, name-filtered
    /// list (see `HierarchyPanel::matches_search`).
    pub hierarchy_search: String,
    /// Whether the viewport draws the ground-plane reference grid. Toggled
    /// by the viewport overlay's grid button.
    pub show_grid: bool,
    /// Editable visibility buffer for the selected entity.
    pub edit_visible: bool,
    prev_selected_id: Option<u64>,

    // Editor mode toggle and play state
    /// Whether the app is running as the editor (vs. a plain game runtime).
    pub editor_mode: bool,
    /// Whether gameplay systems are currently running.
    pub play_state: EditorPlayState,

    // Editor orbit camera parameters
    /// World-space point the orbit camera looks at.
    pub cam_target: [f32; 3],
    /// Distance from the orbit camera to `cam_target`.
    pub cam_distance: f32,
    /// Orbit camera yaw, in radians.
    pub cam_yaw: f32,
    /// Orbit camera pitch, in radians.
    pub cam_pitch: f32,

    // Set by the egui viewport panel each frame; read by the camera system
    /// Whether the mouse cursor is currently over the viewport panel.
    pub viewport_contains_cursor: bool,
    /// Current size of the viewport panel, in logical pixels.
    pub viewport_size: [f32; 2],
    /// Top-left corner of the viewport panel in window screen space, set by
    /// the egui viewport panel each frame. Read by the HUD text overlay so
    /// `Bsengine.setHudText` positions relative to the actual rendered game
    /// view instead of the whole editor window (which would put it under
    /// the toolbar/other dock panels in editor mode).
    pub viewport_pos: [f32; 2],

    // Override view_proj computed by EditorPlugin from orbit state; read by RenderPlugin
    /// Editor-computed view-projection matrix override for the renderer, when in editor mode.
    pub editor_view_proj: Option<[[f32; 4]; 4]>,
    /// Editor orbit camera's projection matrix.
    pub editor_proj: [[f32; 4]; 4],
    /// Editor orbit camera's world-space position.
    pub editor_cam_pos: [f32; 3],

    // Which viewport gizmo is active for the selected entity.
    /// Which viewport gizmo (translate/rotate) is currently active.
    pub gizmo_mode: GizmoMode,

    /// What the Timeline panel wants previewed this frame; `None` when no
    /// preview is active. Written by the panel, consumed by
    /// `bsengine_editor`'s `update_editor_camera` (the camera half) and
    /// `apply_timeline_preview_animation` (the animation half).
    pub timeline_preview: Option<TimelinePreview>,
    /// How particles are previewed while the game is stopped; see
    /// [`ParticlePreview`]. Written by the Particles panel, read by
    /// `ParticlePlugin`'s tick.
    pub particle_preview: ParticlePreview,

    // Terrain brush tool state. See `TerrainBrushKind`/`TerrainBrushSettings`/
    // `TerrainBrushStroke` above for the full picture.
    /// Whether the terrain brush tool is the active viewport tool. When
    /// true, a drag over a selected `Terrain`'s surface paints instead of
    /// manipulating the gizmo.
    pub terrain_brush_active: bool,
    /// Current brush parameters, editable via the brush settings popup.
    pub terrain_brush_settings: TerrainBrushSettings,
    /// This frame's picked point on a `Terrain`'s surface under the mouse,
    /// written by `bsengine-app`'s picking system every frame, read by the
    /// viewport panel to draw a brush cursor and to know where a drag
    /// should paint. `None` when the mouse isn't over any terrain.
    pub terrain_pick: Option<(u64, [f32; 3])>,
    /// Set by the viewport panel every frame a brush drag is in progress;
    /// cleared (`None`) the frame after a drag stops. `bsengine-app`'s
    /// terrain-brush-apply system treats a stroke being present as "keep
    /// live-previewing," and its *absence* immediately after having been
    /// present as "commit: rebuild the collider and persist to disk."
    pub terrain_brush_stroke: Option<TerrainBrushStroke>,

    // Translate-gizmo drag state (viewport panel). `gizmo_drag_axis` is
    // 0=X, 1=Y, 2=Z while a handle is being dragged.
    /// Axis (0=X, 1=Y, 2=Z) of the translate-gizmo handle currently being dragged, if any.
    pub gizmo_drag_axis: Option<u8>,
    /// World-space position of the selected entity when the current translate drag began.
    pub gizmo_drag_start_world: [f32; 3],
    /// Screen-space mouse position when the current translate drag began.
    pub gizmo_drag_start_mouse: [f32; 2],

    // Rotate-gizmo drag state. `gizmo_rotate_axis` is 0=X, 1=Y, 2=Z (world
    // axis) while a ring is being dragged; the angle fields are radians of
    // the mouse's angle around the gizmo's screen-space center.
    /// World axis (0=X, 1=Y, 2=Z) of the rotate-gizmo ring currently being dragged, if any.
    pub gizmo_rotate_axis: Option<u8>,
    /// Entity's Euler rotation, in degrees, when the current rotate drag began.
    pub gizmo_rotate_start_deg: [f32; 3],
    /// Mouse angle, in radians, around the gizmo's screen-space center when the current rotate drag began.
    pub gizmo_rotate_start_angle: f32,

    // Scale-gizmo drag state. Exactly one of `gizmo_scale_axis`/
    // `gizmo_scale_uniform` is active at a time: a per-axis handle drag
    // adds a delta to just that axis of `gizmo_scale_start_world`; the
    // center uniform handle multiplies all three axes by the same factor
    // (see `panels/viewport.rs` for why axis drags are additive but the
    // uniform drag is multiplicative).
    /// Axis (0=X, 1=Y, 2=Z) of the scale-gizmo handle currently being dragged, if any.
    pub gizmo_scale_axis: Option<u8>,
    /// Whether the center uniform-scale handle is currently being dragged.
    pub gizmo_scale_uniform: bool,
    /// Entity's scale when the current scale drag began.
    pub gizmo_scale_start_world: [f32; 3],
    /// Screen-space mouse position when the current scale drag began.
    pub gizmo_scale_start_mouse: [f32; 2],

    // Set by the toolbar/keyboard to request an undo/redo; consumed and
    // cleared by EditorPlugin's history system the same frame.
    /// Set to request an undo of the last edit; cleared once processed.
    pub request_undo: bool,
    /// Set to request a redo of the last undone edit; cleared once processed.
    pub request_redo: bool,

    // Path the scene was loaded from / last saved to, used by Ctrl+S / the
    // Save toolbar button to save in place without prompting for a path.
    /// Path the current scene was loaded from / last saved to.
    pub current_scene_path: Option<String>,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            entities: Vec::new(),
            selected_id: None,
            selected_asset: None,
            asset_import: None,
            asset_import_error: None,
            asset_references: None,
            cmd_queue: Vec::new(),
            reflected_components: Vec::new(),
            edit_pos: [0.0; 3],
            edit_rot: [0.0; 3],
            edit_scale: [1.0; 3],
            hierarchy_search: String::new(),
            show_grid: true,
            edit_visible: true,
            prev_selected_id: None,
            editor_mode: false,
            play_state: EditorPlayState::Stopped,
            cam_target: [0.0; 3],
            cam_distance: 10.0,
            cam_yaw: 0.5,
            cam_pitch: 0.4,
            viewport_contains_cursor: false,
            viewport_size: [1280.0, 720.0],
            viewport_pos: [0.0, 0.0],
            editor_view_proj: None,
            editor_proj: [[0.0; 4]; 4],
            editor_cam_pos: [0.0; 3],
            gizmo_mode: GizmoMode::Translate,
            timeline_preview: None,
            particle_preview: ParticlePreview::default(),
            terrain_brush_active: false,
            terrain_brush_settings: TerrainBrushSettings::default(),
            terrain_pick: None,
            terrain_brush_stroke: None,
            gizmo_drag_axis: None,
            gizmo_drag_start_world: [0.0; 3],
            gizmo_drag_start_mouse: [0.0; 2],
            gizmo_rotate_axis: None,
            gizmo_rotate_start_deg: [0.0; 3],
            gizmo_rotate_start_angle: 0.0,
            gizmo_scale_axis: None,
            gizmo_scale_uniform: false,
            gizmo_scale_start_world: [1.0; 3],
            gizmo_scale_start_mouse: [0.0; 2],
            request_undo: false,
            request_redo: false,
            current_scene_path: None,
        }
    }
}

impl InspectorState {
    /// Creates an `InspectorState` with `editor_mode` set, otherwise using defaults.
    pub fn editor() -> Self {
        Self {
            editor_mode: true,
            ..Default::default()
        }
    }

    /// Refreshes the `edit_*` buffers from the newly selected entity, if the selection changed.
    ///
    /// A newly selected *entity* also drops the asset selection: the
    /// Inspector shows one thing, and the thing picked last is it. Only a
    /// change *to* an entity does this -- the Hierarchy clearing its own
    /// selection (`None`) must not throw away an asset the author just
    /// clicked in the browser.
    pub fn sync_selection(&mut self) {
        if self.selected_id != self.prev_selected_id {
            self.prev_selected_id = self.selected_id;
            if let Some(id) = self.selected_id {
                self.selected_asset = None;
                self.asset_import = None;
                self.asset_import_error = None;
                self.asset_references = None;
                if let Some(info) = self.entities.iter().find(|e| e.id == id) {
                    self.edit_pos = info.position.unwrap_or([0.0; 3]);
                    self.edit_rot = info.rotation.unwrap_or([0.0; 3]);
                    self.edit_scale = info.scale.unwrap_or([1.0; 3]);
                    self.edit_visible = info.visible;
                }
            }
        }
    }

    /// Selects an asset from the Asset Browser, and deselects the entity:
    /// the other half of the one-selection rule `sync_selection` keeps.
    ///
    /// `prev_selected_id` is moved along with `selected_id`, so the next
    /// `sync_selection` sees no entity change and leaves this selection be;
    /// without that, the frame after a browser click would read `Some ->
    /// None` as no-op but a later `None -> Some(same)` as a fresh pick, which
    /// is right, while a stale `prev` would make the *clearing* itself look
    /// like a change and clear the asset straight back.
    pub fn select_asset(&mut self, path: impl Into<String>) {
        let path = path.into();
        if self.selected_asset.as_deref() != Some(path.as_str()) {
            self.asset_import = None;
        }
        // Cleared even for the same asset: clicking it again is how an
        // author retries after fixing the broken sidecar the error named.
        self.asset_import_error = None;
        // Likewise dropped on every click, same asset or not: the references
        // come from a walk of the scene files, and a scene saved since the
        // last walk is invisible to anything but another walk. Clicking the
        // asset again is the refresh.
        self.asset_references = None;
        self.selected_asset = Some(path);
        self.selected_id = None;
        self.prev_selected_id = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_no_selection() {
        let s = InspectorState::default();
        assert!(s.selected_id.is_none());
        assert!(s.selected_asset.is_none());
        assert!(s.entities.is_empty());
        assert!(s.cmd_queue.is_empty());
        assert!(!s.editor_mode);
        assert_eq!(s.play_state, EditorPlayState::Stopped);
    }

    fn snapshot(path: &str) -> AssetImportSnapshot {
        let settings = crate::ImportSettings::Texture(crate::TextureImportSettings::default());
        AssetImportSnapshot {
            path: path.to_string(),
            settings,
            recorded: false,
            edit: settings,
        }
    }

    /// One selection at a time, in both directions, and the cases where
    /// nothing must be cleared: the Hierarchy deselecting (`Some -> None`)
    /// and re-selecting the same asset (its pending edit survives).
    #[test]
    fn selecting_an_asset_and_selecting_an_entity_each_clear_the_other() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 1,
            ..Default::default()
        });
        s.selected_id = Some(1);
        s.sync_selection();

        s.select_asset("assets/textures/wall.png");
        assert_eq!(s.selected_id, None, "an asset pick deselects the entity");
        assert_eq!(
            s.selected_asset.as_deref(),
            Some("assets/textures/wall.png")
        );

        // The frame after: no entity change to see, so the asset stays.
        s.sync_selection();
        assert_eq!(
            s.selected_asset.as_deref(),
            Some("assets/textures/wall.png")
        );

        // A pending edit on the same asset survives re-clicking it...
        s.asset_import = Some(snapshot("assets/textures/wall.png"));
        s.select_asset("assets/textures/wall.png");
        assert!(s.asset_import.is_some(), "same asset: the snapshot is kept");
        // ...and is dropped for a different one, so the editor re-reads.
        s.select_asset("assets/models/fox.glb");
        assert!(
            s.asset_import.is_none(),
            "another asset: the snapshot is stale"
        );

        // The references are dropped on *every* click, unlike the import
        // snapshot: re-clicking is how an author refreshes them after
        // saving a scene, and there is no pending edit in them to lose.
        s.asset_references = Some(AssetReferencesSnapshot {
            path: "assets/models/fox.glb".to_string(),
            reached: true,
            ..Default::default()
        });
        s.select_asset("assets/models/fox.glb");
        assert!(
            s.asset_references.is_none(),
            "same asset: the references are re-walked"
        );

        s.asset_references = Some(AssetReferencesSnapshot::default());
        s.selected_id = Some(1);
        s.sync_selection();
        assert_eq!(s.selected_asset, None, "an entity pick deselects the asset");
        assert!(s.asset_import.is_none());
        assert!(
            s.asset_references.is_none(),
            "and drops the asset's references with it"
        );

        // The Hierarchy clearing its selection is not a pick of anything.
        s.select_asset("assets/models/fox.glb");
        s.selected_id = None;
        s.sync_selection();
        assert_eq!(
            s.selected_asset.as_deref(),
            Some("assets/models/fox.glb"),
            "Some -> None on the entity side must not clear the asset"
        );
    }

    #[test]
    fn sync_selection_loads_entity_transform() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 1,
            name: Some("Player".into()),
            position: Some([1.0, 2.0, 3.0]),
            rotation: Some([10.0, 20.0, 30.0]),
            scale: Some([2.0, 2.0, 2.0]),
            ..Default::default()
        });
        s.selected_id = Some(1);
        s.sync_selection();
        assert_eq!(s.edit_pos, [1.0, 2.0, 3.0]);
        assert_eq!(s.edit_rot, [10.0, 20.0, 30.0]);
        assert_eq!(s.edit_scale, [2.0, 2.0, 2.0]);
    }

    #[test]
    fn sync_selection_no_reset_when_same_entity() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 1,
            name: None,
            position: Some([5.0, 0.0, 0.0]),
            ..Default::default()
        });
        s.selected_id = Some(1);
        s.sync_selection();
        assert_eq!(s.edit_pos[0], 5.0);
        s.edit_pos = [99.0, 0.0, 0.0];
        s.sync_selection();
        assert_eq!(s.edit_pos[0], 99.0);
    }

    #[test]
    fn sync_selection_uses_defaults_when_no_transform() {
        let mut s = InspectorState::default();
        s.entities.push(InspectorEntityInfo {
            id: 2,
            ..Default::default()
        });
        s.selected_id = Some(2);
        s.sync_selection();
        assert_eq!(s.edit_pos, [0.0; 3]);
        assert_eq!(s.edit_rot, [0.0; 3]);
        assert_eq!(s.edit_scale, [1.0; 3]);
    }

    #[test]
    fn editor_cam_default_distance() {
        let s = InspectorState::default();
        assert!((s.cam_distance - 10.0).abs() < 1e-6);
    }

    #[test]
    fn inspector_state_default_has_terrain_brush_inactive() {
        let insp = InspectorState::default();
        assert!(!insp.terrain_brush_active);
        assert_eq!(insp.terrain_pick, None);
        assert_eq!(insp.terrain_brush_stroke, None);
    }

    #[test]
    fn terrain_brush_settings_default_is_a_raising_height_brush() {
        let s = TerrainBrushSettings::default();
        assert_eq!(s.kind, TerrainBrushKind::Height { raise: true });
        assert!(s.radius > 0.0);
    }
}
