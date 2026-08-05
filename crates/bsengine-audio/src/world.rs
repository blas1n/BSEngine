use std::collections::hash_map::Entry;
use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::{Quat, Vec3};
use kira::{
    listener::ListenerHandle,
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    track::{SpatialTrackBuilder, SpatialTrackHandle},
    AudioManager, AudioManagerSettings, DefaultBackend, Tween,
};

use crate::spatial::{to_mint_quat, to_mint_vec};

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
    fn silent() -> Self {
        Self {
            manager: None,
            listener: None,
            emitters: HashMap::new(),
            last_listener_pose: None,
            last_emitter_positions: HashMap::new(),
        }
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
