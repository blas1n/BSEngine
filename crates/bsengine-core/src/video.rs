//! A video playing on an entity, and the frames it shows.

use std::collections::HashMap;
use std::sync::Arc;

use bevy_ecs::prelude::{Component, ReflectComponent, Resource};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Plays a video file: its pictures as a texture any material or UI image can
/// show by name, its sound through the mixer, the two kept in step.
///
/// Unity's `VideoPlayer` (`url`, `playOnAwake`, `isLooping`, a target
/// texture), Unreal's Media Player with its Media Texture, Godot's
/// `VideoStreamPlayer`. Decoded by the platform (`bsengine-video`), so a file
/// plays here if the player's machine plays it: H.264/AAC in MP4 everywhere.
///
/// The pictures land in the texture named [`texture`](Self::texture) -- the
/// video's own path when that is empty -- which a `Material`'s albedo or a UI
/// image names like any texture path.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct VideoPlayer {
    /// The video file, project-relative (`assets/video/intro.mp4`).
    pub path: String,
    /// The texture name the pictures are shown under; empty for `path`.
    pub texture: String,
    /// Whether it plays. On from the start, as Unity's `playOnAwake`;
    /// cleared to pause, set again to resume.
    pub playing: bool,
    /// Whether it starts over at the end.
    pub looping: bool,
    /// Linear volume of its sound, 1 as recorded.
    pub volume: f32,
    /// The mixer bus its sound plays on; empty for Master.
    pub bus: String,
    /// Where playback is: loading, playing, ended, or why it failed. Written
    /// by the player, not authored.
    #[reflect(ignore)]
    pub status: VideoStatus,
    /// Seconds into the video the picture on screen is from. Written by the
    /// player.
    #[reflect(ignore)]
    pub time: f64,
}

impl Default for VideoPlayer {
    fn default() -> Self {
        Self {
            path: String::new(),
            texture: String::new(),
            playing: true,
            looping: false,
            volume: 1.0,
            bus: String::new(),
            status: VideoStatus::Idle,
            time: 0.0,
        }
    }
}

impl VideoPlayer {
    /// The texture name its pictures are shown under.
    pub fn texture_name(&self) -> &str {
        if self.texture.is_empty() {
            &self.path
        } else {
            &self.texture
        }
    }
}

/// Where a [`VideoPlayer`] is.
#[derive(Debug, Clone, Default, PartialEq)]
pub enum VideoStatus {
    /// Nothing asked for yet.
    #[default]
    Idle,
    /// Opening the file.
    Loading,
    /// Showing pictures (or paused on one).
    Playing,
    /// Reached the end without looping.
    Ended,
    /// Could not play, and why: no such file, a format the platform cannot
    /// decode, a platform with no decoder.
    Failed(String),
}

/// One picture to show under a texture name.
#[derive(Debug, Clone)]
pub struct VideoFrameData {
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// Top row first, RGBA.
    pub rgba: Arc<Vec<u8>>,
    /// Changes with every new picture, so the renderer uploads each once.
    pub serial: u64,
}

/// The pictures videos are showing, by texture name: written by the video
/// player, uploaded by the renderer.
#[derive(Resource, Debug, Default)]
pub struct VideoFrames(pub HashMap<String, VideoFrameData>);
