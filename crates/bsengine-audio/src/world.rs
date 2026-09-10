use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::{Quat, Vec3};
use kira::{
    listener::ListenerHandle,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    track::{SpatialTrackBuilder, SpatialTrackHandle, TrackBuilder, TrackHandle},
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Tween,
};

use crate::spatial::{to_mint_quat, to_mint_vec};

/// One live mixer bus.
///
/// `handle` is the only part that needs an audio device. `parent` and
/// `volume_db` are recorded on every machine, so the mixer stays observable
/// where there is no device at all — see
/// [`last_listener_pose`](AudioWorld::last_listener_pose) for why that is not
/// optional.
struct BusState {
    parent: Option<String>,
    volume_db: f32,
    handle: Option<TrackHandle>,
}

/// ECS resource wrapping the `kira` audio manager; `None` if audio backend init failed.
#[derive(Resource)]
pub struct AudioWorld {
    manager: Option<AudioManager<DefaultBackend>>,
    /// The scene's ears. Created lazily on the first listener pose, because a
    /// spatial track can only be built once there is a listener to link it to.
    listener: Option<ListenerHandle>,
    /// One spatial track per emitter entity.
    /// One spatial track per (emitter entity, bus).
    ///
    /// Keyed by the pair rather than by entity because a spatial track is
    /// created *under a particular bus*, and a sound names its bus at the play
    /// call — so an entity playing on two buses needs a track under each. In
    /// practice that is one per entity; the general case costs a wider key.
    emitters: HashMap<(Entity, Option<String>), SpatialTrackHandle>,
    /// Every (entity, bus) pair a track has been requested for, in order.
    ///
    /// Recorded whether or not a backend exists, so the keying is observable
    /// on a machine with no audio device. Without this, a test could only ask
    /// `emitters`, which is empty there, and would pass on nothing.
    emitter_order: Vec<(Entity, Option<String>)>,
    last_listener_pose: Option<(Vec3, Quat)>,
    last_emitter_positions: HashMap<Entity, Vec3>,
    /// Position last pushed to each individual (entity, bus) track.
    ///
    /// Separate from `last_emitter_positions` because `set_emitter_position`
    /// must move *every* one of an entity's tracks, and without a per-track
    /// record that is unobservable on a machine with no audio device: the
    /// track map is empty there, so moving one track or all of them looks
    /// identical. Mutation-testing found exactly that hole.
    last_track_positions: HashMap<(Entity, Option<String>), Vec3>,
    /// Which bus each sound id was routed to, with an unknown name already
    /// resolved to Master. Recorded regardless of backend, for the same reason
    /// `last_listener_pose` is.
    last_sound_buses: HashMap<u32, Option<String>>,
    /// Live mixer buses, in creation order: every bus after its parent.
    ///
    /// A `Vec` rather than a `HashMap` because creation order is load-bearing
    /// (a child sub-track is created on its parent's handle) and because the
    /// count is small enough that a linear lookup is not worth a second index.
    buses: Vec<(String, BusState)>,
}

impl Default for AudioWorld {
    fn default() -> Self {
        match AudioManager::<DefaultBackend>::new(AudioManagerSettings::default()) {
            Ok(manager) => Self {
                manager: Some(manager),
                ..Self::silent()
            },
            Err(e) => {
                tracing::warn!("Audio backend init failed ({e}) — running without audio");
                Self::silent()
            }
        }
    }
}

impl AudioWorld {
    /// An `AudioWorld` with no backend: every play is a no-op, but poses and
    /// emitter positions are still recorded so the rest of the engine behaves
    /// identically with and without an audio device.
    ///
    /// Public because constructing a real `AudioManager` is not always safe.
    /// On Windows with no audio device — CI, and any headless machine — kira
    /// initialises WASAPI/COM on a background thread and creating or dropping
    /// the manager faults the process outright (`STATUS_ACCESS_VIOLATION`),
    /// which is why two tests in this crate are `#[ignore]`d there. A host or
    /// test that wants the positional bookkeeping without that risk can insert
    /// this and [`AudioPlugin`](crate::AudioPlugin) will leave it alone.
    pub fn silent() -> Self {
        Self {
            manager: None,
            listener: None,
            emitters: HashMap::new(),
            emitter_order: Vec::new(),
            last_listener_pose: None,
            last_emitter_positions: HashMap::new(),
            last_track_positions: HashMap::new(),
            last_sound_buses: HashMap::new(),
            buses: Vec::new(),
        }
    }

    /// Rebuilds the mixer from an authored layout, replacing any previous one.
    ///
    /// Buses are created parents-first, which is exactly what
    /// [`BusLayout::creation_order`](crate::bus::BusLayout::creation_order)
    /// guarantees: a child sub-track is created *on* its parent's handle, so
    /// the parent must already exist.
    pub fn apply_bus_layout(&mut self, layout: &crate::bus::BusLayout) {
        for problem in layout.problems() {
            tracing::warn!("{problem}");
        }
        // Dropping the old handles tears down the old tracks.
        self.buses.clear();
        for bus in layout.creation_order() {
            let handle = self.create_bus_track(bus.parent.as_deref(), bus.volume_db);
            self.buses.push((
                bus.name.clone(),
                BusState {
                    parent: bus.parent.clone(),
                    volume_db: bus.volume_db,
                    handle,
                },
            ));
        }
    }

    /// Creates one bus's kira track under `parent`, or under Master when
    /// `parent` is `None`.
    ///
    /// Returns `None` without a backend, which is the ordinary state on a
    /// machine with no audio device rather than an error.
    fn create_bus_track(&mut self, parent: Option<&str>, volume_db: f32) -> Option<TrackHandle> {
        let builder = TrackBuilder::new().volume(Decibels(volume_db));
        match parent {
            None => self.manager.as_mut()?.add_sub_track(builder).ok(),
            Some(name) => {
                let slot = self.buses.iter_mut().find(|(n, _)| n == name)?;
                slot.1.handle.as_mut()?.add_sub_track(builder).ok()
            }
        }
    }

    /// The declared volume of a bus in decibels, or `None` if there is no such
    /// bus.
    pub fn bus_volume(&self, name: &str) -> Option<f32> {
        self.buses
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.volume_db)
    }

    /// The bus a bus feeds into.
    ///
    /// `Some(None)` means "feeds Master"; `None` means there is no such bus.
    pub fn bus_parent(&self, name: &str) -> Option<Option<String>> {
        self.buses
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.parent.clone())
    }

    /// Every bus's name and volume, for the scripting snapshot.
    pub fn bus_volumes(&self) -> Vec<(String, f32)> {
        self.buses
            .iter()
            .map(|(name, b)| (name.clone(), b.volume_db))
            .collect()
    }

    /// Sets a bus's volume in decibels. Returns whether the bus existed.
    ///
    /// This is what an options-menu volume slider drives: it changes every
    /// sound on the bus, present and future, which per-sound volume cannot do.
    pub fn set_bus_volume(&mut self, name: &str, db: f32) -> bool {
        let Some((_, bus)) = self.buses.iter_mut().find(|(n, _)| n == name) else {
            tracing::warn!("[audio] setBusVolume named unknown bus '{name}'");
            return false;
        };
        bus.volume_db = db;
        if let Some(handle) = bus.handle.as_mut() {
            handle.set_volume(Decibels(db), Tween::default());
        }
        true
    }

    /// Returns whether the audio backend initialized successfully and can play sounds.
    pub fn is_available(&self) -> bool {
        self.manager.is_some()
    }

    /// Starts playing the given sound data, returning a handle to it, or `None` if the audio
    /// backend is unavailable or playback failed to start.
    pub fn play(&mut self, data: StaticSoundData) -> Option<StaticSoundHandle> {
        self.manager.as_mut()?.play(data).ok()
    }

    /// Moves the listener — the ears the scene is heard from.
    ///
    /// Records the pose whether or not a backend exists. Without a device the
    /// manager is `None` and every call here would otherwise be unobservable,
    /// which is exactly the condition that produces tests passing on nothing;
    /// see [`last_listener_pose`](Self::last_listener_pose).
    pub fn set_listener_pose(&mut self, position: Vec3, orientation: Quat) {
        self.last_listener_pose = Some((position, orientation));
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        if self.listener.is_none() {
            self.listener = manager
                .add_listener(to_mint_vec(position), to_mint_quat(orientation))
                .ok();
        }
        if let Some(listener) = self.listener.as_mut() {
            listener.set_position(to_mint_vec(position), Tween::default());
            listener.set_orientation(to_mint_quat(orientation), Tween::default());
        }
    }

    /// Moves **every one** of `entity`'s emitter tracks, and ensures it has a
    /// Master one.
    ///
    /// An entity can hold a track per bus, so this updates all of them.
    /// Updating only the first is invisible in a single-bus fixture, which is
    /// exactly why `one_entity_can_emit_on_two_buses` exists.
    ///
    /// Does nothing but record the position if there is no listener yet: a
    /// spatial track is created *linked to* a listener, so there is nothing to
    /// attach one to until the scene has ears.
    pub fn set_emitter_position(&mut self, entity: Entity, position: Vec3) {
        self.last_emitter_positions.insert(entity, position);
        // The Master track is the one this entity has always had; keeping it
        // means a scene with no buses.ron behaves exactly as before.
        self.ensure_emitter_track(entity, None, position);
        let buses: Vec<Option<String>> = self
            .emitter_order
            .iter()
            .filter(|(e, _)| *e == entity)
            .map(|(_, b)| b.clone())
            .collect();
        for bus in buses {
            let key = (entity, bus);
            self.last_track_positions.insert(key.clone(), position);
            if let Some(track) = self.emitters.get_mut(&key) {
                track.set_position(to_mint_vec(position), Tween::default());
            }
        }
    }

    /// Moves — creating on first use — `entity`'s emitter track on one bus.
    pub fn set_emitter_position_on(&mut self, entity: Entity, bus: Option<&str>, position: Vec3) {
        self.last_emitter_positions.insert(entity, position);
        self.ensure_emitter_track(entity, bus, position);
        self.last_track_positions
            .insert((entity, bus.map(str::to_string)), position);
        if let Some(track) = self.emitters.get_mut(&(entity, bus.map(str::to_string))) {
            track.set_position(to_mint_vec(position), Tween::default());
        }
    }

    /// Records the (entity, bus) pair and creates its spatial track if it can.
    ///
    /// The pair is recorded even when no track can be made, so the keying
    /// stays observable without an audio device.
    fn ensure_emitter_track(&mut self, entity: Entity, bus: Option<&str>, position: Vec3) {
        let key = (entity, bus.map(str::to_string));
        if !self.emitter_order.contains(&key) {
            self.emitter_order.push(key.clone());
        }
        if self.emitters.contains_key(&key) {
            return;
        }
        let Some(listener_id) = self.listener.as_ref().map(|l| l.id()) else {
            return;
        };
        let builder = SpatialTrackBuilder::new();
        let created = match bus {
            None => self.manager.as_mut().and_then(|m| {
                m.add_spatial_sub_track(listener_id, to_mint_vec(position), builder)
                    .ok()
            }),
            Some(name) => {
                let slot = self.buses.iter_mut().find(|(n, _)| n == name);
                slot.and_then(|(_, b)| b.handle.as_mut()).and_then(|h| {
                    h.add_spatial_sub_track(listener_id, to_mint_vec(position), builder)
                        .ok()
                })
            }
        };
        if let Some(track) = created {
            self.emitters.insert(key, track);
        }
    }

    /// Position last pushed to one specific (entity, bus) track.
    ///
    /// The observable that makes "moves every track" testable without an
    /// audio device.
    pub fn last_track_position(&self, entity: Entity, bus: Option<&str>) -> Option<Vec3> {
        self.last_track_positions
            .get(&(entity, bus.map(str::to_string)))
            .copied()
    }

    /// Buses this entity has emitter tracks on, in the order they were first
    /// requested. `None` is Master.
    pub fn emitter_buses(&self, entity: Entity) -> Vec<Option<String>> {
        self.emitter_order
            .iter()
            .filter(|(e, _)| *e == entity)
            .map(|(_, b)| b.clone())
            .collect()
    }

    /// Plays a sound from `entity`'s position on Master.
    pub fn play_at(&mut self, entity: Entity, data: StaticSoundData) -> Option<StaticSoundHandle> {
        self.play_at_on_bus(entity, None, data)
    }

    /// Plays a sound from `entity`'s position on a named bus.
    ///
    /// Creates the (entity, bus) track on demand at the entity's last known
    /// position, because the bus is only known at the play call — `sync_emitters`
    /// positions entities without knowing which bus a future sound will use.
    pub fn play_at_on_bus(
        &mut self,
        entity: Entity,
        bus: Option<&str>,
        data: StaticSoundData,
    ) -> Option<StaticSoundHandle> {
        let position = self
            .last_emitter_positions
            .get(&entity)
            .copied()
            .unwrap_or(Vec3::ZERO);
        self.ensure_emitter_track(entity, bus, position);
        self.emitters
            .get_mut(&(entity, bus.map(str::to_string)))?
            .play(data)
            .ok()
    }

    /// Plays a sound on a named mixer bus, falling back to Master.
    pub fn play_on_bus(
        &mut self,
        bus: Option<&str>,
        data: StaticSoundData,
    ) -> Option<StaticSoundHandle> {
        let Some(name) = bus else {
            return self.play(data);
        };
        let slot = self.buses.iter_mut().find(|(n, _)| n == name);
        match slot.and_then(|(_, b)| b.handle.as_mut()) {
            Some(handle) => handle.play(data).ok(),
            // No such bus, or no backend: Master. Reported by `note_sound_bus`.
            None => self.play(data),
        }
    }

    /// Records which bus a sound was routed to, resolving an unknown name to
    /// Master.
    ///
    /// Called for every play so the routing is assertable: a dropped sound and
    /// a Master-routed sound are otherwise both "no error".
    pub fn note_sound_bus(&mut self, id: u32, bus: Option<&str>) {
        let resolved = match bus {
            Some(name) if self.buses.iter().any(|(n, _)| n == name) => Some(name.to_string()),
            Some(name) => {
                tracing::warn!("[audio] sound requested unknown bus '{name}' — playing on Master");
                None
            }
            None => None,
        };
        self.last_sound_buses.insert(id, resolved);
    }

    /// The bus a sound was routed to. `Some(None)` is Master; `None` means no
    /// such sound ever played.
    pub fn bus_of_sound(&self, id: u32) -> Option<Option<String>> {
        self.last_sound_buses.get(&id).cloned()
    }

    /// Drops **every one** of `entity`'s emitter tracks, on all buses.
    pub fn remove_emitter(&mut self, entity: Entity) {
        self.emitters.retain(|(e, _), _| *e != entity);
        self.emitter_order.retain(|(e, _)| *e != entity);
        self.last_track_positions.retain(|(e, _), _| *e != entity);
        self.last_emitter_positions.remove(&entity);
    }

    /// The last pose handed to the listener, whether or not a backend took it.
    ///
    /// This is what this crate is responsible for computing; what `kira` then
    /// does with it is upstream's contract. Tests assert on this because on a
    /// machine with no audio device — CI, for one — there is no other
    /// observable, and a test that asserts through a `None` manager passes
    /// without exercising anything.
    pub fn last_listener_pose(&self) -> Option<(Vec3, Quat)> {
        self.last_listener_pose
    }

    /// The last position handed to `entity`'s emitter. See
    /// [`last_listener_pose`](Self::last_listener_pose) for why this exists.
    pub fn last_emitter_position(&self, entity: Entity) -> Option<Vec3> {
        self.last_emitter_positions.get(&entity).copied()
    }
}

#[cfg(test)]
mod bus_tests {
    use super::*;
    use crate::bus::BusLayout;

    /// Two top-level buses at *different* volumes, plus a real parent link.
    ///
    /// Every detail is load-bearing. Equal volumes would let a bug that
    /// returns the wrong bus's volume read as correct, and without a parent
    /// link nothing distinguishes a tree from a flat list.
    fn fixture() -> BusLayout {
        BusLayout::from_ron(
            r#"BusLayout(buses: [
                Bus(name: "music", parent: None,        volume_db:  0.0),
                Bus(name: "sfx",   parent: None,        volume_db: -6.0),
                Bus(name: "ui",    parent: Some("sfx"), volume_db: -3.0),
            ])"#,
        )
        .expect("fixture parses")
    }

    /// `silent()` on purpose: this is the state every Windows CI runner is in,
    /// and the state in which the mixer must still be observable.
    #[test]
    fn bus_state_is_recorded_without_an_audio_backend() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());

        assert_eq!(world.bus_volume("music"), Some(0.0));
        assert_eq!(world.bus_volume("sfx"), Some(-6.0));
        assert_eq!(world.bus_volume("ui"), Some(-3.0));
        assert_eq!(world.bus_parent("ui"), Some(Some("sfx".to_string())));
        assert_eq!(
            world.bus_parent("sfx"),
            Some(None),
            "top-level feeds Master"
        );
        assert_eq!(world.bus_volume("nope"), None);
    }

    #[test]
    fn setting_a_bus_volume_changes_only_that_bus() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        assert!(world.set_bus_volume("sfx", -20.0));

        assert_eq!(world.bus_volume("sfx"), Some(-20.0));
        assert_eq!(
            world.bus_volume("ui"),
            Some(-3.0),
            "a child's own volume is its own; changing the parent must not rewrite it"
        );
        assert_eq!(world.bus_volume("music"), Some(0.0));
    }

    #[test]
    fn setting_an_unknown_bus_volume_is_reported_as_failure() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        assert!(
            !world.set_bus_volume("nope", -20.0),
            "an unknown bus must report failure rather than silently succeeding"
        );
        assert_eq!(world.bus_volume("nope"), None);
    }

    #[test]
    fn a_layout_replaces_the_previous_one() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        world.apply_bus_layout(
            &BusLayout::from_ron(
                r#"BusLayout(buses: [Bus(name: "only", parent: None, volume_db: -1.0)])"#,
            )
            .expect("parses"),
        );
        assert_eq!(world.bus_volume("only"), Some(-1.0));
        assert_eq!(
            world.bus_volume("sfx"),
            None,
            "the previous layout's buses must be gone, not merged"
        );
    }

    /// One entity emitting on two different buses.
    ///
    /// Without this case the (Entity, bus) keying is untested, and a
    /// set_emitter_position that moved only the first matching track would
    /// pass every other test in this file.
    #[test]
    fn one_entity_can_emit_on_two_buses() {
        use bevy_ecs::prelude::Entity;
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        let e = Entity::from_raw(7);

        world.set_emitter_position_on(e, Some("sfx"), Vec3::new(1.0, 0.0, 0.0));
        world.set_emitter_position_on(e, Some("music"), Vec3::new(1.0, 0.0, 0.0));
        world.set_emitter_position(e, Vec3::new(5.0, 0.0, 0.0));

        let buses = world.emitter_buses(e);
        assert!(
            buses.contains(&Some("sfx".to_string())) && buses.contains(&Some("music".to_string())),
            "both of this entity's emitter buses must be recorded, got {buses:?}"
        );
        // Per track, not per entity. The entity-level record would be updated
        // by moving a single track, so it cannot tell "moved all" from
        // "moved the first" -- mutation-testing found exactly that hole.
        for bus in [Some("sfx"), Some("music"), None] {
            assert_eq!(
                world.last_track_position(e, bus),
                Some(Vec3::new(5.0, 0.0, 0.0)),
                "track on bus {bus:?} was not moved; set_emitter_position must move                  every one of an entity's tracks, not just the first"
            );
        }
    }

    #[test]
    fn removing_an_emitter_drops_all_of_its_buses() {
        use bevy_ecs::prelude::Entity;
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        let e = Entity::from_raw(7);
        world.set_emitter_position_on(e, Some("sfx"), Vec3::ZERO);
        world.set_emitter_position_on(e, Some("music"), Vec3::ZERO);
        assert_eq!(world.emitter_buses(e).len(), 2);

        world.remove_emitter(e);
        assert!(
            world.emitter_buses(e).is_empty(),
            "all buses must go, not just one"
        );
        assert_eq!(world.last_emitter_position(e), None);
    }

    #[test]
    fn a_sound_records_the_bus_it_was_routed_to() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());

        world.note_sound_bus(1, Some("sfx"));
        world.note_sound_bus(2, None);
        world.note_sound_bus(3, Some("nope"));

        assert_eq!(world.bus_of_sound(1), Some(Some("sfx".to_string())));
        assert_eq!(world.bus_of_sound(2), Some(None), "no bus means Master");
        assert_eq!(
            world.bus_of_sound(3),
            Some(None),
            "an unknown bus falls back to Master rather than dropping the sound; assert              the fallback, because a dropped sound and a Master sound are both no-error"
        );
        assert_eq!(
            world.bus_of_sound(99),
            None,
            "a sound that never played has no bus"
        );
    }

    #[test]
    fn bus_volumes_reports_every_bus_for_the_scripting_snapshot() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(&fixture());
        let mut got = world.bus_volumes();
        got.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            got,
            vec![
                ("music".to_string(), 0.0),
                ("sfx".to_string(), -6.0),
                ("ui".to_string(), -3.0),
            ]
        );
    }
}
