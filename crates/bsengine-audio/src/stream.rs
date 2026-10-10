//! Sound pushed in as it is produced -- a video's soundtrack -- rather than
//! loaded whole.
//!
//! A producer (the video decoder) pushes interleaved samples into an
//! [`AudioStream`]; the mixer pulls them out at the output's rate, resampling
//! when the two differ. How much has been pulled -- [`AudioStream::played`]
//! -- is the playback clock a video paces its pictures to, as Unity's,
//! Unreal's and Godot's players all sync video to audio: the sound card's
//! clock is the one that cannot drift.
//!
//! When the buffer runs dry the stream plays silence and the clock stops,
//! so pictures wait for sound rather than run ahead of it.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use kira::sound::{Sound, SoundData};
use kira::{info::Info, Frame};

/// A stream of sound fed by a producer and drained by the mixer. Shared:
/// the producer holds one `Arc`, the mixer's sound another.
pub struct AudioStream {
    samples: Mutex<VecDeque<f32>>,
    channels: u16,
    sample_rate: u32,
    played: AtomicU64,
    paused: AtomicBool,
    stopped: AtomicBool,
    volume: AtomicU32,
}

impl AudioStream {
    /// An empty stream of `channels` interleaved channels at `sample_rate`.
    pub fn new(sample_rate: u32, channels: u16) -> Arc<Self> {
        Arc::new(Self {
            samples: Mutex::new(VecDeque::new()),
            channels: channels.max(1),
            sample_rate: sample_rate.max(1),
            played: AtomicU64::new(0),
            paused: AtomicBool::new(false),
            stopped: AtomicBool::new(false),
            volume: AtomicU32::new(1.0f32.to_bits()),
        })
    }

    /// Appends interleaved samples.
    pub fn push(&self, samples: &[f32]) {
        self.samples
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(samples);
    }

    /// Seconds of sound waiting to be played.
    pub fn buffered(&self) -> f64 {
        let len = self.samples.lock().unwrap_or_else(|e| e.into_inner()).len();
        len as f64 / f64::from(self.channels) / f64::from(self.sample_rate)
    }

    /// Seconds of sound the mixer has played: the playback clock.
    pub fn played(&self) -> f64 {
        self.played.load(Ordering::Acquire) as f64 / f64::from(self.sample_rate)
    }

    /// Pauses or resumes: paused, it plays silence and its clock stands.
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Release);
    }

    /// Linear gain, 1 for as recorded.
    pub fn set_volume(&self, volume: f32) {
        self.volume
            .store(volume.max(0.0).to_bits(), Ordering::Release);
    }

    /// Ends it: the mixer drops its sound.
    pub fn stop(&self) {
        self.stopped.store(true, Ordering::Release);
    }

    /// Takes frames out for the mixer: at most `frames` source frames, as
    /// stereo, counting them as played. Fewer when the buffer is short.
    fn take(&self, frames: usize, out: &mut Vec<Frame>) {
        let mut samples = self.samples.lock().unwrap_or_else(|e| e.into_inner());
        let channels = usize::from(self.channels);
        let available = (samples.len() / channels).min(frames);
        for _ in 0..available {
            let left = samples.pop_front().unwrap_or(0.0);
            let right = if channels > 1 {
                let right = samples.pop_front().unwrap_or(0.0);
                for _ in 2..channels {
                    samples.pop_front();
                }
                right
            } else {
                left
            };
            out.push(Frame { left, right });
        }
        drop(samples);
        self.played.fetch_add(available as u64, Ordering::AcqRel);
    }
}

/// The mixer's side of an [`AudioStream`].
struct StreamSound {
    stream: Arc<AudioStream>,
    /// The source frames bracketing the output's position, and how far
    /// between them it is (0..1): linear resampling.
    previous: Frame,
    next: Option<Frame>,
    fraction: f64,
    scratch: Vec<Frame>,
}

impl Sound for StreamSound {
    fn process(&mut self, out: &mut [Frame], dt: f64, _info: &Info) {
        if self.stream.paused.load(Ordering::Acquire) {
            out.fill(Frame::ZERO);
            return;
        }
        let gain = f32::from_bits(self.stream.volume.load(Ordering::Acquire));
        // Source frames advanced per output frame.
        let step = dt * f64::from(self.stream.sample_rate);
        for slot in out.iter_mut() {
            let Some(next) = self.next.or_else(|| {
                self.scratch.clear();
                self.stream.take(1, &mut self.scratch);
                self.scratch.first().copied()
            }) else {
                // Dry: silence, and the clock waits with it.
                *slot = Frame::ZERO;
                continue;
            };
            self.next = Some(next);
            let t = self.fraction as f32;
            *slot = Frame {
                left: (self.previous.left + (next.left - self.previous.left) * t) * gain,
                right: (self.previous.right + (next.right - self.previous.right) * t) * gain,
            };
            self.fraction += step;
            while self.fraction >= 1.0 {
                self.fraction -= 1.0;
                self.previous = self.next.take().unwrap_or(self.previous);
                self.scratch.clear();
                self.stream.take(1, &mut self.scratch);
                self.next = self.scratch.first().copied();
                if self.next.is_none() {
                    break;
                }
            }
        }
    }

    fn finished(&self) -> bool {
        self.stream.stopped.load(Ordering::Acquire)
    }
}

/// What kira plays: an [`AudioStream`] wrapped as a sound.
pub(crate) struct StreamSoundData(pub(crate) Arc<AudioStream>);

impl SoundData for StreamSoundData {
    type Error = std::convert::Infallible;
    type Handle = ();

    fn into_sound(self) -> Result<(Box<dyn Sound>, Self::Handle), Self::Error> {
        Ok((
            Box::new(StreamSound {
                stream: self.0,
                previous: Frame::ZERO,
                next: None,
                fraction: 0.0,
                scratch: Vec::with_capacity(1),
            }),
            (),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> Info<'static> {
        kira::info::MockInfoBuilder::new().build()
    }

    fn run(sound: &mut StreamSound, frames: usize, output_rate: f64) -> Vec<Frame> {
        let mut out = vec![Frame::ZERO; frames];
        sound.process(&mut out, 1.0 / output_rate, &info());
        out
    }

    fn sound(stream: &Arc<AudioStream>) -> StreamSound {
        let (boxed, ()) = StreamSoundData(stream.clone()).into_sound().unwrap();
        // The concrete type back, for driving it directly.
        let _ = boxed;
        StreamSound {
            stream: stream.clone(),
            previous: Frame::ZERO,
            next: None,
            fraction: 0.0,
            scratch: Vec::new(),
        }
    }

    /// At the source's own rate every pushed frame comes out once, and the
    /// clock counts exactly what was played.
    #[test]
    fn the_clock_counts_what_was_played() {
        let stream = AudioStream::new(48_000, 1);
        stream.push(&vec![0.5; 4_800]);
        let mut s = sound(&stream);
        let out = run(&mut s, 2_400, 48_000.0);
        assert!(
            (stream.played() - 0.05).abs() < 0.001,
            "{}",
            stream.played()
        );
        assert!(out[100].left > 0.4, "the samples come through");
        assert!((stream.buffered() - 0.05).abs() < 0.001, "half left");
    }

    /// Played at twice the source rate, one second of output uses half a
    /// second of source: resampled, not played fast.
    #[test]
    fn a_different_output_rate_is_resampled() {
        let stream = AudioStream::new(24_000, 2);
        stream.push(&vec![0.25; 24_000 * 2]);
        let mut s = sound(&stream);
        run(&mut s, 24_000, 48_000.0);
        assert!(
            (stream.played() - 0.5).abs() < 0.01,
            "half a second of source: {}",
            stream.played()
        );
    }

    /// Dry, it plays silence and the clock stops; paused, likewise.
    #[test]
    fn a_dry_or_paused_stream_is_silent_and_its_clock_stands() {
        let stream = AudioStream::new(48_000, 1);
        stream.push(&[1.0; 480]);
        let mut s = sound(&stream);
        let out = run(&mut s, 960, 48_000.0);
        assert!(out[900].left.abs() < 1e-6, "silence once dry");
        let at = stream.played();
        assert!((at - 0.01).abs() < 0.001, "{at}");
        stream.push(&[1.0; 4_800]);
        stream.set_paused(true);
        let out = run(&mut s, 960, 48_000.0);
        assert!(out.iter().all(|f| f.left == 0.0), "paused: silence");
        assert_eq!(stream.played(), at, "paused: the clock stands");
    }

    /// Volume scales what comes out; stopping finishes the sound.
    #[test]
    fn volume_scales_and_stop_finishes() {
        let stream = AudioStream::new(48_000, 1);
        stream.push(&[1.0; 4_800]);
        stream.set_volume(0.5);
        let mut s = sound(&stream);
        let out = run(&mut s, 100, 48_000.0);
        assert!((out[50].left - 0.5).abs() < 1e-4, "{}", out[50].left);
        assert!(!s.finished());
        stream.stop();
        assert!(s.finished());
    }
}
