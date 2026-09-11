use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};
use glam::{Quat, Vec3};
use kira::{
    effect::{
        compressor::CompressorBuilder, delay::DelayBuilder, distortion::DistortionBuilder,
        eq_filter::EqFilterBuilder, filter::FilterBuilder, panning_control::PanningControlBuilder,
        reverb::ReverbBuilder,
    },
    listener::ListenerHandle,
    modulator::tweener::{TweenerBuilder, TweenerHandle},
    sound::static_sound::{StaticSoundData, StaticSoundHandle},
    track::{SpatialTrackBuilder, SpatialTrackHandle, TrackBuilder, TrackHandle},
    AudioManager, AudioManagerSettings, Decibels, DefaultBackend, Easing, Mapping, Mix, Panning,
    Tween, Value,
};

use crate::spatial::{to_mint_quat, to_mint_vec};

/// Builds kira's reverb from authored parameters.
///
/// Extracted, like the three below, because `ReverbBuilder` derives
/// `PartialEq`: a test can hand-build the expected builder and compare, which
/// is what makes the *parameter routing* observable rather than merely
/// non-panicking. Mutation-testing showed why that matters — swapping
/// `feedback` and `damping` inside a monolithic mapping failed no test at all.
fn reverb_builder(
    feedback: Value<f64>,
    damping: Value<f64>,
    stereo_width: Value<f64>,
    mix: Value<Mix>,
) -> ReverbBuilder {
    ReverbBuilder::new()
        .feedback(feedback)
        .damping(damping)
        .stereo_width(stereo_width)
        .mix(mix)
}

/// Builds kira's filter from authored parameters.
fn filter_builder(
    mode: crate::bus::FilterMode,
    cutoff: Value<f64>,
    resonance: Value<f64>,
    mix: Value<Mix>,
) -> FilterBuilder {
    use crate::bus::FilterMode as M;
    FilterBuilder::new()
        .mode(match mode {
            M::LowPass => kira::effect::filter::FilterMode::LowPass,
            M::BandPass => kira::effect::filter::FilterMode::BandPass,
            M::HighPass => kira::effect::filter::FilterMode::HighPass,
            M::Notch => kira::effect::filter::FilterMode::Notch,
        })
        .cutoff(cutoff)
        .resonance(resonance)
        .mix(mix)
}

/// Builds kira's distortion from authored parameters.
fn distortion_builder(
    kind: crate::bus::DistortionKind,
    drive_db: Value<Decibels>,
    mix: Value<Mix>,
) -> DistortionBuilder {
    use crate::bus::DistortionKind as K;
    DistortionBuilder::new()
        .kind(match kind {
            K::HardClip => kira::effect::distortion::DistortionKind::HardClip,
            K::SoftClip => kira::effect::distortion::DistortionKind::SoftClip,
        })
        .drive(drive_db)
        .mix(mix)
}

/// Builds kira's panning control from an authored position.
fn panning_builder(panning: Value<Panning>) -> PanningControlBuilder {
    PanningControlBuilder(panning)
}

/// Attaches an authored effect chain to a track builder, in order.
///
/// A free function rather than a method: it reads no [`AudioWorld`] state and
/// needs no audio device, because `TrackBuilder` is constructible without an
/// `AudioManager`. That is what makes the enum-to-builder mapping testable on
/// a machine with no sound card — which is every Windows CI runner here.
///
/// This crate writes no DSP. Every arm hands parameters to a `kira` builder
/// that already exists upstream; the conversions are `Mix`, `Decibels`,
/// `Panning` and milliseconds-to-`Duration`.
impl AudioWorld {
    /// Turns an authored [`Param`](crate::bus::Param) into a kira value,
    /// registering a tweener for an exposed one.
    ///
    /// Generic in the output type because kira's parameters are variously
    /// `f64`, `Decibels`, `Mix`, `Panning` and `Duration`, and all of them need
    /// the same treatment: one tweener carrying the *real* number, mapped
    /// identity so a script sets 2000 Hz rather than 0.37.
    ///
    /// kira clamps to `input_range`, which is what makes the authored
    /// `min`/`max` a guard rail rather than decoration — a script asking for a
    /// negative cutoff gets `min`, not an undefined filter.
    fn resolve<T: kira::Tweenable>(
        &mut self,
        param: &crate::bus::Param,
        to: impl Fn(f64) -> T,
    ) -> Value<T> {
        let crate::bus::Param::Exposed {
            name,
            min,
            max,
            initial,
        } = param
        else {
            return Value::Fixed(to(param.initial()));
        };
        // Recorded whether or not a backend exists: without a device there is
        // no tweener at all, and a test asserting through one would be
        // asserting on nothing.
        self.last_param_values
            .entry(name.clone())
            .or_insert((*initial, *min, *max));
        let id = match self.audio_params.get(name) {
            Some(handle) => handle.id(),
            None => {
                let Some(manager) = self.manager.as_mut() else {
                    return Value::Fixed(to(*initial));
                };
                let Ok(handle) = manager.add_modulator(TweenerBuilder {
                    initial_value: *initial,
                }) else {
                    return Value::Fixed(to(*initial));
                };
                let id = handle.id();
                self.audio_params.insert(name.clone(), handle);
                id
            }
        };
        Value::FromModulator {
            id,
            mapping: Mapping {
                input_range: (*min, *max),
                output_range: (to(*min), to(*max)),
                easing: Easing::Linear,
            },
        }
    }

    /// Attaches an authored effect chain to a track builder, in order.
    ///
    /// A method rather than a free function because resolving an exposed
    /// parameter may create a modulator, which needs the manager.
    ///
    /// This crate writes no DSP. Every arm hands values to a `kira` builder
    /// that already exists upstream.
    fn with_effects(
        &mut self,
        mut builder: TrackBuilder,
        effects: &[crate::bus::BusEffect],
    ) -> TrackBuilder {
        use crate::bus::{BusEffect, EqFilterKind};
        use std::time::Duration;

        let db = |v: f64| Decibels(v as f32);
        let mix = |v: f64| Mix(v as f32);
        let pan = |v: f64| Panning(v as f32);
        let ms = |v: f64| Duration::from_secs_f64((v / 1000.0).max(0.0));

        for effect in effects {
            builder = match effect {
                BusEffect::Reverb {
                    feedback,
                    damping,
                    stereo_width,
                    mix: m,
                } => {
                    let f = self.resolve(feedback, |v| v);
                    let d = self.resolve(damping, |v| v);
                    let w = self.resolve(stereo_width, |v| v);
                    let m = self.resolve(m, mix);
                    builder.with_effect(reverb_builder(f, d, w, m))
                }
                BusEffect::Filter {
                    mode,
                    cutoff,
                    resonance,
                    mix: m,
                } => {
                    let c = self.resolve(cutoff, |v| v);
                    let r = self.resolve(resonance, |v| v);
                    let m = self.resolve(m, mix);
                    builder.with_effect(filter_builder(*mode, c, r, m))
                }
                BusEffect::Compressor {
                    threshold,
                    ratio,
                    attack_ms,
                    release_ms,
                    makeup_gain_db,
                    mix: m,
                } => {
                    let t = self.resolve(threshold, |v| v);
                    let ra = self.resolve(ratio, |v| v);
                    let a = self.resolve(attack_ms, ms);
                    let re = self.resolve(release_ms, ms);
                    let g = self.resolve(makeup_gain_db, db);
                    let m = self.resolve(m, mix);
                    builder.with_effect(
                        CompressorBuilder::new()
                            .threshold(t)
                            .ratio(ra)
                            .attack_duration(a)
                            .release_duration(re)
                            .makeup_gain(g)
                            .mix(m),
                    )
                }
                BusEffect::Delay {
                    delay_ms,
                    feedback_db,
                    mix: m,
                } => {
                    let fb = self.resolve(feedback_db, db);
                    let m = self.resolve(m, mix);
                    builder.with_effect(
                        DelayBuilder::new()
                            // The one parameter that is a plain number here:
                            // kira's `delay_time` takes a `Duration`, not a
                            // `Value`, so there is nothing to modulate.
                            .delay_time(ms(*delay_ms))
                            .feedback(fb)
                            .mix(m),
                    )
                }
                BusEffect::Distortion {
                    kind,
                    drive_db,
                    mix: m,
                } => {
                    let d = self.resolve(drive_db, db);
                    let m = self.resolve(m, mix);
                    builder.with_effect(distortion_builder(*kind, d, m))
                }
                BusEffect::EqFilter {
                    kind,
                    frequency,
                    gain_db,
                    q,
                } => {
                    let f = self.resolve(frequency, |v| v);
                    let g = self.resolve(gain_db, db);
                    let q = self.resolve(q, |v| v);
                    builder.with_effect(EqFilterBuilder::new(
                        match kind {
                            EqFilterKind::Bell => kira::effect::eq_filter::EqFilterKind::Bell,
                            EqFilterKind::LowShelf => {
                                kira::effect::eq_filter::EqFilterKind::LowShelf
                            }
                            EqFilterKind::HighShelf => {
                                kira::effect::eq_filter::EqFilterKind::HighShelf
                            }
                        },
                        f,
                        g,
                        q,
                    ))
                }
                BusEffect::Panning { panning } => {
                    let p = self.resolve(panning, pan);
                    builder.with_effect(panning_builder(p))
                }
            };
        }
        builder
    }
}

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
    /// The authored chain, recorded so the authoring contract is observable on
    /// a machine with no audio device — where the handle below is `None` and a
    /// running effect has no observable at all.
    effects: Vec<crate::bus::BusEffect>,
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
    /// One tweener per *exposed* effect parameter, keyed by its authored name.
    ///
    /// Unity's exposed-parameter model: the author names which parameters a
    /// script may reach, and the name is the whole address — nothing here
    /// depends on an effect's position in a chain, so reordering a chain
    /// cannot silently re-point a name.
    audio_params: HashMap<String, TweenerHandle>,
    /// The last value set for each exposed parameter, and its clamp range.
    ///
    /// Recorded whether or not a backend exists, for the same reason every
    /// other observable in this file is: without a device there is no tweener
    /// at all, and a test asserting through one would pass on nothing.
    last_param_values: HashMap<String, (f64, f64, f64)>,
    /// Per-emitter occlusion settings: `(cutoff_hz, volume_db)`.
    ///
    /// Recorded before the track is built, because the filter has to be
    /// attached at *creation* — `SpatialTrackBuilder::with_effect` — and its
    /// cutoff has to bind to a modulator that already exists.
    occlusion_config: HashMap<Entity, (f32, f32)>,
    /// One tweener per occluding emitter, driving both its filter cutoff and
    /// its volume from a single 0..1 "how occluded" value.
    ///
    /// One modulator rather than two because both parameters map from the same
    /// number: `set_occlusion` is then a single `set`, and kira's own tween is
    /// the interpolation — which is how Unreal solves occlusion flicker
    /// (`OcclusionInterpolationTime`) rather than hand-rolled hysteresis.
    occlusion_tweeners: HashMap<Entity, TweenerHandle>,
    /// How occluded each emitter last was, 0.0 clear to 1.0 fully occluded.
    ///
    /// Recorded whether or not a backend exists — on a machine with no audio
    /// device the tweener above does not exist at all, and this is the only
    /// observable the occlusion decision has.
    last_occlusion: HashMap<Entity, f32>,
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
            audio_params: HashMap::new(),
            last_param_values: HashMap::new(),
            occlusion_config: HashMap::new(),
            occlusion_tweeners: HashMap::new(),
            last_occlusion: HashMap::new(),
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
            let handle = self.create_bus_track(bus.parent.as_deref(), bus.volume_db, &bus.effects);
            self.buses.push((
                bus.name.clone(),
                BusState {
                    parent: bus.parent.clone(),
                    volume_db: bus.volume_db,
                    effects: bus.effects.clone(),
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
    fn create_bus_track(
        &mut self,
        parent: Option<&str>,
        volume_db: f32,
        effects: &[crate::bus::BusEffect],
    ) -> Option<TrackHandle> {
        let builder = self.with_effects(TrackBuilder::new().volume(Decibels(volume_db)), effects);
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

    /// The authored effect chain on a bus, in order, or `None` if there is no
    /// such bus.
    ///
    /// This is the authoring contract, and the only part of an effect that is
    /// observable without an audio device: what a running effect does to audio
    /// is kira's to answer, and our wiring into a live track cannot be seen on
    /// hardware with no sound card.
    pub fn bus_effects(&self, name: &str) -> Option<&[crate::bus::BusEffect]> {
        self.buses
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, b)| b.effects.as_slice())
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
        // Both the cutoff and the volume map from one tweener, so a single
        // `set_occlusion` moves them together. Unoccluded (0.0) is 20 kHz and
        // 0 dB — audibly untouched — and fully occluded (1.0) is whatever the
        // `AudioOcclusion` component asked for.
        let builder = match (
            self.occlusion_config.get(&entity).copied(),
            self.occlusion_tweeners.get(&entity).map(|t| t.id()),
        ) {
            (Some((cutoff_hz, volume_db)), Some(id)) => SpatialTrackBuilder::new()
                .volume(Value::FromModulator {
                    id,
                    mapping: Mapping {
                        input_range: (0.0, 1.0),
                        output_range: (Decibels(0.0), Decibels(volume_db)),
                        easing: Easing::Linear,
                    },
                })
                .with_effect(FilterBuilder::new().cutoff(Value::FromModulator {
                    id,
                    mapping: Mapping {
                        input_range: (0.0, 1.0),
                        output_range: (20_000.0, cutoff_hz as f64),
                        easing: Easing::Linear,
                    },
                })),
            _ => SpatialTrackBuilder::new(),
        };
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

    /// Declares that an emitter occludes, and how.
    ///
    /// Must be called before the emitter's spatial track is created: the
    /// low-pass filter is attached at track creation and its cutoff binds to a
    /// modulator, so both have to exist first. `sync_emitters` calls this each
    /// frame for every entity carrying `AudioOcclusion`, which makes the
    /// ordering automatic.
    pub fn set_occlusion_config(&mut self, entity: Entity, cutoff_hz: f32, volume_db: f32) {
        let changed = self.occlusion_config.insert(entity, (cutoff_hz, volume_db))
            != Some((cutoff_hz, volume_db));
        if !changed {
            return;
        }
        // A tweener is only useful with a backend; without one the config is
        // still recorded so the decision half stays observable.
        if self.occlusion_tweeners.contains_key(&entity) {
            return;
        }
        if let Some(manager) = self.manager.as_mut() {
            if let Ok(tweener) = manager.add_modulator(TweenerBuilder { initial_value: 0.0 }) {
                self.occlusion_tweeners.insert(entity, tweener);
            }
        }
    }

    /// The occlusion settings recorded for an emitter, if it occludes.
    pub fn occlusion_config_of(&self, entity: Entity) -> Option<(f32, f32)> {
        self.occlusion_config.get(&entity).copied()
    }

    /// Sets an exposed effect parameter by name. Returns whether it exists.
    ///
    /// This is the thing #1838 deferred: kira's effect handles have almost no
    /// setters, so a parameter is changed by moving the modulator it was bound
    /// to when its track was built.
    pub fn set_audio_param(&mut self, name: &str, value: f64, tween_ms: f32) -> bool {
        let Some((current, min, max)) = self.last_param_values.get_mut(name) else {
            tracing::warn!("[audio] setAudioParam named unknown parameter '{name}'");
            return false;
        };
        let clamped = value.clamp(*min, *max);
        *current = clamped;
        if let Some(tweener) = self.audio_params.get_mut(name) {
            tweener.set(
                clamped,
                Tween {
                    duration: std::time::Duration::from_secs_f32((tween_ms / 1000.0).max(0.0)),
                    ..Default::default()
                },
            );
        }
        true
    }

    /// The last value set for an exposed parameter, or `None` if no parameter
    /// of that name was declared.
    pub fn audio_param(&self, name: &str) -> Option<f64> {
        self.last_param_values.get(name).map(|(v, _, _)| *v)
    }

    /// Every exposed parameter's name and current value, for the scripting
    /// snapshot.
    pub fn audio_params(&self) -> Vec<(String, f64)> {
        self.last_param_values
            .iter()
            .map(|(name, (v, _, _))| (name.clone(), *v))
            .collect()
    }

    /// Sets how occluded an emitter is: 0.0 clear, 1.0 fully occluded.
    ///
    /// This is the seam between the two halves of occlusion. Deciding *whether*
    /// something is in the way needs physics, which this crate deliberately
    /// does not depend on — that half lives in `bsengine-runtime`, which has
    /// both. What happens to the sound lives here.
    ///
    /// `amount` is clamped, and the change is tweened over
    /// `interpolation_ms`, so a value that moves every frame does not click.
    pub fn set_occlusion(&mut self, entity: Entity, amount: f32, interpolation_ms: f32) {
        let amount = amount.clamp(0.0, 1.0);
        self.last_occlusion.insert(entity, amount);
        if let Some(tweener) = self.occlusion_tweeners.get_mut(&entity) {
            tweener.set(
                amount as f64,
                Tween {
                    duration: std::time::Duration::from_secs_f32(
                        (interpolation_ms / 1000.0).max(0.0),
                    ),
                    ..Default::default()
                },
            );
        }
    }

    /// How occluded an emitter last was, or `None` if it was never set.
    pub fn occlusion_of(&self, entity: Entity) -> Option<f32> {
        self.last_occlusion.get(&entity).copied()
    }

    /// Whether this emitter has a live occlusion tweener.
    ///
    /// Always `false` without an audio backend, which is why
    /// [`occlusion_of`](Self::occlusion_of) and not this is what tests assert
    /// on.
    pub fn has_occlusion_tweener(&self, entity: Entity) -> bool {
        self.occlusion_tweeners.contains_key(&entity)
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
        self.occlusion_config.remove(&entity);
        self.occlusion_tweeners.remove(&entity);
        self.last_occlusion.remove(&entity);
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

    /// Every effect variant maps onto a kira builder, with no audio device.
    ///
    /// `TrackBuilder` is constructible without an `AudioManager`, so this
    /// exercises the real enum-to-builder mapping rather than a stand-in. It
    /// is the only coverage that mapping can have here: what a running effect
    /// does to audio is kira's to answer, and our wiring into a *live* track
    /// is invisible on hardware with no sound card.
    ///
    /// Every variant is listed explicitly rather than looped, so adding an
    /// eighth effect without mapping it is a compile error in this test.
    #[test]
    fn every_effect_variant_maps_onto_a_kira_builder() {
        use crate::bus::{BusEffect, DistortionKind, EqFilterKind, FilterMode, Param};

        let all = vec![
            BusEffect::Reverb {
                feedback: Param::Fixed(0.5),
                damping: Param::Fixed(0.25),
                stereo_width: Param::Fixed(0.125),
                mix: Param::Fixed(0.0625),
            },
            BusEffect::Filter {
                mode: FilterMode::Notch,
                cutoff: Param::Fixed(800.0),
                resonance: Param::Fixed(0.25),
                mix: Param::Fixed(0.75),
            },
            BusEffect::Compressor {
                threshold: Param::Fixed(-12.0),
                ratio: Param::Fixed(4.0),
                attack_ms: Param::Fixed(5.0),
                release_ms: Param::Fixed(250.0),
                makeup_gain_db: Param::Fixed(3.0),
                mix: Param::Fixed(0.9),
            },
            BusEffect::Delay {
                delay_ms: 125.0,
                feedback_db: Param::Fixed(-9.0),
                mix: Param::Fixed(0.3),
            },
            BusEffect::Distortion {
                kind: DistortionKind::SoftClip,
                drive_db: Param::Fixed(6.0),
                mix: Param::Fixed(0.8),
            },
            BusEffect::EqFilter {
                kind: EqFilterKind::HighShelf,
                frequency: Param::Fixed(4000.0),
                gain_db: Param::Fixed(-3.0),
                q: Param::Fixed(1.2),
            },
            BusEffect::Panning {
                panning: Param::Fixed(-0.5),
            },
        ];
        assert_eq!(
            all.len(),
            7,
            "seven effects are exposed; kira's volume_control is deliberately absent              because Bus.volume_db already is one"
        );

        // The assertion is that this runs at all: a panic or an unhandled
        // variant is the failure this catches.
        let mut world = AudioWorld::silent();
        let _builder = world.with_effects(TrackBuilder::new(), &all);

        // And each one on its own, so a panic names the culprit.
        for effect in &all {
            let _ = world.with_effects(TrackBuilder::new(), std::slice::from_ref(effect));
        }
    }

    /// Each authored parameter reaches the kira field it names.
    ///
    /// Only four of the seven effects can be checked this way: kira derives
    /// `PartialEq` on `ReverbBuilder`, `FilterBuilder`, `DistortionBuilder`
    /// and `PanningControlBuilder`, and on nothing else — `CompressorBuilder`,
    /// `DelayBuilder` and `EqFilterBuilder` carry no derives at all, so a
    /// built one cannot be compared to anything.
    ///
    /// This test exists because mutation-testing found the gap: swapping
    /// `feedback` and `damping` inside the mapping failed **no** test, since
    /// the only other coverage asserts that mapping does not panic. Every
    /// value below is distinct so a swap cannot survive.
    #[test]
    fn reverb_parameters_reach_their_own_kira_fields() {
        assert_eq!(
            reverb_builder(
                Value::Fixed(0.5),
                Value::Fixed(0.25),
                Value::Fixed(0.125),
                Value::Fixed(Mix(0.0625)),
            ),
            ReverbBuilder::new()
                .feedback(0.5)
                .damping(0.25)
                .stereo_width(0.125)
                .mix(Mix(0.0625)),
            "a parameter reached the wrong kira field"
        );
    }

    #[test]
    fn filter_parameters_reach_their_own_kira_fields() {
        use crate::bus::FilterMode;
        assert_eq!(
            filter_builder(
                FilterMode::Notch,
                Value::Fixed(800.0),
                Value::Fixed(0.25),
                Value::Fixed(Mix(0.75)),
            ),
            FilterBuilder::new()
                .mode(kira::effect::filter::FilterMode::Notch)
                .cutoff(800.0)
                .resonance(0.25)
                .mix(Mix(0.75)),
            "a parameter reached the wrong kira field"
        );
        // And the mode really is translated, not defaulted: LowPass is kira's
        // default, so only a non-default mode proves the match arm runs.
        assert_ne!(
            filter_builder(
                FilterMode::HighPass,
                Value::Fixed(800.0),
                Value::Fixed(0.25),
                Value::Fixed(Mix(0.75))
            ),
            filter_builder(
                FilterMode::LowPass,
                Value::Fixed(800.0),
                Value::Fixed(0.25),
                Value::Fixed(Mix(0.75))
            ),
            "FilterMode is being ignored"
        );
    }

    #[test]
    fn distortion_parameters_reach_their_own_kira_fields() {
        use crate::bus::DistortionKind;
        assert_eq!(
            distortion_builder(
                DistortionKind::SoftClip,
                Value::Fixed(Decibels(6.0)),
                Value::Fixed(Mix(0.8)),
            ),
            DistortionBuilder::new()
                .kind(kira::effect::distortion::DistortionKind::SoftClip)
                .drive(Decibels(6.0))
                .mix(Mix(0.8)),
            "a parameter reached the wrong kira field"
        );
        assert_ne!(
            distortion_builder(
                DistortionKind::HardClip,
                Value::Fixed(Decibels(6.0)),
                Value::Fixed(Mix(0.8))
            ),
            distortion_builder(
                DistortionKind::SoftClip,
                Value::Fixed(Decibels(6.0)),
                Value::Fixed(Mix(0.8))
            ),
            "DistortionKind is being ignored"
        );
    }

    #[test]
    fn panning_reaches_its_own_kira_field() {
        assert_eq!(
            panning_builder(Value::Fixed(Panning(-0.5))),
            PanningControlBuilder(Panning(-0.5).into())
        );
        assert_ne!(
            panning_builder(Value::Fixed(Panning(-0.5))),
            panning_builder(Value::Fixed(Panning(0.5))),
            "panning is being ignored"
        );
    }

    /// An exposed parameter is registered when its bus is built, and settable
    /// by name afterwards.
    ///
    /// Runs against `silent()` — the state every Windows CI runner is in — so
    /// what is asserted is the bookkeeping, not kira's modulator.
    #[test]
    fn an_exposed_parameter_is_registered_and_settable() {
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(
            &BusLayout::from_ron(
                r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: 0.0, effects: [
                    Filter(cutoff: (name: "muffle", min: 200.0, max: 20000.0, initial: 2000.0)),
                ])])"#,
            )
            .expect("parses"),
        );

        assert_eq!(
            world.audio_param("muffle"),
            Some(2000.0),
            "the parameter starts at its authored initial"
        );
        assert!(world.set_audio_param("muffle", 500.0, 0.0));
        assert_eq!(world.audio_param("muffle"), Some(500.0));
    }

    #[test]
    fn setting_an_exposed_parameter_clamps_to_its_authored_range() {
        // kira clamps a modulated value to the mapping's input range, so the
        // authored min/max are a guard rail. This asserts the same bound on
        // the recorded value, which is what a script reads back.
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(
            &BusLayout::from_ron(
                r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: 0.0, effects: [
                    Filter(cutoff: (name: "muffle", min: 200.0, max: 20000.0, initial: 2000.0)),
                ])])"#,
            )
            .expect("parses"),
        );

        world.set_audio_param("muffle", -5.0, 0.0);
        assert_eq!(world.audio_param("muffle"), Some(200.0), "clamped to min");
        world.set_audio_param("muffle", 1.0e9, 0.0);
        assert_eq!(world.audio_param("muffle"), Some(20000.0), "clamped to max");
    }

    #[test]
    fn an_unknown_parameter_name_is_reported_as_failure() {
        let mut world = AudioWorld::silent();
        assert!(
            !world.set_audio_param("nope", 1.0, 0.0),
            "an unknown name must report failure rather than silently succeeding"
        );
        assert_eq!(world.audio_param("nope"), None);
    }

    #[test]
    fn a_fixed_parameter_registers_no_name() {
        // The paired direction: without it, an implementation that exposed
        // every parameter regardless would pass the tests above.
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(
            &BusLayout::from_ron(
                r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: 0.0, effects: [
                    Filter(cutoff: 2000.0, resonance: 0.5),
                ])])"#,
            )
            .expect("parses"),
        );
        assert!(
            world.audio_params().is_empty(),
            "a chain of fixed parameters must expose nothing, got {:?}",
            world.audio_params()
        );
    }

    #[test]
    fn a_buss_authored_chain_is_readable_without_a_backend() {
        use crate::bus::BusEffect;
        let mut world = AudioWorld::silent();
        world.apply_bus_layout(
            &BusLayout::from_ron(
                r#"BusLayout(buses: [
                    Bus(name: "sfx", parent: None, volume_db: 0.0, effects: [
                        Filter(mode: HighPass, cutoff: 800.0, resonance: 0.25, mix: 0.75),
                        Reverb(feedback: 0.5, damping: 0.25, stereo_width: 0.125, mix: 0.0625),
                    ]),
                    Bus(name: "music", parent: None, volume_db: -3.0),
                ])"#,
            )
            .expect("parses"),
        );

        let fx = world.bus_effects("sfx").expect("sfx exists");
        assert_eq!(fx.len(), 2, "both authored effects must be recorded");
        assert!(matches!(fx[0], BusEffect::Filter { .. }), "order preserved");
        assert!(matches!(fx[1], BusEffect::Reverb { .. }), "order preserved");
        assert!(
            world.bus_effects("music").expect("music exists").is_empty(),
            "a bus with no effects has an empty chain, not the other bus's"
        );
        assert!(world.bus_effects("nope").is_none());
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
