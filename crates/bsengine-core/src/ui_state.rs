use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Resource;

#[derive(Clone, Debug)]
pub enum UiWidget {
    Label {
        id: String,
        text: String,
        x: f32,
        y: f32,
        font_size: f32,
    },
    Button {
        id: String,
        label: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    Panel {
        id: String,
        title: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
    TextInput {
        id: String,
        hint: String,
        x: f32,
        y: f32,
        width: f32,
    },
    Image {
        id: String,
        texture_path: String,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
    },
}

impl UiWidget {
    pub fn id(&self) -> &str {
        match self {
            Self::Label { id, .. }
            | Self::Button { id, .. }
            | Self::Panel { id, .. }
            | Self::TextInput { id, .. }
            | Self::Image { id, .. } => id,
        }
    }
}

#[derive(Resource, Default, Clone)]
pub struct UiState {
    pub widgets: Vec<UiWidget>,
    /// Buttons whose click was registered this frame (cleared next render).
    pub clicked: HashSet<String>,
    /// Current text content for TextInput widgets, keyed by widget id.
    pub text_values: HashMap<String, String>,
}

impl UiState {
    pub fn set_widget(&mut self, widget: UiWidget) {
        let id = widget.id().to_string();
        if let Some(pos) = self.widgets.iter().position(|w| w.id() == id) {
            self.widgets[pos] = widget;
        } else {
            self.widgets.push(widget);
        }
    }

    pub fn remove_widget(&mut self, id: &str) {
        self.widgets.retain(|w| w.id() != id);
        self.text_values.remove(id);
    }

    pub fn clear(&mut self) {
        self.widgets.clear();
        self.text_values.clear();
        self.clicked.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_widget_inserts_new() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "Hello".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
        });
        assert_eq!(state.widgets.len(), 1);
    }

    #[test]
    fn set_widget_replaces_existing() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "A".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
        });
        state.set_widget(UiWidget::Label {
            id: "lbl".into(),
            text: "B".into(),
            x: 0.0,
            y: 0.0,
            font_size: 16.0,
        });
        assert_eq!(state.widgets.len(), 1);
        if let UiWidget::Label { text, .. } = &state.widgets[0] {
            assert_eq!(text, "B");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn remove_widget_removes_it() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Button {
            id: "btn".into(),
            label: "Click".into(),
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height: 40.0,
        });
        state.remove_widget("btn");
        assert!(state.widgets.is_empty());
    }

    #[test]
    fn clear_empties_all() {
        let mut state = UiState::default();
        state.set_widget(UiWidget::Panel {
            id: "p".into(),
            title: "T".into(),
            x: 0.0,
            y: 0.0,
            width: 200.0,
            height: 150.0,
        });
        state.clicked.insert("btn".into());
        state.text_values.insert("inp".into(), "val".into());
        state.clear();
        assert!(state.widgets.is_empty());
        assert!(state.clicked.is_empty());
        assert!(state.text_values.is_empty());
    }
}
