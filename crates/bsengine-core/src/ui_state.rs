use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Resource;

/// A single immediate-mode UI element rendered by the HUD/UI system.
#[derive(Clone, Debug)]
pub enum UiWidget {
    /// A static or dynamic text label.
    Label {
        /// Unique widget identifier.
        id: String,
        /// Text content to display.
        text: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Font size, in pixels.
        font_size: f32,
    },
    /// A clickable button.
    Button {
        /// Unique widget identifier.
        id: String,
        /// Text shown on the button.
        label: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Button width, in pixels.
        width: f32,
        /// Button height, in pixels.
        height: f32,
    },
    /// A rectangular container with a title bar.
    Panel {
        /// Unique widget identifier.
        id: String,
        /// Text shown in the panel's title bar.
        title: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Panel width, in pixels.
        width: f32,
        /// Panel height, in pixels.
        height: f32,
    },
    /// An editable single-line text field.
    TextInput {
        /// Unique widget identifier.
        id: String,
        /// Placeholder text shown when the field is empty.
        hint: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Field width, in pixels.
        width: f32,
    },
    /// A texture displayed at a fixed screen position.
    Image {
        /// Unique widget identifier.
        id: String,
        /// Path to the texture asset to display.
        texture_path: String,
        /// X position, in screen-space pixels.
        x: f32,
        /// Y position, in screen-space pixels.
        y: f32,
        /// Image width, in pixels.
        width: f32,
        /// Image height, in pixels.
        height: f32,
    },
}

impl UiWidget {
    /// Returns this widget's unique identifier, regardless of its variant.
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

/// Resource holding the current tree of immediate-mode UI widgets and their
/// per-frame interaction state.
#[derive(Resource, Default, Clone)]
pub struct UiState {
    /// All widgets currently registered for rendering.
    pub widgets: Vec<UiWidget>,
    /// Buttons whose click was registered this frame (cleared next render).
    pub clicked: HashSet<String>,
    /// Current text content for TextInput widgets, keyed by widget id.
    pub text_values: HashMap<String, String>,
}

impl UiState {
    /// Inserts a widget, or replaces the existing widget with the same id.
    pub fn set_widget(&mut self, widget: UiWidget) {
        let id = widget.id().to_string();
        if let Some(pos) = self.widgets.iter().position(|w| w.id() == id) {
            self.widgets[pos] = widget;
        } else {
            self.widgets.push(widget);
        }
    }

    /// Removes the widget with the given id, along with any stored text input value.
    pub fn remove_widget(&mut self, id: &str) {
        self.widgets.retain(|w| w.id() != id);
        self.text_values.remove(id);
    }

    /// Removes all widgets and clears click/text-input state.
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
