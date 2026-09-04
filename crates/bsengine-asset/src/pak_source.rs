//! Reading scene RON out of a packaged build's archive.
//!
//! # Why this exists beside [`crate::pak_reader`]
//!
//! A custom `AssetReader` serves everything that goes through `bevy_asset` —
//! meshes, textures, audio, scripts. Scene RON does not: it is read
//! synchronously at `Startup` with plain `std::fs`, in three places
//! (`bsengine-scene`'s entry scene and prefabs, `bsengine-runtime`'s runtime
//! scene loads). Moving scenes onto the asset server would turn spawning into
//! an async step and shift the frame timing all ten E2E recordings depend on —
//! a refactor with its own risk budget, not something to fold into packaging.
//!
//! # Why the archive is a process global
//!
//! Because it genuinely is one: a process runs one game, out of one archive,
//! and never swaps it. The alternative is threading an `Option<&Pak>` through
//! `instantiate_prefab_from_path` and everything it recurses into — the same
//! plumbing `RESOLVING_PREFABS` already exists in `bsengine-scene` to avoid,
//! for the same reason.
//!
//! The decision-making is kept out of the global: [`read_from`] is a pure
//! function of an explicit `Option<&Pak>`, and [`read_to_string`] is the thin
//! accessor that supplies it. Tests drive the pure one, so no test has to set
//! process-wide state that would then leak into every other test in its binary.

use std::io;
use std::sync::{Arc, OnceLock};

use crate::pak::Pak;

/// The archive this process serves scenes from, and the project directory whose
/// prefix its paths carry.
static ARCHIVE: OnceLock<(Arc<Pak>, String)> = OnceLock::new();

/// Serves scene reads from `pak` for the rest of this process's life.
///
/// `project_dir` is the same string `bsengine_core::resolve_project_path`
/// prepends to every path, and therefore the prefix [`archive_key`] removes.
///
/// Returns whether it was set — `false` means one was already installed, which
/// a host doing this once at startup never sees.
pub fn install(pak: Arc<Pak>, project_dir: impl Into<String>) -> bool {
    ARCHIVE.set((pak, project_dir.into())).is_ok()
}

/// The archive installed by [`install`], if any, and its project directory.
pub fn archive() -> Option<(&'static Arc<Pak>, &'static str)> {
    ARCHIVE.get().map(|(pak, dir)| (pak, dir.as_str()))
}

/// Turns a path as the engine spells it into the key the archive stores it
/// under.
///
/// # The rule, and why it is a rule rather than a guess
///
/// `bsengine_core::resolve_project_path` builds every path in this engine as
/// `format!("{project_dir}/{path}")`, while the cook keys entries by the
/// project-relative `path` alone. So this removes exactly the prefix that
/// function added — nothing cleverer. A build run from inside itself has a
/// project directory of `.`, which is why that case is spelled out; a test or a
/// host that names an absolute directory is why the general case is too.
///
/// Separators are normalised because a path carries whichever the caller wrote,
/// and on Windows both appear.
///
/// A path that does not start with the project directory comes back normalised
/// but otherwise untouched. Forcing it to match would invent an entry, and an
/// archive answering for a file it does not hold is worse than a miss.
///
/// # Why both readers share this
///
/// [`crate::pak_reader`] serves `bevy_asset`'s loads and this module serves
/// scene reads, but they are looking into the same archive with paths built by
/// the same function. Two copies of this rule that drifted apart would mean one
/// of them silently reading from disk — which is exactly the bug the first
/// version of this module shipped, where it stripped only `./` and so missed
/// every absolute project directory.
pub fn archive_key(path: &str, project_dir: &str) -> String {
    let text = path.replace('\\', "/");
    let project = project_dir.replace('\\', "/");
    let project = project.trim_end_matches('/');

    let stripped = if project.is_empty() || project == "." {
        text.strip_prefix("./").unwrap_or(&text)
    } else {
        text.strip_prefix(&format!("{project}/"))
            .unwrap_or_else(|| text.strip_prefix("./").unwrap_or(&text))
    };
    stripped.to_string()
}

/// The text of a scene or prefab, from `pak` if it holds one and from disk
/// otherwise.
///
/// # Why it falls back rather than failing
///
/// The fallback is what lets loose mode and pak mode run the *same* code: one
/// path through the engine, with the archive simply absent in an unpackaged
/// run, instead of two paths that can drift apart. It also means a packaged
/// build whose archive is missing an entry degrades to reading the file if one
/// happens to be there — which is why the packaging tests assert no loose assets
/// sit beside the archive, since otherwise a pak build that read nothing from
/// the archive would look identical to one that worked.
///
/// # Errors
///
/// When the archive holds the path but not as UTF-8, or when neither the
/// archive nor the disk can supply it.
pub fn read_from(pak: Option<&Pak>, project_dir: &str, path: &str) -> io::Result<String> {
    if let Some(pak) = pak {
        if let Some(bytes) = pak.get(&archive_key(path, project_dir)) {
            return String::from_utf8(bytes.to_vec()).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{path} is in the archive but is not UTF-8: {e}"),
                )
            });
        }
    }
    std::fs::read_to_string(path)
}

/// [`read_from`], against whatever archive this process installed.
///
/// # Errors
///
/// Whatever [`read_from`] returns.
pub fn read_to_string(path: &str) -> io::Result<String> {
    match archive() {
        Some((pak, project_dir)) => read_from(Some(pak), project_dir, path),
        None => std::fs::read_to_string(path),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive_of(entries: &[(&str, &str)]) -> Pak {
        let owned: Vec<(String, Vec<u8>)> = entries
            .iter()
            .map(|(path, text)| ((*path).to_string(), text.as_bytes().to_vec()))
            .collect();
        Pak::from_bytes(crate::pak::write_pak_bytes(&owned).expect("write")).expect("open")
    }

    /// Both spellings a scene path arrives in — bare, and prefixed by the `./`
    /// that `resolve_project_path` adds for a build run from inside itself.
    #[test]
    fn a_scene_in_the_archive_is_read_from_it() {
        let pak = archive_of(&[("assets/scenes/main.ron", "(entities: [])")]);

        assert_eq!(
            read_from(Some(&pak), ".", "assets/scenes/main.ron").expect("read"),
            "(entities: [])"
        );
        assert_eq!(
            read_from(Some(&pak), ".", "./assets/scenes/main.ron").expect("read"),
            "(entities: [])",
            "a './'-prefixed path is the shape a build run from inside itself \
             produces, and must reach the same entry"
        );
    }

    /// The shape that shipped broken: a host handed an **absolute** project
    /// directory, which is what `bsengine-runtime`'s own E2E does. The first
    /// version of this module stripped only `./`, so every such path missed the
    /// archive and fell through to a disk read that could not succeed either.
    ///
    /// Caught by the end-to-end test and not by any unit test here, because
    /// every fixture used `.` — the one project directory for which the broken
    /// rule and the correct one agree.
    #[test]
    fn an_absolute_project_directory_still_finds_the_entry() {
        let pak = archive_of(&[("assets/scenes/main.ron", "(entities: [])")]);

        assert_eq!(
            read_from(
                Some(&pak),
                "C:/Temp/bsengine-package-e2e-1",
                "C:/Temp/bsengine-package-e2e-1/assets/scenes/main.ron",
            )
            .expect("read"),
            "(entities: [])"
        );
        assert_eq!(
            archive_key(r"C:\Temp\build\assets\scenes\main.ron", r"C:\Temp\build"),
            "assets/scenes/main.ron",
            "backslashes on both sides must normalise the same way"
        );
    }

    /// The fallback, and the reason loose mode needs no separate code path.
    #[test]
    fn a_path_the_archive_lacks_falls_back_to_disk() {
        let dir = std::env::temp_dir().join(format!("bsengine-pak-source-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create");
        let on_disk = dir.join("loose.ron");
        std::fs::write(&on_disk, "(entities: [])").expect("write");

        let pak = archive_of(&[("assets/scenes/other.ron", "(entities: [])")]);
        let read = read_from(Some(&pak), ".", &on_disk.to_string_lossy()).expect("read");

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read, "(entities: [])");
    }

    /// With no archive at all — an unpackaged run — this is plain disk reading.
    #[test]
    fn without_an_archive_it_reads_the_disk() {
        let dir =
            std::env::temp_dir().join(format!("bsengine-pak-source-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create");
        let on_disk = dir.join("loose.ron");
        std::fs::write(&on_disk, "loose").expect("write");

        let read = read_from(None, ".", &on_disk.to_string_lossy()).expect("read");

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read, "loose");
    }

    /// The archive wins over a file of the same name, which is what makes a
    /// packaged build read its own contents rather than whatever happens to sit
    /// in the working directory.
    #[test]
    fn the_archive_is_preferred_over_a_file_of_the_same_name() {
        let dir =
            std::env::temp_dir().join(format!("bsengine-pak-source-pref-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("assets/scenes")).expect("create");
        let path = "assets/scenes/main.ron";
        let full = dir.join(path);
        std::fs::write(&full, "FROM DISK").expect("write");

        let pak = archive_of(&[(path, "FROM ARCHIVE")]);
        let read = read_from(Some(&pak), ".", path).expect("read");

        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(read, "FROM ARCHIVE");
    }
}
