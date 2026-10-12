//! Linux: GStreamer, behind the `gstreamer` feature.
//!
//! Linux has no decoder the operating system itself guarantees, as Windows
//! has Media Foundation and macOS AVFoundation; its media framework is
//! GStreamer, which distributions ship and which decodes through whatever
//! plugins are installed (`gst-libav` for H.264/AAC). A `uridecodebin` picks
//! them; the pictures are converted to RGBA and the sound to interleaved
//! 32-bit float, each into an `appsink` this module pulls from -- unsynced,
//! as fast as it is asked, since pacing is the player's business.

use std::path::Path;

use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_pbutils as pbutils;

use crate::{AudioChunk, AudioFormat, Packet, VideoDecoder, VideoFrame, VideoInfo};

/// How long one pull may wait before the pipeline is taken to be stuck.
const PULL_TIMEOUT: gst::ClockTime = gst::ClockTime::from_seconds(10);

pub(crate) struct GStreamerDecoder {
    pipeline: gst::Pipeline,
    video: gst_app::AppSink,
    audio: Option<gst_app::AppSink>,
    info: VideoInfo,
    video_at: f64,
    audio_at: f64,
    video_done: bool,
    audio_done: bool,
}

impl Drop for GStreamerDecoder {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl GStreamerDecoder {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let name = path.display().to_string();
        if !path.is_file() {
            return Err(format!("{name}: no such file"));
        }
        gst::init().map_err(|e| format!("cannot start GStreamer: {e}"))?;
        let uri = gst::glib::filename_to_uri(path, None)
            .map_err(|e| format!("cannot open {name}: {e}"))?;

        // What the file holds, before building the pipeline for it: a silent
        // video must not get an audio branch, which would wait for a sound
        // stream that never comes.
        let discovered = pbutils::Discoverer::new(gst::ClockTime::from_seconds(10))
            .and_then(|d| d.discover_uri(&uri))
            .map_err(|e| format!("cannot open {name}: {e}"))?;
        let video_stream = discovered
            .video_streams()
            .into_iter()
            .next()
            .ok_or_else(|| format!("cannot open {name}: no video stream it can decode"))?;
        let audio = discovered
            .audio_streams()
            .into_iter()
            .next()
            .map(|a| AudioFormat {
                sample_rate: a.sample_rate(),
                channels: a.channels() as u16,
            });

        let audio_branch = if audio.is_some() {
            " d. ! queue ! audioconvert ! audioresample \
             ! audio/x-raw,format=F32LE,layout=interleaved \
             ! appsink name=a sync=false max-buffers=16"
        } else {
            ""
        };
        let description = format!(
            "uridecodebin uri=\"{uri}\" name=d \
             d. ! queue ! videoconvert ! video/x-raw,format=RGBA \
             ! appsink name=v sync=false max-buffers=8{audio_branch}"
        );
        let pipeline = gst::parse::launch_full(&description, None, gst::ParseFlags::empty())
            .map_err(|e| format!("cannot open {name}: {e}"))?
            .downcast::<gst::Pipeline>()
            .map_err(|_| format!("cannot open {name}: not a pipeline"))?;
        let sink = |sink_name: &str| {
            pipeline
                .by_name(sink_name)
                .and_then(|e| e.downcast::<gst_app::AppSink>().ok())
        };
        let video = sink("v").ok_or("the video sink is missing")?;
        let audio_sink = if audio.is_some() { sink("a") } else { None };
        pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| format!("cannot play {name}: {e}"))?;

        Ok(Self {
            pipeline,
            video,
            audio: audio_sink,
            info: VideoInfo {
                width: video_stream.width(),
                height: video_stream.height(),
                duration: discovered.duration().map(|d| d.seconds_f64()),
                audio,
            },
            video_at: 0.0,
            audio_at: 0.0,
            video_done: false,
            audio_done: audio.is_none(),
        })
    }

    /// The next sample from `sink`: its time and bytes, or `None` at its end.
    fn pull(&self, sink: &gst_app::AppSink) -> Result<Option<(f64, Vec<u8>)>, String> {
        match sink.try_pull_sample(PULL_TIMEOUT) {
            Some(sample) => {
                let buffer = sample.buffer().ok_or("a sample with no buffer")?;
                let pts = buffer.pts().map_or(0.0, |t| t.seconds_f64());
                let map = buffer
                    .map_readable()
                    .map_err(|e| format!("cannot read a sample: {e}"))?;
                Ok(Some((pts, map.as_slice().to_vec())))
            }
            None if sink.is_eos() => Ok(None),
            None => {
                let why = self
                    .pipeline
                    .bus()
                    .and_then(|bus| bus.pop_filtered(&[gst::MessageType::Error]))
                    .map(|message| match message.view() {
                        gst::MessageView::Error(e) => e.error().to_string(),
                        _ => String::new(),
                    })
                    .unwrap_or_else(|| "no sample within 10 s".to_string());
                Err(format!("decoding failed: {why}"))
            }
        }
    }
}

impl VideoDecoder for GStreamerDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_packet(&mut self) -> Result<Packet, String> {
        loop {
            if self.video_done && self.audio_done {
                return Ok(Packet::End);
            }
            let take_audio =
                !self.audio_done && (self.video_done || self.audio_at <= self.video_at);
            if take_audio {
                let sink = self
                    .audio
                    .clone()
                    .expect("audio is not done, so it has a sink");
                let Some((pts, bytes)) = self.pull(&sink)? else {
                    self.audio_done = true;
                    continue;
                };
                let channels = self.info.audio.map_or(1, |a| a.channels);
                let rate = self.info.audio.map_or(1, |a| a.sample_rate);
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                self.audio_at =
                    pts + (samples.len() / usize::from(channels.max(1))) as f64 / f64::from(rate);
                if samples.is_empty() {
                    continue;
                }
                return Ok(Packet::Audio(AudioChunk {
                    pts,
                    samples,
                    channels,
                }));
            }
            let Some((pts, bytes)) = self.pull(&self.video.clone())? else {
                self.video_done = true;
                continue;
            };
            self.video_at = pts;
            let (w, h) = (self.info.width as usize, self.info.height as usize);
            // RGBA rows are four bytes a pixel and need no padding to align,
            // so the frame is the bytes as they are -- unless the stride says
            // otherwise, in which case each row is cut to its width.
            let rgba = if bytes.len() == w * h * 4 {
                bytes
            } else {
                let stride = bytes.len() / h.max(1);
                (0..h)
                    .flat_map(|y| bytes[y * stride..y * stride + w * 4].iter().copied())
                    .collect()
            };
            return Ok(Packet::Video(VideoFrame {
                pts,
                width: self.info.width,
                height: self.info.height,
                rgba,
            }));
        }
    }

    fn rewind(&mut self) -> Result<(), String> {
        self.pipeline
            .seek_simple(
                gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                gst::ClockTime::ZERO,
            )
            .map_err(|e| format!("rewind: {e}"))?;
        self.video_at = 0.0;
        self.audio_at = 0.0;
        self.video_done = false;
        self.audio_done = self.info.audio.is_none();
        Ok(())
    }
}
