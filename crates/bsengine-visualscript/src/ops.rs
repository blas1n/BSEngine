//! The engine functions a `Call` node can name, with their types.
//!
//! A hand-kept subset of `Bsengine.*` rather than the whole of `api.d.ts`:
//! the generated `.d.ts` types many parameters `unknown` (an options object,
//! a value that may be a `Vec3` or three numbers), and a node port needs one
//! type it can check a connection against. Each entry here names the spelling
//! the graph compiles to, so adding a function is one line -- and a test in
//! `bsengine-scripting` executes the compiled calls, which is what keeps this
//! table honest about the runtime it describes.
//!
//! `pure` is Blueprint's distinction: a pure node has no flow ports and is
//! evaluated wherever its value is read, so a getter can feed several nodes
//! without being sequenced; anything that changes the world runs exactly
//! where the flow reaches it.

use crate::compile::ValueType;

/// One callable engine function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpSpec {
    /// The function's name under `Bsengine.`; also the `Call` node's text.
    pub name: &'static str,
    /// Its parameters, in call order: each is a data input port.
    pub params: &'static [(&'static str, ValueType)],
    /// What it returns, if anything; the `"out"` port's type.
    pub returns: Option<ValueType>,
    /// Whether it reads without changing anything, and so has no flow ports.
    pub pure: bool,
}

const fn pure(
    name: &'static str,
    params: &'static [(&'static str, ValueType)],
    returns: ValueType,
) -> OpSpec {
    OpSpec {
        name,
        params,
        returns: Some(returns),
        pure: true,
    }
}

const fn act(
    name: &'static str,
    params: &'static [(&'static str, ValueType)],
    returns: Option<ValueType>,
) -> OpSpec {
    OpSpec {
        name,
        params,
        returns,
        pure: false,
    }
}

use ValueType::{Bool, Entity, Number, Text, Vec3};

/// Every function a `Call` node may name, alphabetical within each group.
pub const OPS: &[OpSpec] = &[
    // -- reading the world --------------------------------------------------
    pure("distanceTo", &[("a", Entity), ("b", Entity)], Number),
    pure(
        "distanceToPoint",
        &[
            ("entity", Entity),
            ("x", Number),
            ("y", Number),
            ("z", Number),
        ],
        Number,
    ),
    pure("entityExists", &[("entity", Entity)], Bool),
    pure("getAnimationClip", &[("entity", Entity)], Text),
    pure(
        "getClosestEntity",
        &[("x", Number), ("y", Number), ("z", Number)],
        Entity,
    ),
    pure("getDeltaTime", &[], Number),
    pure("getEntityCount", &[], Number),
    pure("getForwardVector", &[("entity", Entity)], Vec3),
    pure("getMass", &[("entity", Entity)], Number),
    pure("getPosition", &[("entity", Entity)], Vec3),
    pure("getRightVector", &[("entity", Entity)], Vec3),
    pure("getSaveField", &[("entity", Entity), ("key", Text)], Text),
    pure("getScale", &[("entity", Entity)], Vec3),
    pure("getShield", &[("entity", Entity)], Number),
    pure("getTime", &[], Number),
    pure("getTimerFraction", &[("entity", Entity)], Number),
    pure("getUpVector", &[("entity", Entity)], Vec3),
    pure("getVelocity", &[("entity", Entity)], Vec3),
    pure("getVisible", &[("entity", Entity)], Bool),
    pure("getWorldPosition", &[("entity", Entity)], Vec3),
    pure("hasNavArrived", &[("entity", Entity)], Bool),
    pure("isKeyDown", &[("key", Text)], Bool),
    pure("isKeyPressed", &[("key", Text)], Bool),
    pure("isKeyUp", &[("key", Text)], Bool),
    pure("isKinematic", &[("entity", Entity)], Bool),
    pure("isMouseDown", &[("button", Number)], Bool),
    pure("isNavIdle", &[("entity", Entity)], Bool),
    pure("isPaused", &[], Bool),
    pure("isTimerFinished", &[("entity", Entity)], Bool),
    // -- changing it ----------------------------------------------------------
    act("addForce", &[("entity", Entity), ("force", Vec3)], None),
    act("addImpulse", &[("entity", Entity), ("impulse", Vec3)], None),
    act("addPosition", &[("entity", Entity), ("delta", Vec3)], None),
    act(
        "addPositionLocal",
        &[("entity", Entity), ("delta", Vec3)],
        None,
    ),
    act("clearHudText", &[("id", Text)], None),
    act(
        "damageShield",
        &[("entity", Entity), ("amount", Number)],
        None,
    ),
    act("destroy", &[("entity", Entity)], None),
    act("loadScene", &[("path", Text)], None),
    act("log", &[("message", Text)], None),
    act("lookAt", &[("entity", Entity), ("target", Vec3)], None),
    act(
        "moveEntity",
        &[
            ("entity", Entity),
            ("dx", Number),
            ("dy", Number),
            ("dz", Number),
        ],
        None,
    ),
    act("pause", &[], None),
    act("playAnimation", &[("entity", Entity), ("clip", Text)], None),
    act("playSound", &[("path", Text)], Some(Number)),
    act("quit", &[], None),
    act("resetTimer", &[("entity", Entity)], None),
    act(
        "restoreShield",
        &[("entity", Entity), ("amount", Number)],
        None,
    ),
    act("resume", &[], None),
    act(
        "setAnimationSpeed",
        &[("entity", Entity), ("speed", Number)],
        None,
    ),
    act(
        "setCameraFov",
        &[("entity", Entity), ("degrees", Number)],
        None,
    ),
    act(
        "setColor",
        &[
            ("entity", Entity),
            ("r", Number),
            ("g", Number),
            ("b", Number),
        ],
        None,
    ),
    act(
        "setEmissive",
        &[
            ("entity", Entity),
            ("r", Number),
            ("g", Number),
            ("b", Number),
        ],
        None,
    ),
    act(
        "setGravityScale",
        &[("entity", Entity), ("scale", Number)],
        None,
    ),
    act("setHudText", &[("id", Text), ("text", Text)], None),
    act(
        "setKinematic",
        &[("entity", Entity), ("kinematic", Bool)],
        None,
    ),
    act(
        "setLifetime",
        &[("entity", Entity), ("seconds", Number)],
        None,
    ),
    act("setMass", &[("entity", Entity), ("mass", Number)], None),
    act(
        "setMetallic",
        &[("entity", Entity), ("value", Number)],
        None,
    ),
    act(
        "setNavDestination",
        &[("entity", Entity), ("target", Vec3)],
        None,
    ),
    act(
        "setNavSpeed",
        &[("entity", Entity), ("speed", Number)],
        None,
    ),
    act(
        "setPointLightIntensity",
        &[("entity", Entity), ("value", Number)],
        None,
    ),
    act(
        "setPosition",
        &[("entity", Entity), ("position", Vec3)],
        None,
    ),
    act(
        "setRoughness",
        &[("entity", Entity), ("value", Number)],
        None,
    ),
    act(
        "setRotationEuler",
        &[("entity", Entity), ("degrees", Vec3)],
        None,
    ),
    act(
        "setSaveField",
        &[("entity", Entity), ("key", Text), ("value", Text)],
        None,
    ),
    act("setScale", &[("entity", Entity), ("scale", Vec3)], None),
    act(
        "setVelocity",
        &[("entity", Entity), ("velocity", Vec3)],
        None,
    ),
    act("setVisible", &[("entity", Entity), ("visible", Bool)], None),
    act("stopSound", &[("id", Number)], None),
];

/// The spec for `name`, or `None` for a function this compiler does not know.
pub fn op(name: &str) -> Option<&'static OpSpec> {
    OPS.iter().find(|o| o.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table is looked up by name, so a duplicate would make one of the
    /// two entries unreachable and its ports silently wrong.
    #[test]
    fn op_names_are_unique_and_looked_up_exactly() {
        let mut seen = std::collections::HashSet::new();
        for o in OPS {
            assert!(seen.insert(o.name), "duplicate op {}", o.name);
        }
        assert_eq!(op("getDeltaTime").map(|o| o.returns), Some(Some(Number)));
        assert!(op("getDeltaTime").is_some_and(|o| o.pure));
        assert!(op("addPosition").is_some_and(|o| !o.pure && o.returns.is_none()));
        assert!(op("playSound").is_some_and(|o| !o.pure && o.returns == Some(Number)));
        assert_eq!(
            op("GetDeltaTime"),
            None,
            "names are case-sensitive, as JavaScript's are"
        );
        assert_eq!(op("fly"), None);
    }

    /// Every name here must be one `api.d.ts` declares: the table describes
    /// the runtime, and a name the runtime does not export would compile to
    /// a call that throws on the first frame.
    #[test]
    fn every_op_is_declared_in_the_typed_api_surface() {
        let dts = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/api.d.ts"
        ))
        .expect("scripts/api.d.ts is generated by bsengine-scripting's tests and committed");
        for o in OPS {
            let needle = format!("    function {}(", o.name);
            assert!(
                dts.contains(&needle),
                "{} is not declared in scripts/api.d.ts; the runtime does not export it",
                o.name
            );
        }
    }
}
