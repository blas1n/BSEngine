//! Video playback through each platform's own decoder.
//!
//! Unity's `VideoPlayer` and Unreal's Media Framework both hand decoding to
//! the operating system -- Media Foundation on Windows, AVFoundation on
//! Apple, the platform's media stack elsewhere -- which plays the formats a
//! player's machine already plays (H.264/AAC in MP4 above all), with
//! hardware decoding where there is some, and no codec shipped or licensed
//! by the engine. This crate is that: one [`VideoDecoder`] interface, one
//! backend per platform.
//!
//! A decoder yields [`Packet`]s in presentation order across both streams --
//! video frames as RGBA, audio as interleaved `f32` -- each with its
//! presentation time. Pacing them against a clock is the player's business,
//! not the decoder's.
//!
//! Backends: Windows (Media Foundation). Elsewhere [`open`] reports the
//! platform unsupported, and a player shows nothing rather than failing.
#![warn(missing_docs)]

#[cfg(windows)]
mod media_foundation;
/// The ECS side: `VideoPlugin` plays every entity's `VideoPlayer`.
pub mod plugin;
pub use plugin::{Playbacks, VideoPlugin};

/// What a video holds.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    /// Frame width, in pixels.
    pub width: u32,
    /// Frame height, in pixels.
    pub height: u32,
    /// Length, in seconds, when the container says.
    pub duration: Option<f64>,
    /// The audio track's format, or `None` for a silent video.
    pub audio: Option<AudioFormat>,
}

/// An audio track's format: what [`AudioChunk`] samples are in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    /// Frames per second.
    pub sample_rate: u32,
    /// Interleaved channels per frame.
    pub channels: u16,
}

/// One decoded picture.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// When it is shown, in seconds from the start.
    pub pts: f64,
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// Top row first, four bytes per pixel, alpha 255.
    pub rgba: Vec<u8>,
}

/// A stretch of decoded sound.
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// When its first frame plays, in seconds from the start.
    pub pts: f64,
    /// Interleaved samples, `channels` per frame.
    pub samples: Vec<f32>,
    /// Channels per frame.
    pub channels: u16,
}

/// What a decoder produces next.
#[derive(Debug, Clone)]
pub enum Packet {
    /// A picture.
    Video(VideoFrame),
    /// Some sound.
    Audio(AudioChunk),
    /// Both streams have ended.
    End,
}

/// A platform's decoder for one video.
pub trait VideoDecoder {
    /// What the video holds.
    fn info(&self) -> &VideoInfo;
    /// The next packet, in presentation order across the streams.
    ///
    /// # Errors
    ///
    /// The decoder's failure, described.
    fn next_packet(&mut self) -> Result<Packet, String>;
    /// Back to the start, for looping.
    ///
    /// # Errors
    ///
    /// The decoder's failure, described.
    fn rewind(&mut self) -> Result<(), String>;
}

/// Opens `path` with this platform's decoder.
///
/// # Errors
///
/// A file the platform cannot open or decode, or a platform with no backend
/// yet, described.
pub fn open(path: &std::path::Path) -> Result<Box<dyn VideoDecoder>, String> {
    #[cfg(windows)]
    {
        media_foundation::MediaFoundationDecoder::open(path)
            .map(|d| Box::new(d) as Box<dyn VideoDecoder>)
    }
    #[cfg(not(windows))]
    {
        Err(format!(
            "cannot play {}: video playback has no decoder on this platform yet",
            path.display()
        ))
    }
}
