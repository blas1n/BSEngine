//! JavaScript scripting runtime for BSEngine, via Deno Core (V8).
//!
//! `ScriptingPlugin` loads `.js` files (`Script`) and exposes ECS
//! operations to scripts as async Deno ops (`ops` module);
//! `ScriptRuntime` wraps the underlying V8 isolate, and `save` handles
//! script-driven save-game serialization.
#![recursion_limit = "2048"]
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]
// bsengine-scripting
/// Deno ops exposing ECS state and mutations to JS scripts as `Bsengine.*` calls.
pub mod dts;
pub mod ops;
/// The `ScriptingPlugin` Bevy plugin and its supporting components/resources.
pub mod plugin;
/// The `ScriptRuntime` V8 isolate wrapper used to execute script source.
pub mod runtime;
/// Script-driven save-game serialization to/from JSON.
pub mod save;
/// `ScriptSource`, the `bevy_asset` asset a `.js` file loads into.
pub mod script_asset;
pub use bsengine_core::ProjectDir;
pub use plugin::{
    load_scripts, load_scripts_with, Bootstrap, Script, ScriptRuntimeResource, ScriptTimingState,
    ScriptingPlugin, SoundHandles, KEY_MAPPINGS,
};
pub use runtime::ScriptRuntime;
pub use script_asset::{ScriptSource, ScriptSourceLoader};

#[cfg(test)]
mod dts_tests {
    use std::collections::BTreeMap;

    /// The checked-in `scripts/api.d.ts` must match what the engine exposes.
    ///
    /// This is the `catalog --check` pattern: generate from the source of
    /// truth, compare to what is committed, and fail with the command that
    /// fixes it. The file this replaces was hand-written and had drifted to
    /// describing two of roughly three hundred functions -- which is what
    /// hand-maintenance does to a generated-shaped artifact, and why a check
    /// rather than a one-off regeneration is the actual fix.
    #[test]
    fn the_checked_in_typings_match_the_api() {
        let mut rt = crate::runtime::ScriptRuntime::new_with_ops();
        rt.exec_source(crate::ops::BOOTSTRAP_JS, "<bootstrap>")
            .expect("the prelude must evaluate");
        let json = rt
            .eval(crate::dts::REFLECT_JS)
            .expect("walking Bsengine must succeed");
        let exports: Vec<crate::dts::Exported> =
            serde_json::from_str(&json).expect("reflection must return JSON");

        assert!(
            exports.len() > 200,
            "walking `Bsengine` found only {} functions; the prelude exposes \
             hundreds, so this found the wrong object or the walk stopped early",
            exports.len()
        );

        // Rust signatures for every op, keyed by name.
        let scanned = bsengine_catalog::Catalog::scan(std::path::Path::new("../.."))
            .expect("scanning the workspace for ops");
        let ops: BTreeMap<String, crate::dts::OpSig> = scanned
            .ops
            .iter()
            .map(|o| {
                (
                    o.name.clone(),
                    crate::dts::OpSig {
                        params: o
                            .params
                            .iter()
                            .map(|p| (p.name.clone(), p.ty.clone()))
                            .collect(),
                        returns: o.returns.clone(),
                    },
                )
            })
            .collect();
        assert!(
            !ops.is_empty(),
            "no ops were scanned; without them every parameter would be typed \
             `unknown` and this test would still pass"
        );

        let generated = crate::dts::render(&exports, &ops);
        let path = std::path::Path::new("../../scripts/api.d.ts");
        if std::env::var("BSENGINE_UPDATE_DTS").is_ok() {
            std::fs::write(path, &generated).expect("writing the typings");
            return;
        }
        let committed = std::fs::read_to_string(path).unwrap_or_default();
        assert_eq!(
            committed.replace("\r\n", "\n"),
            generated.replace("\r\n", "\n"),
            "scripts/api.d.ts is out of date. Regenerate it with:\n    \
             BSENGINE_UPDATE_DTS=1 cargo test -p bsengine-scripting dts"
        );
    }

    /// Types must come from the ops, not from a default.
    #[test]
    fn parameters_are_typed_from_the_rust_signature() {
        let exports = vec![crate::dts::Exported {
            path: "ui.setLabel".into(),
            params: vec!["id".into(), "x".into(), "on".into()],
            ops: vec!["bsengine_ui_set_label".into()],
            op_args: vec!["id".into(), "x".into(), "on".into()],
        }];
        let ops = BTreeMap::from([(
            "bsengine_ui_set_label".to_string(),
            crate::dts::OpSig {
                params: vec![
                    ("id".to_string(), "String".to_string()),
                    ("x".to_string(), "f32".to_string()),
                    ("on".to_string(), "bool".to_string()),
                ],
                returns: None,
            },
        )]);
        let out = crate::dts::render(&exports, &ops);
        assert!(
            out.contains("function setLabel(id: string, x: number, on: boolean): void;"),
            "each parameter must take the TypeScript type of its Rust \
             counterpart, got:\n{out}"
        );
    }

    /// A parameter the wrapper transforms before forwarding must not be typed
    /// from the op it eventually reaches.
    ///
    /// `setContainer` takes `'horizontal' | 'vertical' | 'grid'` and converts it
    /// to a number before calling a ten-argument op. Typing by position said
    /// `direction: number` and `opts: number` -- both confidently wrong, which
    /// is worse than `unknown` because an author would believe them.
    #[test]
    fn a_transformed_parameter_is_not_typed_from_the_op_it_reaches() {
        let exports = vec![crate::dts::Exported {
            path: "ui.setContainer".into(),
            params: vec!["id".into(), "direction".into(), "opts".into()],
            ops: vec!["bsengine_ui_set_container".into()],
            // The wrapper passes `dir`, not `direction`, and never passes
            // `opts` at all.
            op_args: vec!["id".into(), "dir".into(), "o.columns ?? 1".into()],
        }];
        let ops = BTreeMap::from([(
            "bsengine_ui_set_container".to_string(),
            crate::dts::OpSig {
                params: vec![
                    ("id".to_string(), "String".to_string()),
                    ("direction".to_string(), "u32".to_string()),
                    ("columns".to_string(), "u32".to_string()),
                ],
                returns: None,
            },
        )]);
        let out = crate::dts::render(&exports, &ops);
        assert!(
            out.contains("function setContainer(id: string, direction: unknown, opts?: Record<string, unknown>): void;"),
            "`id` forwards directly and is typed; `direction` is converted first \
             so it stays unknown; `opts` is the options bag. Got:\n{out}"
        );
    }

    /// A wrapper the engine cannot type must say so rather than claim `any`.
    #[test]
    fn an_untypeable_parameter_is_unknown_not_any() {
        let exports = vec![crate::dts::Exported {
            path: "Vec3.add".into(),
            params: vec!["a".into(), "b".into()],
            ops: Vec::new(),
            op_args: Vec::new(),
        }];
        let out = crate::dts::render(&exports, &BTreeMap::new());
        assert!(
            out.contains("function add(a: unknown, b: unknown): unknown;"),
            "a pure-JavaScript helper has no op to take types from; `unknown` \
             forces the author to narrow, where `any` would silently accept \
             anything. Got:\n{out}"
        );
    }
}
