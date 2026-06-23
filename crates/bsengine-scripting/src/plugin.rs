use crate::runtime::ScriptRuntime;
use bevy_app::{App, Plugin};

pub struct ScriptRuntimeResource(pub ScriptRuntime);

pub struct ScriptingPlugin;

impl Plugin for ScriptingPlugin {
    fn build(&self, app: &mut App) {
        app.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));
    }
}

#[cfg(test)]
mod tests {
    use super::{ScriptRuntimeResource, ScriptingPlugin};
    use bsengine_app::new_app;

    #[test]
    fn scripting_plugin_registers_runtime() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin);
        assert!(app
            .world()
            .get_non_send_resource::<ScriptRuntimeResource>()
            .is_some());
    }

    #[test]
    fn scripting_plugin_runtime_can_eval() {
        let mut app = new_app();
        app.add_plugins(ScriptingPlugin);

        let result = app
            .world_mut()
            .get_non_send_resource_mut::<ScriptRuntimeResource>()
            .expect("ScriptRuntimeResource not found")
            .0
            .eval("40 + 2");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "42");
    }
}
