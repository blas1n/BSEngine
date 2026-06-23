use std::marker::PhantomData;

pub struct Handle<T> {
    id: u64,
    _phantom: PhantomData<T>,
}

impl<T> Handle<T> {
    pub fn new(id: u64) -> Self {
        Self { id, _phantom: PhantomData }
    }
    pub fn id(&self) -> u64 { self.id }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}
impl<T> Eq for Handle<T> {}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self { Self::new(self.id) }
}

impl<T> std::fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Handle({})", self.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyAsset(Vec<u8>);

    #[test]
    fn handle_has_typed_id() {
        let h1: Handle<DummyAsset> = Handle::new(1);
        let h2: Handle<DummyAsset> = Handle::new(2);
        assert_ne!(h1.id(), h2.id());
    }

    #[test]
    fn handle_equality() {
        let h1: Handle<DummyAsset> = Handle::new(42);
        let h2: Handle<DummyAsset> = Handle::new(42);
        assert_eq!(h1, h2);
    }
}
