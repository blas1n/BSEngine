use bevy_ecs::prelude::{Component, Entity};
use glam::Vec3;

/// The rider's seat offset from the mount entity's origin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeatOffset {
    pub position: Vec3,
    /// Seat index — a mount can have multiple seats (e.g. driver + passenger).
    pub index: u8,
}

impl SeatOffset {
    pub fn new(position: Vec3) -> Self {
        Self { position, index: 0 }
    }

    pub fn with_index(mut self, index: u8) -> Self {
        self.index = index;
        self
    }
}

/// Mountable state — attach to the vehicle/creature entity.
/// The mount system reads `riders` to position passengers and transfer input.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Mount {
    /// Current rider entities and the seat they occupy.
    pub riders: Vec<(Entity, u8)>,
    /// Maximum simultaneous riders.
    pub max_riders: u8,
    /// Seat definitions in local space.
    pub seats: Vec<SeatOffset>,
    /// Speed multiplier applied to the mount's movement.
    pub speed_scale: f32,
    /// Whether riders dismount when the mount takes damage above this threshold.
    pub forced_dismount_damage: Option<f32>,
    pub enabled: bool,
}

impl Mount {
    pub fn new(max_riders: u8) -> Self {
        Self {
            riders: Vec::new(),
            max_riders: max_riders.max(1),
            seats: Vec::new(),
            speed_scale: 1.0,
            forced_dismount_damage: None,
            enabled: true,
        }
    }

    pub fn with_seat(mut self, seat: SeatOffset) -> Self {
        self.seats.push(seat);
        self
    }

    pub fn with_speed_scale(mut self, scale: f32) -> Self {
        self.speed_scale = scale.max(0.0);
        self
    }

    pub fn with_forced_dismount(mut self, damage_threshold: f32) -> Self {
        self.forced_dismount_damage = Some(damage_threshold.max(0.0));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn is_full(&self) -> bool {
        self.riders.len() >= self.max_riders as usize
    }

    pub fn rider_count(&self) -> usize {
        self.riders.len()
    }

    /// Add a rider to the given seat. Returns `false` if already full or seat taken.
    pub fn board(&mut self, rider: Entity, seat_index: u8) -> bool {
        if !self.enabled || self.is_full() {
            return false;
        }
        if self.riders.iter().any(|(_, s)| *s == seat_index) {
            return false;
        }
        self.riders.push((rider, seat_index));
        true
    }

    /// Remove a rider. Returns `true` if they were present.
    pub fn dismount(&mut self, rider: Entity) -> bool {
        let before = self.riders.len();
        self.riders.retain(|(e, _)| *e != rider);
        self.riders.len() < before
    }

    pub fn seat_offset(&self, seat_index: u8) -> Option<Vec3> {
        self.seats
            .iter()
            .find(|s| s.index == seat_index)
            .map(|s| s.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn entity() -> Entity {
        World::new().spawn_empty().id()
    }

    #[test]
    fn mount_board_and_dismount() {
        let mut m = Mount::new(2);
        let rider = entity();
        assert!(m.board(rider, 0));
        assert_eq!(m.rider_count(), 1);
        assert!(m.dismount(rider));
        assert_eq!(m.rider_count(), 0);
    }

    #[test]
    fn mount_full_rejects() {
        let mut m = Mount::new(1);
        let a = entity();
        let b = entity();
        assert!(m.board(a, 0));
        assert!(!m.board(b, 1));
    }

    #[test]
    fn mount_seat_taken_rejects() {
        let mut m = Mount::new(2);
        let a = entity();
        let b = entity();
        assert!(m.board(a, 0));
        assert!(!m.board(b, 0));
    }

    #[test]
    fn mount_seat_offset() {
        let m = Mount::new(1).with_seat(SeatOffset::new(Vec3::new(0.0, 1.5, 0.0)));
        assert_eq!(m.seat_offset(0), Some(Vec3::new(0.0, 1.5, 0.0)));
        assert_eq!(m.seat_offset(1), None);
    }

    #[test]
    fn mount_disabled_rejects_boarding() {
        let mut m = Mount::new(2).disabled();
        assert!(!m.board(entity(), 0));
    }
}
