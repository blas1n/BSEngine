use bsengine_app::new_app;
use bsengine_asset::AssetPlugin;
use bsengine_scripting::{ScriptRuntimeResource, ScriptingPlugin};

#[test]
fn scripting_plugin_in_full_app() {
    let mut app = new_app();
    app.add_plugins(AssetPlugin);
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
    app.add_plugins(AssetPlugin);
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

#[test]
fn a_scripted_burst_reaches_the_emitter() {
    // The op-level test proves the command is enqueued. This proves the handler
    // finds the entity and calls burst() -- the half a script depends on, and
    // the half `setLifetime` was missing for the whole time its own op test was
    // passing.
    //
    // Driven by a real script file rather than by eval'ing the op, because
    // `run_scripts` returns early when no entity has a `Script`: a command
    // queued with none attached is never drained. Reasonable (nothing to run),
    // but it means "eval the op and update" is not a configuration the engine
    // ever produces, so a test built on it would prove nothing about the game.
    use bsengine_app::{ParticlePlugin, TimePlugin};
    use bsengine_core::{ParticleEmitter, Time, Transform};
    use bsengine_scene::{Name, ScriptPath};

    let script_path =
        std::env::temp_dir().join(format!("bsengine_test_burst_{}.js", std::process::id()));
    std::fs::write(
        &script_path,
        "let fired = false;
         function onUpdate(self) { if (!fired) { fired = true; Bsengine.burstParticles(self); } }",
    )
    .unwrap();

    let mut app = new_app();
    app.add_plugins(AssetPlugin);
    app.add_plugins(TimePlugin);
    app.add_plugins(ParticlePlugin);
    // Empty project_dir, not `default()`: the default is ".", which gets
    // prepended to the script path, and the temp file's path is absolute.
    app.add_plugins(ScriptingPlugin {
        project_dir: String::new(),
    });
    app.insert_resource(Time::fixed(1.0 / 60.0));

    let e = app
        .world_mut()
        .spawn((
            Name("Sparks".to_string()),
            Transform::default(),
            ScriptPath(script_path.to_string_lossy().to_string()),
            ParticleEmitter {
                rate: 0.0,
                burst_count: 9,
                particle_lifetime: 100.0,
                ..Default::default()
            },
        ))
        .id();

    // Run until the burst lands rather than for a fixed number of frames: how
    // long the script takes to load is `bevy_asset`'s business, and pinning a
    // count here would turn a slower filesystem into a failure that says
    // nothing about this code.
    let mut frames = 0;
    loop {
        app.update();
        frames += 1;
        if !app
            .world()
            .get::<ParticleEmitter>(e)
            .unwrap()
            .live
            .is_empty()
        {
            break;
        }
        assert!(
            frames < 600,
            "no particles after {frames} frames; the script never ran, or the              command never reached the emitter"
        );
    }

    assert_eq!(
        app.world().get::<ParticleEmitter>(e).unwrap().live.len(),
        9,
        "a burst issued from a script emits exactly burst_count particles"
    );

    let _ = std::fs::remove_file(&script_path);
}
