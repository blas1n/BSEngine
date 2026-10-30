//! The asset source rooted at the project, and the key every asset is loaded by.
//!
//! # What this replaces
//!
//! `bevy_asset` used to be rooted at the process's working directory with
//! `UnapprovedPathMode::Allow`, and every asset was loaded by the *engine form*
//! of its path -- `<ProjectDir>/assets/x.glb`, CWD-relative or absolute. Two
//! things were wrong with that. Nothing stopped a path from naming a file
//! outside the project (the root was the CWD, and `Allow` switched off the one
//! check Bevy has). And an asset's identity -- the key of `AssetStatuses`, of
//! the watcher, of the archive -- was a string that contained the machine's
//! spelling of the project directory.
//!
//! Now the default source is [`ProjectAssetReader`], rooted at the project, and
//! the key is the **project path**: `assets/x.glb`, relative to the project,
//! forward slashes, validated by [`bsengine_core::project_path`]. Unity
//! (`Assets/...`), Unreal (`/Game/...`) and Godot (`res://`) all key an asset by
//! a project-relative virtual path and turn it into a file only where the disk
//! is read; so does this.
//!
//! # Why the components keep their resolved strings
//!
//! `GltfAsset`, `CustomShader`, `SkyboxPath` and the texture paths hold the
//! engine form, and everything that reads them (the inspector, scene saving,
//! prefab merging, the render caches) expects it. Changing them would touch all
//! of those. So the conversion happens at the one place a path becomes a load:
//! [`asset_key`], called by [`crate::load_async`] and by the few callers that
//! reach `AssetServer` directly.
//!
//! # How a path finds its project without a `World`
//!
//! `load_async` is handed an `&AssetServer` and a path -- the corner
//! [`crate::load_mode`] already argued itself out of for the identity index, by
//! keeping the published projects in a process-global registry and finding a
//! path's project by prefix. This does the same: [`register_project`] adds a
//! project directory, [`asset_key`] strips the **longest** registered one.
//! Several `App`s in one process (this crate's tests are a dozen) register
//! their own directories and do not disturb each other.
//!
//! A path that starts with no registered project is taken as *already* project
//! relative -- which is what a script's `assets/x.glb` is -- and goes through
//! the same validation. An absolute path under no registered project is refused:
//! that is the escape this exists to close.

use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, PoisonError, RwLock};

use bevy_app::{First, Plugin, PreStartup};
use bevy_asset::io::{AssetReader, AssetReaderError, PathStream, VecReader};
use bevy_ecs::prelude::{Res, Resource};
use bsengine_core::{project_path, ProjectDir, ProjectPathError};

/// The project directories this process has an `App` for, as `ProjectDir`
/// spells them (forward slashes, no trailing slash).
///
/// One entry per distinct project directory ever synced, which is one in every
/// host and one per probe in a test run. Nothing is removed: a project that was
/// closed leaves a prefix that can only matter to a path that carries it, and
/// such a path *is* under that project.
static PROJECTS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(RwLock::default);

/// Adds `project_dir` (as `ProjectDir` holds it) to the registry [`asset_key`]
/// reads. Idempotent. An empty directory -- the project is the working
/// directory -- has no prefix to strip and is not recorded.
pub fn register_project(project_dir: &str) {
    let dir = normalise_dir(project_dir);
    if dir.is_empty() {
        return;
    }
    let mut projects = PROJECTS.write().unwrap_or_else(PoisonError::into_inner);
    if !projects.contains(&dir) {
        projects.push(dir);
    }
}

fn normalise_dir(project_dir: &str) -> String {
    project_dir
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_owned()
}

/// The key `path` is loaded by: the project path it names.
///
/// `path` may be the engine form (`<ProjectDir>/assets/x.glb`, whose project is
/// the longest registered directory it starts with) or already project
/// relative (`assets/x.glb`). Either way the result is validated, so `..` that
/// climbs out, a root, a drive, a `scheme://` and the empty path are errors.
pub fn asset_key(path: &str) -> Result<String, ProjectPathError> {
    let text = path.replace('\\', "/");
    let rest = {
        let projects = PROJECTS.read().unwrap_or_else(PoisonError::into_inner);
        projects
            .iter()
            .filter(|dir| {
                text.strip_prefix(dir.as_str())
                    .is_some_and(|rest| rest.starts_with('/'))
            })
            .max_by_key(|dir| dir.len())
            // `dir` is a prefix of `text` followed by a `/`, checked above.
            // What follows is separators and then the project path: the engine
            // form is a string join, and `<dir>//assets/x` names the same file
            // as `<dir>/assets/x`. Only here -- a path under *no* project that
            // starts with `/` is a root, and stays one.
            .map(|dir| text[dir.len()..].trim_start_matches('/').to_owned())
    };
    project_path(rest.as_deref().unwrap_or(&text))
}

/// [`asset_key`] for a caller with no error to return: the key, or -- with a
/// warning -- the path unchanged, which `bevy_asset` and the reader then refuse
/// and the load fails visibly instead of reading something else.
pub fn asset_key_or_raw(path: &str) -> String {
    asset_key(path).unwrap_or_else(|why| {
        tracing::warn!("[asset] not loading {path:?}: {why}");
        path.to_owned()
    })
}

/// The directory the default asset source reads from.
///
/// Shared between the [`ProjectAssetReader`] that `bevy_asset` owns and the
/// system that follows the `ProjectDir` resource, so a project opened later
/// (the editor's file dialog) moves the root without rebuilding the source,
/// which `bevy_asset` fixes at build time.
#[derive(Resource, Clone, Debug)]
pub struct ProjectRoot(Arc<RwLock<PathBuf>>);

impl ProjectRoot {
    /// A root at `dir`. `dir` should be absolute.
    pub fn new(dir: PathBuf) -> Self {
        Self(Arc::new(RwLock::new(dir)))
    }

    /// The directory loads currently resolve against.
    pub fn get(&self) -> PathBuf {
        self.0
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Moves the root. Loads already read keep what they read.
    pub fn set(&self, dir: PathBuf) {
        let mut root = self.0.write().unwrap_or_else(PoisonError::into_inner);
        if *root != dir {
            *root = dir;
        }
    }
}

/// `project_dir` as an absolute directory: the working directory for an empty
/// one, and `project_dir` itself when it already is absolute (`join` replaces
/// its base then).
pub fn absolute_project_dir(project_dir: &str) -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_default();
    if project_dir.is_empty() {
        cwd
    } else {
        cwd.join(project_dir)
    }
}

/// Follows the `ProjectDir` resource: registers it and moves the root.
///
/// Runs in `PreStartup` -- so a `Startup` system that loads sees the right
/// root -- and again in `First`, for a project opened while running.
pub(crate) fn sync_project_root(project_dir: Option<Res<ProjectDir>>, root: Res<ProjectRoot>) {
    let dir = project_dir.as_deref().map_or("", |pd| pd.0.as_str());
    register_project(dir);
    root.set(absolute_project_dir(dir));
}

/// Reads the default source's files from [`ProjectRoot`], and from nowhere
/// else.
///
/// Every request is checked twice. The path must be a project path (no `..`
/// out, no root, no drive: [`bsengine_core::project_path`]), and the file it
/// names, once the file system has followed any link in the way, must still be
/// inside the root -- a symlink in the project that points out of it is an
/// escape no spelling check can see.
pub struct ProjectAssetReader {
    root: ProjectRoot,
}

impl ProjectAssetReader {
    /// A reader over `root`.
    pub fn new(root: ProjectRoot) -> Self {
        Self { root }
    }

    fn file_for(&self, path: &Path) -> Result<PathBuf, AssetReaderError> {
        let denied = |why: &dyn std::fmt::Display| {
            AssetReaderError::Io(Arc::new(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!("{}: outside the project ({why})", path.display()),
            )))
        };
        let relative = project_path(&path.to_string_lossy()).map_err(|e| denied(&e))?;
        let root = self.root.get();
        let file = root.join(relative);
        if let (Ok(real), Ok(real_root)) = (file.canonicalize(), root.canonicalize()) {
            if !real.starts_with(&real_root) {
                return Err(denied(&"a link leads out of it"));
            }
        }
        Ok(file)
    }
}

impl AssetReader for ProjectAssetReader {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        let file = self.file_for(path)?;
        match std::fs::read(&file) {
            Ok(bytes) => Ok(VecReader::new(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(AssetReaderError::NotFound(path.to_path_buf()))
            }
            Err(e) => Err(AssetReaderError::Io(Arc::new(e))),
        }
    }

    /// Always `NotFound`: the engine builds `bevy_asset` with
    /// `AssetMetaCheck::Never` because its identity sidecars use the filename
    /// `bevy_asset` reserves for its own meta (see [`crate::AssetPlugin`]).
    async fn read_meta<'a>(&'a self, path: &'a Path) -> Result<VecReader, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    /// Refused: nothing in this engine loads a directory, and an empty listing
    /// would read as "there, and holding nothing".
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        Err(AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        Ok(self.file_for(path)?.is_dir())
    }
}

/// Registers [`ProjectAssetReader`] as the default asset source and starts
/// following `ProjectDir`. Added by [`crate::AssetPlugin`] before `bevy_asset`'s
/// own plugin, because sources are built there and a later one is ignored.
pub(crate) struct ProjectRootPlugin;

impl Plugin for ProjectRootPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        use bevy_asset::io::{AssetSourceBuilder, AssetSourceBuilders, AssetSourceId};
        use bevy_asset::AssetApp;

        let initial = app
            .world()
            .get_resource::<ProjectDir>()
            .map_or(String::new(), |pd| pd.0.clone());
        register_project(&initial);
        let root = ProjectRoot::new(absolute_project_dir(&initial));

        // A packaged build registered the archive as the default source
        // (`PakAssetPlugin`, added before this); leave it, but still publish the
        // root so `ProjectDir` is followed for everything else that reads it.
        let taken = app
            .world_mut()
            .get_resource_mut::<AssetSourceBuilders>()
            .is_some_and(|mut sources| sources.get_mut(AssetSourceId::Default).is_some());
        if !taken {
            let reader_root = root.clone();
            app.register_asset_source(
                AssetSourceId::Default,
                AssetSourceBuilder::new(move || {
                    Box::new(ProjectAssetReader::new(reader_root.clone()))
                }),
            );
        }
        app.insert_resource(root)
            .add_systems(PreStartup, sync_project_root)
            .add_systems(First, sync_project_root);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A project directory unique to the caller, so parallel tests registering
    /// theirs do not collide with each other's prefixes.
    fn project(name: &str) -> String {
        format!("proj-{name}-{}", std::process::id())
    }

    #[test]
    fn a_path_under_a_registered_project_is_keyed_by_what_follows_it() {
        let dir = project("strip");
        register_project(&dir);
        assert_eq!(
            asset_key(&format!("{dir}/assets/x.glb")).as_deref(),
            Ok("assets/x.glb")
        );
        // The spellings the engine form really has.
        assert_eq!(
            asset_key(&format!("{dir}\\assets\\x.glb")).as_deref(),
            Ok("assets/x.glb")
        );
        assert_eq!(
            asset_key(&format!("{dir}//assets/./x.glb")).as_deref(),
            Ok("assets/x.glb")
        );
        // Already a project path, as a script writes it.
        assert_eq!(asset_key("assets/x.glb").as_deref(), Ok("assets/x.glb"));
    }

    /// The hard side of the registry: the longest project wins, and a prefix
    /// that is only a prefix of a *name* is not a project.
    #[test]
    fn the_longest_registered_project_wins_and_a_name_prefix_is_not_one() {
        let outer = project("outer");
        let inner = format!("{outer}/inner");
        register_project(&outer);
        register_project(&inner);
        assert_eq!(
            asset_key(&format!("{inner}/assets/y.png")).as_deref(),
            Ok("assets/y.png"),
            "the nested project's own prefix, not the outer one's"
        );
        assert_eq!(
            asset_key(&format!("{outer}/other/y.png")).as_deref(),
            Ok("other/y.png")
        );
        // `{outer}-sibling` starts with the text `{outer}` but not with
        // `{outer}/`: it is not under it, so nothing is stripped.
        let sibling = format!("{outer}-sibling/assets/z.png");
        assert_eq!(asset_key(&sibling).as_deref(), Ok(sibling.as_str()));
    }

    /// The escapes this exists to close: an absolute path under no registered
    /// project, a `..` that climbs out of the project it is under, a drive, a
    /// `scheme://`.
    #[test]
    fn a_path_that_leaves_every_project_has_no_key() {
        let dir = project("escape");
        register_project(&dir);
        for path in [
            "/etc/passwd".to_string(),
            "C:/Windows/x.png".to_string(),
            "other://assets/x.png".to_string(),
            "../x.png".to_string(),
            format!("{dir}/../x.png"),
            format!("{dir}/assets/../../x.png"),
            String::new(),
        ] {
            assert!(asset_key(&path).is_err(), "{path:?} must have no key");
        }
    }

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        bevy_tasks::block_on(future)
    }

    /// A project on disk: `assets/ok.txt` inside, and `secret.txt` beside it.
    fn disk_project(name: &str) -> (bsengine_core::ProjectDir, PathBuf, ProjectAssetReader) {
        let base =
            std::env::temp_dir().join(format!("bsengine_root_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("proj/assets")).unwrap();
        std::fs::write(base.join("proj/assets/ok.txt"), "inside").unwrap();
        std::fs::write(base.join("secret.txt"), "outside").unwrap();
        let root = ProjectRoot::new(base.join("proj"));
        (
            bsengine_core::ProjectDir(base.join("proj").to_string_lossy().into_owned()),
            base,
            ProjectAssetReader::new(root),
        )
    }

    #[test]
    fn the_reader_serves_the_project_and_nothing_beside_it() {
        let (_dir, base, reader) = disk_project("serves");
        let read = |p: &str| block_on(reader.read(Path::new(p))).map(|r| r.bytes.len());
        assert_eq!(
            read("assets/ok.txt").ok(),
            Some(6),
            "the control: a file inside is read"
        );
        for escape in [
            "../secret.txt",
            "assets/../../secret.txt",
            "assets\\..\\..\\secret.txt",
        ] {
            assert!(
                matches!(read(escape), Err(AssetReaderError::Io(_))),
                "{escape:?} must be refused, and not as 'missing'"
            );
        }
        let absolute = base.join("secret.txt");
        assert!(
            read(&absolute.to_string_lossy()).is_err(),
            "an absolute path is refused"
        );
        // A file that is simply not there is NotFound, so the two outcomes stay
        // distinguishable in a load error.
        assert!(matches!(
            read("assets/nope.txt"),
            Err(AssetReaderError::NotFound(_))
        ));
        // The spelling check on its own: an escape to a file that does *not*
        // exist. The link check cannot see this one (there is nothing to
        // canonicalise), so without the spelling check the reader would go
        // and look outside the project and answer "missing" -- refused is the
        // answer that never touches the disk there.
        assert!(
            matches!(read("../no_such_file.txt"), Err(AssetReaderError::Io(_))),
            "an escape is refused before the disk outside the project is looked at"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    /// The root moves with the project: a reader built before the editor opened
    /// another project reads the new one afterwards.
    #[test]
    fn moving_the_root_moves_what_the_reader_reads() {
        let (_dir, base, reader) = disk_project("moves");
        std::fs::create_dir_all(base.join("other/assets")).unwrap();
        std::fs::write(base.join("other/assets/ok.txt"), "elsewhere").unwrap();
        let read = |reader: &ProjectAssetReader| {
            block_on(reader.read(Path::new("assets/ok.txt"))).map(|r| r.bytes.len())
        };
        assert_eq!(read(&reader).ok(), Some(6));
        reader.root.set(base.join("other"));
        assert_eq!(
            read(&reader).ok(),
            Some(9),
            "the same key now names the other project's file"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    /// A link inside the project that points out of it. Creating one needs a
    /// privilege Windows withholds from an ordinary account, so where it cannot
    /// be made this checks nothing and says so rather than passing quietly.
    #[test]
    fn a_link_that_leads_out_of_the_project_is_refused() {
        let (_dir, base, reader) = disk_project("link");
        let link = base.join("proj/assets/leak.txt");
        #[cfg(unix)]
        let made = std::os::unix::fs::symlink(base.join("secret.txt"), &link).is_ok();
        #[cfg(windows)]
        let made = std::os::windows::fs::symlink_file(base.join("secret.txt"), &link).is_ok();
        if !made {
            eprintln!("SKIPPED: this account cannot create symbolic links");
            let _ = std::fs::remove_dir_all(base);
            return;
        }
        assert!(
            matches!(
                block_on(reader.read(Path::new("assets/leak.txt"))),
                Err(AssetReaderError::Io(_))
            ),
            "a symlink to a file outside the project must not be followed"
        );
        let _ = std::fs::remove_dir_all(base);
    }

    /// The editor opens another project while running: the `ProjectDir`
    /// resource changes, and the next load reads from the new project, through
    /// a real app. The hard side is a file that exists *only* in the second
    /// project -- with the root left behind, the load fails; the control is the
    /// first project's own file loading before the switch.
    #[test]
    fn a_project_opened_while_running_is_where_the_next_load_reads() {
        use crate::types::TextureAsset;
        use bevy_asset::{AssetServer, Assets, Handle};

        let base =
            std::env::temp_dir().join(format!("bsengine_root_switch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        for (dir, file) in [("first", "in_first.png"), ("second", "in_second.png")] {
            std::fs::create_dir_all(base.join(dir)).unwrap();
            image::RgbaImage::from_pixel(1, 1, image::Rgba([1, 2, 3, 255]))
                .save(base.join(dir).join(file))
                .unwrap();
        }
        let project = |dir: &str| ProjectDir(base.join(dir).to_string_lossy().into_owned());

        let mut app = bsengine_app::new_app();
        app.insert_resource(project("first"));
        app.add_plugins(crate::plugin::AssetPlugin);
        let loaded = |app: &mut bevy_app::App, key: &str| -> bool {
            let handle: Handle<TextureAsset> =
                app.world().resource::<AssetServer>().load(key.to_string());
            for _ in 0..200 {
                app.update();
                if app
                    .world()
                    .resource::<Assets<TextureAsset>>()
                    .get(&handle)
                    .is_some()
                {
                    return true;
                }
            }
            false
        };
        assert!(
            loaded(&mut app, "in_first.png"),
            "the control: the first project's file loads"
        );

        app.insert_resource(project("second"));
        app.update();
        assert!(
            loaded(&mut app, "in_second.png"),
            "after the project changed, a file only the second project has must load"
        );
        let _ = std::fs::remove_dir_all(base);
    }
}
