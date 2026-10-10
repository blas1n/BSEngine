//! Decoding a real file through the platform's decoder.
//!
//! `assets/two_colors.mp4` (made with ffmpeg; H.264 baseline + AAC, 20 KB):
//! 64x48 at 30 fps for two seconds -- the first second red over blue (top
//! half red, bottom half blue), the second all green -- with a 440 Hz mono
//! tone at 48 kHz throughout. Every property checked below is one the file
//! was made to have, so a decoder that got any of them wrong shows here.

use std::path::PathBuf;

fn asset() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/assets/two_colors.mp4")
}

#[cfg(windows)]
mod platform {
    use super::asset;
    use bsengine_video::{open, Packet, VideoFrame};

    fn pixel(frame: &VideoFrame, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * frame.width + x) * 4) as usize;
        [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
    }

    fn near(actual: [u8; 3], expected: [u8; 3]) -> bool {
        actual
            .iter()
            .zip(expected)
            .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() < 50)
    }

    /// Everything until the end: (video frames, audio samples, every pts in
    /// the order the packets came).
    fn drain(
        decoder: &mut dyn bsengine_video::VideoDecoder,
    ) -> (Vec<VideoFrame>, Vec<f32>, Vec<f64>) {
        let (mut frames, mut audio, mut times) = (Vec::new(), Vec::new(), Vec::new());
        for _ in 0..10_000 {
            match decoder.next_packet().expect("decode") {
                Packet::Video(f) => {
                    times.push(f.pts);
                    frames.push(f);
                }
                Packet::Audio(a) => {
                    times.push(a.pts);
                    audio.extend(a.samples);
                }
                Packet::End => return (frames, audio, times),
            }
        }
        panic!("no end after 10,000 packets");
    }

    /// What the file says it holds.
    #[test]
    fn the_info_is_the_files() {
        let decoder = open(&asset()).expect("open");
        let info = decoder.info();
        assert_eq!((info.width, info.height), (64, 48));
        let duration = info.duration.expect("a duration");
        assert!(
            (duration - 2.0).abs() < 0.1,
            "about two seconds: {duration}"
        );
        let audio = info.audio.expect("an audio track");
        assert_eq!((audio.sample_rate, audio.channels), (48_000, 1));
    }

    /// The pictures: every frame, top row first, in the file's colours, at
    /// the times they were made for.
    #[test]
    fn the_frames_are_the_files_pictures() {
        let mut decoder = open(&asset()).expect("open");
        let (frames, _, _) = drain(decoder.as_mut());
        assert!(
            (58..=62).contains(&frames.len()),
            "about 60 frames: {}",
            frames.len()
        );
        let first = &frames[0];
        assert!(first.pts < 0.05, "the first at the start: {}", first.pts);
        assert!(
            near(pixel(first, 32, 4), [255, 0, 0]),
            "red on top: {:?}",
            pixel(first, 32, 4)
        );
        assert!(
            near(pixel(first, 32, 44), [0, 0, 255]),
            "blue below -- the image is not upside down: {:?}",
            pixel(first, 32, 44)
        );
        let late = frames
            .iter()
            .find(|f| f.pts > 1.2)
            .expect("a frame after a second");
        assert!(
            near(pixel(late, 32, 4), [0, 255, 0]) && near(pixel(late, 32, 44), [0, 255, 0]),
            "all green in the second second: {:?}",
            pixel(late, 32, 4)
        );
        assert!(
            frames.windows(2).all(|w| w[1].pts > w[0].pts),
            "in presentation order"
        );
    }

    /// The sound: two seconds of samples, at 440 Hz -- 880 zero crossings a
    /// second -- and interleaved with the pictures by time.
    #[test]
    fn the_audio_is_the_files_tone_in_step_with_the_pictures() {
        let mut decoder = open(&asset()).expect("open");
        let (_, audio, times) = drain(decoder.as_mut());
        let seconds = audio.len() as f64 / 48_000.0;
        assert!(
            (seconds - 2.0).abs() < 0.1,
            "about two seconds of sound: {seconds}"
        );
        let middle = &audio[24_000..72_000];
        let crossings = middle
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        assert!(
            (850..=910).contains(&crossings),
            "a 440 Hz tone crosses zero 880 times a second: {crossings}"
        );
        // Neither stream runs ahead: no packet is more than a chunk (well
        // under 0.2 s) behind the latest time already given out.
        let mut latest = 0.0f64;
        for t in &times {
            assert!(*t > latest - 0.2, "{t} came after {latest}");
            latest = latest.max(*t);
        }
    }

    /// Rewinding starts it over: the first picture again, from time 0.
    #[test]
    fn rewinding_plays_it_again() {
        let mut decoder = open(&asset()).expect("open");
        drain(decoder.as_mut());
        decoder.rewind().expect("rewind");
        let (frames, audio, _) = drain(decoder.as_mut());
        assert!(frames[0].pts < 0.05, "from the start: {}", frames[0].pts);
        assert!(
            frames.len() >= 58 && audio.len() > 90_000,
            "all of it again"
        );
    }

    /// A file that is not a video is an error with its name in it, not a
    /// panic.
    #[test]
    fn a_file_that_is_not_a_video_is_refused() {
        let not_video = std::env::temp_dir().join("bsengine-not-a-video.mp4");
        std::fs::write(&not_video, b"not a video at all").unwrap();
        let error = open(&not_video).err().expect("refused");
        assert!(error.contains("bsengine-not-a-video"), "{error}");
        let _ = std::fs::remove_file(&not_video);
    }
}

/// Where there is no backend yet, opening says so -- it does not panic, and
/// it does not pretend.
#[cfg(not(windows))]
#[test]
fn without_a_backend_opening_says_so() {
    let error = bsengine_video::open(&asset()).err().expect("no decoder");
    assert!(error.contains("no decoder on this platform"), "{error}");
}
