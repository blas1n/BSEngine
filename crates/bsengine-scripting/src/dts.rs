//! Generating `scripts/api.d.ts` from the API that actually exists.
//!
//! The file this produces is the only description of the scripting surface a
//! game author's editor can read. It used to be hand-written, and had decayed
//! to ten lines declaring two functions -- `log` and `version` -- in a
//! namespace `bsengine` that did not even match the real global `Bsengine`,
//! against a surface of ~300 prelude functions and 282 ops. Hand-maintenance is
//! how it got there, so regenerating it by hand would only restart the decay;
//! it is generated, and a test fails when it drifts.
//!
//! # Why reflection rather than parsing
//!
//! The function list comes from evaluating the prelude and walking the live
//! `Bsengine` object, not from a parser reading `prelude.js`. A parser is a
//! second implementation of "what does the prelude expose", and this codebase
//! has already paid twice for one rule living in two places. Reflection cannot
//! disagree with the object it is reflecting over.
//!
//! Types come from the other side: each wrapper names the op it forwards to,
//! and [`bsengine_catalog`] knows that op's Rust signature. So the names are
//! JavaScript's and the types are Rust's, each taken from where it is true.

use std::collections::BTreeMap;

/// An op's Rust signature, as the catalogue reports it.
#[derive(Debug, Clone, Default)]
pub struct OpSig {
    /// Parameter names and Rust types, in order.
    pub params: Vec<(String, String)>,
    /// The Rust return type, or `None` for `()`.
    pub returns: Option<String>,
}

/// One exported function, as found on the live `Bsengine` object.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Exported {
    /// Dotted path below `Bsengine`, e.g. `ui.setLabel`.
    pub path: String,
    /// Parameter names as written in the prelude, in order.
    pub params: Vec<String>,
    /// The `bsengine_*` ops the body calls, in order of first appearance.
    ///
    /// Usually one. A wrapper that calls none is pure JavaScript (`Vec3.add`);
    /// one that calls several is doing something a single op cannot express,
    /// and takes its types from the first.
    pub ops: Vec<String>,
    /// The argument expressions passed to the first op call, in order.
    ///
    /// Needed because a parameter's index in the wrapper is not its index in
    /// the op. `setContainer` takes seven arguments and calls a ten-argument
    /// op; matching by position typed its `opts` as `columns: u32`. Recording
    /// what each argument slot actually contains lets a parameter be typed from
    /// the slot it reaches, and left `unknown` when it reaches none.
    pub op_args: Vec<String>,
}

/// The JavaScript expression that walks `Bsengine` and reports what it finds.
///
/// Returns JSON, so the Rust side does no JavaScript parsing at all. Arrow
/// functions stringify as `(a, b) => ...` and shorthand methods as
/// `name(a, b) {`, so both shapes are handled rather than assuming the one the
/// prelude happens to use today.
pub const REFLECT_JS: &str = r#"
(() => {
  const out = [];
  const paramsOf = (fn) => {
    const src = Function.prototype.toString.call(fn);
    const open = src.indexOf('(');
    if (open < 0) return [];
    let depth = 0, close = -1;
    for (let i = open; i < src.length; i++) {
      if (src[i] === '(') depth++;
      else if (src[i] === ')') { depth--; if (depth === 0) { close = i; break; } }
    }
    if (close < 0) return [];
    const inner = src.slice(open + 1, close).trim();
    if (!inner) return [];
    // Split on top-level commas so a default like `{ a: 1, b: 2 }` stays one
    // parameter.
    const parts = [];
    let d = 0, start = 0;
    for (let i = 0; i < inner.length; i++) {
      const c = inner[i];
      if (c === '(' || c === '[' || c === '{') d++;
      else if (c === ')' || c === ']' || c === '}') d--;
      else if (c === ',' && d === 0) { parts.push(inner.slice(start, i)); start = i + 1; }
    }
    parts.push(inner.slice(start));
    return parts
      .map((p) => p.split('=')[0].trim())
      .filter((p) => p.length > 0);
  };
  const opsOf = (fn) => {
    const src = Function.prototype.toString.call(fn);
    const found = [];
    const re = /Deno\.core\.ops\.(bsengine_[A-Za-z0-9_]+)/g;
    let m;
    while ((m = re.exec(src)) !== null) {
      if (!found.includes(m[1])) found.push(m[1]);
    }
    return found;
  };
  // The argument expressions of the first `Deno.core.ops.*(...)` call, split on
  // top-level commas so `a ?? 1` and `f(x, y)` each stay one argument.
  const opArgsOf = (fn) => {
    const src = Function.prototype.toString.call(fn);
    const m = /Deno\.core\.ops\.bsengine_[A-Za-z0-9_]+\s*\(/.exec(src);
    if (!m) return [];
    const open = m.index + m[0].length - 1;
    let depth = 0, close = -1;
    for (let i = open; i < src.length; i++) {
      if (src[i] === '(') depth++;
      else if (src[i] === ')') { depth--; if (depth === 0) { close = i; break; } }
    }
    if (close < 0) return [];
    const inner = src.slice(open + 1, close);
    const parts = [];
    let d = 0, start = 0;
    for (let i = 0; i < inner.length; i++) {
      const c = inner[i];
      if (c === '(' || c === '[' || c === '{') d++;
      else if (c === ')' || c === ']' || c === '}') d--;
      else if (c === ',' && d === 0) { parts.push(inner.slice(start, i)); start = i + 1; }
    }
    parts.push(inner.slice(start));
    return parts.map((p) => p.trim()).filter((p) => p.length > 0);
  };
  const walk = (obj, prefix, depth) => {
    if (depth > 3) return;
    for (const key of Object.keys(obj)) {
      if (key.startsWith('_')) continue;
      const value = obj[key];
      const path = prefix ? prefix + '.' + key : key;
      if (typeof value === 'function') {
        out.push({
          path,
          params: paramsOf(value),
          ops: opsOf(value),
          op_args: opArgsOf(value),
        });
      } else if (value && typeof value === 'object' && !Array.isArray(value)) {
        walk(value, path, depth + 1);
      }
    }
  };
  walk(Bsengine, '', 0);
  out.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  return JSON.stringify(out);
})()
"#;

/// The type of a trailing `opts` parameter.
///
/// Optional, because every call site treats it as optional, and a bag rather
/// than a named shape because what it accepts differs per function -- anchors,
/// parent, fill, spacing, columns.
const OPTIONS_TYPE: &str = "?: Record<string, unknown>";

/// The TypeScript type for a Rust type in an op signature.
///
/// Deliberately exhaustive over what the ops actually use rather than a
/// catch-all: an unrecognised type becomes `unknown`, which makes a new Rust
/// type show up as a compile error in a game rather than silently typed as
/// `any`.
pub fn ts_type(rust: &str) -> &'static str {
    match rust {
        "String" | "&str" => "string",
        "f32" | "f64" | "u32" | "i32" | "u64" | "i64" | "usize" => "number",
        "bool" => "boolean",
        "Vec<f32>" | "Vec<f64>" | "Vec<u32>" => "number[]",
        "Option<Vec<f32>>" => "number[] | null",
        "Option<TransformJson>" => "object | null",
        "Option<String>" => "string | null",
        "Option<f32>" | "Option<f64>" => "number | null",
        "Option<bool>" => "boolean | null",
        _ => "unknown",
    }
}

/// The TypeScript type of one wrapper parameter.
///
/// Found by locating the op argument the parameter is forwarded into, rather
/// than by index. A parameter that reaches no argument -- or reaches one shared
/// with another parameter, where the mapping would be a guess -- is `unknown`.
///
/// `setContainer`'s `direction` is why this matters beyond arity: the wrapper
/// takes `'horizontal' | 'vertical' | 'grid'` and converts it to a number
/// before the call, so the op's `u32` describes what the *op* receives and not
/// what the *wrapper* accepts. It reaches the call as `dir`, not as
/// `direction`, so it correctly comes out `unknown` instead of a confident
/// `number` an author would believe.
fn param_type(e: &Exported, param: &str, ops: &BTreeMap<String, OpSig>) -> &'static str {
    if param == "opts" {
        return "";
    }
    let Some(sig) = e.ops.first().and_then(|op| ops.get(op)) else {
        return "unknown";
    };
    // Which argument slots mention this parameter as a whole identifier.
    let slots: Vec<usize> = e
        .op_args
        .iter()
        .enumerate()
        .filter(|(_, arg)| mentions_identifier(arg, param))
        .map(|(i, _)| i)
        .collect();
    match slots.as_slice() {
        [only] => sig
            .params
            .get(*only)
            .map(|(_, rust)| ts_type(rust))
            .unwrap_or("unknown"),
        _ => "unknown",
    }
}

/// Whether `expr` uses `name` as a standalone identifier.
///
/// Word-boundary matching, so `fontSize ?? 20` counts for `fontSize` while
/// `fontSizeScale` does not.
fn mentions_identifier(expr: &str, name: &str) -> bool {
    let bytes = expr.as_bytes();
    let mut from = 0;
    while let Some(pos) = expr[from..].find(name) {
        let start = from + pos;
        let end = start + name.len();
        let before_ok = start == 0 || !is_ident_byte(bytes[start - 1]);
        let after_ok = end == bytes.len() || !is_ident_byte(bytes[end]);
        if before_ok && after_ok {
            return true;
        }
        from = end;
    }
    false
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// Renders the whole `.d.ts`.
///
/// `exports` comes from [`REFLECT_JS`]; `ops` from the catalogue. A wrapper
/// whose op is unknown -- pure-JavaScript helpers like `Vec3.add`, and the two
/// ops-less entries -- gets `unknown` parameters rather than being dropped:
/// the author still gets the name and arity, and `unknown` says plainly that
/// the engine cannot state the type.
pub fn render(exports: &[Exported], ops: &BTreeMap<String, OpSig>) -> String {
    let mut out = String::new();
    out.push_str(
        "// GENERATED by `cargo test -p bsengine-scripting`. Do not edit.\n\
         //\n\
         // Regenerate with: BSENGINE_UPDATE_DTS=1 cargo test -p bsengine-scripting dts\n\
         //\n\
         // Names come from walking the live `Bsengine` object; types come from the\n\
         // Rust signature of the op each wrapper forwards to. A parameter typed\n\
         // `unknown` is one the engine cannot state a type for -- a pure-JavaScript\n\
         // helper, or an options object -- not one that accepts anything.\n\n",
    );
    out.push_str("declare namespace Bsengine {\n");

    // Grouped by namespace so the file reads like the API does.
    let mut groups: BTreeMap<&str, Vec<&Exported>> = BTreeMap::new();
    for e in exports {
        let ns = e.path.rsplit_once('.').map(|(a, _)| a).unwrap_or("");
        groups.entry(ns).or_default().push(e);
    }

    for (ns, entries) in &groups {
        let indent = if ns.is_empty() { "    " } else { "        " };
        if !ns.is_empty() {
            out.push_str(&format!("    namespace {ns} {{\n"));
        }
        for e in entries {
            let name = e.path.rsplit('.').next().unwrap_or(&e.path);
            let sig = e
                .params
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    // A parameter named `opts` is this codebase's options-bag
                    // convention, never the op parameter that happens to share
                    // its index. `setContainer`'s op takes ten arguments where
                    // its wrapper takes seven, so position 6 is `columns: u32`
                    // and a positional rule typed `opts` as `number` -- a
                    // confidently wrong type, which is worse than `unknown`
                    // because an author would believe it.
                    let _ = i;
                    let ty = param_type(e, p, ops);
                    if ty.is_empty() {
                        format!("{p}{OPTIONS_TYPE}")
                    } else {
                        format!("{p}: {ty}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            // A wrapper with no op behind it returns something this generator
            // cannot name, so it says `unknown` rather than `void`. Declaring a
            // getter `void` is worse than `unknown`: TypeScript rejects
            // assigning from it, so the typings would actively reject code the
            // engine supports. 123 of 282 ops return a value.
            let ret = match e.ops.first().and_then(|op| ops.get(op)) {
                Some(sig) => sig.returns.as_deref().map(ts_type).unwrap_or("void"),
                None => "unknown",
            };
            out.push_str(&format!("{indent}function {name}({sig}): {ret};\n"));
        }
        if !ns.is_empty() {
            out.push_str("    }\n");
        }
    }
    out.push_str("}\n");
    out
}
