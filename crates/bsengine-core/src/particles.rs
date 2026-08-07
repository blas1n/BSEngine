//! Short-lived billboarded particles and the emitter that owns them.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::Vec3;

use crate::reflect_color::ReflectColor;

/// A deterministic pseudo-random source, one per emitter.
///
/// Replays pin the clock so a recording reproduces exactly, and particle spawn
/// directions have to be repeatable for the same reason. An xorshift kept on
/// the emitter is enough, and avoids taking a dependency on `rand` for what
/// amounts to one f32 at a time.
#[derive(Debug, Clone)]
pub struct Rng {
    state: u32,
}

impl Rng {
    /// A generator that always produces the same sequence for `seed`.
    pub fn seeded(seed: u32) -> Self {
        // Zero is a fixed point of xorshift -- it would return zero forever --
        // so it can never be the state.
        Self {
            state: if seed == 0 { 0x9E37_79B9 } else { seed },
        }
    }

    /// The next value in `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        // 24 bits is every bit of mantissa an f32 has for [0, 1).
        (x >> 8) as f32 / (1u32 << 24) as f32
    }
}

impl Default for Rng {
    fn default() -> Self {
        Self::seeded(0x2545_F491)
    }
}

/// One live particle.
///
/// Plain data, never a component: particles belong to their emitter, not to the
/// ECS. An entity per particle would put thousands of archetype rows in front
/// of a system that only ever walks them in bulk.
#[derive(Debug, Clone, Copy)]
pub struct Particle {
    /// World-space position.
    pub position: Vec3,
    /// World-space velocity, in units per second.
    pub velocity: Vec3,
    /// Seconds since this particle was emitted.
    pub age: f32,
}

/// Emits and owns a cloud of short-lived billboarded particles.
///
/// # What is reflected, and what is not
///
/// Every parameter is reflected; `live`, `pending_burst`, `rng` and
/// `spawn_debt` are `#[reflect(ignore)]`. R1 asks that a public component be
/// *visible* -- that the Inspector shows the entity has an emitter and that MCP
/// can see it is attached -- and the parameters are the whole of what a human
/// or an agent would set. The ignored four are per-frame simulation state: an
/// array rewritten every tick, a counter the scripting op sets, a generator
/// whose value means nothing outside the simulation, and a fractional carry.
/// Serialising them would write thousands of transient positions into a file
/// whose job is to describe a starting state.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct ParticleEmitter {
    /// Particles per second while enabled. Zero means burst-only.
    pub rate: f32,
    /// How many particles one burst emits.
    pub burst_count: u32,
    /// How long each particle lives, in seconds.
    ///
    /// Named `particle_lifetime` rather than `lifetime` deliberately:
    /// [`crate::Lifetime`] already owns that word and it means "despawn this
    /// entity". An emitter field called `lifetime` would read as "this emitter
    /// disappears after N seconds", which is not what it does.
    pub particle_lifetime: f32,
    /// Speed given to a particle at birth, in units per second.
    pub initial_speed: f32,
    /// Half-angle of the emission cone, in degrees. 180 is a full sphere.
    pub spread_degrees: f32,
    /// Billboard half-size at birth.
    pub start_size: f32,
    /// Billboard half-size at death, interpolated across the particle's life.
    pub end_size: f32,
    /// Colour at birth.
    pub start_color: ReflectColor,
    /// Colour at death, interpolated across the particle's life.
    pub end_color: ReflectColor,
    /// Downward acceleration applied to every particle, in units per second².
    pub gravity: f32,
    /// Whether continuous emission runs. Bursts are unaffected by this.
    pub enabled: bool,
    /// The particles currently alive. Not reflected: see the type-level note.
    #[reflect(ignore)]
    pub live: Vec<Particle>,
    /// Particles owed by burst requests, emitted on the next tick.
    /// Not reflected: see the type-level note.
    #[reflect(ignore)]
    pub pending_burst: u32,
    /// Not reflected: see the type-level note.
    #[reflect(ignore)]
    pub rng: Rng,
    /// Fractional particles carried between frames, so a rate below one per
    /// frame still emits at the right average. Not reflected.
    #[reflect(ignore)]
    pub spawn_debt: f32,
}

impl Default for ParticleEmitter {
    fn default() -> Self {
        Self {
            rate: 0.0,
            burst_count: 24,
            particle_lifetime: 0.6,
            initial_speed: 4.0,
            spread_degrees: 35.0,
            start_size: 0.12,
            end_size: 0.0,
            start_color: Vec3::new(1.0, 0.8, 0.3).into(),
            end_color: Vec3::new(1.0, 0.2, 0.0).into(),
            gravity: 9.0,
            enabled: true,
            live: Vec::new(),
            pending_burst: 0,
            rng: Rng::default(),
            spawn_debt: 0.0,
        }
    }
}

impl ParticleEmitter {
    /// Queues `burst_count` particles to be emitted on the next tick.
    ///
    /// Queued rather than emitted here because emission needs the emitter's
    /// world position, which this type does not have -- the simulation reads it
    /// from the entity's `Transform`.
    pub fn burst(&mut self) {
        self.pending_burst += self.burst_count;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_produces_the_same_sequence() {
        // Replays pin the clock for reproducibility, and a test that asserts a
        // particle's position can only do so if the randomness repeats.
        let mut a = Rng::seeded(7);
        let mut b = Rng::seeded(7);
        let one: Vec<f32> = (0..5).map(|_| a.unit()).collect();
        let two: Vec<f32> = (0..5).map(|_| b.unit()).collect();
        assert_eq!(one, two);
    }

    #[test]
    fn unit_stays_within_zero_and_one() {
        let mut rng = Rng::seeded(1);
        for _ in 0..1000 {
            let v = rng.unit();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn different_seeds_diverge() {
        // Without this, a generator that always returned the same number would
        // satisfy both tests above.
        let mut a = Rng::seeded(1);
        let mut b = Rng::seeded(2);
        let one: Vec<f32> = (0..5).map(|_| a.unit()).collect();
        let two: Vec<f32> = (0..5).map(|_| b.unit()).collect();
        assert_ne!(one, two);
    }

    #[test]
    fn a_zero_seed_still_generates() {
        // Zero is xorshift's fixed point; seeded() substitutes for it. Without
        // that, an emitter left at seed 0 would emit every particle in exactly
        // the same direction.
        let mut rng = Rng::seeded(0);
        let values: Vec<f32> = (0..3).map(|_| rng.unit()).collect();
        assert!(
            values.iter().any(|v| *v != values[0]),
            "a zero seed produced a constant sequence: {values:?}"
        );
    }

    #[test]
    fn burst_queues_its_count() {
        let mut emitter = ParticleEmitter {
            burst_count: 7,
            ..Default::default()
        };
        emitter.burst();
        assert_eq!(emitter.pending_burst, 7);
        emitter.burst();
        assert_eq!(emitter.pending_burst, 14, "two bursts owe twice as many");
    }
}
