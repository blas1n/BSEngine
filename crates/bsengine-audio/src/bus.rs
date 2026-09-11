//! The authored bus layout: parsing `buses.ron` and making sense of it.
//!
//! Deliberately free of `kira`, of the audio device, and of ECS, so that
//! validation and creation ordering are testable as plain data on any
//! machine — including the Windows CI runners, which have no audio device at
//! all and where anything asserted through a `None` audio manager passes
//! without exercising a thing.

use serde::Deserialize;

/// A numeric effect parameter: a fixed number, or one a script can change at
/// runtime by name.
///
/// ```ron
/// Filter(cutoff: 2000.0)                                          // fixed
/// Filter(cutoff: (name: "muffle", min: 200.0, max: 20000.0, initial: 2000.0))
/// ```
///
/// This is Unity's exposed-parameter model: the author picks which parameters
/// a script may reach and names them, rather than the script addressing an
/// effect by position. Godot addresses by chain index, which silently
/// re-points every name when the chain is reordered; naming at the parameter
/// cannot be misaddressed at all.
///
/// `min`/`max` are required because kira clamps a modulated value to the
/// mapping's input range. They are the guard rail: a script that sets a cutoff
/// of -5 gets `min`, not an undefined filter.
#[derive(Clone, Debug, PartialEq)]
pub enum Param {
    /// A constant, written as a bare number.
    Fixed(f64),
    /// A value a script sets by name, clamped to `min..=max`.
    Exposed {
        /// Name scripts use with `Bsengine.setAudioParam`.
        name: String,
        /// Lowest value the parameter can take.
        min: f64,
        /// Highest value the parameter can take.
        max: f64,
        /// Value before any script sets it.
        initial: f64,
    },
}

impl Param {
    /// The value this parameter starts at, whichever form it takes.
    pub fn initial(&self) -> f64 {
        match self {
            Self::Fixed(v) => *v,
            Self::Exposed { initial, .. } => *initial,
        }
    }

    /// The name a script reaches this parameter by, if it is exposed.
    pub fn exposed_name(&self) -> Option<&str> {
        match self {
            Self::Fixed(_) => None,
            Self::Exposed { name, .. } => Some(name),
        }
    }

    /// The range an exposed value is clamped to.
    pub fn range(&self) -> Option<(f64, f64)> {
        match self {
            Self::Fixed(_) => None,
            Self::Exposed { min, max, .. } => Some((*min, *max)),
        }
    }
}

impl From<f64> for Param {
    fn from(v: f64) -> Self {
        Self::Fixed(v)
    }
}

const PARAM_EXPECTING: &str = "a number such as 2000.0, or an exposed parameter such as \
     (name: \"muffle\", min: 200.0, max: 20000.0, initial: 2000.0)";

/// Hand-written rather than `#[serde(untagged)]`, for the reason `AssetRef` in
/// `bsengine-scene` is: untagged parses both forms correctly under `ron` 0.8,
/// but every failure — an unknown field, a missing `max`, a typo'd `name` —
/// collapses to "data did not match any variant", and an unknown field
/// alongside valid ones is *silently dropped*. In a mixer file that means a
/// parameter quietly not being exposed, which surfaces much later as "why does
/// my script do nothing".
impl<'de> Deserialize<'de> for Param {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(ParamVisitor)
    }
}

struct ParamVisitor;

#[derive(Deserialize)]
#[serde(field_identifier, rename_all = "lowercase")]
enum ParamField {
    Name,
    Min,
    Max,
    Initial,
}

impl<'de> serde::de::Visitor<'de> for ParamVisitor {
    type Value = Param;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(PARAM_EXPECTING)
    }

    fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Param, E> {
        Ok(Param::Fixed(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Param, E> {
        Ok(Param::Fixed(v as f64))
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Param, E> {
        Ok(Param::Fixed(v as f64))
    }

    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Param, A::Error> {
        use serde::de::Error as _;
        let mut name: Option<String> = None;
        let mut min: Option<f64> = None;
        let mut max: Option<f64> = None;
        let mut initial: Option<f64> = None;
        while let Some(key) = map.next_key::<ParamField>()? {
            match key {
                ParamField::Name => {
                    if name.is_some() {
                        return Err(A::Error::duplicate_field("name"));
                    }
                    name = Some(map.next_value()?);
                }
                ParamField::Min => {
                    if min.is_some() {
                        return Err(A::Error::duplicate_field("min"));
                    }
                    min = Some(map.next_value()?);
                }
                ParamField::Max => {
                    if max.is_some() {
                        return Err(A::Error::duplicate_field("max"));
                    }
                    max = Some(map.next_value()?);
                }
                ParamField::Initial => {
                    if initial.is_some() {
                        return Err(A::Error::duplicate_field("initial"));
                    }
                    initial = Some(map.next_value()?);
                }
            }
        }
        let name = name.ok_or_else(|| A::Error::missing_field("name"))?;
        let min = min.ok_or_else(|| A::Error::missing_field("min"))?;
        let max = max.ok_or_else(|| A::Error::missing_field("max"))?;
        // `initial` defaults to `min` rather than being required: a parameter
        // whose starting value is its floor is the common case (a muffle that
        // starts off), and requiring it adds noise to every declaration.
        let initial = initial.unwrap_or(min);
        Ok(Param::Exposed {
            name,
            min,
            max,
            initial,
        })
    }
}

/// One DSP effect in a bus's chain.
///
/// Every variant maps to a `kira` effect builder that already exists upstream —
/// this crate writes no DSP. Defaults mirror kira's own `Default` impls exactly
/// (read from its source), so omitting a field gives what kira would have given
/// rather than a number invented here.
///
/// Units are in the field names (`attack_ms`, `gain_db`) because RON has no
/// `Duration` literal, and a bare `attack: 0.01` invites a 1000x error that no
/// test would catch — both values parse.
///
/// kira's `volume_control` is deliberately absent: [`Bus::volume_db`] already
/// *is* a volume control on that track, and this crate's `lib.rs` records
/// removing a redundant second path rather than leaving two ways to do one
/// thing.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub enum BusEffect {
    /// Reverberation.
    Reverb {
        /// How much signal is fed back, 0.0 to 1.0.
        #[serde(default = "reverb_feedback")]
        feedback: Param,
        /// High-frequency damping, 0.0 to 1.0.
        #[serde(default = "reverb_damping")]
        damping: Param,
        /// Stereo spread, 0.0 to 1.0.
        #[serde(default = "one")]
        stereo_width: Param,
        /// Dry/wet balance: 0.0 dry, 1.0 wet.
        #[serde(default = "half")]
        mix: Param,
    },
    /// A resonant filter — the effect occlusion will later drive.
    Filter {
        /// Which frequencies to remove.
        #[serde(default)]
        mode: FilterMode,
        /// Corner frequency in hertz.
        #[serde(default = "filter_cutoff")]
        cutoff: Param,
        /// Resonance at the cutoff.
        #[serde(default = "zero")]
        resonance: Param,
        /// Dry/wet balance: 0.0 dry, 1.0 wet.
        #[serde(default = "one")]
        mix: Param,
    },
    /// Dynamic range compression.
    Compressor {
        /// Level above which gain starts being reduced, in decibels.
        #[serde(default = "zero")]
        threshold: Param,
        /// Compression ratio; 1.0 is no compression.
        #[serde(default = "one")]
        ratio: Param,
        /// How quickly compression engages, in milliseconds.
        #[serde(default = "compressor_attack_ms")]
        attack_ms: Param,
        /// How quickly compression releases, in milliseconds.
        #[serde(default = "compressor_release_ms")]
        release_ms: Param,
        /// Gain applied after compression, in decibels.
        #[serde(default = "zero")]
        makeup_gain_db: Param,
        /// Dry/wet balance: 0.0 dry, 1.0 wet.
        #[serde(default = "one")]
        mix: Param,
    },
    /// An echo.
    Delay {
        /// Time between repeats, in milliseconds.
        ///
        /// The one effect parameter that cannot be exposed to scripts: kira's
        /// `DelayBuilder::delay_time` takes a plain `Duration`, not a `Value`,
        /// so there is nothing for a modulator to drive. Stated here rather
        /// than left to be discovered.
        #[serde(default = "delay_ms")]
        delay_ms: f64,
        /// Level of each repeat relative to the last, in decibels.
        #[serde(default = "delay_feedback_db")]
        feedback_db: Param,
        /// Dry/wet balance: 0.0 dry, 1.0 wet.
        #[serde(default = "half")]
        mix: Param,
    },
    /// Waveshaping distortion.
    Distortion {
        /// Which waveshaping curve to use.
        #[serde(default)]
        kind: DistortionKind,
        /// Gain applied before the curve, in decibels.
        #[serde(default = "zero")]
        drive_db: Param,
        /// Dry/wet balance: 0.0 dry, 1.0 wet.
        #[serde(default = "one")]
        mix: Param,
    },
    /// A single parametric EQ band.
    ///
    /// The only effect with no defaults: kira's `EqFilterBuilder::new` takes
    /// all four positionally, so there is no upstream default to mirror and
    /// inventing one here would be exactly the drift the other variants avoid.
    EqFilter {
        /// Which band shape to apply.
        kind: EqFilterKind,
        /// Centre or corner frequency in hertz.
        frequency: Param,
        /// Gain applied to the band, in decibels.
        gain_db: Param,
        /// Bandwidth control; higher is narrower.
        q: Param,
    },
    /// Static stereo placement for the whole bus.
    Panning {
        /// -1.0 hard left, 0.0 centre, 1.0 hard right.
        #[serde(default = "zero")]
        panning: Param,
    },
}

/// Which frequencies a [`BusEffect::Filter`] removes. Mirrors kira's `FilterMode`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum FilterMode {
    /// Removes frequencies above the cutoff.
    #[default]
    LowPass,
    /// Removes frequencies above and below the cutoff.
    BandPass,
    /// Removes frequencies below the cutoff.
    HighPass,
    /// Removes frequencies around the cutoff.
    Notch,
}

/// Which waveshaping curve a [`BusEffect::Distortion`] uses. Mirrors kira's
/// `DistortionKind`.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum DistortionKind {
    /// Clamps hard at the signal limits.
    #[default]
    HardClip,
    /// Eases towards the limits instead of clamping.
    SoftClip,
}

/// Which band shape a [`BusEffect::EqFilter`] applies. Mirrors kira's
/// `EqFilterKind`.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub enum EqFilterKind {
    /// Adjusts frequencies around the given frequency.
    Bell,
    /// Adjusts the given frequency and everything below it.
    LowShelf,
    /// Adjusts the given frequency and everything above it.
    HighShelf,
}

// serde needs functions for non-zero defaults. Each value is kira's own,
// read from its `Default` impls rather than chosen here.
fn zero() -> Param {
    Param::Fixed(0.0)
}
fn one() -> Param {
    Param::Fixed(1.0)
}
fn half() -> Param {
    Param::Fixed(0.5)
}
fn reverb_feedback() -> Param {
    Param::Fixed(0.9)
}
fn reverb_damping() -> Param {
    Param::Fixed(0.1)
}
fn filter_cutoff() -> Param {
    Param::Fixed(1000.0)
}
fn compressor_attack_ms() -> Param {
    Param::Fixed(10.0)
}
fn compressor_release_ms() -> Param {
    Param::Fixed(100.0)
}
fn delay_ms() -> f64 {
    500.0
}
fn delay_feedback_db() -> Param {
    Param::Fixed(-6.0)
}

/// One declared mixer bus.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Bus {
    /// Name sounds use to select this bus. Case-sensitive, as in Godot.
    pub name: String,
    /// Bus this one feeds into, or `None` to feed Master directly.
    ///
    /// Master is implicit and never declared, so it can be neither renamed
    /// nor deleted.
    pub parent: Option<String>,
    /// Volume of this bus in decibels, matching `setSoundVolume`'s unit
    /// rather than introducing a second one.
    pub volume_db: f32,
    /// DSP effects applied to this bus, in order.
    ///
    /// `#[serde(default)]` so every layout written before effects existed
    /// keeps parsing with the field absent. Note this is plain serde through
    /// `ron::from_str`, not the scene/`bevy_reflect` path where `default` is
    /// inert — a distinction close enough to a known hazard here that
    /// `a_bus_with_no_effects_field_parses` asserts it rather than trusting it.
    #[serde(default)]
    pub effects: Vec<BusEffect>,
}

/// A parsed and validated `buses.ron`.
#[derive(Clone, Debug, Default)]
pub struct BusLayout {
    ordered: Vec<Bus>,
    problems: Vec<String>,
}

/// What `buses.ron` deserialises into before validation.
///
/// Renamed for serde because RON matches the struct name in the file, and the
/// authored file says `BusLayout(...)` — which is the name the format
/// documents and a user would write. Without the rename every layout fails to
/// parse with `ExpectedDifferentStructName`.
#[derive(Deserialize)]
#[serde(rename = "BusLayout")]
struct RawLayout {
    buses: Vec<Bus>,
}

impl BusLayout {
    /// Parses and validates a layout.
    ///
    /// Returns `Err` only for RON that does not parse. Everything else — an
    /// unknown parent, a duplicate name, a cycle — produces a *usable* layout
    /// plus an entry in [`problems`](Self::problems), because a broken mixer
    /// file should still make noise. Silence is the worst failure mode in
    /// audio: a game whose layout has a typo is far easier to debug loud and
    /// wrong than quiet and wrong.
    pub fn from_ron(src: &str) -> Result<Self, ron::error::SpannedError> {
        let raw: RawLayout = ron::from_str(src)?;
        Ok(Self::validate(raw.buses))
    }

    /// Applies the validation rules, in order, then sorts for creation.
    fn validate(buses: Vec<Bus>) -> Self {
        let mut problems = Vec::new();

        // 1. Duplicate names: the first declaration wins.
        let mut ordered: Vec<Bus> = Vec::new();
        for bus in buses {
            if ordered.iter().any(|b| b.name == bus.name) {
                problems.push(format!(
                    "[audio] duplicate bus '{}' — keeping the first declaration",
                    bus.name
                ));
                continue;
            }
            ordered.push(bus);
        }

        // 2. Unknown parent: attach to Master rather than dropping the bus.
        let names: Vec<String> = ordered.iter().map(|b| b.name.clone()).collect();
        for bus in &mut ordered {
            if let Some(parent) = bus.parent.clone() {
                if !names.contains(&parent) {
                    problems.push(format!(
                        "[audio] bus '{}' names parent '{}', which does not exist — \
                         attaching it to Master",
                        bus.name, parent
                    ));
                    bus.parent = None;
                }
            }
        }

        // 3. Cycles: break them by re-parenting every member to Master.
        //    This is not optional tidiness — creation is parents-first, so a
        //    cycle has no valid creation order and step 4 could not finish.
        let in_cycle: Vec<String> = ordered
            .iter()
            .filter(|b| Self::reaches_itself(&ordered, &b.name))
            .map(|b| b.name.clone())
            .collect();
        for name in &in_cycle {
            problems.push(format!(
                "[audio] bus '{name}' is part of a parent cycle — attaching it to Master"
            ));
            if let Some(bus) = ordered.iter_mut().find(|b| &b.name == name) {
                bus.parent = None;
            }
        }

        // 4. Topological order: every bus appears after its parent, because a
        //    child sub-track is created *on* its parent's handle.
        let mut sorted: Vec<Bus> = Vec::with_capacity(ordered.len());
        while sorted.len() < ordered.len() {
            let before = sorted.len();
            for bus in &ordered {
                if sorted.iter().any(|b| b.name == bus.name) {
                    continue;
                }
                let parent_ready = match &bus.parent {
                    None => true,
                    Some(p) => sorted.iter().any(|b| &b.name == p),
                };
                if parent_ready {
                    sorted.push(bus.clone());
                }
            }
            if sorted.len() == before {
                // Unreachable: step 3 broke every cycle, so each remaining bus
                // has either no parent or one already placed. Bailing rather
                // than spinning forever if that ever stops holding.
                break;
            }
        }

        Self {
            ordered: sorted,
            problems,
        }
    }

    /// Whether following `name`'s parent chain arrives back at `name`.
    fn reaches_itself(buses: &[Bus], name: &str) -> bool {
        let mut seen: Vec<String> = Vec::new();
        let mut current = name.to_string();
        loop {
            let Some(bus) = buses.iter().find(|b| b.name == current) else {
                return false;
            };
            let Some(parent) = bus.parent.clone() else {
                return false;
            };
            if parent == name {
                return true;
            }
            if seen.contains(&parent) {
                // A cycle further up that does not include `name` itself.
                return false;
            }
            seen.push(parent.clone());
            current = parent;
        }
    }

    /// Buses in the order they must be created: every bus after its parent.
    pub fn creation_order(&self) -> &[Bus] {
        &self.ordered
    }

    /// Looks a bus up by name.
    pub fn get(&self, name: &str) -> Option<&Bus> {
        self.ordered.iter().find(|b| b.name == name)
    }

    /// Descriptions of everything wrong with the authored file and what was
    /// done about it. Empty for a clean layout.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout(src: &str) -> BusLayout {
        BusLayout::from_ron(src).expect("fixture should parse")
    }

    #[test]
    fn buses_are_created_parents_before_children() {
        // `ui` is a child of `sfx`, so `sfx`'s kira track must exist first --
        // a child track is created *on* its parent's handle. Listed in the
        // wrong order on purpose.
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "ui",    parent: Some("sfx"), volume_db: -3.0),
                Bus(name: "sfx",   parent: None,        volume_db: -6.0),
            ])"#,
        );
        let order: Vec<&str> = l.creation_order().iter().map(|b| b.name.as_str()).collect();
        assert_eq!(
            order,
            vec!["sfx", "ui"],
            "a child must not be created before its parent, whatever order the file lists"
        );
    }

    #[test]
    fn a_declared_parent_is_preserved() {
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "sfx", parent: None,        volume_db: -6.0),
                Bus(name: "ui",  parent: Some("sfx"), volume_db: -3.0),
            ])"#,
        );
        assert_eq!(l.get("ui").unwrap().parent.as_deref(), Some("sfx"));
        assert_eq!(
            l.get("sfx").unwrap().parent,
            None,
            "a top-level bus hangs off Master"
        );
    }

    #[test]
    fn distinct_volumes_survive_parsing() {
        // Different values on purpose: equal volumes would let a bug that
        // returns the wrong bus's volume read as correct.
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "music", parent: None, volume_db:  0.0),
                Bus(name: "sfx",   parent: None, volume_db: -6.0),
            ])"#,
        );
        assert_eq!(l.get("music").unwrap().volume_db, 0.0);
        assert_eq!(l.get("sfx").unwrap().volume_db, -6.0);
    }

    #[test]
    fn an_unknown_parent_falls_back_to_master() {
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "ui", parent: Some("nope"), volume_db: -3.0),
            ])"#,
        );
        assert_eq!(
            l.get("ui").unwrap().parent,
            None,
            "a bus naming a parent that does not exist attaches to Master rather than vanishing"
        );
        assert!(
            l.problems().iter().any(|p| p.contains("nope")),
            "the fallback must be reported, not silent; problems were {:?}",
            l.problems()
        );
    }

    #[test]
    fn a_duplicate_name_keeps_the_first_and_reports_it() {
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "sfx", parent: None, volume_db: -6.0),
                Bus(name: "sfx", parent: None, volume_db: -12.0),
            ])"#,
        );
        assert_eq!(
            l.creation_order().len(),
            1,
            "two buses named sfx must collapse to one"
        );
        assert_eq!(
            l.get("sfx").unwrap().volume_db,
            -6.0,
            "the first declaration wins, so the second's -12 must not be what survives"
        );
        assert!(l.problems().iter().any(|p| p.contains("sfx")));
    }

    #[test]
    fn a_cycle_is_broken_rather_than_hanging() {
        // Creation is parents-first, so a cycle has no valid order at all.
        let l = layout(
            r#"BusLayout(buses: [
                Bus(name: "a", parent: Some("b"), volume_db: 0.0),
                Bus(name: "b", parent: Some("a"), volume_db: 0.0),
            ])"#,
        );
        assert_eq!(
            l.creation_order().len(),
            2,
            "both buses must survive, just re-parented"
        );
        assert_eq!(l.get("a").unwrap().parent, None);
        assert_eq!(l.get("b").unwrap().parent, None);
        assert!(
            l.problems()
                .iter()
                .any(|p| p.to_lowercase().contains("cycle")),
            "problems were {:?}",
            l.problems()
        );
    }

    #[test]
    fn a_self_parent_is_a_cycle_too() {
        let l = layout(r#"BusLayout(buses: [Bus(name: "a", parent: Some("a"), volume_db: 0.0)])"#);
        assert_eq!(l.get("a").unwrap().parent, None);
        assert!(l
            .problems()
            .iter()
            .any(|p| p.to_lowercase().contains("cycle")));
    }

    #[test]
    fn an_empty_layout_is_valid_and_has_no_problems() {
        let l = layout("BusLayout(buses: [])");
        assert!(l.creation_order().is_empty());
        assert!(l.problems().is_empty());
    }

    /// A layout written before effects existed must still parse.
    ///
    /// Asserted rather than reasoned about: this repo has a recorded hazard
    /// that `#[serde(default)]` is inert on the scene/`bevy_reflect` path.
    /// `BusLayout` is plain serde through `ron::from_str`, where it works --
    /// but the distinction is close enough to be worth a test rather than a
    /// belief.
    #[test]
    fn a_bus_with_no_effects_field_parses() {
        let l = layout(r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: -6.0)])"#);
        assert!(
            l.get("sfx").unwrap().effects.is_empty(),
            "an absent effects field must mean an empty chain, not a parse error"
        );
        assert!(l.problems().is_empty());
    }

    #[test]
    fn an_effect_chain_keeps_its_authored_order() {
        // Order is the contract: a chain is applied front to back, so Filter
        // then Reverb is not the same sound as Reverb then Filter.
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: 0.0, effects: [
                Filter(mode: HighPass, cutoff: 800.0, resonance: 0.25, mix: 0.75),
                Reverb(feedback: 0.5, damping: 0.25, stereo_width: 0.125, mix: 0.0625),
            ])])"#,
        );
        let fx = &l.get("sfx").unwrap().effects;
        assert_eq!(fx.len(), 2);
        assert!(
            matches!(fx[0], BusEffect::Filter { .. }),
            "Filter was authored first; got {:?}",
            fx[0]
        );
        assert!(
            matches!(fx[1], BusEffect::Reverb { .. }),
            "Reverb was authored second; got {:?}",
            fx[1]
        );
    }

    /// Every field a *different* value, so a mapping that reads the wrong one
    /// cannot read as correct. This is the assertion the parameter-swap
    /// mutation exists to trip.
    #[test]
    fn every_parameter_lands_in_its_own_field() {
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Reverb(feedback: 0.5, damping: 0.25, stereo_width: 0.125, mix: 0.0625),
            ])])"#,
        );
        match &l.get("b").unwrap().effects[0] {
            BusEffect::Reverb {
                feedback,
                damping,
                stereo_width,
                mix,
            } => {
                assert_eq!(*feedback, Param::Fixed(0.5));
                assert_eq!(*damping, Param::Fixed(0.25));
                assert_eq!(*stereo_width, Param::Fixed(0.125));
                assert_eq!(*mix, Param::Fixed(0.0625));
            }
            other => panic!("expected Reverb, got {other:?}"),
        }
    }

    #[test]
    fn omitted_effect_parameters_take_kiras_defaults() {
        // Not values chosen here -- kira's own, read from its Default impls.
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Reverb(),
                Delay(),
            ])])"#,
        );
        let fx = &l.get("b").unwrap().effects;
        match &fx[0] {
            BusEffect::Reverb {
                feedback,
                damping,
                stereo_width,
                mix,
            } => {
                assert_eq!(
                    (
                        feedback.initial(),
                        damping.initial(),
                        stereo_width.initial(),
                        mix.initial()
                    ),
                    (0.9, 0.1, 1.0, 0.5)
                );
            }
            other => panic!("expected Reverb, got {other:?}"),
        }
        match &fx[1] {
            BusEffect::Delay {
                delay_ms,
                feedback_db,
                mix,
            } => {
                assert_eq!(
                    (*delay_ms, feedback_db.initial(), mix.initial()),
                    (500.0, -6.0, 0.5)
                );
            }
            other => panic!("expected Delay, got {other:?}"),
        }
    }

    #[test]
    fn a_parameter_can_be_written_as_a_bare_number_or_exposed() {
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Filter(
                    cutoff: (name: "muffle", min: 200.0, max: 20000.0, initial: 2000.0),
                    resonance: 0.25,
                ),
            ])])"#,
        );
        match &l.get("b").unwrap().effects[0] {
            BusEffect::Filter {
                cutoff, resonance, ..
            } => {
                assert_eq!(cutoff.exposed_name(), Some("muffle"));
                assert_eq!(cutoff.range(), Some((200.0, 20000.0)));
                assert_eq!(cutoff.initial(), 2000.0);
                assert_eq!(
                    *resonance,
                    Param::Fixed(0.25),
                    "a bare number beside an exposed one must still be fixed"
                );
                assert_eq!(
                    resonance.exposed_name(),
                    None,
                    "a fixed parameter has no name for a script to reach"
                );
            }
            other => panic!("expected Filter, got {other:?}"),
        }
    }

    #[test]
    fn an_exposed_parameter_defaults_its_initial_to_min() {
        // The common case is a parameter that starts at its floor -- a muffle
        // that starts off -- so requiring `initial` would be noise.
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Filter(cutoff: (name: "m", min: 500.0, max: 20000.0)),
            ])])"#,
        );
        match &l.get("b").unwrap().effects[0] {
            BusEffect::Filter { cutoff, .. } => assert_eq!(cutoff.initial(), 500.0),
            other => panic!("expected Filter, got {other:?}"),
        }
    }

    /// A malformed exposure must name what it accepts.
    ///
    /// This is why `Param` hand-writes `Deserialize` instead of using
    /// `#[serde(untagged)]`: untagged parses both forms correctly, but a
    /// missing field collapses to "data did not match any variant" and an
    /// unknown field beside valid ones is *silently dropped* -- which in a
    /// mixer file means a parameter quietly not being exposed.
    #[test]
    fn a_malformed_exposed_parameter_says_what_it_expected() {
        let err = BusLayout::from_ron(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Filter(cutoff: (name: "m", min: 200.0)),
            ])])"#,
        )
        .expect_err("a missing `max` must be an error");
        let text = err.to_string();
        assert!(
            text.contains("max"),
            "the error must name the missing field; got {text}"
        );
    }

    #[test]
    fn an_unknown_field_in_an_exposure_is_reported_not_dropped() {
        let err = BusLayout::from_ron(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Filter(cutoff: (name: "m", min: 200.0, max: 2000.0, wobble: 3.0)),
            ])])"#,
        )
        .expect_err("an unknown field must be an error, not silently dropped");
        let text = err.to_string();
        assert!(
            text.contains("wobble") || text.to_lowercase().contains("unknown"),
            "the error must point at the unknown field; got {text}"
        );
    }

    #[test]
    fn an_unknown_effect_name_is_a_parse_error() {
        // Loud rather than silently dropped: a typo'd effect that vanished
        // would leave an author wondering why their bus sounds dry.
        assert!(BusLayout::from_ron(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                Flanger(depth: 1.0),
            ])])"#
        )
        .is_err());
    }

    #[test]
    fn eq_filter_requires_all_four_parameters() {
        // The one effect with no defaults, because kira's own constructor
        // takes all four positionally.
        let l = layout(
            r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                EqFilter(kind: LowShelf, frequency: 220.0, gain_db: -4.0, q: 0.7),
            ])])"#,
        );
        match &l.get("b").unwrap().effects[0] {
            BusEffect::EqFilter {
                kind,
                frequency,
                gain_db,
                q,
            } => {
                assert_eq!(*kind, EqFilterKind::LowShelf);
                assert_eq!(frequency.initial(), 220.0);
                assert_eq!(gain_db.initial(), -4.0);
                assert_eq!(q.initial(), 0.7);
            }
            other => panic!("expected EqFilter, got {other:?}"),
        }
        assert!(
            BusLayout::from_ron(
                r#"BusLayout(buses: [Bus(name: "b", parent: None, volume_db: 0.0, effects: [
                    EqFilter(kind: Bell, frequency: 220.0),
                ])])"#
            )
            .is_err(),
            "a missing required EqFilter parameter must be an error, not a silent default"
        );
    }

    #[test]
    fn malformed_ron_is_an_error_not_a_panic() {
        assert!(BusLayout::from_ron("this is not ron").is_err());
    }
}
