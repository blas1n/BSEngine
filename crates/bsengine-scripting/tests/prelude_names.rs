//! Every `Bsengine.<name>` a shipped script calls must actually exist.
//!
//! `games/net-2p-demo` called `Bsengine.setPosition` for as long as it existed
//! and no such function was ever defined -- every frame, inside `onUpdate`, in
//! a demo whose entire content is two players moving in circles. Nothing caught
//! it: that project has no replay recording, no test reads its scripts, and a
//! `TypeError` inside `onUpdate` is caught and logged rather than fatal.
//!
//! # Why this asks the runtime instead of reading the source
//!
//! A first version parsed `prelude.js` for `name:` keys and `Bsengine.name =`
//! assignments. It immediately reported `onUpdate`, `onMessage` and
//! `onCollision` as missing -- they are declared with ES6 method shorthand,
//! which that parser did not know about. Any hand-written parser has more of
//! those waiting in it, and each one is a false alarm that trains a reader to
//! ignore this test.
//!
//! Loading the prelude into a real runtime and reading `Object.keys(Bsengine)`
//! has no such failure mode: it measures what a script would actually find.

use bsengine_scripting::ScriptRuntime;
use std::collections::BTreeSet;

/// Every property a script can reach as `Bsengine.<name>`, asked of a real
/// runtime with the real prelude loaded.
fn defined_names() -> BTreeSet<String> {
    let mut rt = ScriptRuntime::new_with_ops();
    rt.exec_source(bsengine_scripting::ops::BOOTSTRAP_JS, "<bootstrap>")
        .expect("the prelude must load");
    let json = rt
        .eval("JSON.stringify(Object.keys(Bsengine))")
        .expect("Bsengine must exist after the prelude runs");
    serde_json::from_str::<Vec<String>>(json.trim())
        .unwrap_or_else(|e| panic!("expected a JSON array of keys, got {json:?}: {e}"))
        .into_iter()
        .collect()
}

/// Every `Bsengine.<name>` referenced by a `.js` anywhere under `games/`,
/// paired with the file that references it.
fn called_names() -> BTreeSet<(String, String)> {
    let games = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../games")
        .canonicalize()
        .expect("games/ sits next to crates/ in this workspace");

    let mut out = BTreeSet::new();
    let mut stack = vec![games];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).expect("readable directory") {
            let path = entry.expect("readable directory entry").path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|e| e == "js") {
                let raw = std::fs::read_to_string(&path).expect("readable script");
                let text = without_comments(&raw);
                for (at, _) in text.match_indices("Bsengine.") {
                    let name: String = text[at + "Bsengine.".len()..]
                        .chars()
                        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                        .collect();
                    if !name.is_empty() {
                        out.insert((name, path.display().to_string()));
                    }
                }
            }
        }
    }
    out
}

/// `src` with `//` and `/* */` comments blanked out.
///
/// A comment naming an API -- "this used to call `Bsengine.onUpdate`" -- is
/// ordinary prose, and a guard that fires on prose is one people learn to
/// ignore. Strings are not tracked: a `Bsengine.` inside a string literal
/// would still be counted, which errs towards flagging rather than missing.
fn without_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if bytes[i] == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            // Pushed byte-wise, so a multi-byte character is reassembled
            // intact; only ASCII delimiters are ever inspected above.
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

#[test]
fn every_name_a_game_script_calls_exists_in_the_prelude() {
    let defined = defined_names();
    let missing: Vec<String> = called_names()
        .into_iter()
        .filter(|(name, _)| !defined.contains(name))
        .map(|(name, file)| format!("Bsengine.{name} — called from {file}"))
        .collect();

    assert!(
        missing.is_empty(),
        "these scripts call names that do not exist, so they throw at runtime \
         and nothing else notices:\n  {}",
        missing.join("\n  ")
    );
}

#[test]
fn the_guard_can_actually_fail() {
    // Without this, a `defined_names` that returned everything would make the
    // test above pass forever while measuring nothing.
    let defined = defined_names();
    assert!(
        !defined.contains("definitelyNotAnEngineFunction"),
        "defined_names is over-matching, which would make the guard vacuous"
    );
    for real in ["getPosition", "setPosition", "getTransform", "vec3", "Vec3"] {
        assert!(
            defined.contains(real),
            "defined_names missed {real}, so the guard would flag real calls"
        );
    }
}

#[test]
fn the_games_are_actually_being_read() {
    // A `called_names` that silently walked an empty tree -- a renamed
    // directory, a changed manifest path -- would also make the guard pass
    // forever.
    let called = called_names();
    assert!(
        called.len() > 50,
        "expected the games' scripts to reference many engine names, found {} \
         -- has games/ moved?",
        called.len()
    );
}
