use bevy_ecs::prelude::Component;
use std::collections::HashMap;

/// A simple key-value save slot attached to a player entity.
/// Values are stored as raw bytes; higher-level systems serialize/deserialize them.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SaveData {
    pub slot: u32,
    /// Named fields — key = field name, value = raw serialised bytes.
    pub fields: HashMap<String, Vec<u8>>,
    /// Timestamp (Unix seconds) of the last write, or 0 if never written.
    pub last_saved_at: u64,
    /// Whether there are unsaved changes since the last flush.
    pub dirty: bool,
    pub enabled: bool,
}

impl SaveData {
    pub fn new(slot: u32) -> Self {
        Self {
            slot,
            fields: HashMap::new(),
            last_saved_at: 0,
            dirty: false,
            enabled: true,
        }
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Write a field. Marks dirty.
    pub fn set(&mut self, key: impl Into<String>, value: Vec<u8>) {
        self.fields.insert(key.into(), value);
        self.dirty = true;
    }

    /// Read a field.
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.fields.get(key).map(|v| v.as_slice())
    }

    /// Remove a field. Marks dirty.
    pub fn remove(&mut self, key: &str) -> bool {
        let removed = self.fields.remove(key).is_some();
        if removed {
            self.dirty = true;
        }
        removed
    }

    /// Mark as saved at the given Unix timestamp, clears dirty flag.
    pub fn mark_saved(&mut self, timestamp: u64) {
        self.last_saved_at = timestamp;
        self.dirty = false;
    }

    pub fn field_count(&self) -> usize {
        self.fields.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_data_set_get() {
        let mut sd = SaveData::new(0);
        sd.set("hp", vec![100]);
        assert_eq!(sd.get("hp"), Some([100u8].as_slice()));
    }

    #[test]
    fn save_data_dirty_on_write() {
        let mut sd = SaveData::new(0);
        assert!(!sd.dirty);
        sd.set("score", vec![0]);
        assert!(sd.dirty);
    }

    #[test]
    fn save_data_mark_saved_clears_dirty() {
        let mut sd = SaveData::new(0);
        sd.set("x", vec![1]);
        sd.mark_saved(1_700_000_000);
        assert!(!sd.dirty);
        assert_eq!(sd.last_saved_at, 1_700_000_000);
    }

    #[test]
    fn save_data_remove() {
        let mut sd = SaveData::new(0);
        sd.set("key", vec![7]);
        sd.mark_saved(0);
        assert!(sd.remove("key"));
        assert!(sd.dirty);
        assert!(sd.get("key").is_none());
    }

    #[test]
    fn save_data_field_count() {
        let mut sd = SaveData::new(1);
        sd.set("a", vec![]);
        sd.set("b", vec![]);
        assert_eq!(sd.field_count(), 2);
    }
}
