//! Where a game's save files live.

use std::path::{Path, PathBuf};

use bevy_ecs::prelude::Resource;

use crate::project_dir::{project_path, ProjectPathError};

/// The directory `Bsengine.save` / `Bsengine.load` paths are relative to.
///
/// Until this existed a save path was relative to the process's working
/// directory: a packaged game's install directory may be read-only and is
/// shared by every user of the machine, and mini-arena wrote its checkpoint
/// into whichever checkout it was run from. All three reference engines put
/// saves under a per-user directory -- Unity's `Application.persistentDataPath`,
/// Unreal's `Saved/SaveGames` under `%LOCALAPPDATA%\<Game>`, Godot's `user://`
/// -- and so does this: the per-user directory the log and crash reports
/// already use ([`crate::crash::user_data_dir`]), in a `saves` folder.
///
/// A folder of its own, as Unreal does it (Unity and Godot hand the script the
/// root), because that directory also holds `logs/` and `crashes/`: rooted
/// there, `Bsengine.save("logs/game.log")` would overwrite the running game's
/// own log.
///
/// No resource, no saves. A browser build has no file system (Unity and Godot
/// keep web saves in IndexedDB; this engine does not yet), and an app that
/// never said where saves go refuses them rather than guessing -- the guess
/// used to be the working directory, which is how a replay run from the
/// repository root left `games/mini-arena/checkpoint.json` behind.
#[derive(Resource, Debug, Clone, PartialEq, Eq)]
pub struct SaveDir(pub PathBuf);

impl SaveDir {
    /// `saves` inside a per-user directory -- the one
    /// [`crate::crash::init_for_project`] returns, in a running game.
    pub fn in_user_dir(user_dir: &Path) -> Self {
        Self(user_dir.join("saves"))
    }

    /// A directory no earlier run has written to, for one `--test` app.
    ///
    /// A replay must play the same way every time, and a save left behind by
    /// the run before -- or by the player, if replays shared the game's real
    /// save folder -- is state the recording never saw: a game that loads its
    /// checkpoint at start would replay differently on a second run than on
    /// the first. So every test app starts with no saves at all.
    ///
    /// Nothing is created here; `save_world` makes the directory on the first
    /// save, so a game that never saves leaves nothing in the temp directory.
    /// A leftover from an earlier process with the same id is removed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn scratch() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        // Per app, not per process: the test binaries build many test apps in
        // one process, and two that shared a directory would see each other's
        // saves.
        static NEXT: AtomicU64 = AtomicU64::new(0);
        Self::emptied(std::env::temp_dir().join(format!(
            "bsengine-saves-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        )))
    }

    /// `dir`, with whatever an earlier process with the same id left there
    /// removed.
    #[cfg(not(target_arch = "wasm32"))]
    fn emptied(dir: PathBuf) -> Self {
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }

    /// Where a script's save path points: checked as [`project_path`] checks a
    /// project path -- no `..` above the directory, no root, no drive -- and
    /// joined onto it. The check is the one scripts already had against the
    /// working directory (`saveGame("../../x")`, `saveGame("C:/x")` once
    /// wrote wherever the process could).
    pub fn resolve(&self, path: &str) -> Result<PathBuf, ProjectPathError> {
        Ok(self.0.join(project_path(path)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_are_a_folder_of_the_user_directory() {
        let dir = SaveDir::in_user_dir(Path::new("/users/someone/Game"));
        assert_eq!(dir.0, Path::new("/users/someone/Game/saves"));
    }

    #[test]
    fn a_save_path_resolves_inside_the_save_directory_or_not_at_all() {
        let dir = SaveDir(PathBuf::from("/u/saves"));
        assert_eq!(
            dir.resolve("slot1/checkpoint.json").unwrap(),
            Path::new("/u/saves/slot1/checkpoint.json")
        );
        assert_eq!(
            dir.resolve("a\\..\\b.json").unwrap(),
            Path::new("/u/saves/b.json")
        );
        assert_eq!(
            dir.resolve("../logs/game.log"),
            Err(ProjectPathError::Escapes)
        );
        assert_eq!(dir.resolve("/etc/x"), Err(ProjectPathError::Absolute));
        assert_eq!(dir.resolve("C:/x"), Err(ProjectPathError::Colon));
        assert_eq!(dir.resolve(""), Err(ProjectPathError::Empty));
    }

    #[test]
    fn every_scratch_directory_is_new_and_empty() {
        let a = SaveDir::scratch();
        let b = SaveDir::scratch();
        assert_ne!(a, b, "two test apps share a save directory");
        assert!(!a.0.exists(), "scratch created its directory up front");

        // A directory an earlier process with the same id left behind: the
        // premise is that it is there, with a save in it.
        let stale =
            std::env::temp_dir().join(format!("bsengine-saves-stale-probe-{}", std::process::id()));
        std::fs::create_dir_all(&stale).unwrap();
        std::fs::write(stale.join("checkpoint.json"), "{}").unwrap();
        assert!(stale.join("checkpoint.json").is_file());
        let fresh = SaveDir::emptied(stale.clone());
        assert_eq!(fresh.0, stale);
        assert!(!stale.exists(), "a stale save survived into a new app");
    }
}
