//! Where a game's save files live.

use std::collections::BTreeMap;
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
/// A browser build has no file system and keeps its saves in a [`SaveStore`]
/// instead. With neither resource there are no saves: an app that never said
/// where saves go refuses them rather than guessing -- the guess
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

/// Save files held in memory, for a build with no file system to put them in
/// -- a browser.
///
/// Unity's WebGL player and Godot's web export both keep saves in the page's
/// IndexedDB (Unity mounts it at `persistentDataPath`, Godot at `user://`),
/// and both work the same way round: IndexedDB is asynchronous and a script's
/// save is not, so the files are read into memory once, before the game
/// starts, every save changes the memory copy at once and is written back to
/// IndexedDB behind it (Emscripten's IDBFS, which both use, syncs exactly
/// so). This is that memory copy. The host fills it with what it found
/// ([`Self::with_files`]) and drains [`Self::take_written`] into IndexedDB;
/// nothing here knows about the browser, so the same store is testable on a
/// desktop.
///
/// Paths are checked and normalised the way [`SaveDir::resolve`] checks them,
/// so a script's `save("a\\..\\b.json")` names the same file in both and
/// `save("../x")` is refused in both.
#[derive(Resource, Debug, Clone, Default, PartialEq, Eq)]
pub struct SaveStore {
    files: BTreeMap<String, String>,
    /// Paths saved since the host last took them, oldest first, each once.
    written: Vec<String>,
}

impl SaveStore {
    /// A store holding `files` (normalised path -> contents), as the host
    /// read them back -- none of them waiting to be written.
    pub fn with_files(files: impl IntoIterator<Item = (String, String)>) -> Self {
        Self {
            files: files.into_iter().collect(),
            written: Vec::new(),
        }
    }

    /// How many save files the store holds.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether the store holds no save files at all.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The saved contents at `path`, if a file is there.
    pub fn read(&self, path: &str) -> Result<Option<&str>, ProjectPathError> {
        Ok(self.files.get(&project_path(path)?).map(String::as_str))
    }

    /// Saves `contents` at `path`, replacing what was there, and marks it for
    /// the host to write back.
    pub fn write(&mut self, path: &str, contents: String) -> Result<(), ProjectPathError> {
        let path = project_path(path)?;
        if !self.written.contains(&path) {
            self.written.push(path.clone());
        }
        self.files.insert(path, contents);
        Ok(())
    }

    /// Every file saved since the last call, with its contents now: what the
    /// host has to write back. A file saved twice in between is written once,
    /// as it is now -- mini-arena saves its checkpoint every frame, and the
    /// browser has no use for the 59 copies of it a second overwrote.
    pub fn take_written(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.written)
            .into_iter()
            .filter_map(|path| {
                let contents = self.files.get(&path)?.clone();
                Some((path, contents))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stored_save_reads_back_under_the_path_it_was_saved_at() {
        let mut store = SaveStore::default();
        store
            .write("slot1/checkpoint.json", "one".to_string())
            .unwrap();
        assert_eq!(store.read("slot1/checkpoint.json"), Ok(Some("one")));
        // Normalised as `SaveDir::resolve` normalises: the same file.
        assert_eq!(store.read("slot1\\x\\..\\checkpoint.json"), Ok(Some("one")));
        assert_eq!(store.read("slot2.json"), Ok(None));
        store
            .write("slot1/checkpoint.json", "two".to_string())
            .unwrap();
        assert_eq!(store.read("slot1/checkpoint.json"), Ok(Some("two")));
    }

    #[test]
    fn a_store_refuses_the_paths_a_save_directory_refuses() {
        let mut store = SaveStore::default();
        assert_eq!(
            store.write("../logs/game.log", String::new()),
            Err(ProjectPathError::Escapes)
        );
        assert_eq!(
            store.write("C:/x", String::new()),
            Err(ProjectPathError::Colon)
        );
        assert_eq!(store.read("/etc/x"), Err(ProjectPathError::Absolute));
        assert!(
            store.take_written().is_empty(),
            "a refused save left something to write back"
        );
    }

    #[test]
    fn what_is_written_back_is_each_saved_file_once_as_it_is_now() {
        let mut store = SaveStore::with_files([("old.json".to_string(), "kept".to_string())]);
        assert!(
            store.take_written().is_empty(),
            "the files the host read back are not waiting to be written"
        );
        store.write("a.json", "1".to_string()).unwrap();
        store.write("b.json", "x".to_string()).unwrap();
        store.write("a.json", "2".to_string()).unwrap();
        assert_eq!(
            store.take_written(),
            vec![
                ("a.json".to_string(), "2".to_string()),
                ("b.json".to_string(), "x".to_string()),
            ]
        );
        assert!(store.take_written().is_empty(), "taken once");
        assert_eq!(store.read("old.json"), Ok(Some("kept")));
    }

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
