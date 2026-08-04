//! `fixup`: spending the recovery affordance instead of living on it.
//!
//! Recovering a reference through a former path — [`crate::load_async`] for the
//! paths inside JavaScript string literals, `bsengine-scene` for the ones inside
//! a scene — is a **development-time affordance with an expiry**, and this is
//! the expiry. Both surfaces warn every time they recover, deliberately, because
//! a recovery nobody is told about turns a broken reference into a permanent,
//! invisible indirection layer. `fixup` is what a developer spends that warning
//! on.
//!
//! # Why an expiry exists at all
//!
//! Of the three engines this design looked at, Unreal's documented pain is
//! entirely about forwarding that was never cleaned up: redirectors accumulate
//! and slow asset lookups, the cooker can break a redirector chain so a packaged
//! build fails to load what worked in the editor, and source-control locks make
//! fixing them up unsafe on a team. Godot sidesteps the whole class by treating
//! UIDs as editor-only and invalidating them on export. Neither treats
//! forwarding as a permanent layer. Ours is not permanent either, and this is
//! the "Fix Up Redirectors" that makes that true.
//!
//! # It never rewrites JavaScript
//!
//! A path in a script can be built, concatenated, or shared with a string that
//! is not a path at all — `const NEXT_SCENE = "assets/scenes/" + level + ".ron"`
//! is ordinary — and nothing outside the running script knows which characters
//! are a path. Machine-rewriting them is risk without matching reward, so they
//! are reported instead: file, line, the stale path, and where the asset went,
//! which is everything needed to make the edit mechanical even though the edit
//! itself is a human's.
//!
//! That is also why the pruning rule is what it is: **a former path is forgotten
//! only when nothing `fixup` can read still names it.** A `.js` mention pins it
//! in place indefinitely, because that reference cannot be repaired here and
//! forgetting the path would break it outright; a `.ron` mention that survives
//! the rewrite pass means the rewrite did not take — a read-only scene, a file
//! open in an editor, a scene that will not parse — and the memory is still
//! load-bearing. Pruning is the one irreversible thing this tool does, so it is
//! decided from what is on disk *after* every edit, not from what the edits were
//! supposed to achieve.
//!
//! # It needs no engine
//!
//! Everything here is a directory walk, a RON parse and a text edit. There is no
//! `App`, no `World` and no running game, so `fixup` works on a project that has
//! never been launched — which is what makes it usable from a script, from CI,
//! or from an agent that has no session open.

use super::index::AssetIndex;
use super::scan::{scan, ASSETS_DIR, RECORDINGS_DIR};
use super::sidecar::{Sidecar, SIDECAR_EXTENSION};
use super::AssetGuid;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// Extension of the files `fixup` rewrites: scenes, and anything else stored as
/// RON beside them.
const SCENE_EXTENSION: &str = "ron";

/// Extension of the files `fixup` reads and reports on but never writes.
const SCRIPT_EXTENSION: &str = "js";

/// What one `fixup` run did and could not do.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FixupReport {
    /// Stale scene references rewritten to where the asset now lives.
    pub rewritten: Vec<Rewrite>,
    /// Stale paths found in JavaScript, which `fixup` never edits.
    pub scripts: Vec<ScriptReference>,
    /// Former paths removed from a sidecar because nothing needs them.
    pub pruned: Vec<Pruned>,
    /// Former paths kept, and what is still holding each one in place.
    pub retained: Vec<Retained>,
    /// Everything `fixup` tried to do and could not.
    pub problems: Vec<String>,
}

/// One stale scene reference rewritten in place.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Rewrite {
    /// Project-relative path of the scene file that was edited.
    pub file: String,
    /// 1-based line the reference sits on.
    pub line: usize,
    /// The path the reference used to name.
    pub from: String,
    /// The path it names now.
    pub to: String,
}

/// One stale path spelled inside a JavaScript source file.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ScriptReference {
    /// Project-relative path of the script.
    pub file: String,
    /// 1-based line the stale path appears on.
    pub line: usize,
    /// The path as spelled in the script.
    pub stale_path: String,
    /// Where the asset that used to be there lives now.
    pub now_at: String,
}

/// One former path forgotten.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Pruned {
    /// Project-relative path of the sidecar it was removed from.
    pub sidecar: String,
    /// The path the asset will no longer answer to.
    pub former_path: String,
}

/// One former path deliberately kept.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Retained {
    /// Project-relative path of the sidecar that keeps it.
    pub sidecar: String,
    /// The path still being remembered.
    pub former_path: String,
    /// What is still holding it in place.
    pub because: String,
}

impl FixupReport {
    /// Whether the run had nothing at all to say.
    pub fn is_empty(&self) -> bool {
        self.rewritten.is_empty()
            && self.scripts.is_empty()
            && self.pruned.is_empty()
            && self.retained.is_empty()
            && self.problems.is_empty()
    }
}

/// Reads like the report a human wants back from a tool that edited their
/// source tree: what changed, what they have to change themselves, and what it
/// could not do.
impl fmt::Display for FixupReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return writeln!(
                f,
                "fixup: nothing to do — no reference names a path an asset left"
            );
        }
        for rewrite in &self.rewritten {
            writeln!(
                f,
                "rewrote  {}:{}  '{}' -> '{}'",
                rewrite.file, rewrite.line, rewrite.from, rewrite.to
            )?;
        }
        for script in &self.scripts {
            writeln!(
                f,
                "BY HAND  {}:{}  names '{}', which is now at '{}' \
                 (fixup never edits JavaScript)",
                script.file, script.line, script.stale_path, script.now_at
            )?;
        }
        for pruned in &self.pruned {
            writeln!(
                f,
                "forgot   {}  no longer answers to '{}'",
                pruned.sidecar, pruned.former_path
            )?;
        }
        for retained in &self.retained {
            writeln!(
                f,
                "kept     {}  still answers to '{}': {}",
                retained.sidecar, retained.former_path, retained.because
            )?;
        }
        for problem in &self.problems {
            writeln!(f, "PROBLEM  {problem}")?;
        }
        writeln!(
            f,
            "\n{} reference(s) rewritten, {} to fix by hand, {} former path(s) \
             forgotten, {} kept, {} problem(s)",
            self.rewritten.len(),
            self.scripts.len(),
            self.pruned.len(),
            self.retained.len(),
            self.problems.len()
        )
    }
}

/// Rewrites what it can, reports what it must not, and forgets what nothing
/// needs any more.
///
/// Four passes, in this order because each depends on the one before:
///
/// 1. **Scan.** `fixup` resolves against exactly the [`AssetIndex`] the engine
///    would build, so a reference it leaves alone is one the engine resolves and
///    a reference it rewrites is one the engine was already recovering. Using a
///    different rule here would let the tool "fix" a project into behaving
///    differently from the one it was run on.
/// 2. **Rewrite scenes**, repointing every reference that only resolves through
///    a former path — keeping its GUID, which is the part that survives the
///    *next* rename.
/// 3. **Report scripts**, without touching one byte of them.
/// 4. **Prune**, from what is on disk once passes 2 and 3 are done.
///
/// # Errors
///
/// Only when `<project_dir>/assets` cannot be walked at all, which is
/// [`scan`]'s one error and almost always means the wrong directory was named.
/// Everything found *inside* the project is a line in the report instead: a
/// scene that will not parse, a file that cannot be written, a reference too
/// ambiguous to touch. A tool that stopped at the first of those would leave a
/// project half-fixed, which is worse than one that finishes and says what it
/// could not do.
pub fn fixup(project_dir: impl AsRef<Path>) -> io::Result<FixupReport> {
    let project_dir = project_dir.as_ref();
    let index = scan(project_dir)?;

    let mut report = FixupReport::default();
    let project = Project::collect(project_dir, &mut report);
    let redirects = redirects(project_dir, &index, &project);

    rewrite_scenes(&project, &index, &redirects, &mut report);
    if !report.rewritten.is_empty() {
        // A rewritten scene is an asset whose contents changed, so its own
        // sidecar's recorded hash and size are now stale. Left that way, the
        // *next* run's scan would refresh them — meaning a second `fixup` on an
        // already-fixed project would still write to the source tree, and
        // "safe to run twice" would be false in the one way nobody looks for.
        // Rescanning here spends one re-hash of the files this run edited and
        // puts that right inside the run that caused it. Nothing downstream
        // reads the index, but the sidecars on disk are now current, which is
        // why `prune` re-reads each one instead of trusting what the walk saw.
        scan(project_dir)?;
    }
    report_scripts(&project, &redirects, &mut report);
    prune(&project, &mut report);

    Ok(report)
}

/// One project's files, located once and shared by every pass.
struct Project {
    /// Every `.ron` and `.js` under `assets/`, project-relative path first.
    texts: Vec<TextFile>,
    /// Every `.meta` under `assets/`, with the sidecar it holds.
    sidecars: Vec<SidecarFile>,
}

/// A file `fixup` reads: a scene it may rewrite, or a script it never will.
struct TextFile {
    /// Project-relative, forward-slashed — the spelling a reference uses.
    relative: String,
    /// Where to actually open it.
    path: PathBuf,
    /// Whether this is JavaScript, and therefore read-only to this tool.
    is_script: bool,
}

/// A sidecar and where it lives.
struct SidecarFile {
    /// Project-relative path of the `.meta` itself.
    relative: String,
    /// Where to actually write it.
    path: PathBuf,
    /// What it currently says.
    sidecar: Sidecar,
}

impl Project {
    /// Walks `<project_dir>/assets` once, collecting the files every pass
    /// needs.
    ///
    /// Skips exactly what [`scan`] skips — `assets/tests/`, the E2E recordings
    /// — because the prune decision is "nothing in the project names this", and
    /// two walks that disagreed about what the project contains would make that
    /// answer depend on which one asked.
    fn collect(project_dir: &Path, report: &mut FixupReport) -> Self {
        let mut project = Self {
            texts: Vec::new(),
            sidecars: Vec::new(),
        };
        project.walk(&project_dir.join(ASSETS_DIR), ASSETS_DIR, report);
        project
    }

    fn walk(&mut self, dir: &Path, prefix: &str, report: &mut FixupReport) {
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) => {
                report.problems.push(format!(
                    "cannot open {prefix} ({e}); nothing inside it was checked"
                ));
                return;
            }
        };
        // Sorted, so a report reads the same on every machine and in every
        // filesystem's iteration order.
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);

        for entry in entries {
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let relative = format!("{prefix}/{name}");
            let path = entry.path();
            let Ok(file_type) = entry.file_type() else {
                continue;
            };

            if file_type.is_dir() {
                if prefix == ASSETS_DIR && name == RECORDINGS_DIR {
                    continue;
                }
                self.walk(&path, &relative, report);
                continue;
            }
            if !file_type.is_file() {
                continue;
            }

            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            match extension.as_str() {
                SIDECAR_EXTENSION => match Sidecar::read(&path) {
                    Ok(Some(sidecar)) => self.sidecars.push(SidecarFile {
                        relative,
                        path,
                        sidecar,
                    }),
                    // Absent is impossible — it was just walked — and broken is
                    // scan's to warn about. Either way this sidecar's former
                    // paths cannot be read, so none of them is pruned.
                    Ok(None) => {}
                    Err(e) => report.problems.push(format!(
                        "{relative} cannot be read ({e}); it was left alone"
                    )),
                },
                SCENE_EXTENSION | SCRIPT_EXTENSION => self.texts.push(TextFile {
                    relative,
                    path,
                    is_script: extension == SCRIPT_EXTENSION,
                }),
                _ => {}
            }
        }
    }
}

/// Every path an asset has moved away from, mapped to where it went.
///
/// The lookup is [`AssetIndex::guid_for_former_path`] followed by
/// [`AssetIndex::path_for_guid`] and then the filesystem — the same three steps,
/// in the same order, that `bsengine-scene` and [`crate::load_async`] use to
/// recover a reference at run time. That is the point: this map holds exactly
/// the redirects the engine is *already performing silently*, so rewriting one
/// changes what a file says without changing what it loads.
///
/// A former path two assets claim answers for neither (the index refuses to pick
/// a winner), and one some live asset now occupies is not stale at all — a file
/// that is there beats the memory of the one that left. Both simply do not
/// appear here, which means they are never rewritten, never reported, and — for
/// want of anything that made them unnecessary — never pruned either.
fn redirects(
    project_dir: &Path,
    index: &AssetIndex,
    project: &Project,
) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for file in &project.sidecars {
        for former in &file.sidecar.former_paths {
            if out.contains_key(former) {
                continue;
            }
            let Some(guid) = index.guid_for_former_path(former) else {
                continue;
            };
            let Some(current) = index.path_for_guid(guid) else {
                continue;
            };
            if project_dir.join(former).exists() {
                continue;
            }
            out.insert(former.clone(), current.to_string());
        }
    }
    out
}

/// One asset reference as a scene file spells it.
struct Reference {
    /// The identity, if the reference carries one.
    guid: Option<String>,
    /// The path it names.
    path: String,
}

/// Repoints every stale reference in every scene at where its asset now lives.
fn rewrite_scenes(
    project: &Project,
    index: &AssetIndex,
    redirects: &BTreeMap<String, String>,
    report: &mut FixupReport,
) {
    for file in project.texts.iter().filter(|file| !file.is_script) {
        let text = match std::fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(e) => {
                report.problems.push(format!(
                    "{} cannot be read ({e}); its references were not checked",
                    file.relative
                ));
                continue;
            }
        };
        // Parsed rather than pattern-matched, for the reason `game_validate`
        // learned the hard way: a reference has two spellings, and a text
        // matcher that stops recognising one of them checks nothing and says
        // nothing. The structure is what decides *which* strings are
        // references; the text edit below is only how the decision is applied.
        let value: ron::Value = match ron::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                report.problems.push(format!(
                    "{} is not valid RON ({e}); its references were not checked, \
                     and every former path it names stays remembered",
                    file.relative
                ));
                continue;
            }
        };

        let mut references = Vec::new();
        collect_references(&value, &mut references);

        let mut plans: BTreeMap<String, String> = BTreeMap::new();
        let mut ambiguous: BTreeSet<String> = BTreeSet::new();
        for reference in references {
            let Some(to) = destination(&reference, index, redirects) else {
                continue;
            };
            match plans.get(&reference.path) {
                // One spelling, two destinations: the same path referenced
                // twice under different identities. Rewriting the text would
                // have to pick one, and a wrong pick is silent, so neither is
                // touched and the contradiction is named.
                Some(existing) if *existing != to => {
                    ambiguous.insert(reference.path);
                }
                _ => {
                    plans.insert(reference.path, to);
                }
            }
        }
        for path in &ambiguous {
            plans.remove(path);
            report.problems.push(format!(
                "{} references '{path}' under two identities that live in \
                 different places; rewriting it would have to guess, so it was \
                 left alone",
                file.relative
            ));
        }
        if plans.is_empty() {
            continue;
        }

        let (rewritten, hits) = apply(&text, &plans);
        if hits.is_empty() {
            continue;
        }
        match std::fs::write(&file.path, &rewritten) {
            Ok(()) => report.rewritten.extend(hits.into_iter().map(|hit| Rewrite {
                file: file.relative.clone(),
                line: hit.line,
                from: hit.from,
                to: hit.to,
            })),
            // A scene checked out read-only, locked by source control, or open
            // in an editor that holds it. Reported rather than retried, and —
            // because the file still names the old path — the former path it
            // needs survives the prune pass below on its own.
            Err(e) => report.problems.push(format!(
                "{} has {} stale reference(s) but could not be written ({e}); \
                 they are unchanged",
                file.relative,
                hits.len()
            )),
        }
    }
}

/// Where a reference should point, or `None` if it already points there.
///
/// Deliberately the same order [`bsengine_scene`'s resolver and
/// [`crate::load_async`] use — identity, then the stored path, then a path the
/// project remembers the asset leaving. A tool that resolved in a different
/// order from the engine would rewrite a file to name something other than what
/// it currently loads, which is the one thing a fixer must never do.
///
/// [`bsengine_scene`]: https://docs.rs/bsengine-scene
fn destination(
    reference: &Reference,
    index: &AssetIndex,
    redirects: &BTreeMap<String, String>,
) -> Option<String> {
    if let Some(guid_text) = &reference.guid {
        if let Ok(guid) = guid_text.parse::<AssetGuid>() {
            match index.path_for_guid(guid) {
                // The identity is live and the path has drifted: this is the
                // rename item 30 exists to survive, and rewriting it is exactly
                // the "re-save the scene to update the stored path" the engine
                // asks for every time it resolves this reference.
                Some(current) if current != reference.path => return Some(current.to_string()),
                Some(_) => return None,
                // A stale identity falls through to the path, which may itself
                // be recoverable — an asset deleted and its replacement moved
                // into place produces exactly this pair.
                None => {}
            }
        }
        // An identity that is not a GUID at all falls through the same way: it
        // is a spelling to correct by hand, and the path is all there is to go
        // on meanwhile.
    }
    redirects.get(&reference.path).cloned()
}

/// Collects every asset reference a parsed scene holds, in both spellings.
///
/// A bare string counts only when it is spelled the way the index keys a path —
/// project-relative, starting at `assets/`. Every reference in every scene is,
/// by construction, and the guard is what keeps an entity *name* that happens to
/// match a former path out of the rewrite.
fn collect_references(value: &ron::Value, out: &mut Vec<Reference>) {
    match value {
        ron::Value::Map(map) => {
            let field = |wanted: &str| {
                map.iter().find_map(|(key, value)| match (key, value) {
                    (ron::Value::String(key), ron::Value::String(text)) if key == wanted => {
                        Some(text.clone())
                    }
                    _ => None,
                })
            };
            // A map with a `path` is the `(guid: "…", path: "…")` pair, and is
            // not recursed into: its `path` is one reference, reported once.
            if let Some(path) = field("path") {
                out.push(Reference {
                    guid: field("guid"),
                    path,
                });
                return;
            }
            for (_, value) in map.iter() {
                collect_references(value, out);
            }
        }
        ron::Value::Seq(items) => items.iter().for_each(|item| collect_references(item, out)),
        ron::Value::Option(Some(inner)) => collect_references(inner, out),
        ron::Value::String(text) if is_project_relative(text) => out.push(Reference {
            guid: None,
            path: text.clone(),
        }),
        _ => {}
    }
}

/// Whether a string is spelled the way this crate keys an asset path.
fn is_project_relative(text: &str) -> bool {
    text.strip_prefix(ASSETS_DIR)
        .is_some_and(|rest| rest.starts_with('/'))
}

/// One quoted path replaced, and where.
struct Hit {
    line: usize,
    from: String,
    to: String,
}

/// Applies every planned replacement to a scene's text in a single pass.
///
/// # Why a text edit rather than a re-serialisation
///
/// Because the file belongs to the user. Parsing a scene and writing it back out
/// would reformat every line of it, reflow the layout somebody chose, and drop
/// their comments — turning a two-character fix into a whole-file diff nobody
/// can review. Only the quoted paths that were found *structurally* are touched;
/// every other byte, including line endings, comes through unchanged.
///
/// # Why one pass
///
/// Applying replacements one after another would let them cascade: rename `a` to
/// `b` and `b` to `c` in one project, and a second `replace` would carry the
/// first's output into the second's input. Here each replacement's text is
/// written to the output and never looked at again. Needles are tried longest
/// first for the same class of reason — so a path that is a prefix of another
/// cannot claim it.
fn apply(text: &str, plans: &BTreeMap<String, String>) -> (String, Vec<Hit>) {
    let mut needles: Vec<(String, &String, &String)> = plans
        .iter()
        .map(|(from, to)| (format!("\"{from}\""), from, to))
        .collect();
    needles.sort_by_key(|(needle, _, _)| std::cmp::Reverse(needle.len()));

    let mut out = String::with_capacity(text.len());
    let mut hits = Vec::new();
    // `split_inclusive` keeps each line's terminator with it, so CRLF survives
    // on a project whose scenes were written on Windows and a file that does
    // not end in a newline does not gain one.
    for (index, line) in text.split_inclusive('\n').enumerate() {
        let mut rest = line;
        while !rest.is_empty() {
            match needles
                .iter()
                .find(|(needle, _, _)| rest.starts_with(needle.as_str()))
            {
                Some((needle, from, to)) => {
                    out.push('"');
                    out.push_str(to);
                    out.push('"');
                    hits.push(Hit {
                        line: index + 1,
                        from: (*from).clone(),
                        to: (*to).clone(),
                    });
                    rest = &rest[needle.len()..];
                }
                None => {
                    let character = rest.chars().next().expect("rest is not empty");
                    out.push(character);
                    rest = &rest[character.len_utf8()..];
                }
            }
        }
    }
    (out, hits)
}

/// Reports every stale path spelled in a script, and edits none of them.
fn report_scripts(
    project: &Project,
    redirects: &BTreeMap<String, String>,
    report: &mut FixupReport,
) {
    if redirects.is_empty() {
        return;
    }
    for file in project.texts.iter().filter(|file| file.is_script) {
        let text = match std::fs::read_to_string(&file.path) {
            Ok(text) => text,
            Err(e) => {
                report.problems.push(format!(
                    "{} cannot be read ({e}); it was not checked for stale paths",
                    file.relative
                ));
                continue;
            }
        };
        for (index, line) in text.lines().enumerate() {
            for (stale, now_at) in redirects {
                if line.contains(stale.as_str()) {
                    report.scripts.push(ScriptReference {
                        file: file.relative.clone(),
                        line: index + 1,
                        stale_path: stale.clone(),
                        now_at: now_at.clone(),
                    });
                }
            }
        }
    }
}

/// Forgets every former path nothing names any more, and says what is holding
/// the rest.
///
/// # The rule
///
/// A former path is pruned when **no `.ron` and no `.js` file under `assets/`
/// still contains it**, read from disk after the rewrite pass has finished. Not
/// "after fixup planned a rewrite" — after the write actually landed. A scene
/// that could not be written still names the old path and still needs it
/// remembered, and deciding from intent rather than from the disk is how a tool
/// prunes the one record that was still doing something.
///
/// The match is a plain substring, which errs towards keeping: a former path
/// that happens to be a prefix of some other string pins itself needlessly,
/// which costs a line in a `.meta` file, whereas the opposite mistake costs a
/// reference that silently stops resolving.
///
/// # When it prunes nothing at all
///
/// If any of those files could not be read, no former path in the project is
/// pruned. "Nothing names this" is not a conclusion that survives not having
/// looked everywhere, and the alternative — prune what the readable files do not
/// mention — is a coin flip on the contents of the file that failed.
fn prune(project: &Project, report: &mut FixupReport) {
    let mut readable: Vec<(&TextFile, String)> = Vec::new();
    let mut unreadable: Vec<&str> = Vec::new();
    for file in &project.texts {
        match std::fs::read_to_string(&file.path) {
            Ok(text) => readable.push((file, text)),
            Err(_) => unreadable.push(file.relative.as_str()),
        }
    }
    let blocked = (!unreadable.is_empty()).then(|| {
        format!(
            "nothing was pruned anywhere: {} could not be read, and a former \
             path cannot be shown to be unused by files that were not looked at",
            unreadable.join(", ")
        )
    });
    if let Some(reason) = &blocked {
        report.problems.push(reason.clone());
    }

    for file in &project.sidecars {
        if file.sidecar.former_paths.is_empty() {
            continue;
        }
        // Re-read rather than reused from the walk: the rescan above refreshes
        // the recorded hash and size of every asset this run edited, and a
        // scene that both moved *and* held a stale reference would otherwise
        // have that refresh silently written back out of date here.
        let current = match Sidecar::read(&file.path) {
            Ok(Some(current)) => current,
            Ok(None) | Err(_) => continue,
        };

        let mut kept = Vec::new();
        let mut pruned = Vec::new();
        for former in &current.former_paths {
            match blocked.clone().or_else(|| mention(&readable, former)) {
                Some(because) => {
                    report.retained.push(Retained {
                        sidecar: file.relative.clone(),
                        former_path: former.clone(),
                        because,
                    });
                    kept.push(former.clone());
                }
                None => pruned.push(former.clone()),
            }
        }
        if pruned.is_empty() {
            continue;
        }

        let updated = Sidecar {
            former_paths: kept,
            ..current
        };
        match updated.write(&file.path) {
            Ok(()) => report
                .pruned
                .extend(pruned.into_iter().map(|former_path| Pruned {
                    sidecar: file.relative.clone(),
                    former_path,
                })),
            Err(e) => report.problems.push(format!(
                "{} still remembers {} path(s) nothing names, but could not be \
                 written ({e})",
                file.relative,
                pruned.len()
            )),
        }
    }
}

/// The first file and line still naming `former`, phrased as the reason it is
/// being kept.
fn mention(readable: &[(&TextFile, String)], former: &str) -> Option<String> {
    readable.iter().find_map(|(file, text)| {
        text.lines()
            .position(|line| line.contains(former))
            .map(|line| {
                let what = if file.is_script {
                    "which fixup does not rewrite"
                } else {
                    "which fixup did not rewrite"
                };
                format!("{}:{} names it, {what}", file.relative, line + 1)
            })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::sidecar::{sidecar_path, Sidecar};
    use crate::test_support::{unique, ProbeDir};
    use std::path::PathBuf;

    /// The asset a scene points at, and the path it moved away from.
    const MODEL_NOW: &str = "assets/models/fox.glb";
    const MODEL_WAS: &str = "assets/models/old_fox.glb";
    /// The asset a *script* points at. A second asset rather than the same one
    /// so the two halves of this feature -- rewrite the scene, report the
    /// script -- cannot pass by accident on one shared record.
    const SOUND_NOW: &str = "assets/sounds/hit.wav";
    const SOUND_WAS: &str = "assets/sounds/thud.wav";

    fn write(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().expect("probe path has a parent"))
            .expect("create probe directories");
        std::fs::write(path, contents).expect("write probe file");
    }

    /// Moves an already-identified asset, exactly as `identity::rename` does
    /// when the watcher sees one move while the engine is running: the sidecar
    /// travels with the asset and remembers where it came from.
    fn move_asset(project: &Path, from: &str, to: &str) {
        let (from_abs, to_abs) = (project.join(from), project.join(to));
        std::fs::create_dir_all(to_abs.parent().expect("parent")).expect("create directories");
        std::fs::rename(&from_abs, &to_abs).expect("move the asset");
        let (from_meta, to_meta) = (sidecar_path(&from_abs), sidecar_path(&to_abs));
        std::fs::rename(&from_meta, &to_meta).expect("move the sidecar");
        let mut sidecar = Sidecar::read(&to_meta)
            .expect("read the sidecar")
            .expect("the scan must have written one");
        sidecar.remember_former_path(from);
        sidecar.write(&to_meta).expect("write the sidecar");
    }

    /// A project in which one asset a scene names and one asset a script names
    /// have both moved, and both references still spell the old path.
    struct Probe {
        dir: ProbeDir,
        scene: PathBuf,
        script: PathBuf,
    }

    impl Probe {
        fn create(tag: &str) -> Self {
            let dir = ProbeDir(std::env::temp_dir().join(unique(tag)));
            let project = &dir.0;
            write(&project.join(MODEL_WAS), "fake glb");
            write(&project.join(SOUND_WAS), "fake wav");
            // Minted here, so both assets carry a real identity before they
            // move -- which is what makes the move recoverable at all.
            scan(project).expect("seed scan");

            move_asset(project, MODEL_WAS, MODEL_NOW);
            move_asset(project, SOUND_WAS, SOUND_NOW);

            let scene = project.join("assets/scenes/main.ron");
            let script = project.join("assets/scripts/player.js");
            Self {
                scene: scene.clone(),
                script: script.clone(),
                dir,
            }
        }

        fn project(&self) -> &Path {
            &self.dir.0
        }

        /// The identity the scan gave the asset now at `path`.
        fn guid_of(&self, path: &str) -> String {
            Sidecar::read(sidecar_path(self.project().join(path)))
                .expect("read sidecar")
                .expect("sidecar present")
                .guid
                .to_string()
        }

        fn former_paths_of(&self, path: &str) -> Vec<String> {
            Sidecar::read(sidecar_path(self.project().join(path)))
                .expect("read sidecar")
                .expect("sidecar present")
                .former_paths
        }
    }

    /// A scene naming `path`, with `guid` if one is given -- both spellings a
    /// scene reference has.
    fn scene_text(guid: Option<&str>, path: &str) -> String {
        let reference = match guid {
            Some(guid) => format!(r#"(guid: "{guid}", path: "{path}")"#),
            None => format!(r#""{path}""#),
        };
        format!(
            "SceneDescriptor(entities: [\n    \
             EntityDescriptor(name: \"Fox\", gltf: Some({reference})),\n])\n"
        )
    }

    // ---- the four things one run has to do -------------------------------

    /// The whole of task 3 in one run: the scene reference is repointed at
    /// where the asset actually is, its identity is untouched, the script is
    /// *reported* rather than edited, and the former path the rewrite made
    /// unnecessary is forgotten.
    #[test]
    fn one_run_rewrites_the_scene_reports_the_script_and_prunes_what_it_settled() {
        let probe = Probe::create("fixup-round-trip");
        let guid = probe.guid_of(MODEL_NOW);
        write(&probe.scene, &scene_text(Some(&guid), MODEL_WAS));
        write(
            &probe.script,
            &format!(
                "function onUpdate(self) {{\n  \
                 Bsengine.playSound(\"{SOUND_WAS}\");\n}}\n"
            ),
        );
        let script_before = std::fs::read(&probe.script).expect("read script");

        let report = fixup(probe.project()).expect("fixup");

        // 1. The scene now names where the asset is, and keeps its identity.
        let scene = std::fs::read_to_string(&probe.scene).expect("read scene");
        assert!(
            scene.contains(MODEL_NOW) && !scene.contains(MODEL_WAS),
            "the stale reference was not rewritten:\n{scene}"
        );
        assert!(
            scene.contains(&guid),
            "the rewrite dropped the identity, which is the one part of a \
             reference that survives the next rename:\n{scene}"
        );
        assert_eq!(
            report.rewritten.len(),
            1,
            "a run has to say what it changed: {report:?}"
        );
        let rewrite = &report.rewritten[0];
        assert_eq!(rewrite.from, MODEL_WAS);
        assert_eq!(rewrite.to, MODEL_NOW);
        assert!(rewrite.file.ends_with("scenes/main.ron"), "{rewrite:?}");
        assert_eq!(rewrite.line, 2, "{rewrite:?}");

        // 2. The script is untouched, byte for byte.
        assert_eq!(
            std::fs::read(&probe.script).expect("read script"),
            script_before,
            "fixup must never rewrite JavaScript: a path there can be built, \
             concatenated or shared with a non-asset string, so machine-editing \
             it is risk without matching reward"
        );

        // 3. ...and reported well enough to act on by hand.
        assert_eq!(report.scripts.len(), 1, "{report:?}");
        let found = &report.scripts[0];
        assert!(found.file.ends_with("scripts/player.js"), "{found:?}");
        assert_eq!(found.line, 2, "{found:?}");
        assert_eq!(found.stale_path, SOUND_WAS);
        assert_eq!(
            found.now_at, SOUND_NOW,
            "a report that does not say where the asset went leaves the user \
             to go and find it"
        );

        // 4. The former path the rewrite made unnecessary is gone.
        assert_eq!(
            probe.former_paths_of(MODEL_NOW),
            Vec::<String>::new(),
            "nothing names the old model path any more, so remembering it \
             forever is the accumulated forwarding this task exists to end"
        );
        assert!(
            report
                .pruned
                .iter()
                .any(|p| p.former_path == MODEL_WAS && p.sidecar.contains("fox.glb")),
            "a prune has to be reported, not done silently: {report:?}"
        );

        // ...and the one a script still needs is not.
        assert_eq!(
            probe.former_paths_of(SOUND_NOW),
            vec![SOUND_WAS.to_string()],
            "a former path a .js file still names must survive: fixup cannot \
             rewrite that reference, so forgetting it would break the script"
        );
        assert!(
            report
                .retained
                .iter()
                .any(|r| r.former_path == SOUND_WAS && r.because.contains("player.js")),
            "a former path kept has to say what is holding it: {report:?}"
        );
        assert!(report.problems.is_empty(), "{report:?}");
    }

    /// The requirement most likely to be quietly violated by a helpful
    /// implementation, asserted on its own so it fails alone and loudly.
    #[test]
    fn a_javascript_file_is_never_modified_even_when_it_is_the_only_thing_stale() {
        let probe = Probe::create("fixup-js-untouched");
        write(
            &probe.script,
            &format!(
                "const MODEL = \"{MODEL_WAS}\";\n\
                 function onUpdate(self) {{ Bsengine.playSound(\"{SOUND_WAS}\"); }}\n"
            ),
        );
        let before = std::fs::read(&probe.script).expect("read script");

        let report = fixup(probe.project()).expect("fixup");

        assert_eq!(
            std::fs::read(&probe.script).expect("read script"),
            before,
            "fixup rewrote a JavaScript file"
        );
        assert_eq!(
            report.scripts.len(),
            2,
            "both stale paths have to be reported, one per line: {report:?}"
        );
        assert!(report
            .scripts
            .iter()
            .any(|s| s.line == 1 && s.stale_path == MODEL_WAS));
        assert!(report
            .scripts
            .iter()
            .any(|s| s.line == 2 && s.stale_path == SOUND_WAS));
    }

    // ---- what pruning is allowed to touch --------------------------------

    /// The pruning rule, stated as a test: a former path a script names is
    /// kept *because* the script names it, not because of what kind of asset
    /// it is. Same asset, same sidecar, one run with the script and one
    /// without.
    #[test]
    fn a_former_path_is_pruned_exactly_when_nothing_still_names_it() {
        for (tag, script, expected) in [
            (
                "named",
                format!("Bsengine.playSound(\"{SOUND_WAS}\");\n"),
                vec![SOUND_WAS.to_string()],
            ),
            (
                "unnamed",
                "function onUpdate(self) {}\n".to_string(),
                Vec::new(),
            ),
        ] {
            let probe = Probe::create(&format!("fixup-prune-{tag}"));
            write(&probe.script, &script);

            fixup(probe.project()).expect("fixup");

            assert_eq!(
                probe.former_paths_of(SOUND_NOW),
                expected,
                "{tag}: pruning must follow what still names the path"
            );
        }
    }

    // ---- running it twice -------------------------------------------------

    /// A tool that edits source files has to be safe to run again. The second
    /// run must find nothing left to change and leave every byte alone.
    #[test]
    fn a_second_run_changes_nothing() {
        let probe = Probe::create("fixup-idempotent");
        let guid = probe.guid_of(MODEL_NOW);
        write(&probe.scene, &scene_text(Some(&guid), MODEL_WAS));
        write(
            &probe.script,
            &format!("function onUpdate(self) {{ Bsengine.playSound(\"{SOUND_WAS}\"); }}\n"),
        );

        fixup(probe.project()).expect("first run");
        let scene_after_first = std::fs::read(&probe.scene).expect("read scene");
        let script_after_first = std::fs::read(&probe.script).expect("read script");
        let sound_meta_after_first =
            std::fs::read(sidecar_path(probe.project().join(SOUND_NOW))).expect("read meta");
        // The scene's *own* sidecar, which the first run's rewrite invalidated:
        // its recorded size no longer matched the file, so a second run's scan
        // would re-hash and rewrite it. That is a write to the user's source
        // tree by a run that found nothing to do, and nothing else here would
        // notice it.
        let scene_meta_after_first =
            std::fs::read(sidecar_path(&probe.scene)).expect("read scene meta");

        let second = fixup(probe.project()).expect("second run");

        assert!(
            second.rewritten.is_empty() && second.pruned.is_empty(),
            "the second run found work to do, so the first did not finish it: {second:?}"
        );
        assert_eq!(
            std::fs::read(&probe.scene).expect("read scene"),
            scene_after_first
        );
        assert_eq!(
            std::fs::read(&probe.script).expect("read script"),
            script_after_first
        );
        assert_eq!(
            std::fs::read(sidecar_path(probe.project().join(SOUND_NOW))).expect("read meta"),
            sound_meta_after_first,
            "a sidecar rewritten with the same contents is still a modified \
             file in the user's source tree and a line in their next diff"
        );
        assert_eq!(
            std::fs::read(sidecar_path(&probe.scene)).expect("read scene meta"),
            scene_meta_after_first,
            "the first run rewrote the scene without bringing the scene's own \
             sidecar back in step, so every later run re-hashes and rewrites it"
        );
        assert!(
            !second.scripts.is_empty(),
            "the script reference is still there and still unfixed, so it must \
             still be reported -- silence would read as 'resolved'"
        );
    }

    // ---- what it must not touch ------------------------------------------

    /// A reference that resolves is not fixup's business, whichever spelling
    /// it is written in. Without this, an implementation that rewrote every
    /// reference to its indexed path would pass every test above.
    #[test]
    fn a_reference_that_already_resolves_is_left_exactly_as_written() {
        let probe = Probe::create("fixup-leaves-good-alone");
        let guid = probe.guid_of(MODEL_NOW);
        for text in [
            scene_text(Some(&guid), MODEL_NOW),
            scene_text(None, MODEL_NOW),
        ] {
            write(&probe.scene, &text);

            let report = fixup(probe.project()).expect("fixup");

            assert_eq!(
                std::fs::read_to_string(&probe.scene).expect("read scene"),
                text,
                "a reference that resolves must not be reformatted or repointed"
            );
            assert!(report.rewritten.is_empty(), "{report:?}");
        }
    }

    /// A file that is *there* beats the memory of the one that left, which is
    /// the order both live resolvers use. Getting this backwards would rewrite
    /// a working reference to point somewhere else entirely.
    #[test]
    fn a_path_something_now_occupies_is_not_treated_as_stale() {
        let probe = Probe::create("fixup-occupied");
        write(&probe.project().join(MODEL_WAS), "a different model");
        let text = scene_text(None, MODEL_WAS);
        write(&probe.scene, &text);

        let report = fixup(probe.project()).expect("fixup");

        assert_eq!(
            std::fs::read_to_string(&probe.scene).expect("read scene"),
            text,
            "a reference that names a real file must load that file: {report:?}"
        );
    }

    // ---- when it cannot do its job ---------------------------------------

    /// A scene open in an editor, checked out read-only, or locked by source
    /// control. The rewrite fails; what must not happen is the former path
    /// being pruned anyway, which would leave the reference pointing at
    /// nothing with no way back.
    #[test]
    fn a_scene_that_cannot_be_written_is_reported_and_keeps_its_former_path() {
        let probe = Probe::create("fixup-readonly");
        let guid = probe.guid_of(MODEL_NOW);
        write(&probe.scene, &scene_text(Some(&guid), MODEL_WAS));
        let mut permissions = std::fs::metadata(&probe.scene)
            .expect("stat scene")
            .permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&probe.scene, permissions).expect("make the scene read-only");

        let report = fixup(probe.project()).expect("fixup");

        assert!(
            report.problems.iter().any(|p| p.contains("main.ron")),
            "a rewrite that could not be applied has to be reported: {report:?}"
        );
        assert_eq!(
            probe.former_paths_of(MODEL_NOW),
            vec![MODEL_WAS.to_string()],
            "the scene still names the old path, so forgetting it would turn a \
             recoverable reference into a broken one"
        );

        let mut permissions = std::fs::metadata(&probe.scene)
            .expect("stat scene")
            .permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&probe.scene, permissions).expect("restore permissions");
    }

    /// A project with no `assets` directory is an error rather than an empty
    /// report, for the same reason `scan` says so: it almost always means the
    /// wrong directory was named, and "nothing to fix" would be a lie.
    #[test]
    fn a_project_with_nothing_to_scan_is_an_error_rather_than_a_clean_report() {
        let dir = ProbeDir(std::env::temp_dir().join(unique("fixup-absent")));
        std::fs::create_dir_all(&dir.0).expect("create probe directory");
        let err = fixup(&dir.0).expect_err("a project with no assets/ must not report success");
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
