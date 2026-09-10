use std::collections::hash_map::Entry;
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
    emitters: HashMap<Entity, SpatialTrackHandle>,
    last_listener_pose: Option<(Vec3, Quat)>,
    last_emitter_positions: HashMap<Entity, Vec3>,
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
            last_listener_pose: None,
            last_emitter_positions: HashMap::new(),
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

    /// Moves `entity`'s emitter, creating its spatial track on first use.
    ///
    /// Does nothing but record the position if there is no listener yet: a
    /// spatial track is created *linked to* a listener, so there is nothing to
    /// attach one to until the scene has ears.
    pub fn set_emitter_position(&mut self, entity: Entity, position: Vec3) {
        self.last_emitter_positions.insert(entity, position);
        let Some(manager) = self.manager.as_mut() else {
            return;
        };
        let Some(listener) = self.listener.as_ref() else {
            return;
        };
        match self.emitters.entry(entity) {
            Entry::Occupied(mut track) => {
                track
                    .get_mut()
                    .set_position(to_mint_vec(position), Tween::default());
            }
            Entry::Vacant(slot) => {
                if let Ok(track) = manager.add_spatial_sub_track(
                    listener.id(),
                    to_mint_vec(position),
                    SpatialTrackBuilder::new(),
                ) {
                    slot.insert(track);
                }
            }
        }
    }

    /// Plays a sound from `entity`'s position, or returns `None` if that entity
    /// has no emitter track — because it is not an emitter, or because nothing
    /// is listening yet.
    pub fn play_at(&mut self, entity: Entity, data: StaticSoundData) -> Option<StaticSoundHandle> {
        self.emitters.get_mut(&entity)?.play(data).ok()
    }

    /// Drops `entity`'s emitter track. Called when an emitter despawns so its
    /// track does not outlive it.
    pub fn remove_emitter(&mut self, entity: Entity) {
        self.emitters.remove(&entity);
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
