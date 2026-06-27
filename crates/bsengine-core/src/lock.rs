use bevy_ecs::prelude::Component;

/// State of a lockable entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    /// Entity is locked; interaction is blocked.
    Locked,
    /// Entity is unlocked; interaction proceeds normally.
    Unlocked,
    /// Lock is broken or jammed; cannot be opened or locked again without repair.
    Jammed,
}

/// Lockable entity component — doors, chests, containers, mechanisms.
///
/// The interaction system calls `try_unlock(key_id)` when a player with a
/// matching key interacts with the entity. The pick-locking system calls
/// `pick()`. When `max_attempts > 0`, exceeding it jams the lock.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Lock {
    pub state: LockState,
    /// Difficulty level (0 = trivial, higher = harder to pick).
    pub lock_level: u32,
    /// Key identifier that opens this lock (0 = no key required when unlocked via key).
    pub key_id: u32,
    /// Whether this lock can be picked without the matching key.
    pub is_pickable: bool,
    /// Number of failed unlock/pick attempts.
    pub attempts: u32,
    /// Maximum attempts before the lock jams (0 = unlimited).
    pub max_attempts: u32,
    /// True on the frame this lock transitions to Unlocked.
    pub just_unlocked: bool,
    /// True on the frame this lock transitions to Locked.
    pub just_locked: bool,
    pub enabled: bool,
}

impl Lock {
    pub fn new(key_id: u32, lock_level: u32) -> Self {
        Self {
            state: LockState::Locked,
            lock_level,
            key_id,
            is_pickable: true,
            attempts: 0,
            max_attempts: 0,
            just_unlocked: false,
            just_locked: false,
            enabled: true,
        }
    }

    pub fn unlocked(key_id: u32) -> Self {
        let mut l = Self::new(key_id, 0);
        l.state = LockState::Unlocked;
        l
    }

    pub fn with_max_attempts(mut self, max: u32) -> Self {
        self.max_attempts = max;
        self
    }

    pub fn not_pickable(mut self) -> Self {
        self.is_pickable = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Attempt to unlock with a key. Returns true if succeeded.
    pub fn try_unlock(&mut self, key_id: u32) -> bool {
        if !self.enabled || self.state != LockState::Locked {
            return false;
        }
        if key_id == self.key_id {
            self.state = LockState::Unlocked;
            self.just_unlocked = true;
            return true;
        }
        self.record_failed_attempt();
        false
    }

    /// Attempt to pick the lock. Returns true if succeeded.
    pub fn pick(&mut self) -> bool {
        if !self.enabled || !self.is_pickable || self.state != LockState::Locked {
            return false;
        }
        self.state = LockState::Unlocked;
        self.just_unlocked = true;
        true
    }

    /// Lock the entity again (e.g., player leaves range, reset after use).
    pub fn lock(&mut self) {
        if self.state == LockState::Unlocked {
            self.state = LockState::Locked;
            self.just_locked = true;
        }
    }

    /// Clear one-frame event flags. Call at the start of each frame.
    pub fn tick(&mut self) {
        self.just_unlocked = false;
        self.just_locked = false;
    }

    pub fn is_locked(&self) -> bool {
        self.state == LockState::Locked
    }

    pub fn is_jammed(&self) -> bool {
        self.state == LockState::Jammed
    }

    fn record_failed_attempt(&mut self) {
        self.attempts += 1;
        if self.max_attempts > 0 && self.attempts >= self.max_attempts {
            self.state = LockState::Jammed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_key_unlocks() {
        let mut l = Lock::new(42, 1);
        assert!(l.try_unlock(42));
        assert_eq!(l.state, LockState::Unlocked);
        assert!(l.just_unlocked);
    }

    #[test]
    fn wrong_key_fails() {
        let mut l = Lock::new(42, 1);
        assert!(!l.try_unlock(99));
        assert_eq!(l.state, LockState::Locked);
    }

    #[test]
    fn pick_unlocks_when_pickable() {
        let mut l = Lock::new(42, 1);
        assert!(l.pick());
        assert_eq!(l.state, LockState::Unlocked);
    }

    #[test]
    fn not_pickable_blocks_pick() {
        let mut l = Lock::new(42, 1).not_pickable();
        assert!(!l.pick());
        assert_eq!(l.state, LockState::Locked);
    }

    #[test]
    fn max_attempts_jams_lock() {
        let mut l = Lock::new(42, 1).with_max_attempts(3);
        l.try_unlock(0); // wrong key
        l.try_unlock(0);
        l.try_unlock(0);
        assert!(l.is_jammed());
    }

    #[test]
    fn lock_and_unlock_cycle() {
        let mut l = Lock::new(1, 0);
        l.try_unlock(1);
        l.tick();
        assert_eq!(l.state, LockState::Unlocked);
        l.lock();
        assert!(l.just_locked);
        assert!(l.is_locked());
    }
}
