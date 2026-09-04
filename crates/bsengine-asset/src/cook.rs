//! Collecting exactly the assets a project reaches, and writing a build that
//! runs without the editor.
//!
//! # Why a static walk
//!
//! The alternative — running the game and recording what it loaded — reports
//! only what *that* run reached. A level the run never entered, a sound that
//! plays on a branch it never took, and a prefab spawned by a rule it never
//! triggered are all silently dropped, and the failure arrives after shipping.
//! A static walk can be wrong in the other direction (it may collect something
//! unreachable), and that is much the cheaper mistake: a build carrying one
//! file too many still runs.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use super::identity::fixup::{collect_references, is_project_relative, Reference};
use super::identity::{scan, AssetIndex};

/// Extension of the files the walk parses as scenes and descends into.
const SCENE_EXTENSION: &str = "ron";
/// Extension of the files the walk scans for quoted paths.
const SCRIPT_EXTENSION: &str = "js";

/// One reference that names nothing the engine could load.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct MissingReference {
    /// The file that names it, project-relative — without this a report says
    /// only that something is missing, and not where to go and fix it.
    pub referrer: String,
    /// The path as that file spells it.
    pub path: String,
}

/// One quoted path in a script that resolves to nothing.
///
/// Reported, never fatal: a quoted `assets/…` in JavaScript is a *guess* that
/// the string is a path, and it can equally be dead code, a commented-out
/// branch, or a deliberate probe of something absent. [`super::identity::fixup`]
/// draws the same line for the same reason — failing on it would mean a project
/// could never be clean.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub struct ScriptMention {
    /// The script that spells it, project-relative.
    pub script: String,
    /// The path as the script spells it.
    pub path: String,
}

/// What one cook found.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct CookedProject {
    /// Every asset the project reaches, project-relative and sorted. Sorted
    /// because a set built from a directory walk otherwise varies in order
    /// between runs, and a report a human compares against a previous one has
    /// to be stable.
    pub assets: BTreeSet<String>,
    /// References that resolve to nothing. A non-empty list fails the build.
    pub missing: Vec<MissingReference>,
    /// Everything else that stopped the cook from seeing a file it reached: a
    /// scene that will not parse, one that cannot be read. Kept apart from
    /// [`Self::missing`] because it reads differently — "this file is broken",
    /// not "this file names something absent" — and a report that phrased the
    /// two the same way would be lying about one of them. Also fails the build:
    /// a scene the cook could not read is a scene whose references went
    /// unchecked.
    pub problems: Vec<String>,
    /// Script-spelled paths that resolve to nothing. Never fails the build.
    pub script_mentions: Vec<ScriptMention>,
}

impl CookedProject {
    /// Whether this cook found a reason not to package.
    pub fn is_ok(&self) -> bool {
        self.missing.is_empty() && self.problems.is_empty()
    }
}

/// Collects every asset the project reaches from `entry_scene`.
///
/// `entry_scene` is passed in rather than read here so that `project.toml` is
/// parsed in exactly one place — `bsengine-runtime` already owns that manifest,
/// and a second parser is a second thing to keep in step with it.
///
/// # What counts as a reference
///
/// Whatever [`collect_references`] says, which is every string spelled
/// `assets/…` anywhere in a parsed scene — including inside the RON of a
/// reflected component, where a `Terrain`'s heightmap and layer textures live.
/// Reusing that function rather than growing a second walker is the point: two
/// collectors that disagree would mean this validates a different project from
/// the one the engine loads.
///
/// # `extra_assets`, and why it has to exist
///
/// A path a script *builds* — by concatenation, or chosen from data — is
/// invisible to any static walk, and so is a file nothing references but a
/// build still needs, like the attribution notice a model's licence requires.
/// Without a way to name those, such a project simply cannot be packaged
/// correctly, and the tool would be quietly wrong rather than usefully limited.
/// This is that way.
///
/// # Errors
///
/// Only when `<project_dir>/assets` cannot be walked at all, which is [`scan`]'s
/// one error and almost always means the wrong directory was named. Everything
/// found *inside* the project is a line in the report instead: a scene that will
/// not parse, a reference that resolves to nothing. A cook that stopped at the
/// first would report one problem per run, and a user would fix them one build
/// at a time.
pub fn cook(
    project_dir: impl AsRef<Path>,
    entry_scene: &str,
    extra_assets: &[String],
) -> io::Result<CookedProject> {
    let project_dir = project_dir.as_ref();
    // A project need not have an `assets/` directory at all --
    // `games/net-2p-demo/client` keeps its scene and script at its root -- and
    // the index is only consulted to recover a path an asset has moved away
    // from, which such a project has no sidecars to record. So an absent
    // directory means an empty index, not a failure. Any *other* error still
    // is one: it means `assets/` is there and unreadable.
    let index = match scan(project_dir) {
        Ok(index) => index,
        Err(e) if e.kind() == io::ErrorKind::NotFound => AssetIndex::default(),
        Err(e) => return Err(e),
    };

    let mut report = CookedProject::default();
    // The referrer travels with the path: a report that says only "missing"
    // leaves the reader grepping for who asked.
    let mut queue: Vec<(String, String)> = vec![(entry_scene.to_string(), "project.toml".into())];
    // Named by a human rather than guessed at, so these are hard like a scene's
    // references and unlike a script's: a typo in a list somebody wrote on
    // purpose is a mistake worth stopping for. They are walked like anything
    // else, so listing a scene pulls in what that scene references.
    queue.extend(
        extra_assets
            .iter()
            .map(|path| (path.clone(), "project.toml".to_string())),
    );
    // Doubles as the cycle guard — a path already visited is never walked
    // again, so a prefab that references itself terminates.
    let mut visited: BTreeSet<String> = BTreeSet::new();

    while let Some((path, referrer)) = queue.pop() {
        let Some(resolved) = resolve(&path, &index, project_dir) else {
            report.missing.push(MissingReference { referrer, path });
            continue;
        };
        if !visited.insert(resolved.clone()) {
            continue;
        }
        report.assets.insert(resolved.clone());

        let on_disk = project_dir.join(&resolved);
        match extension_of(&resolved) {
            Some(SCENE_EXTENSION) => {
                let text = match std::fs::read_to_string(&on_disk) {
                    Ok(text) => text,
                    Err(e) => {
                        report.problems.push(format!(
                            "{resolved} cannot be read ({e}); its references were not checked"
                        ));
                        continue;
                    }
                };
                // Parsed rather than pattern-matched, for the reason
                // `collect_references` records: the structure is what decides
                // which strings are references, and a text matcher that stops
                // recognising one of a reference's two spellings checks nothing
                // and says nothing.
                let value: ron::Value = match ron::from_str(&text) {
                    Ok(value) => value,
                    Err(e) => {
                        report.problems.push(format!(
                            "{resolved} is not valid RON ({e}); its references were not checked"
                        ));
                        continue;
                    }
                };
                let mut found = Vec::new();
                collect_references(&value, &mut found);
                collect_nested_references(&value, &mut found);
                collect_rootless_references(&value, project_dir, &mut found);
                for Reference { path: next, .. } in found {
                    queue.push((next, resolved.clone()));
                }
            }
            Some(SCRIPT_EXTENSION) => {
                // Reported rather than skipped, for the same reason an
                // unreadable scene is: a script the cook could not read is one
                // whose `loadScene` calls went unseen, and the level behind one
                // of them would go missing from the build in silence.
                let text = match std::fs::read_to_string(&on_disk) {
                    Ok(text) => text,
                    Err(e) => {
                        report.problems.push(format!(
                            "{resolved} cannot be read ({e}); the paths it names were not collected"
                        ));
                        continue;
                    }
                };
                for mention in quoted_paths(&text) {
                    // The guess is soft; its consequences are not. A mention
                    // that resolves is walked like any other reference, so the
                    // scene behind `loadScene("assets/scenes/level2.ron")`
                    // brings its own models with it.
                    if resolve(&mention, &index, project_dir).is_some() {
                        queue.push((mention, resolved.clone()));
                    } else {
                        report.script_mentions.push(ScriptMention {
                            script: resolved.clone(),
                            path: mention,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    report.missing.sort();
    report.missing.dedup();
    report.script_mentions.sort();
    report.script_mentions.dedup();
    Ok(report)
}

/// Cooks `project_dir` and writes a runnable build into `out_dir`.
///
/// `runtime_exe` is the binary to ship. The caller passes it rather than this
/// function finding one, because the only correct answer is
/// [`std::env::current_exe`] — `bsengine-runtime` *is* the shipping runtime, so
/// the process doing the packaging is already the exact binary the game needs —
/// and a library that called `current_exe` itself could not be tested.
///
/// Each asset ships with its `.meta` sidecar. The engine builds its
/// [`AssetIndex`] from those at startup and resolves references through it, so a
/// build without them would resolve differently from the project it was cooked
/// from — failing on exactly the references the cook had just certified.
///
/// Nothing is written when the cook reports a missing reference. A build that
/// exists is a build somebody ships.
///
/// # Errors
///
/// When `assets/` cannot be walked, when `out_dir` exists and is not empty, or
/// when a copy fails.
pub fn package(
    project_dir: impl AsRef<Path>,
    entry_scene: &str,
    extra_assets: &[String],
    runtime_exe: &Path,
    out_dir: &Path,
) -> io::Result<CookedProject> {
    let project_dir = project_dir.as_ref();

    // Refused rather than cleared: a leftover asset from a previous build is
    // precisely what "only the assets it uses" promises not to ship, and
    // deleting a directory the caller named is not a decision a build command
    // should make on its own.
    if out_dir
        .read_dir()
        .is_ok_and(|mut entries| entries.next().is_some())
    {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{} is not empty; remove it and package again (a leftover asset \
                 from a previous build would ship with this one)",
                out_dir.display()
            ),
        ));
    }

    let cooked = cook(project_dir, entry_scene, extra_assets)?;
    if !cooked.is_ok() {
        return Ok(cooked);
    }

    std::fs::create_dir_all(out_dir)?;
    copy(
        &project_dir.join("project.toml"),
        &out_dir.join("project.toml"),
    )?;
    let exe_name = runtime_exe.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "the runtime has no file name")
    })?;
    copy(runtime_exe, &out_dir.join(exe_name))?;

    for asset in &cooked.assets {
        copy(&project_dir.join(asset), &out_dir.join(asset))?;
        // An asset without a sidecar is normal — nothing has scanned the
        // project yet — so only one that exists and cannot be copied is worth
        // failing on.
        let sidecar = format!("{asset}.meta");
        if project_dir.join(&sidecar).is_file() {
            copy(&project_dir.join(&sidecar), &out_dir.join(&sidecar))?;
        }
    }

    Ok(cooked)
}

/// Collects references from RON documents nested *inside* a scene's strings.
///
/// # Why this is needed on top of [`collect_references`]
///
/// A scene stores a reflected component as a `(type name, RON source)` pair, so
/// a `Terrain`'s heightmap and its four layer textures are paths inside a string
/// inside the scene — one document deeper than any walk of the outer value can
/// reach. [`collect_references`] sees only the whole inner document as a single
/// string, rejects it (it does not start with `assets/`), and moves on.
///
/// So a string that is *not itself* a path is tried as RON, and whatever parses
/// is walked with the same collector. The recursion also handles a component
/// nested in a component, which nothing writes today.
///
/// # Why this lives here and not in `fixup`
///
/// `fixup` has the same blind spot, which means it has never rewritten a
/// terrain's heightmap path through a rename. Fixing that is a change to a tool
/// that *edits users' scene files*, and deciding how it should rewrite a path
/// inside an escaped string is its own piece of work — not something to fold
/// into a packaging change. Reading is the safe half, and this is the reading
/// half.
fn collect_nested_references(value: &ron::Value, out: &mut Vec<Reference>) {
    match value {
        ron::Value::Map(map) => map
            .iter()
            .for_each(|(_, value)| collect_nested_references(value, out)),
        ron::Value::Seq(items) => items
            .iter()
            .for_each(|item| collect_nested_references(item, out)),
        ron::Value::Option(Some(inner)) => collect_nested_references(inner, out),
        ron::Value::String(text) if !is_project_relative(text) => {
            // Most strings here are entity names and type names, and a failed
            // or fruitless parse costs nothing. The one shape that must not
            // recurse is a string parsing back to a string: `"abc"` yields
            // `String("abc")`, which would parse to itself forever.
            match ron::from_str::<ron::Value>(text) {
                Ok(ron::Value::String(_)) | Err(_) => {}
                Ok(inner) => {
                    collect_references(&inner, out);
                    collect_nested_references(&inner, out);
                }
            }
        }
        _ => {}
    }
}

/// Collects references that do not live under `assets/` at all.
///
/// # The project shape this exists for
///
/// `games/net-2p-demo/client` has no `assets/` directory: its scene sits at the
/// project root and names its script `scripts: ["main.js"]`. The engine loads
/// that project happily, but [`is_project_relative`] — the guard that keeps an
/// entity *name* from being mistaken for a path — recognises only `assets/…`,
/// so nothing in the identity system has ever seen such a project's references.
/// Without this pass a build of one would ship its scene and omit its script.
///
/// # The rule, and why it cannot produce false positives
///
/// A bare string counts only when **a file of that name is actually there**. An
/// entity called `"Cam"`, a mesh called `"Cube"`, a colour, a type name — none
/// of them name a file, so none of them are collected. Nothing is *lost* by
/// being wrong in the other direction either: a string that happens to match a
/// file ships one file too many, and a build carrying an extra file still runs.
///
/// This deliberately does not report a bare string that resolves to nothing.
/// Under `assets/` the prefix is what makes a string a reference, so an absent
/// file is a broken reference; here existence is the only evidence there is, and
/// treating every non-matching string as a broken reference would fail every
/// project on its own entity names.
fn collect_rootless_references(value: &ron::Value, project_dir: &Path, out: &mut Vec<Reference>) {
    match value {
        ron::Value::Map(map) => map
            .iter()
            .for_each(|(_, value)| collect_rootless_references(value, project_dir, out)),
        ron::Value::Seq(items) => items
            .iter()
            .for_each(|item| collect_rootless_references(item, project_dir, out)),
        ron::Value::Option(Some(inner)) => collect_rootless_references(inner, project_dir, out),
        ron::Value::String(text)
            if !is_project_relative(text) && project_dir.join(text).is_file() =>
        {
            out.push(Reference {
                guid: None,
                path: text.clone(),
            });
        }
        _ => {}
    }
}

/// Where a reference actually loads from, or `None` if nothing answers it.
///
/// The same order [`super::identity::fixup`]'s resolver, `bsengine-scene` and
/// [`crate::load_async`] use — the stored path, then a path the project
/// remembers the asset leaving. A cook that resolved differently from the engine
/// would reject projects that run, and pass ones that do not.
///
/// The reference's own GUID is deliberately not consulted first: `fixup` uses it
/// to decide where a drifted path *should be rewritten to*, whereas the only
/// question here is whether something loads, and a reference whose path is on
/// disk loads from that path.
fn resolve(path: &str, index: &AssetIndex, project_dir: &Path) -> Option<String> {
    if project_dir.join(path).is_file() {
        return Some(path.to_string());
    }
    let guid = index.guid_for_former_path(path)?;
    let current = index.path_for_guid(guid)?;
    project_dir
        .join(current)
        .is_file()
        .then(|| current.to_string())
}

/// The extension of a project-relative path.
fn extension_of(path: &str) -> Option<&str> {
    path.rsplit_once('.').map(|(_, extension)| extension)
}

/// Every quoted string in a script that is spelled the way this crate keys an
/// asset path.
///
/// Both quote characters, because JavaScript uses both and a scan that knew only
/// one would miss half of a real project's calls.
fn quoted_paths(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    for quote in ['"', '\''] {
        let mut rest = text;
        while let Some(open) = rest.find(quote) {
            rest = &rest[open + 1..];
            let Some(close) = rest.find(quote) else { break };
            let candidate = &rest[..close];
            if is_project_relative(candidate) {
                found.push(candidate.to_string());
            }
            rest = &rest[close + 1..];
        }
    }
    found.sort();
    found.dedup();
    found
}

/// Copies one file, creating the destination's parent directories.
fn copy(from: &Path, to: &Path) -> io::Result<()> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(from, to)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A throwaway project, removed on drop.
    struct Probe(PathBuf);

    impl Drop for Probe {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).ok();
        }
    }

    impl Probe {
        fn create() -> Self {
            static N: AtomicU32 = AtomicU32::new(0);
            let dir = std::env::temp_dir().join(format!(
                "bsengine-cook-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).expect("create probe");
            Probe(dir)
        }

        /// Writes a project-relative file, creating parents.
        fn write(&self, relative: &str, contents: &str) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().expect("parent")).expect("create dirs");
            std::fs::write(path, contents).expect("write");
        }

        fn cook(&self) -> CookedProject {
            super::cook(&self.0, "assets/scenes/main.ron", &[]).expect("cook")
        }

        /// A stand-in for the runtime, so `package` has something to copy.
        fn fake_runtime(&self) -> PathBuf {
            let exe = self.0.join("fake-runtime.exe");
            std::fs::write(&exe, "MZ").expect("write exe");
            exe
        }
    }

    /// Both halves matter: a cook that collects nothing passes "the stray is
    /// excluded", and one that collects the whole directory passes "everything
    /// reachable is included". Only the exact set tells a correct cook from
    /// either.
    #[test]
    fn collects_what_the_scene_reaches_and_nothing_else() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), script: Some(Path("assets/scripts/hero.js")))])"#,
        );
        probe.write("assets/models/hero.glb", "glb");
        probe.write("assets/scripts/hero.js", "// hero");
        probe.write("assets/textures/unused.png", "png");

        let cooked = probe.cook();

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        let expected: BTreeSet<String> = [
            "assets/scenes/main.ron",
            "assets/models/hero.glb",
            "assets/scripts/hero.js",
        ]
        .iter()
        .map(|s| (*s).to_string())
        .collect();
        assert_eq!(
            cooked.assets, expected,
            "the cook must collect every reachable asset and no other"
        );
    }

    /// A `Terrain`'s heightmap and layer textures are strings inside a
    /// reflected component, which a scene stores as a `(type name, RON source)`
    /// pair — so the paths are one RON document deep, inside a string. Neither
    /// a field-by-field walk nor a single-level value walk sees them.
    ///
    /// The fixture is copied from `games/terrain-demo/assets/scenes/main.ron`
    /// rather than invented, down to the type name and the escaping: a fixture
    /// in a shape the engine never writes can certify a property the real
    /// format violates.
    #[test]
    fn finds_paths_inside_reflected_components() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Ground", components: [
                ("bsengine_scene::types::Terrain", "(heightmap_path: \"assets/terrain/height.png\", chunk_count: (2, 2), layer0_texture_path: \"assets/terrain/grass.png\", splatmap_path: None)"),
            ])])"#,
        );
        probe.write("assets/terrain/height.png", "png");
        probe.write("assets/terrain/grass.png", "png");

        let cooked = probe.cook();

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        assert!(
            cooked.assets.contains("assets/terrain/height.png")
                && cooked.assets.contains("assets/terrain/grass.png"),
            "paths inside a reflected component must be collected; got {:?}",
            cooked.assets
        );
    }

    /// Prefabs nest, and one that references itself must terminate. The
    /// assertion is what was collected, not merely that it returned: a walk
    /// that bailed out on first sight of a cycle would also return.
    #[test]
    fn follows_nested_prefabs_and_survives_a_cycle() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Spawn", prefab: Some(Path("assets/prefabs/outer.ron")))])"#,
        );
        probe.write(
            "assets/prefabs/outer.ron",
            r#"(entities: [(name: "Inner", prefab: Some(Path("assets/prefabs/inner.ron")))])"#,
        );
        // Points back at its own parent: the cycle.
        probe.write(
            "assets/prefabs/inner.ron",
            r#"(entities: [(name: "Loop", prefab: Some(Path("assets/prefabs/outer.ron")), texture: Some(Path("assets/textures/deep.png")))])"#,
        );
        probe.write("assets/textures/deep.png", "png");

        let cooked = probe.cook();

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        assert!(
            cooked.assets.contains("assets/prefabs/inner.ron")
                && cooked.assets.contains("assets/textures/deep.png"),
            "the walk must reach through nested prefabs to the far side of a \
             cycle, not stop at it; got {:?}",
            cooked.assets
        );
    }

    /// The integrity check. Paired with the success cases above — an
    /// implementation that always reported a missing reference would pass this
    /// test on its own.
    #[test]
    fn a_dangling_scene_reference_is_reported_with_its_referrer() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/gone.glb")))])"#,
        );

        let cooked = probe.cook();

        assert!(!cooked.is_ok(), "a missing model must fail the cook");
        assert_eq!(
            cooked.missing,
            vec![MissingReference {
                referrer: "assets/scenes/main.ron".to_string(),
                path: "assets/models/gone.glb".to_string(),
            }],
            "the report must name both the missing path and the file naming it"
        );
    }

    /// `games/net-2p-demo/client`'s shape, copied rather than invented: no
    /// `assets/` directory, the scene at the project root, and its script named
    /// by a bare `scripts: ["main.js"]`. Both halves are asserted, because the
    /// script is the half that goes missing and the entity name is the half a
    /// too-eager rule would wrongly collect.
    #[test]
    fn a_project_without_an_assets_directory_is_collected() {
        let probe = Probe::create();
        probe.write(
            "scene.ron",
            r#"(entities: [(name: "Cam", scripts: ["main.js"]), (name: "Cube")])"#,
        );
        probe.write("main.js", "// client");

        let cooked = super::cook(&probe.0, "scene.ron", &[]).expect("cook");

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.problems);
        let expected: BTreeSet<String> = ["scene.ron", "main.js"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        assert_eq!(
            cooked.assets, expected,
            "a rootless project's script must be collected, and its entity \
             names must not be mistaken for files"
        );
    }

    /// The escape hatch for what a static walk cannot see. Both halves are
    /// asserted: the listed file comes in, *and* what it in turn references
    /// comes with it — an implementation that merely copied the named file
    /// would pass the first half and ship a scene with no model.
    #[test]
    fn an_explicitly_listed_asset_is_collected_and_walked() {
        let probe = Probe::create();
        probe.write("assets/scenes/main.ron", "(entities: [])");
        probe.write("assets/models/CREDITS.md", "CC-BY, by somebody");
        probe.write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Boss", gltf: Some(Path("assets/models/boss.glb")))])"#,
        );
        probe.write("assets/models/boss.glb", "glb");

        let cooked = super::cook(
            &probe.0,
            "assets/scenes/main.ron",
            &[
                "assets/models/CREDITS.md".to_string(),
                "assets/scenes/level2.ron".to_string(),
            ],
        )
        .expect("cook");

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        assert!(
            cooked.assets.contains("assets/models/CREDITS.md"),
            "a listed file nothing references must still ship; got {:?}",
            cooked.assets
        );
        assert!(
            cooked.assets.contains("assets/models/boss.glb"),
            "a listed scene must be walked like any other, so its references \
             come with it; got {:?}",
            cooked.assets
        );
    }

    /// The list is written by hand, so a typo in it is a mistake rather than a
    /// guess — unlike a path spelled only in a script.
    #[test]
    fn a_listed_asset_that_does_not_exist_fails_the_build() {
        let probe = Probe::create();
        probe.write("assets/scenes/main.ron", "(entities: [])");

        let cooked = super::cook(
            &probe.0,
            "assets/scenes/main.ron",
            &["assets/models/typo.md".to_string()],
        )
        .expect("cook");

        assert!(!cooked.is_ok(), "a listed file that is absent must fail");
        assert_eq!(
            cooked.missing,
            vec![MissingReference {
                referrer: "project.toml".to_string(),
                path: "assets/models/typo.md".to_string(),
            }],
            "and the report must point at project.toml, where it is listed"
        );
    }

    /// A scene the cook cannot parse is a scene whose references went
    /// unchecked, so it fails the build too — but it is a different kind of
    /// trouble from a reference that names something absent, and the report
    /// keeps them apart rather than phrasing one as the other.
    #[test]
    fn an_unparseable_scene_is_a_problem_not_a_missing_reference() {
        let probe = Probe::create();
        probe.write("assets/scenes/main.ron", "(entities: [ this is not RON");

        let cooked = probe.cook();

        assert!(!cooked.is_ok(), "a scene that will not parse must fail");
        assert!(
            cooked.missing.is_empty(),
            "nothing is missing here -- the file is present and broken: {:?}",
            cooked.missing
        );
        assert_eq!(cooked.problems.len(), 1, "got {:?}", cooked.problems);
        assert!(
            cooked.problems[0].contains("assets/scenes/main.ron")
                && cooked.problems[0].contains("not valid RON"),
            "the problem must name the file and say what is wrong with it: {}",
            cooked.problems[0]
        );
    }

    /// A second scene is reachable only through `Bsengine.loadScene`. Without
    /// the script scan a two-level game ships with one level.
    #[test]
    fn a_scene_named_only_by_a_script_is_collected() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", script: Some(Path("assets/scripts/hero.js")))])"#,
        );
        probe.write(
            "assets/scripts/hero.js",
            "function update() { Bsengine.loadScene(\"assets/scenes/level2.ron\"); }",
        );
        probe.write(
            "assets/scenes/level2.ron",
            r#"(entities: [(name: "Boss", gltf: Some(Path("assets/models/boss.glb")))])"#,
        );
        probe.write("assets/models/boss.glb", "glb");

        let cooked = probe.cook();

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        assert!(
            cooked.assets.contains("assets/scenes/level2.ron")
                && cooked.assets.contains("assets/models/boss.glb"),
            "a script-named scene must be walked like any other, so its own \
             references come with it; got {:?}",
            cooked.assets
        );
    }

    /// The guess is soft; its consequences are not.
    #[test]
    fn an_unresolvable_script_path_warns_but_does_not_fail() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", script: Some(Path("assets/scripts/hero.js")))])"#,
        );
        probe.write(
            "assets/scripts/hero.js",
            "// Bsengine.playSound(\"assets/sounds/never.wav\");",
        );

        let cooked = probe.cook();

        assert!(
            cooked.is_ok(),
            "a path spelled only in a script must not fail the build: {:?}",
            cooked.missing
        );
        assert_eq!(
            cooked.script_mentions,
            vec![ScriptMention {
                script: "assets/scripts/hero.js".to_string(),
                path: "assets/sounds/never.wav".to_string(),
            }],
            "but it must be reported, or a real typo is invisible"
        );
    }

    /// The sidecar travels with its asset: the engine builds its `AssetIndex`
    /// from `.meta` files at startup, so a build without them resolves
    /// differently from the project it was cooked from.
    #[test]
    fn writes_each_asset_with_its_sidecar_and_the_manifest() {
        let probe = Probe::create();
        probe.write(
            "project.toml",
            "[project]\nname = \"P\"\nentry_scene = \"assets/scenes/main.ron\"\n",
        );
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")))])"#,
        );
        probe.write("assets/models/hero.glb", "glb");
        probe.write(
            "assets/models/hero.glb.meta",
            "(guid: \"0193a7c1-8f2e-7c44-9d61-3b5a0e7f2c19\", hash: \"blake3:00\", size: None, former_paths: [])",
        );
        probe.write("assets/textures/unused.png", "png");
        let exe = probe.fake_runtime();
        let out = probe.0.join("dist");

        let cooked = package(&probe.0, "assets/scenes/main.ron", &[], &exe, &out).expect("package");

        assert!(cooked.is_ok(), "unexpected problems: {:?}", cooked.missing);
        assert!(out.join("project.toml").is_file(), "the manifest must ship");
        assert!(
            out.join("fake-runtime.exe").is_file(),
            "the executable must ship"
        );
        assert!(
            out.join("assets/models/hero.glb").is_file(),
            "a reached asset must ship"
        );
        assert!(
            out.join("assets/models/hero.glb.meta").is_file(),
            "an asset's sidecar must ship with it, or the packaged build \
             resolves references differently from the project"
        );
        assert!(
            !out.join("assets/textures/unused.png").exists(),
            "an unreachable asset must not ship"
        );
    }

    /// A leftover from a previous run is exactly what "only used assets"
    /// promises not to ship — and deleting a directory the caller named is not
    /// something a build command should decide to do.
    #[test]
    fn refuses_a_non_empty_output_directory() {
        let probe = Probe::create();
        probe.write("assets/scenes/main.ron", "(entities: [])");
        let exe = probe.fake_runtime();
        let out = probe.0.join("dist");
        std::fs::create_dir_all(&out).expect("create out");
        std::fs::write(out.join("stale.txt"), "old").expect("write stale");

        let error = package(&probe.0, "assets/scenes/main.ron", &[], &exe, &out)
            .expect_err("a non-empty output directory must be refused");

        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(
            out.join("stale.txt").is_file(),
            "refusing must not delete what is already there"
        );
    }

    /// Nothing is written when a reference is broken: a build that exists is a
    /// build somebody ships.
    #[test]
    fn writes_nothing_when_a_reference_is_missing() {
        let probe = Probe::create();
        probe.write(
            "assets/scenes/main.ron",
            r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/gone.glb")))])"#,
        );
        let exe = probe.fake_runtime();
        let out = probe.0.join("dist");

        let cooked = package(&probe.0, "assets/scenes/main.ron", &[], &exe, &out).expect("package");

        assert!(!cooked.is_ok(), "the cook must report the missing model");
        assert!(
            !out.exists(),
            "a build with a broken reference must not be written at all"
        );
    }
}
