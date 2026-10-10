//! Windows: Media Foundation's Source Reader.
//!
//! The Source Reader opens a file, picks the decoders the system has for its
//! streams, and -- with video processing enabled -- converts whatever the
//! decoder produces (NV12, usually) to the format asked for. Video is asked
//! for as 32-bit RGB, audio as interleaved 32-bit float, so this module only
//! has to copy bytes out and timestamp them.

use std::path::Path;

use windows::core::{GUID, HSTRING};
use windows::Win32::Media::MediaFoundation::*;
use windows::Win32::System::Com::StructuredStorage::PROPVARIANT;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VT_I8;

use crate::{AudioChunk, AudioFormat, Packet, VideoDecoder, VideoFrame, VideoInfo};

const VIDEO: u32 = MF_SOURCE_READER_FIRST_VIDEO_STREAM.0 as u32;
const AUDIO: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;
const MEDIA_SOURCE: u32 = MF_SOURCE_READER_MEDIASOURCE.0 as u32;

/// Media Foundation time: 100 ns units.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

pub(crate) struct MediaFoundationDecoder {
    reader: IMFSourceReader,
    info: VideoInfo,
    /// The stride Media Foundation lays video rows out at; negative when the
    /// image is stored bottom row first.
    stride: i32,
    /// Where each stream has got to, so the one behind is read next and the
    /// packets come out in presentation order.
    video_at: f64,
    audio_at: f64,
    video_done: bool,
    audio_done: bool,
}

fn describe(what: &str, e: windows::core::Error) -> String {
    format!("{what}: {e}")
}

impl MediaFoundationDecoder {
    pub(crate) fn open(path: &Path) -> Result<Self, String> {
        // SAFETY (all of this function): plain Media Foundation calls on
        // interfaces created here; every out-pointer is a local.
        unsafe {
            // Already initialised, in either model, is fine: this only needs
            // COM to be up on this thread.
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            MFStartup(MF_VERSION, MFSTARTUP_FULL).map_err(|e| describe("MFStartup", e))?;

            let mut attributes = None;
            MFCreateAttributes(&mut attributes, 1).map_err(|e| describe("attributes", e))?;
            let attributes = attributes.ok_or("no attributes")?;
            // Let the reader convert the decoder's output (NV12, mostly) to
            // the RGB asked for below.
            attributes
                .SetUINT32(&MF_SOURCE_READER_ENABLE_VIDEO_PROCESSING, 1)
                .map_err(|e| describe("video processing", e))?;

            let url = HSTRING::from(path.as_os_str());
            let reader = MFCreateSourceReaderFromURL(&url, &attributes)
                .map_err(|e| describe(&format!("cannot open {}", path.display()), e))?;

            let video = MFCreateMediaType().map_err(|e| describe("media type", e))?;
            video
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)
                .and_then(|()| video.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32))
                .map_err(|e| describe("video type", e))?;
            reader
                .SetCurrentMediaType(VIDEO, None, &video)
                .map_err(|e| describe(&format!("no video stream in {}", path.display()), e))?;
            let current = reader
                .GetCurrentMediaType(VIDEO)
                .map_err(|e| describe("video format", e))?;
            let size = current
                .GetUINT64(&MF_MT_FRAME_SIZE)
                .map_err(|e| describe("frame size", e))?;
            let (width, height) = ((size >> 32) as u32, size as u32);
            let stride = current
                .GetUINT32(&MF_MT_DEFAULT_STRIDE)
                .map(|s| s as i32)
                .unwrap_or((width * 4) as i32);

            // Audio is optional: a silent video has no audio stream, and the
            // reader says so by refusing a type for it.
            let audio_type = MFCreateMediaType().map_err(|e| describe("media type", e))?;
            audio_type
                .SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)
                .and_then(|()| audio_type.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float))
                .map_err(|e| describe("audio type", e))?;
            let audio = match reader.SetCurrentMediaType(AUDIO, None, &audio_type) {
                Ok(()) => {
                    let current = reader
                        .GetCurrentMediaType(AUDIO)
                        .map_err(|e| describe("audio format", e))?;
                    Some(AudioFormat {
                        sample_rate: current
                            .GetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND)
                            .map_err(|e| describe("sample rate", e))?,
                        channels: current
                            .GetUINT32(&MF_MT_AUDIO_NUM_CHANNELS)
                            .map_err(|e| describe("channels", e))?
                            as u16,
                    })
                }
                Err(_) => None,
            };

            let duration = reader
                .GetPresentationAttribute(MEDIA_SOURCE, &MF_PD_DURATION)
                .ok()
                .map(|v| v.Anonymous.Anonymous.Anonymous.uhVal as f64 / TICKS_PER_SECOND);

            Ok(Self {
                reader,
                info: VideoInfo {
                    width,
                    height,
                    duration,
                    audio,
                },
                stride,
                video_at: 0.0,
                audio_at: 0.0,
                video_done: false,
                audio_done: audio.is_none(),
            })
        }
    }

    /// Reads one sample from `stream`: its time and its bytes, or `None` at
    /// the stream's end.
    fn read(&self, stream: u32) -> Result<Option<(f64, Vec<u8>)>, String> {
        // SAFETY: the out-pointers are locals; the buffer is locked only for
        // the copy and unlocked before it is released.
        unsafe {
            let mut flags = 0u32;
            let mut time = 0i64;
            let mut sample: Option<IMFSample> = None;
            self.reader
                .ReadSample(
                    stream,
                    0,
                    None,
                    Some(&mut flags),
                    Some(&mut time),
                    Some(&mut sample),
                )
                .map_err(|e| describe("ReadSample", e))?;
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(None);
            }
            // A gap, a format change: no sample this time, but not the end.
            let Some(sample) = sample else {
                return Ok(Some((time as f64 / TICKS_PER_SECOND, Vec::new())));
            };
            let buffer = sample
                .ConvertToContiguousBuffer()
                .map_err(|e| describe("buffer", e))?;
            let mut data = std::ptr::null_mut();
            let mut length = 0u32;
            buffer
                .Lock(&mut data, None, Some(&mut length))
                .map_err(|e| describe("lock", e))?;
            let bytes = std::slice::from_raw_parts(data, length as usize).to_vec();
            let _ = buffer.Unlock();
            Ok(Some((time as f64 / TICKS_PER_SECOND, bytes)))
        }
    }

    /// 32-bit RGB rows (BGRX, `stride` apart, bottom first when negative) as
    /// top-first RGBA.
    fn to_rgba(&self, bytes: &[u8]) -> Vec<u8> {
        let (w, h) = (self.info.width as usize, self.info.height as usize);
        let pitch = self.stride.unsigned_abs() as usize;
        let mut rgba = vec![255u8; w * h * 4];
        for y in 0..h {
            let source_row = if self.stride < 0 { h - 1 - y } else { y };
            let Some(row) = bytes.get(source_row * pitch..source_row * pitch + w * 4) else {
                break;
            };
            for x in 0..w {
                let (s, d) = (x * 4, (y * w + x) * 4);
                rgba[d] = row[s + 2];
                rgba[d + 1] = row[s + 1];
                rgba[d + 2] = row[s];
            }
        }
        rgba
    }
}

impl VideoDecoder for MediaFoundationDecoder {
    fn info(&self) -> &VideoInfo {
        &self.info
    }

    fn next_packet(&mut self) -> Result<Packet, String> {
        loop {
            // Whichever stream is behind goes next.
            let take_audio =
                !self.audio_done && (self.video_done || self.audio_at <= self.video_at);
            if self.video_done && self.audio_done {
                return Ok(Packet::End);
            }
            if take_audio {
                let Some((pts, bytes)) = self.read(AUDIO)? else {
                    self.audio_done = true;
                    continue;
                };
                let channels = self.info.audio.map_or(1, |a| a.channels);
                let rate = self.info.audio.map_or(1, |a| a.sample_rate);
                let samples: Vec<f32> = bytes
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                    .collect();
                let frames = samples.len() / usize::from(channels.max(1));
                self.audio_at = pts + frames as f64 / f64::from(rate);
                if samples.is_empty() {
                    continue;
                }
                return Ok(Packet::Audio(AudioChunk {
                    pts,
                    samples,
                    channels,
                }));
            }
            let Some((pts, bytes)) = self.read(VIDEO)? else {
                self.video_done = true;
                continue;
            };
            self.video_at = pts;
            if bytes.is_empty() {
                continue;
            }
            return Ok(Packet::Video(VideoFrame {
                pts,
                width: self.info.width,
                height: self.info.height,
                rgba: self.to_rgba(&bytes),
            }));
        }
    }

    fn rewind(&mut self) -> Result<(), String> {
        // SAFETY: a zeroed PROPVARIANT given a type tag and a value, as
        // SetCurrentPosition reads it.
        unsafe {
            let mut position: PROPVARIANT = std::mem::zeroed();
            (*position.Anonymous.Anonymous).vt = VT_I8;
            (*position.Anonymous.Anonymous).Anonymous.hVal = 0;
            self.reader
                .SetCurrentPosition(&GUID::zeroed(), &position)
                .map_err(|e| describe("rewind", e))?;
        }
        self.video_at = 0.0;
        self.audio_at = 0.0;
        self.video_done = false;
        self.audio_done = self.info.audio.is_none();
        Ok(())
    }
}
