//! `query_entities`: one composable query over the editor snapshot, in place
//! of a tool per filter.
//!
//! # Why one tool
//!
//! The editor once exposed over a thousand MCP tools, most of them the same
//! filter written out for each field, comparison and verb --
//! `select_entities_with_position_y_above`, `count_entities_with_name_prefix`,
//! `deselect_entities_with_tag_count_between`, and three spellings of "name
//! starts with". An agent pays for every tool's schema in its context on
//! every request, and still cannot ask anything the list did not anticipate
//! ("lights above y = 3 whose name contains `Lamp`" needed a tool nobody had
//! written). The reference engines expose the opposite shape: Unity's
//! `FindObjectsByType` plus a predicate, Godot's `get_nodes_in_group` plus
//! script, Unreal's `GetAllActorsOfClass` plus Blueprint filters -- a small
//! set of queries and a way to combine them. This is that shape for MCP:
//! conditions on named fields, AND-combined (`where`) with an optional OR
//! group (`any`), a sort, a limit, and a verb (`action`).
//!
//! # Contract
//!
//! Errors name what is accepted. An unknown field, operator, action or sort
//! key, an operator that does not apply to a field's type, or a value of the
//! wrong type is an error, never silently ignored -- a typo'd filter that
//! matched everything would select the whole scene.
//!
//! A condition on a field the entity does not have (`light.intensity` on a
//! mesh) is false, except `missing`, which is how to ask for its absence.
//! `ne` and `not_contains` are false for an absent field too: "intensity is
//! not 5" is not a claim about an entity with no light.

use crate::snapshot::EntityInfo;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Float equality for `eq`/`ne` on number fields. Snapshot values are `f32`
/// that went through JSON; a position set to 1.1 reads back as
/// 1.100000023841858, and an exact comparison would never match it.
const EPSILON: f64 = 1e-4;

/// Every field a condition, a sort or a `tags` histogram can name. The list
/// in the tool's schema and in its error messages is this one.
pub(crate) const FIELDS: &[&str] = &[
    "id",
    "name",
    "name_length",
    "tags",
    "tag_count",
    "position",
    "position.x",
    "position.y",
    "position.z",
    "rotation",
    "rotation.x",
    "rotation.y",
    "rotation.z",
    "scale",
    "scale.x",
    "scale.y",
    "scale.z",
    "scale.max",
    "mesh_id",
    "light.type",
    "light.intensity",
    "light.range",
    "camera.fov",
    "parent",
    "ancestors",
    "depth",
    "children_count",
    "visible",
    "selected",
    "prefab_instance",
    "components",
    "duplicate_name",
    "distance",
];

/// Every operator, for error messages.
pub(crate) const OPS: &[&str] = &[
    "eq",
    "ne",
    "lt",
    "le",
    "gt",
    "ge",
    "between",
    "in",
    "contains",
    "not_contains",
    "starts_with",
    "ends_with",
    "any_of",
    "all_of",
    "none_of",
    "exists",
    "missing",
];

/// Every verb.
pub(crate) const ACTIONS: &[&str] = &[
    "get",
    "count",
    "select",
    "deselect",
    "select_only",
    "tags",
    "bounds",
];

/// A field's value on one entity. `Absent` is a field the entity lacks.
#[derive(Debug, Clone, PartialEq)]
enum FieldValue {
    Absent,
    Num(f64),
    Str(String),
    Bool(bool),
    StrList(Vec<String>),
    NumList(Vec<f64>),
    /// A vector (position, rotation, scale): only `exists`/`missing` apply;
    /// its components are fields of their own.
    Vector,
}

impl FieldValue {
    fn is_present(&self) -> bool {
        match self {
            FieldValue::Absent => false,
            FieldValue::StrList(l) => !l.is_empty(),
            FieldValue::NumList(l) => !l.is_empty(),
            _ => true,
        }
    }

    fn to_json(&self) -> Value {
        match self {
            FieldValue::Absent | FieldValue::Vector => Value::Null,
            FieldValue::Num(n) => json!(n),
            FieldValue::Str(s) => json!(s),
            FieldValue::Bool(b) => json!(b),
            FieldValue::StrList(l) => json!(l),
            FieldValue::NumList(l) => json!(l),
        }
    }
}

/// The kind of value a field holds, to reject an operator that cannot apply
/// before any entity is looked at.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Num,
    Str,
    Bool,
    StrList,
    NumList,
    Vector,
}

fn kind_of(field: &str) -> Kind {
    match field {
        "name" | "light.type" => Kind::Str,
        "tags" | "components" => Kind::StrList,
        "ancestors" => Kind::NumList,
        "position" | "rotation" | "scale" => Kind::Vector,
        "visible" | "selected" | "prefab_instance" | "duplicate_name" => Kind::Bool,
        _ => Kind::Num,
    }
}

fn ops_for(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::Num => &[
            "eq", "ne", "lt", "le", "gt", "ge", "between", "in", "exists", "missing",
        ],
        Kind::Str => &[
            "eq",
            "ne",
            "in",
            "contains",
            "not_contains",
            "starts_with",
            "ends_with",
            "exists",
            "missing",
        ],
        Kind::Bool => &["eq", "ne", "exists", "missing"],
        Kind::StrList | Kind::NumList => &[
            "contains",
            "not_contains",
            "any_of",
            "all_of",
            "none_of",
            "exists",
            "missing",
        ],
        Kind::Vector => &["exists", "missing"],
    }
}

/// One parsed condition.
#[derive(Debug, Clone)]
struct Condition {
    field: String,
    op: String,
    value: Value,
}

/// What the scene looks like around each entity: the facts a field needs
/// that no single `EntityInfo` carries (children, depth, ancestors, whether
/// another entity shares the name, distance from the query's origin).
struct Context<'a> {
    by_id: HashMap<u64, &'a EntityInfo>,
    children: HashMap<u64, usize>,
    name_counts: HashMap<&'a str, usize>,
    selection: &'a HashSet<u64>,
    origin: Option<[f64; 3]>,
}

impl<'a> Context<'a> {
    fn new(entities: &'a [EntityInfo], selection: &'a HashSet<u64>) -> Self {
        let by_id = entities.iter().map(|e| (e.id, e)).collect();
        let mut children = HashMap::new();
        let mut name_counts = HashMap::new();
        for e in entities {
            if let Some(p) = e.parent_id {
                *children.entry(p).or_insert(0) += 1;
            }
            if let Some(n) = e.name.as_deref() {
                *name_counts.entry(n).or_insert(0) += 1;
            }
        }
        Self {
            by_id,
            children,
            name_counts,
            selection,
            origin: None,
        }
    }

    /// Parent chain, nearest first. Stops at a cycle rather than looping,
    /// which a hand-edited scene can contain.
    fn ancestors(&self, e: &EntityInfo) -> Vec<u64> {
        let mut out = Vec::new();
        let mut seen = HashSet::from([e.id]);
        let mut cur = e.parent_id;
        while let Some(p) = cur {
            if !seen.insert(p) {
                break;
            }
            out.push(p);
            cur = self.by_id.get(&p).and_then(|pe| pe.parent_id);
        }
        out
    }

    fn value(&self, e: &EntityInfo, field: &str) -> FieldValue {
        let v3 = |v: Option<[f32; 3]>, i: usize| {
            v.map_or(FieldValue::Absent, |a| FieldValue::Num(a[i] as f64))
        };
        let vec = |v: Option<[f32; 3]>| v.map_or(FieldValue::Absent, |_| FieldValue::Vector);
        let num = |v: Option<f32>| v.map_or(FieldValue::Absent, |n| FieldValue::Num(n as f64));
        match field {
            "id" => FieldValue::Num(e.id as f64),
            "name" => e.name.clone().map_or(FieldValue::Absent, FieldValue::Str),
            "name_length" => e.name.as_deref().map_or(FieldValue::Absent, |n| {
                FieldValue::Num(n.chars().count() as f64)
            }),
            "tags" => FieldValue::StrList(e.tags.clone()),
            "tag_count" => FieldValue::Num(e.tags.len() as f64),
            "position" => vec(e.position),
            "position.x" => v3(e.position, 0),
            "position.y" => v3(e.position, 1),
            "position.z" => v3(e.position, 2),
            "rotation" => vec(e.rotation),
            "rotation.x" => v3(e.rotation, 0),
            "rotation.y" => v3(e.rotation, 1),
            "rotation.z" => v3(e.rotation, 2),
            "scale" => vec(e.scale),
            "scale.x" => v3(e.scale, 0),
            "scale.y" => v3(e.scale, 1),
            "scale.z" => v3(e.scale, 2),
            "scale.max" => e.scale.map_or(FieldValue::Absent, |s| {
                FieldValue::Num(s[0].max(s[1]).max(s[2]) as f64)
            }),
            "mesh_id" => e
                .mesh_id
                .map_or(FieldValue::Absent, |m| FieldValue::Num(m as f64)),
            "light.type" => e
                .light_type
                .clone()
                .map_or(FieldValue::Absent, FieldValue::Str),
            "light.intensity" => num(e.light_intensity),
            "light.range" => num(e.light_range),
            "camera.fov" => num(e.camera_fov),
            "parent" => e
                .parent_id
                .map_or(FieldValue::Absent, |p| FieldValue::Num(p as f64)),
            "ancestors" => {
                FieldValue::NumList(self.ancestors(e).into_iter().map(|a| a as f64).collect())
            }
            "depth" => FieldValue::Num(self.ancestors(e).len() as f64),
            "children_count" => {
                FieldValue::Num(self.children.get(&e.id).copied().unwrap_or(0) as f64)
            }
            "visible" => FieldValue::Bool(e.visible),
            "selected" => FieldValue::Bool(self.selection.contains(&e.id)),
            "prefab_instance" => FieldValue::Bool(e.is_prefab_instance),
            "components" => {
                FieldValue::StrList(e.extra_components.iter().map(|(n, _)| n.clone()).collect())
            }
            "duplicate_name" => FieldValue::Bool(
                e.name
                    .as_deref()
                    .is_some_and(|n| self.name_counts.get(n).copied().unwrap_or(0) > 1),
            ),
            "distance" => match (self.origin, e.position) {
                (Some(o), Some(p)) => {
                    let d = [p[0] as f64 - o[0], p[1] as f64 - o[1], p[2] as f64 - o[2]];
                    FieldValue::Num((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
                }
                _ => FieldValue::Absent,
            },
            _ => FieldValue::Absent,
        }
    }
}

fn as_f64(v: &Value, what: &str) -> Result<f64, String> {
    v.as_f64()
        .ok_or_else(|| format!("{what} must be a number, got {v}"))
}

fn as_str<'v>(v: &'v Value, what: &str) -> Result<&'v str, String> {
    v.as_str()
        .ok_or_else(|| format!("{what} must be a string, got {v}"))
}

fn as_array<'v>(v: &'v Value, what: &str) -> Result<&'v Vec<Value>, String> {
    v.as_array()
        .ok_or_else(|| format!("{what} must be an array, got {v}"))
}

/// Checks a condition's field, operator and value shape once, before any
/// entity, so a malformed query fails the same way on an empty scene as on
/// a full one.
fn validate(c: &Condition) -> Result<(), String> {
    if !FIELDS.contains(&c.field.as_str()) {
        return Err(format!(
            "unknown field `{}`; fields are: {}",
            c.field,
            FIELDS.join(", ")
        ));
    }
    if !OPS.contains(&c.op.as_str()) {
        return Err(format!(
            "unknown op `{}`; operators are: {}",
            c.op,
            OPS.join(", ")
        ));
    }
    let kind = kind_of(&c.field);
    if !ops_for(kind).contains(&c.op.as_str()) {
        return Err(format!(
            "op `{}` does not apply to field `{}`; it takes: {}",
            c.op,
            c.field,
            ops_for(kind).join(", ")
        ));
    }
    let what = format!("the value of `{}` `{}`", c.field, c.op);
    match c.op.as_str() {
        "exists" | "missing" => {}
        "between" => {
            let a = as_array(&c.value, &what)?;
            if a.len() != 2 {
                return Err(format!("{what} must be [low, high], got {}", c.value));
            }
            as_f64(&a[0], &what)?;
            as_f64(&a[1], &what)?;
        }
        "in" | "any_of" | "all_of" | "none_of" => {
            for item in as_array(&c.value, &what)? {
                check_scalar(kind, item, &what)?;
            }
        }
        _ => check_scalar(kind, &c.value, &what)?,
    }
    Ok(())
}

/// The value's type for one element of `kind`: a list field's elements are
/// strings or numbers, a scalar field's value is its own type.
fn check_scalar(kind: Kind, v: &Value, what: &str) -> Result<(), String> {
    match kind {
        Kind::Num | Kind::NumList => as_f64(v, what).map(|_| ()),
        Kind::Str | Kind::StrList => as_str(v, what).map(|_| ()),
        Kind::Bool => v
            .as_bool()
            .map(|_| ())
            .ok_or_else(|| format!("{what} must be true or false, got {v}")),
        Kind::Vector => Ok(()),
    }
}

fn num_eq(a: f64, b: f64) -> bool {
    (a - b).abs() <= EPSILON
}

/// Whether one entity's field value satisfies a validated condition.
fn holds(v: &FieldValue, c: &Condition) -> bool {
    match c.op.as_str() {
        "exists" => return v.is_present(),
        "missing" => return !v.is_present(),
        _ => {}
    }
    match v {
        FieldValue::Absent | FieldValue::Vector => false,
        FieldValue::Num(n) => {
            let n = *n;
            let x = || c.value.as_f64().unwrap_or(f64::NAN);
            match c.op.as_str() {
                "eq" => num_eq(n, x()),
                "ne" => !num_eq(n, x()),
                "lt" => n < x(),
                "le" => n <= x() || num_eq(n, x()),
                "gt" => n > x(),
                "ge" => n >= x() || num_eq(n, x()),
                "between" => {
                    let a = c.value.as_array().expect("validated");
                    let (lo, hi) = (a[0].as_f64().unwrap(), a[1].as_f64().unwrap());
                    (n >= lo || num_eq(n, lo)) && (n <= hi || num_eq(n, hi))
                }
                "in" => c
                    .value
                    .as_array()
                    .expect("validated")
                    .iter()
                    .any(|i| i.as_f64().is_some_and(|i| num_eq(n, i))),
                _ => false,
            }
        }
        FieldValue::Str(s) => {
            let x = c.value.as_str().unwrap_or("");
            match c.op.as_str() {
                "eq" => s == x,
                "ne" => s != x,
                "contains" => s.contains(x),
                "not_contains" => !s.contains(x),
                "starts_with" => s.starts_with(x),
                "ends_with" => s.ends_with(x),
                "in" => c
                    .value
                    .as_array()
                    .expect("validated")
                    .iter()
                    .any(|i| i.as_str() == Some(s.as_str())),
                _ => false,
            }
        }
        FieldValue::Bool(b) => {
            let x = c.value.as_bool().unwrap_or(false);
            match c.op.as_str() {
                "eq" => *b == x,
                "ne" => *b != x,
                _ => false,
            }
        }
        FieldValue::StrList(l) => {
            let has = |x: &Value| x.as_str().is_some_and(|x| l.iter().any(|t| t == x));
            list_holds(&c.op, &c.value, has)
        }
        FieldValue::NumList(l) => {
            let has = |x: &Value| x.as_f64().is_some_and(|x| l.iter().any(|t| num_eq(*t, x)));
            list_holds(&c.op, &c.value, has)
        }
    }
}

fn list_holds(op: &str, value: &Value, has: impl Fn(&Value) -> bool) -> bool {
    let items = || value.as_array().expect("validated").iter();
    match op {
        "contains" => has(value),
        "not_contains" => !has(value),
        "any_of" => items().any(&has),
        "all_of" => items().all(&has),
        "none_of" => !items().any(&has),
        _ => false,
    }
}

fn parse_conditions(input: &Value, key: &str) -> Result<Vec<Condition>, String> {
    let Some(raw) = input.get(key) else {
        return Ok(Vec::new());
    };
    let list = as_array(raw, &format!("`{key}`"))?;
    let mut out = Vec::with_capacity(list.len());
    for (i, item) in list.iter().enumerate() {
        let obj = item
            .as_object()
            .ok_or_else(|| format!("`{key}[{i}]` must be {{field, op, value}}, got {item}"))?;
        for k in obj.keys() {
            if !["field", "op", "value"].contains(&k.as_str()) {
                return Err(format!(
                    "`{key}[{i}]` has unknown key `{k}`; a condition is {{field, op, value}}"
                ));
            }
        }
        let field = as_str(
            obj.get("field").unwrap_or(&Value::Null),
            &format!("`{key}[{i}].field`"),
        )?;
        let op = as_str(
            obj.get("op").unwrap_or(&Value::Null),
            &format!("`{key}[{i}].op`"),
        )?;
        let c = Condition {
            field: field.to_string(),
            op: op.to_string(),
            value: obj.get("value").cloned().unwrap_or(Value::Null),
        };
        validate(&c).map_err(|e| format!("`{key}[{i}]`: {e}"))?;
        out.push(c);
    }
    Ok(out)
}

/// The shorthand flags `query_entities` took before conditions existed,
/// as conditions -- so an agent or test written against them keeps working.
fn legacy_conditions(input: &Value) -> Result<Vec<Condition>, String> {
    let mut out = Vec::new();
    let flag = |key: &str, field: &str, out: &mut Vec<Condition>| -> Result<(), String> {
        if let Some(v) = input.get(key) {
            let b = v
                .as_bool()
                .ok_or_else(|| format!("`{key}` must be true or false, got {v}"))?;
            out.push(Condition {
                field: field.to_string(),
                op: if b { "exists" } else { "missing" }.to_string(),
                value: Value::Null,
            });
        }
        Ok(())
    };
    flag("has_mesh", "mesh_id", &mut out)?;
    flag("has_light", "light.type", &mut out)?;
    flag("has_position", "position", &mut out)?;
    if let Some(v) = input.get("light_type") {
        let t = as_str(v, "`light_type`")?;
        out.push(Condition {
            field: "light.type".to_string(),
            op: "eq".to_string(),
            value: json!(t),
        });
    }
    Ok(out)
}

const TOP_KEYS: &[&str] = &[
    "where",
    "any",
    "sort",
    "limit",
    "from",
    "from_entity",
    "action",
    "has_mesh",
    "has_light",
    "has_position",
    "light_type",
];

/// Runs a query against the snapshot's entities and the live selection.
/// `select`, `deselect` and `select_only` change `selection`; every other
/// action only reads.
///
/// # Errors
///
/// A message naming what was wrong and what is accepted.
pub(crate) fn execute(
    input: &Value,
    entities: &[EntityInfo],
    selection: &mut HashSet<u64>,
) -> Result<Value, String> {
    let empty = Map::new();
    let obj = match input {
        Value::Null => &empty,
        Value::Object(o) => o,
        other => return Err(format!("the input must be an object, got {other}")),
    };
    for k in obj.keys() {
        if !TOP_KEYS.contains(&k.as_str()) {
            return Err(format!(
                "unknown key `{k}`; query_entities takes: {}",
                TOP_KEYS.join(", ")
            ));
        }
    }

    let mut all = legacy_conditions(input)?;
    all.extend(parse_conditions(input, "where")?);
    let any = parse_conditions(input, "any")?;

    let action = match input.get("action") {
        None => "get",
        Some(v) => as_str(v, "`action`")?,
    };
    if !ACTIONS.contains(&action) {
        return Err(format!(
            "unknown action `{action}`; actions are: {}",
            ACTIONS.join(", ")
        ));
    }

    let sort = match input.get("sort") {
        None => None,
        Some(s) => {
            let by = as_str(s.get("by").unwrap_or(&Value::Null), "`sort.by`")?;
            if !FIELDS.contains(&by) {
                return Err(format!(
                    "unknown sort field `{by}`; fields are: {}",
                    FIELDS.join(", ")
                ));
            }
            if !matches!(kind_of(by), Kind::Num | Kind::Str | Kind::Bool) {
                return Err(format!(
                    "cannot sort by `{by}`, which is not a single value"
                ));
            }
            let desc = match s.get("desc") {
                None => false,
                Some(d) => d
                    .as_bool()
                    .ok_or_else(|| format!("`sort.desc` must be true or false, got {d}"))?,
            };
            Some((by.to_string(), desc))
        }
    };
    let limit = match input.get("limit") {
        None => None,
        Some(l) => Some(
            l.as_u64()
                .ok_or_else(|| format!("`limit` must be a non-negative integer, got {l}"))?
                as usize,
        ),
    };

    let frozen_selection = selection.clone();
    let mut ctx = Context::new(entities, &frozen_selection);
    ctx.origin = match (input.get("from"), input.get("from_entity")) {
        (Some(_), Some(_)) => return Err("give `from` or `from_entity`, not both".into()),
        (Some(f), None) => {
            let a = as_array(f, "`from`")?;
            if a.len() != 3 {
                return Err(format!("`from` must be [x, y, z], got {f}"));
            }
            Some([
                as_f64(&a[0], "`from`")?,
                as_f64(&a[1], "`from`")?,
                as_f64(&a[2], "`from`")?,
            ])
        }
        (None, Some(id)) => {
            let id = id
                .as_u64()
                .ok_or_else(|| format!("`from_entity` must be an entity id, got {id}"))?;
            let e = ctx
                .by_id
                .get(&id)
                .ok_or_else(|| format!("`from_entity` {id} is not in the scene"))?;
            let p = e
                .position
                .ok_or_else(|| format!("`from_entity` {id} has no position"))?;
            Some([p[0] as f64, p[1] as f64, p[2] as f64])
        }
        (None, None) => None,
    };
    let uses_distance = all.iter().chain(&any).any(|c| c.field == "distance")
        || sort.as_ref().is_some_and(|(by, _)| by == "distance");
    if uses_distance && ctx.origin.is_none() {
        return Err("`distance` needs an origin: give `from` [x, y, z] or `from_entity`".into());
    }

    let mut matched: Vec<&EntityInfo> = entities
        .iter()
        .filter(|e| {
            all.iter().all(|c| holds(&ctx.value(e, &c.field), c))
                && (any.is_empty() || any.iter().any(|c| holds(&ctx.value(e, &c.field), c)))
        })
        .collect();

    // Entities without the sort field go last, in id order; ties break by
    // id so the same query on the same scene answers the same way.
    matched.sort_by_key(|e| e.id);
    if let Some((by, desc)) = &sort {
        matched.sort_by(|a, b| {
            let (va, vb) = (ctx.value(a, by), ctx.value(b, by));
            use std::cmp::Ordering::*;
            let ord = match (&va, &vb) {
                (FieldValue::Absent, FieldValue::Absent) => Equal,
                (FieldValue::Absent, _) => return Greater,
                (_, FieldValue::Absent) => return Less,
                (FieldValue::Num(x), FieldValue::Num(y)) => x.partial_cmp(y).unwrap_or(Equal),
                (FieldValue::Str(x), FieldValue::Str(y)) => x.cmp(y),
                (FieldValue::Bool(x), FieldValue::Bool(y)) => x.cmp(y),
                _ => Equal,
            };
            if *desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }
    if let Some(n) = limit {
        matched.truncate(n);
    }

    let ids: Vec<u64> = matched.iter().map(|e| e.id).collect();
    Ok(match action {
        "get" => {
            let list: Vec<Value> = matched
                .iter()
                .map(|e| {
                    let mut o = json!({
                        "id": e.id,
                        "name": e.name,
                        "position": e.position,
                        "rotation": e.rotation,
                        "scale": e.scale,
                        "tags": e.tags,
                        "parent": e.parent_id,
                        "mesh_id": e.mesh_id,
                        "light_type": e.light_type,
                    });
                    if ctx.origin.is_some() {
                        o["distance"] = ctx.value(e, "distance").to_json();
                    }
                    if let Some((by, _)) = &sort {
                        o["sort_value"] = ctx.value(e, by).to_json();
                    }
                    o
                })
                .collect();
            json!({ "count": list.len(), "entities": list })
        }
        "count" => json!({ "count": ids.len() }),
        "select" => {
            let added = ids.iter().filter(|id| selection.insert(**id)).count();
            json!({ "count": ids.len(), "added_count": added })
        }
        "deselect" => {
            let removed = ids.iter().filter(|id| selection.remove(*id)).count();
            json!({ "count": ids.len(), "removed_count": removed })
        }
        "select_only" => {
            selection.clear();
            selection.extend(ids.iter().copied());
            json!({ "count": ids.len() })
        }
        "tags" => {
            let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
            for e in &matched {
                for t in &e.tags {
                    *hist.entry(t.as_str()).or_insert(0) += 1;
                }
            }
            json!({ "count": ids.len(), "tags": hist })
        }
        "bounds" => {
            let points: Vec<[f32; 3]> = matched.iter().filter_map(|e| e.position).collect();
            if points.is_empty() {
                json!({ "count": 0, "min": null, "max": null, "center": null })
            } else {
                let mut min = [f32::MAX; 3];
                let mut max = [f32::MIN; 3];
                for p in &points {
                    for i in 0..3 {
                        min[i] = min[i].min(p[i]);
                        max[i] = max[i].max(p[i]);
                    }
                }
                let center = [
                    (min[0] + max[0]) / 2.0,
                    (min[1] + max[1]) / 2.0,
                    (min[2] + max[2]) / 2.0,
                ];
                json!({ "count": points.len(), "min": min, "max": max, "center": center })
            }
        }
        _ => unreachable!("validated above"),
    })
}

/// The tool's JSON schema: the field, operator and action lists above, so
/// an agent reads the vocabulary from the schema rather than guessing it.
pub(crate) fn input_schema() -> Value {
    let condition = json!({
        "type": "object",
        "properties": {
            "field": { "type": "string", "enum": FIELDS },
            "op":    { "type": "string", "enum": OPS },
            "value": { "description": "number, string, bool, [low, high] for between, or a list for in/any_of/all_of/none_of; omitted for exists/missing" }
        },
        "required": ["field", "op"]
    });
    json!({
        "type": "object",
        "properties": {
            "where":  { "type": "array", "items": condition, "description": "all must hold" },
            "any":    { "type": "array", "items": condition, "description": "at least one must hold (when given)" },
            "sort":   { "type": "object", "properties": {
                "by":   { "type": "string", "enum": FIELDS },
                "desc": { "type": "boolean" } }, "required": ["by"] },
            "limit":  { "type": "integer", "minimum": 0 },
            "from":   { "type": "array", "items": { "type": "number" }, "description": "[x,y,z] origin for the `distance` field" },
            "from_entity": { "type": "integer", "description": "an entity whose position is the origin for `distance`" },
            "action": { "type": "string", "enum": ACTIONS, "description": "get (default): matching entities; count; select/deselect: add to/remove from the selection; select_only: replace it; tags: tag histogram of the matches; bounds: min/max/center of their positions" },
            "has_mesh":     { "type": "boolean", "description": "shorthand for mesh_id exists/missing" },
            "has_light":    { "type": "boolean", "description": "shorthand for light.type exists/missing" },
            "has_position": { "type": "boolean", "description": "shorthand for position exists/missing" },
            "light_type":   { "type": "string", "description": "shorthand for light.type eq" }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ent(id: u64, name: &str) -> EntityInfo {
        EntityInfo {
            id,
            name: Some(name.to_string()),
            visible: true,
            ..Default::default()
        }
    }

    fn at(mut e: EntityInfo, p: [f32; 3]) -> EntityInfo {
        e.position = Some(p);
        e
    }

    fn tagged(mut e: EntityInfo, tags: &[&str]) -> EntityInfo {
        e.tags = tags.iter().map(|t| t.to_string()).collect();
        e
    }

    fn run(input: Value, es: &[EntityInfo]) -> Value {
        execute(&input, es, &mut HashSet::new()).expect("query must succeed")
    }

    fn ids(v: &Value) -> Vec<u64> {
        v["entities"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["id"].as_u64().unwrap())
            .collect()
    }

    fn cond(field: &str, op: &str, value: Value) -> Value {
        json!({ "field": field, "op": op, "value": value })
    }

    /// A scene where each filter below has something to keep *and*
    /// something to drop, asserted by `the_fixture_separates_every_filter`.
    fn scene() -> Vec<EntityInfo> {
        let mut lamp = tagged(at(ent(1, "LampA"), [0.0, 5.0, 0.0]), &["light", "indoor"]);
        lamp.light_type = Some("point".into());
        lamp.light_intensity = Some(3.0);
        lamp.light_range = Some(10.0);
        let mut sun = at(ent(2, "Sun"), [0.0, 50.0, 0.0]);
        sun.light_type = Some("directional".into());
        sun.light_intensity = Some(1.0);
        let mut crate_a = tagged(at(ent(3, "CrateA"), [2.0, 0.0, 0.0]), &["prop"]);
        crate_a.mesh_id = Some(7);
        crate_a.scale = Some([1.0, 1.0, 1.0]);
        let mut crate_b = tagged(at(ent(4, "CrateB"), [-3.0, 0.0, 4.0]), &["prop", "indoor"]);
        crate_b.mesh_id = Some(7);
        crate_b.scale = Some([2.0, 0.5, 1.0]);
        crate_b.parent_id = Some(3);
        let mut child = ent(5, "CrateA");
        child.parent_id = Some(4);
        child.visible = false;
        let mut cam = at(ent(6, "Cam"), [0.0, 2.0, -10.0]);
        cam.camera_fov = Some(60.0);
        cam.rotation = Some([0.0, 90.0, 0.0]);
        vec![lamp, sun, crate_a, crate_b, child, cam]
    }

    #[test]
    fn with_no_conditions_every_entity_matches_in_id_order() {
        let es = scene();
        assert_eq!(ids(&run(json!({}), &es)), vec![1, 2, 3, 4, 5, 6]);
        assert_eq!(run(json!({"action": "count"}), &es)["count"], 6);
    }

    /// Each numeric comparison, on a field where the scene has values on
    /// both sides of the threshold (so neither "all" nor "none" passes).
    #[test]
    fn number_comparisons_keep_the_right_side_of_the_threshold() {
        let es = scene();
        let q = |op: &str, v: Value| ids(&run(json!({"where": [cond("position.y", op, v)]}), &es));
        assert_eq!(q("gt", json!(2.0)), vec![1, 2]);
        assert_eq!(q("ge", json!(2.0)), vec![1, 2, 6]);
        assert_eq!(q("lt", json!(2.0)), vec![3, 4]);
        assert_eq!(q("le", json!(2.0)), vec![3, 4, 6]);
        assert_eq!(q("eq", json!(5.0)), vec![1]);
        // 5 has no position: `ne` is false for an absent field.
        assert_eq!(q("ne", json!(5.0)), vec![2, 3, 4, 6]);
        assert_eq!(q("between", json!([1.0, 10.0])), vec![1, 6]);
        assert_eq!(q("in", json!([0.0, 50.0])), vec![2, 3, 4]);
    }

    /// Equality survives the f32 → JSON round trip: 1.1 stored as f32 is
    /// 1.100000023841858 as f64.
    #[test]
    fn number_equality_tolerates_float_round_trip() {
        let es = vec![
            at(ent(1, "A"), [1.1, 0.0, 0.0]),
            at(ent(2, "B"), [1.2, 0.0, 0.0]),
        ];
        let r = run(
            json!({"where": [cond("position.x", "eq", json!(1.1))]}),
            &es,
        );
        assert_eq!(ids(&r), vec![1]);
    }

    #[test]
    fn string_operators_match_names() {
        let es = scene();
        let q = |op: &str, v: Value| ids(&run(json!({"where": [cond("name", op, v)]}), &es));
        assert_eq!(q("starts_with", json!("Crate")), vec![3, 4, 5]);
        assert_eq!(q("ends_with", json!("A")), vec![1, 3, 5]);
        assert_eq!(q("contains", json!("am")), vec![1, 6]);
        assert_eq!(
            q("not_contains", json!("a")),
            vec![2],
            "every other name has an `a`"
        );
        assert_eq!(q("eq", json!("Sun")), vec![2]);
        assert_eq!(q("ne", json!("Sun")), vec![1, 3, 4, 5, 6]);
        assert_eq!(q("in", json!(["Sun", "Cam"])), vec![2, 6]);
        let len = ids(&run(
            json!({"where": [cond("name_length", "eq", json!(3))]}),
            &es,
        ));
        assert_eq!(len, vec![2, 6]);
    }

    #[test]
    fn tag_list_operators() {
        let es = scene();
        let q = |op: &str, v: Value| ids(&run(json!({"where": [cond("tags", op, v)]}), &es));
        assert_eq!(q("contains", json!("prop")), vec![3, 4]);
        assert_eq!(q("not_contains", json!("prop")), vec![1, 2, 5, 6]);
        assert_eq!(q("any_of", json!(["light", "prop"])), vec![1, 3, 4]);
        assert_eq!(q("all_of", json!(["prop", "indoor"])), vec![4]);
        assert_eq!(q("none_of", json!(["indoor"])), vec![2, 3, 5, 6]);
        assert_eq!(q("exists", Value::Null), vec![1, 3, 4]);
        assert_eq!(q("missing", Value::Null), vec![2, 5, 6]);
        let two = ids(&run(
            json!({"where": [cond("tag_count", "ge", json!(2))]}),
            &es,
        ));
        assert_eq!(two, vec![1, 4]);
    }

    /// Presence is the old `has_mesh`/`has_light`/… question; absence is
    /// only reachable through `missing`, since every other operator is
    /// false on an absent field.
    #[test]
    fn exists_and_missing_ask_about_components() {
        let es = scene();
        let q = |f: &str, op: &str| ids(&run(json!({"where": [cond(f, op, Value::Null)]}), &es));
        assert_eq!(q("mesh_id", "exists"), vec![3, 4]);
        assert_eq!(q("light.type", "exists"), vec![1, 2]);
        assert_eq!(q("camera.fov", "exists"), vec![6]);
        assert_eq!(q("position", "missing"), vec![5]);
        assert_eq!(q("parent", "missing"), vec![1, 2, 3, 6]);
        // Absent is false for a comparison, not "less than everything".
        let dim = ids(&run(
            json!({"where": [cond("light.intensity", "lt", json!(2.0))]}),
            &es,
        ));
        assert_eq!(dim, vec![2]);
    }

    /// The flags the tool took before conditions still mean what they did.
    #[test]
    fn the_old_shorthand_flags_still_filter() {
        let es = scene();
        assert_eq!(ids(&run(json!({"has_mesh": true}), &es)), vec![3, 4]);
        assert_eq!(
            ids(&run(json!({"has_light": false}), &es)),
            vec![3, 4, 5, 6]
        );
        assert_eq!(ids(&run(json!({"has_position": false}), &es)), vec![5]);
        assert_eq!(ids(&run(json!({"light_type": "point"}), &es)), vec![1]);
        let both = run(
            json!({"has_mesh": true, "where": [cond("tags", "contains", json!("indoor"))]}),
            &es,
        );
        assert_eq!(ids(&both), vec![4], "a flag and a condition AND together");
    }

    #[test]
    fn hierarchy_fields_come_from_the_whole_scene() {
        let es = scene();
        let q = |f: &str, op: &str, v: Value| ids(&run(json!({"where": [cond(f, op, v)]}), &es));
        assert_eq!(q("depth", "eq", json!(0)), vec![1, 2, 3, 6]);
        assert_eq!(q("depth", "eq", json!(2)), vec![5]);
        assert_eq!(q("children_count", "gt", json!(0)), vec![3, 4]);
        assert_eq!(
            q("ancestors", "contains", json!(3)),
            vec![4, 5],
            "every descendant of 3"
        );
        assert_eq!(q("parent", "eq", json!(4)), vec![5]);
    }

    #[test]
    fn duplicate_names_and_visibility() {
        let es = scene();
        let q = |f: &str, v: bool| ids(&run(json!({"where": [cond(f, "eq", json!(v))]}), &es));
        assert_eq!(q("duplicate_name", true), vec![3, 5]);
        assert_eq!(q("visible", false), vec![5]);
    }

    /// `any` is an OR group; together with `where` it is (all of where)
    /// AND (any of any).
    #[test]
    fn any_is_or_and_combines_with_where() {
        let es = scene();
        let r = run(
            json!({
                "where": [cond("position", "exists", Value::Null)],
                "any": [cond("light.type", "eq", json!("point")), cond("camera.fov", "exists", Value::Null)]
            }),
            &es,
        );
        assert_eq!(ids(&r), vec![1, 6]);
    }

    #[test]
    fn sort_limit_and_distance() {
        let es = scene();
        let nearest = run(
            json!({"where": [cond("position", "exists", Value::Null)],
                   "sort": {"by": "distance"}, "from": [2.0, 0.0, 0.0], "limit": 2}),
            &es,
        );
        assert_eq!(ids(&nearest), vec![3, 1]);
        assert_eq!(nearest["entities"][0]["distance"], 0.0);
        let tallest = run(
            json!({"sort": {"by": "position.y", "desc": true}, "limit": 1}),
            &es,
        );
        assert_eq!(ids(&tallest), vec![2]);
        // Entities without the key sort last, whatever the direction.
        let by_fov = run(json!({"sort": {"by": "camera.fov", "desc": true}}), &es);
        assert_eq!(ids(&by_fov)[0], 6);
        assert_eq!(ids(&by_fov)[1..], [1, 2, 3, 4, 5]);
        let by_name = run(json!({"sort": {"by": "name"}}), &es);
        assert_eq!(ids(&by_name), vec![6, 3, 5, 4, 1, 2], "ties break by id");
        let within = run(
            json!({"where": [cond("distance", "le", json!(6.0))], "from_entity": 3}),
            &es,
        );
        assert_eq!(
            ids(&within),
            vec![1, 3],
            "LampA is sqrt(29) = 5.4 away, CrateB sqrt(41) = 6.4"
        );
    }

    #[test]
    fn select_deselect_and_select_only_change_the_selection() {
        let es = scene();
        let mut sel = HashSet::from([6]);
        let props = json!({"where": [cond("tags", "contains", json!("prop"))], "action": "select"});
        let r = execute(&props, &es, &mut sel).unwrap();
        assert_eq!(
            (r["count"].as_u64(), r["added_count"].as_u64()),
            (Some(2), Some(2))
        );
        assert_eq!(sel, HashSet::from([3, 4, 6]));
        let again = execute(&props, &es, &mut sel).unwrap();
        assert_eq!(again["added_count"], 0, "already selected");

        // `selected` reads the live selection, so a query can narrow it.
        let r = execute(
            &json!({"where": [cond("selected", "eq", json!(true)), cond("mesh_id", "exists", Value::Null)],
                    "action": "deselect"}),
            &es,
            &mut sel,
        )
        .unwrap();
        assert_eq!(r["removed_count"], 2);
        assert_eq!(sel, HashSet::from([6]));

        execute(
            &json!({"light_type": "point", "action": "select_only"}),
            &es,
            &mut sel,
        )
        .unwrap();
        assert_eq!(sel, HashSet::from([1]));

        // Read-only actions leave it alone.
        for action in ["get", "count", "tags", "bounds"] {
            execute(&json!({"action": action}), &es, &mut sel).unwrap();
            assert_eq!(
                sel,
                HashSet::from([1]),
                "{action} must not touch the selection"
            );
        }
    }

    #[test]
    fn tags_and_bounds_aggregate_the_matches() {
        let es = scene();
        let t = run(json!({"action": "tags"}), &es);
        assert_eq!(t["tags"], json!({"indoor": 2, "light": 1, "prop": 2}));
        let props = run(
            json!({"where": [cond("tags", "contains", json!("prop"))], "action": "tags"}),
            &es,
        );
        assert_eq!(props["tags"], json!({"indoor": 1, "prop": 2}));
        let b = run(json!({"has_mesh": true, "action": "bounds"}), &es);
        assert_eq!(b["min"], json!([-3.0, 0.0, 0.0]));
        assert_eq!(b["max"], json!([2.0, 0.0, 4.0]));
        assert_eq!(b["center"], json!([-0.5, 0.0, 2.0]));
        let none = run(
            json!({"where": [cond("position", "missing", Value::Null)], "action": "bounds"}),
            &es,
        );
        assert_eq!(none["min"], Value::Null);
    }

    /// A malformed query is an error on an empty scene too -- it must not
    /// pass as "no matches".
    #[test]
    fn malformed_queries_are_errors_that_name_what_is_accepted() {
        let err = |q: Value| execute(&q, &[], &mut HashSet::new()).unwrap_err();
        assert!(err(json!({"where": [cond("nmae", "eq", json!("x"))]}))
            .contains("unknown field `nmae`"));
        assert!(
            err(json!({"where": [cond("name", "gte", json!("x"))]})).contains("unknown op `gte`")
        );
        assert!(err(json!({"where": [cond("name", "gt", json!("x"))]})).contains("does not apply"));
        assert!(
            err(json!({"where": [cond("position.y", "gt", json!("high"))]}))
                .contains("must be a number")
        );
        assert!(
            err(json!({"where": [cond("position.y", "between", json!([1]))]}))
                .contains("[low, high]")
        );
        assert!(
            err(json!({"where": [{"field": "name", "op": "eq", "vale": 1}]}))
                .contains("unknown key `vale`")
        );
        assert!(err(json!({"action": "delete"})).contains("unknown action"));
        assert!(err(json!({"sort": {"by": "tags"}})).contains("cannot sort"));
        assert!(err(json!({"wher": []})).contains("unknown key `wher`"));
        assert!(err(json!({"sort": {"by": "distance"}})).contains("needs an origin"));
        assert!(err(json!({"from": [0, 0, 0], "from_entity": 1})).contains("not both"));
    }

    /// The fixture makes every filter above a real choice: for each
    /// condition the tests use, the scene has an entity it keeps and one it
    /// drops. Without this, a scene where every entity had a mesh would let
    /// a `has_mesh` that ignored its input pass.
    #[test]
    fn the_fixture_separates_every_filter() {
        let es = scene();
        let n = es.len();
        for c in [
            cond("position.y", "gt", json!(2.0)),
            cond("name", "starts_with", json!("Crate")),
            cond("tags", "contains", json!("prop")),
            cond("mesh_id", "exists", Value::Null),
            cond("light.type", "exists", Value::Null),
            cond("depth", "eq", json!(0)),
            cond("duplicate_name", "eq", json!(true)),
            cond("visible", "eq", json!(false)),
        ] {
            let k = ids(&run(json!({"where": [c.clone()]}), &es)).len();
            assert!(
                k > 0 && k < n,
                "{c} must keep some and drop some, kept {k} of {n}"
            );
        }
    }

    #[test]
    fn every_field_has_a_kind_and_every_kind_an_operator() {
        for f in FIELDS {
            assert!(!ops_for(kind_of(f)).is_empty(), "{f}");
        }
        let es = scene();
        let no_selection = HashSet::new();
        let ctx = Context::new(&es, &no_selection);
        for f in FIELDS {
            if *f == "distance" {
                continue;
            }
            // Every listed field is readable, not silently Absent for all.
            let found = es
                .iter()
                .any(|e| !matches!(ctx.value(e, f), FieldValue::Absent));
            assert!(found, "field `{f}` reads as absent on every fixture entity");
        }
    }
}
