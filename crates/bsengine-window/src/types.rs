use bsengine_ecs::{Event, Resource};

#[derive(Resource, Debug, Clone)]
pub struct WindowDescriptor {
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub resizable: bool,
}

impl Default for WindowDescriptor {
    fn default() -> Self {
        Self {
            title: "BSEngine".to_string(),
            width: 1280,
            height: 720,
            resizable: true,
        }
    }
}

#[derive(Event, Debug, Clone)]
pub struct WindowResized {
    pub width: u32,
    pub height: u32,
}

#[derive(Event, Debug, Clone)]
pub struct WindowCreated;

#[derive(Event, Debug, Clone)]
pub struct WindowClosed;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_descriptor_default() {
        let desc = WindowDescriptor::default();
        assert_eq!(desc.width, 1280);
        assert_eq!(desc.height, 720);
        assert_eq!(desc.title, "BSEngine");
        assert!(desc.resizable);
    }
}
