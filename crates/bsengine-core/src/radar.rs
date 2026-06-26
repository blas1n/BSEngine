use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// What the radar is set to detect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarFilter {
    /// Detects all entities regardless of layer.
    All,
    /// Detects only entities whose layer mask overlaps this mask.
    LayerMask(u32),
    /// Detects only entities on a specific faction (tag-based matching done externally).
    FactionMask(u32),
}

/// A tracked contact entry in the radar.
#[derive(Debug, Clone, PartialEq)]
pub struct RadarContact {
    pub entity: Entity,
    /// Position of the contact at last update.
    pub position: Vec3,
    /// Approximate distance to the owning entity.
    pub distance: f32,
}

/// Proximity radar — maintains a sorted list of nearby entity contacts within `range`.
///
/// The detection system updates `contacts` each `scan_interval` seconds by checking
/// all entities against this component's `range` and `filter`. The consumer can then
/// query `contacts` for targeting, minimap drawing, etc.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Radar {
    /// Detection radius in world units.
    pub range: f32,
    /// Contacts currently inside range, sorted by distance (nearest first).
    pub contacts: Vec<RadarContact>,
    /// How often (in seconds) the system refreshes `contacts`.
    pub scan_interval: f32,
    /// Time accumulated since last scan.
    pub scan_timer: f32,
    pub filter: RadarFilter,
    pub enabled: bool,
}

impl Radar {
    pub fn new(range: f32) -> Self {
        Self {
            range: range.max(0.0),
            contacts: Vec::new(),
            scan_interval: 0.25,
            scan_timer: 0.0,
            filter: RadarFilter::All,
            enabled: true,
        }
    }

    pub fn with_scan_interval(mut self, seconds: f32) -> Self {
        self.scan_interval = seconds.max(0.0);
        self
    }

    pub fn with_filter(mut self, filter: RadarFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance scan timer. Returns `true` when a new scan should be triggered.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        self.scan_timer += dt;
        if self.scan_timer >= self.scan_interval {
            self.scan_timer = 0.0;
            return true;
        }
        false
    }

    /// Update the contact list from a slice of `(entity, position)` pairs.
    /// Filters by range and sorts nearest-first.
    pub fn update_contacts(&mut self, own_pos: Vec3, candidates: &[(Entity, Vec3)]) {
        self.contacts.clear();
        for &(entity, pos) in candidates {
            let dist = own_pos.distance(pos);
            if dist <= self.range {
                self.contacts.push(RadarContact {
                    entity,
                    position: pos,
                    distance: dist,
                });
            }
        }
        self.contacts
            .sort_unstable_by(|a, b| a.distance.partial_cmp(&b.distance).unwrap());
    }

    /// Nearest contact, if any.
    pub fn nearest(&self) -> Option<&RadarContact> {
        self.contacts.first()
    }

    pub fn has_contacts(&self) -> bool {
        !self.contacts.is_empty()
    }
}

impl Default for Radar {
    fn default() -> Self {
        Self::new(20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::prelude::World;

    fn entities(n: usize) -> Vec<Entity> {
        let mut w = World::new();
        (0..n).map(|_| w.spawn_empty().id()).collect()
    }

    #[test]
    fn radar_detects_in_range() {
        let es = entities(2);
        let mut r = Radar::new(10.0);
        r.update_contacts(
            Vec3::ZERO,
            &[(es[0], Vec3::X * 5.0), (es[1], Vec3::X * 15.0)],
        );
        assert_eq!(r.contacts.len(), 1);
        assert_eq!(r.contacts[0].entity, es[0]);
    }

    #[test]
    fn radar_contacts_sorted_nearest_first() {
        let es = entities(2);
        let mut r = Radar::new(20.0);
        r.update_contacts(
            Vec3::ZERO,
            &[(es[0], Vec3::X * 10.0), (es[1], Vec3::X * 3.0)],
        );
        assert_eq!(r.nearest().unwrap().entity, es[1]);
    }

    #[test]
    fn radar_tick_triggers_scan() {
        let mut r = Radar::new(10.0).with_scan_interval(0.5);
        assert!(!r.tick(0.3));
        assert!(r.tick(0.3));
    }

    #[test]
    fn radar_disabled_no_scan() {
        let mut r = Radar::new(10.0).disabled();
        assert!(!r.tick(10.0));
    }

    #[test]
    fn radar_clear_on_update() {
        let es = entities(1);
        let mut r = Radar::new(10.0);
        r.update_contacts(Vec3::ZERO, &[(es[0], Vec3::X * 2.0)]);
        assert!(r.has_contacts());
        r.update_contacts(Vec3::ZERO, &[]);
        assert!(!r.has_contacts());
    }
}
