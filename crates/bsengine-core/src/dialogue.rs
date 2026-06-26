use bevy_ecs::prelude::Component;

/// A single line of dialogue spoken by a participant.
#[derive(Debug, Clone, PartialEq)]
pub struct DialogueLine {
    /// Speaker identifier (e.g. `"npc:innkeeper"`, `"player"`).
    pub speaker: String,
    /// Localisation key or raw text content.
    pub text: String,
    /// Optional audio cue to play with this line.
    pub audio_cue: Option<String>,
    /// How long the line is displayed in seconds. `None` = wait for player advance.
    pub display_time: Option<f32>,
}

impl DialogueLine {
    pub fn new(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            audio_cue: None,
            display_time: None,
        }
    }

    pub fn with_audio(mut self, cue: impl Into<String>) -> Self {
        self.audio_cue = Some(cue.into());
        self
    }

    pub fn timed(mut self, seconds: f32) -> Self {
        self.display_time = Some(seconds.max(0.0));
        self
    }
}

/// A linear dialogue sequence attached to an NPC or trigger entity.
/// The dialogue system plays lines in order and advances on player input or timer.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Dialogue {
    pub lines: Vec<DialogueLine>,
    /// Index of the currently displayed line.
    pub current_index: usize,
    pub looping: bool,
    pub enabled: bool,
}

impl Dialogue {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_index: 0,
            looping: false,
            enabled: true,
        }
    }

    pub fn with_line(mut self, line: DialogueLine) -> Self {
        self.lines.push(line);
        self
    }

    pub fn looping(mut self) -> Self {
        self.looping = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn current_line(&self) -> Option<&DialogueLine> {
        self.lines.get(self.current_index)
    }

    pub fn is_finished(&self) -> bool {
        !self.looping && self.current_index >= self.lines.len()
    }

    /// Advance to the next line. Returns the new current line, or `None` when finished.
    pub fn advance(&mut self) -> Option<&DialogueLine> {
        if !self.enabled || self.lines.is_empty() {
            return None;
        }
        self.current_index += 1;
        if self.looping && self.current_index >= self.lines.len() {
            self.current_index = 0;
        }
        self.lines.get(self.current_index)
    }

    /// Reset to the first line.
    pub fn reset(&mut self) {
        self.current_index = 0;
    }
}

impl Default for Dialogue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dialogue_current_line() {
        let d = Dialogue::new()
            .with_line(DialogueLine::new("npc", "Hello!"))
            .with_line(DialogueLine::new("player", "Hi."));
        assert_eq!(d.current_line().unwrap().text, "Hello!");
    }

    #[test]
    fn dialogue_advance() {
        let mut d = Dialogue::new()
            .with_line(DialogueLine::new("npc", "First"))
            .with_line(DialogueLine::new("npc", "Second"));
        d.advance();
        assert_eq!(d.current_line().unwrap().text, "Second");
    }

    #[test]
    fn dialogue_finished_non_looping() {
        let mut d = Dialogue::new().with_line(DialogueLine::new("npc", "Only line"));
        d.advance();
        assert!(d.is_finished());
    }

    #[test]
    fn dialogue_looping_wraps() {
        let mut d = Dialogue::new()
            .looping()
            .with_line(DialogueLine::new("npc", "A"))
            .with_line(DialogueLine::new("npc", "B"));
        d.advance();
        d.advance();
        assert_eq!(d.current_index, 0);
    }

    #[test]
    fn dialogue_disabled_returns_none() {
        let mut d = Dialogue::new()
            .with_line(DialogueLine::new("npc", "Hi"))
            .disabled();
        assert!(d.advance().is_none());
    }
}
