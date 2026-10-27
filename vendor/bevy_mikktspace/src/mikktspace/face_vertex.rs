/// New-type index for a particular vertex on a particular face.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct FaceVertex(usize);

impl FaceVertex {
    /// Construct a new [`FaceVertex`] for the given `face` and `vertex`.
    ///
    /// # Panics
    ///
    /// Panics if `vertex` is larger than 3, or if `face` has it's 2 most significant
    /// bits set.
    pub(crate) const fn new(face: usize, vertex: u8) -> Self {
        debug_assert!(vertex <= 3);
        debug_assert!(face <= (usize::MAX >> 2));

        let value = Self(face << 2 | ((vertex & 0x3) as usize));

        debug_assert!(value.face() == face);
        debug_assert!(value.vertex() == vertex);

        value
    }

    /// Get's the `face` of this [`FaceVertex`].
    pub(crate) const fn face(&self) -> usize {
        self.0 >> 2
    }

    /// Get's the `vertex` of this [`FaceVertex`].
    pub(crate) const fn vertex(&self) -> u8 {
        (self.0 & 0x3) as u8
    }
}
