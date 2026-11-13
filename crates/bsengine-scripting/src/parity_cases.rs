// How an op reads each kind of argument, as V8 + `deno_core` 0.404 read it:
// a JavaScript expression and what the op receives (`None`: it throws
// `TypeError: expected <ty>`). Measured against V8, not taken from any
// specification -- it is not ToNumber/ToUint32/ToBoolean (#2067).
//
// One table, `include!`d by both sides so they cannot drift apart:
// `ops.rs`'s engine-parity tests, which call real ops with these in the V8
// build and the QuickJS one, and `tests/browser_runtime.rs`, which gives the
// same values to the browser build's conversions. A case changed here is
// therefore re-checked against V8 by the native run.

/// A float slot: a Number as it is, a BigInt by its value, anything else
/// throws. `NaN` is checked apart (it equals nothing).
const F32_CASES: &[(&str, Option<f32>)] = &[
    ("1.5", Some(1.5)),
    ("-2.9", Some(-2.9)),
    ("4294967297", Some(4_294_967_297.0)),
    ("Infinity", Some(f32::INFINITY)),
    ("10n", Some(10.0)),
    ("'2.5'", None),
    ("''", None),
    ("null", None),
    ("undefined", None),
    ("true", None),
    ("{}", None),
    ("[]", None),
    ("{ valueOf() { return 4; } }", None),
];

/// A `u32` slot: an integer in the int32 or uint32 range wraps (-1 is
/// 4294967295), whatever its representation; any other number is truncated
/// and saturated through u64 (-2.9 is 0, Infinity 4294967295, 4294967297 is
/// 1). Non-numbers throw.
const U32_CASES: &[(&str, Option<u32>)] = &[
    ("7", Some(7)),
    ("1.5", Some(1)),
    ("2.9", Some(2)),
    ("-1", Some(u32::MAX)),
    ("-2", Some(u32::MAX - 1)),
    // -2 again, computed: QuickJS holds this one as a float, and V8 still
    // reads it as the int32 it equals.
    ("1.5 - 3.5", Some(u32::MAX - 1)),
    ("-2.9", Some(0)),
    ("-0.5", Some(0)),
    ("-3e9", Some(0)),
    ("4294967295", Some(u32::MAX)),
    ("4294967297", Some(1)),
    ("NaN", Some(0)),
    ("Infinity", Some(u32::MAX)),
    ("10n", Some(10)),
    ("'7'", None),
    ("null", None),
    ("undefined", None),
    ("false", None),
    ("{}", None),
];

/// A `bool` slot: `true` only for `true` -- not truthiness -- and it never
/// throws.
const BOOL_CASES: &[(&str, bool)] = &[
    ("true", true),
    ("false", false),
    ("1", false),
    ("'0'", false),
    ("'yes'", false),
    ("{}", false),
    ("[]", false),
    ("null", false),
    ("undefined", false),
    ("10n", false),
];

/// A `#[string]` slot: the string, or `""` for anything that is not one --
/// not its `String()`.
const STRING_CASES: &[(&str, &str)] = &[
    ("'cam'", "cam"),
    ("42", ""),
    ("undefined", ""),
    ("null", ""),
    ("true", ""),
    ("{}", ""),
];
