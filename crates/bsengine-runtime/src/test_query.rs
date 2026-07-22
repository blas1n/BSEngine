//! Read-only state queries and mechanical assertion evaluation for the
//! headless test runtime. Queries are evaluated directly against the ECS
//! `World` — no scripting/V8 involvement — using the same shapes as the
//! `Bsengine.*` JS getters for consistency.

use bevy_ecs::world::World;
use bsengine_core::{Transform, Visible};
use bsengine_scene::Name;
use serde_json::{json, Value};

pub fn get_transform(world: &mut World, name: &str) -> Value {
    let mut q = world.query::<(&Name, &Transform)>();
    for (n, t) in q.iter(world) {
        if n.0 == name {
            return json!({
                "x": t.translation.0.x, "y": t.translation.0.y, "z": t.translation.0.z,
                "rx": t.rotation.0.x, "ry": t.rotation.0.y, "rz": t.rotation.0.z, "rw": t.rotation.0.w,
                "sx": t.scale.0.x, "sy": t.scale.0.y, "sz": t.scale.0.z,
            });
        }
    }
    Value::Null
}

pub fn get_visible(world: &mut World, name: &str) -> Value {
    let mut q = world.query::<(&Name, Option<&Visible>)>();
    for (n, v) in q.iter(world) {
        if n.0 == name {
            return json!(v.map(|v| v.is_visible).unwrap_or(true));
        }
    }
    json!(true)
}

pub fn get_entity_names(world: &mut World) -> Value {
    let mut q = world.query::<&Name>();
    let names: Vec<String> = q.iter(world).map(|n| n.0.clone()).collect();
    json!(names)
}

pub fn run_query(world: &mut World, tool: &str, args: &Value) -> Result<Value, String> {
    match tool {
        "get_transform" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_transform requires string 'name'".to_string())?;
            Ok(get_transform(world, name))
        }
        "get_visible" => {
            let name = args
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "get_visible requires string 'name'".to_string())?;
            Ok(get_visible(world, name))
        }
        "get_entity_names" => Ok(get_entity_names(world)),
        other => Err(format!("unknown query tool: {other}")),
    }
}

pub fn eval_path<'a>(value: &'a Value, path: &str) -> Option<&'a Value> {
    if path.is_empty() {
        return Some(value);
    }
    let mut current = value;
    for part in path.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

pub fn eval_op(actual: &Value, op: &str, expected: &Value) -> Result<bool, String> {
    match op {
        "exists" => Ok(!actual.is_null()),
        "==" => Ok(actual == expected),
        "!=" => Ok(actual != expected),
        ">" | ">=" | "<" | "<=" => {
            let a = actual
                .as_f64()
                .ok_or_else(|| format!("actual value {actual} is not numeric"))?;
            let e = expected
                .as_f64()
                .ok_or_else(|| format!("expected value {expected} is not numeric"))?;
            Ok(match op {
                ">" => a > e,
                ">=" => a >= e,
                "<" => a < e,
                "<=" => a <= e,
                _ => unreachable!(),
            })
        }
        other => Err(format!("unknown operator: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn get_transform_returns_null_when_entity_missing() {
        let mut world = World::new();
        assert_eq!(get_transform(&mut world, "Ghost"), Value::Null);
    }

    #[test]
    fn get_transform_returns_position_for_named_entity() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        ));
        let result = get_transform(&mut world, "Player");
        assert_eq!(result["x"], json!(1.0));
        assert_eq!(result["y"], json!(2.0));
        assert_eq!(result["z"], json!(3.0));
    }

    #[test]
    fn get_visible_defaults_true_when_no_visible_component() {
        let mut world = World::new();
        world.spawn((Name("Player".to_string()), Transform::default()));
        assert_eq!(get_visible(&mut world, "Player"), json!(true));
    }

    #[test]
    fn get_visible_reflects_visible_component() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::default(),
            Visible { is_visible: false },
        ));
        assert_eq!(get_visible(&mut world, "Player"), json!(false));
    }

    #[test]
    fn get_entity_names_lists_all_named_entities() {
        let mut world = World::new();
        world.spawn((Name("A".to_string()), Transform::default()));
        world.spawn((Name("B".to_string()), Transform::default()));
        let result = get_entity_names(&mut world);
        let names: Vec<String> =
            serde_json::from_value(result).expect("should deserialize as Vec<String>");
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
    }

    #[test]
    fn run_query_dispatches_get_transform() {
        let mut world = World::new();
        world.spawn((
            Name("Player".to_string()),
            Transform::from_translation(Vec3::new(0.0, 0.0, 5.0)),
        ));
        let result = run_query(&mut world, "get_transform", &json!({"name": "Player"})).unwrap();
        assert_eq!(result["z"], json!(5.0));
    }

    #[test]
    fn run_query_unknown_tool_errors() {
        let mut world = World::new();
        assert!(run_query(&mut world, "nope", &json!({})).is_err());
    }

    #[test]
    fn eval_path_extracts_nested_field() {
        let v = json!({"x": 1, "nested": {"y": 2}});
        assert_eq!(eval_path(&v, "x"), Some(&json!(1)));
        assert_eq!(eval_path(&v, "nested.y"), Some(&json!(2)));
    }

    #[test]
    fn eval_path_empty_returns_whole_value() {
        let v = json!({"x": 1});
        assert_eq!(eval_path(&v, ""), Some(&v));
    }

    #[test]
    fn eval_op_greater_than_numeric() {
        assert!(eval_op(&json!(5), ">", &json!(3)).unwrap());
        assert!(!eval_op(&json!(2), ">", &json!(3)).unwrap());
    }

    #[test]
    fn eval_op_exists_checks_non_null() {
        assert!(!eval_op(&json!(null), "exists", &Value::Null).unwrap());
        assert!(eval_op(&json!(1), "exists", &Value::Null).unwrap());
    }

    #[test]
    fn eval_op_unknown_operator_errors() {
        assert!(eval_op(&json!(1), "~=", &json!(1)).is_err());
    }
}
