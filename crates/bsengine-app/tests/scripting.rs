use bsengine_app::new_app;
use bsengine_scripting::{ScriptRuntimeResource, ScriptingPlugin};

#[test]
fn scripting_plugin_in_full_app() {
    let mut app = new_app();
    app.add_plugins(ScriptingPlugin::default());
    app.update();

    let result = app
        .world_mut()
        .get_non_send_resource_mut::<ScriptRuntimeResource>()
        .expect("ScriptRuntimeResource not found")
        .0
        .eval("'BSEngine ' + 'scripting'");
    assert!(result.is_ok());
    assert!(result.unwrap().contains("BSEngine scripting"));
}

#[test]
fn log_op_callable_in_full_app() {
    let mut app = new_app();
    app.add_plugins(ScriptingPlugin::default());
    app.update();

    let result = app
        .world_mut()
        .get_non_send_resource_mut::<ScriptRuntimeResource>()
        .expect("ScriptRuntimeResource not found")
        .0
        .eval(
            r#"
            Deno.core.ops.bsengine_log("integration test log");
            Deno.core.ops.bsengine_version()
        "#,
        );
    assert!(result.is_ok(), "op eval failed: {:?}", result);
    assert!(result.unwrap().contains("0.1"));
}
