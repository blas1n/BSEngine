use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

fn game_fixture_path() -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    format!("{manifest_dir}/../../games/cube-evader")
}

fn send(stdin: &mut std::process::ChildStdin, json: &str) {
    writeln!(stdin, "{json}").unwrap();
    stdin.flush().unwrap();
}

fn read_response(reader: &mut impl BufRead) -> serde_json::Value {
    let mut line = String::new();
    reader
        .read_line(&mut line)
        .expect("failed to read response line");
    serde_json::from_str(&line).unwrap_or_else(|e| panic!("invalid JSON response {line:?}: {e}"))
}

#[test]
fn press_key_and_step_moves_player_forward() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(game_fixture_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bsengine-runtime --test");

    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    send(&mut stdin, r#"{"cmd":"press_key","key":"W"}"#);
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true, "press_key failed: {resp}");

    send(&mut stdin, r#"{"cmd":"step","frames":20}"#);
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true, "step failed: {resp}");
    assert_eq!(resp["data"]["frame"], 20);

    send(
        &mut stdin,
        r#"{"cmd":"assert","query":{"tool":"get_transform","args":{"name":"Player"}},"path":"z","op":"<","value":-1.5,"label":"player moved forward after holding W for 20 frames"}"#,
    );
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true, "assert command failed: {resp}");
    assert_eq!(
        resp["data"]["passed"], true,
        "assertion did not pass: {resp}"
    );

    send(&mut stdin, r#"{"cmd":"shutdown"}"#);
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true);

    let status = child.wait().expect("failed to wait for child process");
    assert!(status.success(), "process exited with {status:?}");
}

#[test]
fn get_entity_names_lists_scene_entities() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_bsengine-runtime"))
        .arg("--test")
        .arg(game_fixture_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to spawn bsengine-runtime --test");

    let mut stdin = child.stdin.take().unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());

    send(&mut stdin, r#"{"cmd":"step","frames":1}"#);
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true, "initial step failed: {resp}");

    send(
        &mut stdin,
        r#"{"cmd":"query","tool":"get_entity_names","args":{}}"#,
    );
    let resp = read_response(&mut reader);
    assert_eq!(resp["ok"], true, "query failed: {resp}");
    let names = resp["data"].as_array().expect("data should be an array");
    let names: Vec<&str> = names.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(names.contains(&"Player"), "names: {names:?}");
    assert!(names.contains(&"Enemy1"), "names: {names:?}");

    send(&mut stdin, r#"{"cmd":"shutdown"}"#);
    read_response(&mut reader);
    child.wait().unwrap();
}
