//! The authored bus layout: parsing `buses.ron` and making sense of it.
//!
//! Deliberately free of `kira`, of the audio device, and of ECS, so that
//! validation and creation ordering are testable as plain data on any
//! machine — including the Windows CI runners, which have no audio device at
//! all and where anything asserted through a `None` audio manager passes
//! without exercising a thing.

use serde::Deserialize;

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

    #[test]
    fn malformed_ron_is_an_error_not_a_panic() {
        assert!(BusLayout::from_ron("this is not ron").is_err());
    }
}
