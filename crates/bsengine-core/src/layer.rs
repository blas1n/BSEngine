use bevy_ecs::prelude::Component;

/// Bitmask layer membership for an entity.
/// Cameras and physics systems filter entities by layer bits.
/// An entity is visible/collidable with another if `(a.0 & b.0) != 0`.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Layer(pub u32);

impl Default for Layer {
    fn default() -> Self {
        Self::ALL
    }
}

impl Layer {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(u32::MAX);
    pub const LAYER_0: Self = Self(1 << 0);
    pub const LAYER_1: Self = Self(1 << 1);
    pub const LAYER_2: Self = Self(1 << 2);
    pub const LAYER_3: Self = Self(1 << 3);
    pub const LAYER_4: Self = Self(1 << 4);
    pub const LAYER_5: Self = Self(1 << 5);
    pub const LAYER_6: Self = Self(1 << 6);
    pub const LAYER_7: Self = Self(1 << 7);

    pub fn new(bits: u32) -> Self {
        Self(bits)
    }

    pub fn bits(self) -> u32 {
        self.0
    }

    /// Returns true if this layer and `other` share at least one bit.
    pub fn overlaps(self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Returns true if all bits of `mask` are set.
    pub fn contains(self, mask: Self) -> bool {
        (self.0 & mask.0) == mask.0
    }

    pub fn with(self, mask: Self) -> Self {
        Self(self.0 | mask.0)
    }

    pub fn without(self, mask: Self) -> Self {
        Self(self.0 & !mask.0)
    }
}

impl std::ops::BitOr for Layer {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitAnd for Layer {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl std::ops::Not for Layer {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_all() {
        assert_eq!(Layer::default(), Layer::ALL);
    }

    #[test]
    fn none_overlaps_nothing() {
        assert!(!Layer::NONE.overlaps(Layer::LAYER_0));
        assert!(!Layer::NONE.overlaps(Layer::ALL));
    }

    #[test]
    fn all_overlaps_any_nonzero() {
        assert!(Layer::ALL.overlaps(Layer::LAYER_0));
        assert!(Layer::ALL.overlaps(Layer::LAYER_7));
    }

    #[test]
    fn overlaps_shared_bit() {
        let a = Layer::LAYER_0 | Layer::LAYER_2;
        let b = Layer::LAYER_1 | Layer::LAYER_2;
        assert!(a.overlaps(b));
    }

    #[test]
    fn no_overlap_distinct_bits() {
        assert!(!Layer::LAYER_0.overlaps(Layer::LAYER_1));
    }

    #[test]
    fn contains_checks_all_bits() {
        let a = Layer::LAYER_0 | Layer::LAYER_1 | Layer::LAYER_2;
        assert!(a.contains(Layer::LAYER_0 | Layer::LAYER_1));
        assert!(!a.contains(Layer::LAYER_3));
    }

    #[test]
    fn with_adds_bit() {
        let a = Layer::LAYER_0.with(Layer::LAYER_1);
        assert!(a.contains(Layer::LAYER_0));
        assert!(a.contains(Layer::LAYER_1));
    }

    #[test]
    fn without_removes_bit() {
        let a = (Layer::LAYER_0 | Layer::LAYER_1).without(Layer::LAYER_0);
        assert!(!a.contains(Layer::LAYER_0));
        assert!(a.contains(Layer::LAYER_1));
    }

    #[test]
    fn bitwise_ops() {
        assert_eq!(Layer::LAYER_0 | Layer::LAYER_1, Layer::new(0b11));
        assert_eq!(Layer::new(0b111) & Layer::new(0b101), Layer::new(0b101));
        assert_eq!(!Layer::NONE, Layer::ALL);
    }
}
