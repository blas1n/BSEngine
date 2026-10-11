//! macOS: AVFoundation's `AVAssetReader`.
//!
//! An asset reader decodes a file's tracks through the system's codecs --
//! hardware-accelerated where the Mac has it -- and hands back sample
//! buffers in the format asked for: 32-bit BGRA pictures and interleaved
//! 32-bit float sound. This module copies them out and timestamps them, as
//! the Media Foundation backend does on Windows.
//!
//! A reader reads forwards only, so rewinding makes a new one; the asset and
//! its tracks are kept.

#![allow(deprecated)] // `tracksWithMediaType`/`duration`: the synchronous forms, fine on a decoding thread.

use std::path::Path;
use std::ptr::NonNull;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeAudio,
    AVMediaTypeVideo, AVURLAsset,
};
use objc2_core_media::{CMAudioFormatDescriptionGetStreamBasicDescription, CMSampleBuffer};
use objc2_core_video::{
    kCVPixelFormatType_32BGRA, CVPixelBufferGetBaseAddress, CVPixelBufferGetBytesPerRow,
    CVPixelBufferGetHeight, CVPixelBufferGetWidth, CVPixelBufferLockBaseAddress,
    CVPixelBufferLockFlags, CVPixelBufferUnlockBaseAddress,
};
use objc2_foundation::{NSDictionary, NSNumber, NSString, NSURL};

use crate::{AudioChunk, AudioFormat, Packet, VideoDecoder, VideoFrame, VideoInfo};

/// `kAudioFormatLinearPCM`, the four-character code `'lpcm'`.
const LINEAR_PCM: u32 = 0x6C70_636D;

/// One pass over the file: the reader and its two outputs.
struct Reader {
    reader: Retained<AVAssetReader>,
    video: Retained<AVAssetReaderTrackOutput>,
    audio: Option<Retained<AVAssetReaderTrackOutput>>,
}

pub(crate) struct AvFoundationDecoder {
    asset: Retained<AVURLAsset>,
    video_track: Retained<AVAssetTrack>,
    audio_track: Option<Retained<AVAssetTrack>>,
    reader: Reader,
    info: VideoInfo,
    /// The first sound, read at open to learn the audio format, waiting to
    /// be handed out.
    pending_audio: Option<AudioChunk>,
    video_at: f64,
    audio_at: f64,
    video_done: bool,
    audio_done: bool,
}

/// A settings dictionary from (key, number) pairs.
fn settings(pairs: &[(&str, Retained<NSNumber>)]) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: Vec<Retained<NSString>> = pairs.iter().map(|(k, _)| NSString::from_str(k)).collect();
    let key_refs: Vec<&NSString> = keys.iter().map(|k| &**k).collect();
    let values: Vec<&AnyObject> = pairs.iter().map(|(_, v)| v.as_ref()).collect();
    NSDictionary::from_slices(&key_refs, &values)
}

fn start_reader(
    asset: &AVURLAsset,
    video_track: &AVAssetTrack,
    audio_track: Option<&AVAssetTrack>,
) -> Result<Reader, String> {
    // SAFETY: plain AVFoundation calls on objects created here or held by
    // the decoder, all on the one thread that made them.
    unsafe {
        let reader = AVAssetReader::assetReaderWithAsset_error(asset)
            .map_err(|e| format!("cannot read the video: {}", e.localizedDescription()))?;
        // `kCVPixelBufferPixelFormatTypeKey`'s value.
        let video_settings = settings(&[(
            "PixelFormatType",
            NSNumber::numberWithUnsignedInt(kCVPixelFormatType_32BGRA),
        )]);
        let video = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
            video_track,
            Some(&video_settings),
        );
        // Frames are copied out at once; the reader need not copy them first.
        video.setAlwaysCopiesSampleData(false);
        reader.addOutput(&video);
        let audio = audio_track.map(|track| {
            // The AVFoundation keys' values are their own names.
            let audio_settings = settings(&[
                ("AVFormatIDKey", NSNumber::numberWithUnsignedInt(LINEAR_PCM)),
                (
                    "AVLinearPCMBitDepthKey",
                    NSNumber::numberWithUnsignedInt(32),
                ),
                ("AVLinearPCMIsFloatKey", NSNumber::numberWithBool(true)),
                (
                    "AVLinearPCMIsNonInterleaved",
                    NSNumber::numberWithBool(false),
                ),
                ("AVLinearPCMIsBigEndianKey", NSNumber::numberWithBool(false)),
            ]);
            let output = AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
                track,
                Some(&audio_settings),
            );
            output.setAlwaysCopiesSampleData(false);
            reader.addOutput(&output);
            output
        });
        if !reader.startReading() {
            let why = reader
                .error()
                .map(|e| e.localizedDescription().to_string())
                .unwrap_or_else(|| "unknown error".to_string());
            return Err(format!("cannot start reading the video: {why}"));
        }
        Ok(Reader {
            reader,
            video,
            audio,
        })
    }
}

/// The next sample buffer from `output`; `None` at its end. A reader that
/// failed rather than finished is an error.
fn next_buffer(
    reader: &AVAssetReader,
    output: &AVAssetReaderTrackOutput,
) -> Result<Option<Retained<CMSampleBuffer>>, String> {
    // SAFETY: as in `start_reader`.
    unsafe {
        match output.copyNextSampleBuffer() {
            Some(buffer) => Ok(Some(buffer)),
            None if reader.status() == AVAssetReaderStatus::Failed => Err(format!(
                "decoding failed: {}",
                reader
                    .error()
                    .map(|e| e.localizedDescription().to_string())
                    .unwrap_or_default()
            )),
            None => Ok(None),
        }
    }
}

/// A sound sample buffer's bytes as `f32`s.
fn audio_samples(buffer: &CMSampleBuffer) -> Vec<f32> {
    // SAFETY: the copy is bounded by the buffer's own length.
    unsafe {
        let Some(data) = buffer.data_buffer() else {
            return Vec::new();
        };
        let length = data.data_length();
        let mut bytes = vec![0u8; length];
        if length == 0
            || data.copy_data_bytes(0, length, NonNull::new_unchecked(bytes.as_mut_ptr().cast()))
                != 0
        {
            return Vec::new();
        }
        bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect()
    }
}

/// A picture sample buffer's BGRA rows as top-first RGBA.
fn picture(buffer: &CMSampleBuffer) -> Option<(u32, u32, Vec<u8>)> {
    // SAFETY: the base address is read only between lock and unlock, and
    // only within `height` rows of `bytes_per_row`.
    unsafe {
        let image = buffer.image_buffer()?;
        if CVPixelBufferLockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return None;
        }
        let base = CVPixelBufferGetBaseAddress(&image) as *const u8;
        let pitch = CVPixelBufferGetBytesPerRow(&image);
        let (w, h) = (
            CVPixelBufferGetWidth(&image),
            CVPixelBufferGetHeight(&image),
        );
        let mut rgba = vec![255u8; w * h * 4];
        if !base.is_null() {
            for y in 0..h {
                let row = std::slice::from_raw_parts(base.add(y * pitch), w * 4);
                for x in 0..w {
                    let (s, d) = (x * 4, (y * w + x) * 4);
                    rgba[d] = row[s + 2];
                    rgba[d + 1] = row[s + 1];
                    rgba[d + 2] = row[s];
                }
            }
        }
        CVPixelBufferUnlockBaseAddress(&image, CVPixelBufferLockFlags::ReadOnly);
        Some((w as u32, h as u32, rgba))
    }
}

impl AvFoundationDecoder {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        let name = path.display().to_string();
        if !path.is_file() {
            return Err(format!("{name}: no such file"));
        }
        // SAFETY: as in `start_reader`.
        unsafe {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()));
            let asset = AVURLAsset::URLAssetWithURL_options(&url, None);
            let video_type = AVMediaTypeVideo.ok_or("AVFoundation has no video media type")?;
            let video_track = asset
                .tracksWithMediaType(video_type)
                .firstObject()
                .ok_or_else(|| format!("cannot open {name}: no video stream it can decode"))?;
            let audio_track = AVMediaTypeAudio
                .and_then(|audio_type| asset.tracksWithMediaType(audio_type).firstObject());
            let size = video_track.naturalSize();
            let duration = asset.duration().seconds();
            let mut reader = start_reader(&asset, &video_track, audio_track.as_deref())
                .map_err(|e| format!("cannot open {name}: {e}"))?;

            // The audio format is in the sound's own buffers: read the first
            // now, and hand it out first.
            let mut audio = None;
            let mut pending_audio = None;
            if let Some(output) = &reader.audio {
                if let Some(buffer) = next_buffer(&reader.reader, output)? {
                    if let Some(format) = buffer.format_description() {
                        let basic = CMAudioFormatDescriptionGetStreamBasicDescription(&format);
                        if let Some(basic) = basic.as_ref() {
                            audio = Some(AudioFormat {
                                sample_rate: basic.mSampleRate as u32,
                                channels: basic.mChannelsPerFrame as u16,
                            });
                        }
                    }
                    pending_audio = audio.map(|format| AudioChunk {
                        pts: buffer.presentation_time_stamp().seconds(),
                        samples: audio_samples(&buffer),
                        channels: format.channels,
                    });
                }
            }
            if audio.is_none() {
                reader.audio = None;
            }
            Ok(Self {
                asset,
                video_track,
                audio_track,
                reader,
                info: VideoInfo {
                    width: size.width as u32,
                    height: size.height as u32,
                    duration: duration.is_finite().then_some(duration),
                    audio,
                },
                pending_audio,
                video_at: 0.0,
                audio_at: 0.0,
                video_done: false,
                audio_done: audio.is_none(),
            })
        }
    }

    fn next_audio(&mut self) -> Result<Option<AudioChunk>, String> {
        if let Some(chunk) = self.pending_audio.take() {
            return Ok(Some(chunk));
        }
        let Some(output) = &self.reader.audio else {
            return Ok(None);
        };
        let channels = self.info.audio.map_or(1, |a| a.channels);
        Ok(
            next_buffer(&self.reader.reader, output)?.map(|buffer| AudioChunk {
                // SAFETY: a CoreMedia getter on a buffer this thread holds.
                pts: unsafe { buffer.presentation_time_stamp().seconds() },
                samples: audio_samples(&buffer),
                channels,
            }),
        )
    }
}

impl VideoDecoder for AvFoundationDecoder {
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
                let Some(chunk) = self.next_audio()? else {
                    self.audio_done = true;
                    continue;
                };
                let rate = self.info.audio.map_or(1, |a| a.sample_rate);
                let frames = chunk.samples.len() / usize::from(chunk.channels.max(1));
                self.audio_at = chunk.pts + frames as f64 / f64::from(rate);
                if chunk.samples.is_empty() {
                    continue;
                }
                return Ok(Packet::Audio(chunk));
            }
            let Some(buffer) = next_buffer(&self.reader.reader, &self.reader.video)? else {
                self.video_done = true;
                continue;
            };
            // SAFETY: a CoreMedia getter on a buffer this thread holds.
            let pts = unsafe { buffer.presentation_time_stamp().seconds() };
            self.video_at = pts;
            let Some((width, height, rgba)) = picture(&buffer) else {
                continue;
            };
            return Ok(Packet::Video(VideoFrame {
                pts,
                width,
                height,
                rgba,
            }));
        }
    }

    fn rewind(&mut self) -> Result<(), String> {
        // SAFETY: as in `start_reader`.
        unsafe { self.reader.reader.cancelReading() };
        self.reader = start_reader(
            &self.asset,
            &self.video_track,
            self.audio_track
                .as_deref()
                .filter(|_| self.info.audio.is_some()),
        )?;
        self.pending_audio = None;
        self.video_at = 0.0;
        self.audio_at = 0.0;
        self.video_done = false;
        self.audio_done = self.info.audio.is_none();
        Ok(())
    }
}
