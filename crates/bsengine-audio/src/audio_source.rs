use bevy_asset::Asset;
use bevy_reflect::TypePath;
use kira::sound::static_sound::StaticSoundData;

/// Decoded audio sample data, ready to play (possibly repeatedly — kira's
/// `StaticSoundData` clones cheaply, its sample frames are reference-counted).
///
/// Not re-exported at the crate root as a bare `AudioSource` — this crate
/// already has an ECS [`Component`](bevy_ecs::prelude::Component) of that
/// name in [`crate::components`] (pre-loaded playback data attached directly
/// to an entity, a different concept from this `bevy_asset::Asset`). Refer to
/// this type via its module path, `bsengine_audio::audio_source::AudioSource`.
#[derive(Asset, TypePath, Clone)]
pub struct AudioSource(pub StaticSoundData);

/// Reads and decodes an audio file from disk.
pub fn load_audio_source(path: &str) -> Result<AudioSource, String> {
    StaticSoundData::from_file(path)
        .map(AudioSource)
        .map_err(|e| e.to_string())
}

use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};

/// Backs `LoadMode::Async` for audio via `AssetServer::load`.
#[derive(Default)]
pub struct AudioSourceLoader;

impl AssetLoader for AudioSourceLoader {
    type Asset = AudioSource;
    type Settings = ();
    type Error = String;

    async fn load<'a>(
        &'a self,
        reader: &'a mut Reader<'_>,
        _settings: &'a Self::Settings,
        _load_context: &'a mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        use bevy_asset::io::AsyncReadExt;
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|e| format!("read: {e}"))?;
        let cursor = std::io::Cursor::new(bytes);
        StaticSoundData::from_cursor(cursor)
            .map(AudioSource)
            .map_err(|e| format!("decode: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_audio_source_missing_file_errors() {
        assert!(load_audio_source("definitely/missing.wav").is_err());
    }

    /// Hand-assembles the smallest possible valid FLAC file (one metadata
    /// block + one frame, mono, 16-bit, 8kHz, 192 samples of silence via a
    /// `CONSTANT` subframe) rather than relying on a checked-in binary
    /// fixture.
    ///
    /// No `.wav`/`.ogg`/`.mp3`/`.flac` file exists anywhere in this repo
    /// (`git ls-files` turns up nothing), so there is no real fixture to
    /// point at. Worse, this workspace's `kira` dependency
    /// (`features = ["cpal", "wav", "ogg", "mp3", "flac"]` in the root
    /// `Cargo.toml`) only pulls in `symphonia`'s WAV/OGG *container* readers,
    /// not the "pcm"/"vorbis" *codec* features kira's own `default` feature
    /// set would otherwise include — so even a real checked-in `.wav`
    /// (PCM) or `.ogg` (Vorbis) file would fail to decode today
    /// (`symphonia-codec-pcm`/`symphonia-codec-vorbis` aren't even in
    /// Cargo.lock). Only `.mp3`/`.flac` are fully wired (format + codec
    /// bundled together in `symphonia-bundle-mp3`/`symphonia-bundle-flac`,
    /// both present in Cargo.lock), and FLAC's `CONSTANT` subframe type is
    /// simple enough to hand-encode without an encoder library. This is
    /// exercised for real by `audio_source_loads_async_and_becomes_available`
    /// below (decoded by the same `symphonia` backend kira uses in
    /// production), not just asserted structurally.
    fn minimal_flac_silence() -> Vec<u8> {
        struct BitWriter {
            bits: Vec<bool>,
        }
        impl BitWriter {
            fn new() -> Self {
                Self { bits: Vec::new() }
            }
            fn push(&mut self, value: u64, n: u32) {
                for i in (0..n).rev() {
                    self.bits.push((value >> i) & 1 == 1);
                }
            }
            fn into_bytes(self) -> Vec<u8> {
                assert_eq!(self.bits.len() % 8, 0, "bitstream must be byte-aligned");
                let mut out = vec![0u8; self.bits.len() / 8];
                for (i, bit) in self.bits.iter().enumerate() {
                    if *bit {
                        out[i / 8] |= 1 << (7 - (i % 8));
                    }
                }
                out
            }
        }

        // FLAC's frame-header CRC-8 (poly 0x07) and frame-footer CRC-16
        // (poly 0x8005): both MSB-first, unreflected, initial value 0.
        fn crc8(data: &[u8]) -> u8 {
            let mut crc: u8 = 0;
            for &byte in data {
                crc ^= byte;
                for _ in 0..8 {
                    crc = if crc & 0x80 != 0 {
                        (crc << 1) ^ 0x07
                    } else {
                        crc << 1
                    };
                }
            }
            crc
        }
        fn crc16(data: &[u8]) -> u16 {
            let mut crc: u16 = 0;
            for &byte in data {
                crc ^= (byte as u16) << 8;
                for _ in 0..8 {
                    crc = if crc & 0x8000 != 0 {
                        (crc << 1) ^ 0x8005
                    } else {
                        crc << 1
                    };
                }
            }
            crc
        }

        const SAMPLE_RATE: u64 = 8000;
        const BLOCK_SIZE: u64 = 192;
        const BITS_PER_SAMPLE: u64 = 16;
        // Symphonia's FLAC packet parser has no explicit frame-length field to
        // rely on — it locates each frame's end by scanning ahead for the
        // *next* frame's sync bytes. A single frame smaller than
        // `FLAC_MAX_FRAME_HEADER_SIZE` (16 bytes) makes that scan wrap back
        // over the frame's own header and misidentify it as its own
        // successor (confirmed empirically: a 1-frame version of this
        // fixture fails with a spurious `UnexpectedEof`/"end of stream").
        // Four small frames give the parser genuine look-ahead room.
        const NUM_FRAMES: u64 = 4;

        // STREAMINFO body (34 bytes): 18 bytes of bit-packed fields, then a
        // 16-byte MD5 signature (left all-zero; decoders don't require it to
        // match to decode correctly, only to verify).
        let mut w = BitWriter::new();
        w.push(BLOCK_SIZE, 16); // min blocksize
        w.push(BLOCK_SIZE, 16); // max blocksize
        w.push(0, 24); // min framesize (unknown)
        w.push(0, 24); // max framesize (unknown)
        w.push(SAMPLE_RATE, 20);
        w.push(0, 3); // channels - 1 (mono)
        w.push(BITS_PER_SAMPLE - 1, 5);
        w.push(BLOCK_SIZE * NUM_FRAMES, 36); // total samples in stream
        let mut streaminfo = w.into_bytes();
        streaminfo.extend_from_slice(&[0u8; 16]); // MD5, unset
        assert_eq!(streaminfo.len(), 34);

        let mut metadata_block_header = vec![0x80u8]; // last-metadata-block=1, type=0 (STREAMINFO)
        let len = streaminfo.len() as u32;
        metadata_block_header.push((len >> 16) as u8);
        metadata_block_header.push((len >> 8) as u8);
        metadata_block_header.push(len as u8);

        // Frame header (6 bytes once CRC-8 is appended): sync(14) + reserved(1)
        // + fixed-blocksize(1) + block-size-code(4)=192 + sample-rate-code(4)=
        // "get from STREAMINFO" + channel-assignment(4)=mono + sample-size-
        // code(3)="get from STREAMINFO" + reserved(1), then the frame number
        // (UTF8-like coded; values 0-3 each encode as a single byte), then
        // CRC-8. Followed by one CONSTANT subframe (silence) and a CRC-16
        // footer.
        let build_frame = |frame_number: u8| -> Vec<u8> {
            let mut hw = BitWriter::new();
            hw.push(0b11111111111110, 14); // sync code
            hw.push(0, 1); // reserved
            hw.push(0, 1); // fixed blocksize
            hw.push(0b0001, 4); // block size = 192 (lookup)
            hw.push(0b0000, 4); // sample rate: from STREAMINFO
            hw.push(0b0000, 4); // channel assignment: mono
            hw.push(0b000, 3); // sample size: from STREAMINFO
            hw.push(0, 1); // reserved
            let mut frame = hw.into_bytes();
            frame.push(frame_number); // frame number, UTF8-like (< 0x80 => single byte)

            // CRC-8 covers only the header just written (sync + desc + frame
            // number) — it must be appended before the subframe bytes below,
            // not after (the decoder reads it right after the frame number).
            frame.push(crc8(&frame));

            // Subframe: CONSTANT type, value 0 (silence).
            frame.push(0x00); // zero-bit + type=CONSTANT(000000) + no wasted bits
            frame.push(0x00); // sample value, high byte
            frame.push(0x00); // sample value, low byte

            let footer = crc16(&frame);
            frame.push((footer >> 8) as u8);
            frame.push(footer as u8);
            frame
        };

        let mut out = b"fLaC".to_vec();
        out.extend_from_slice(&metadata_block_header);
        out.extend_from_slice(&streaminfo);
        for frame_number in 0..NUM_FRAMES as u8 {
            out.extend_from_slice(&build_frame(frame_number));
        }
        out
    }

    #[test]
    fn minimal_flac_silence_decodes_via_kira() {
        // Sanity-checks the hand-rolled fixture directly against kira's
        // synchronous loader (same decode path `load_audio_source` uses)
        // before trusting it in the async-loader test below.
        let bytes = minimal_flac_silence();
        let cursor = std::io::Cursor::new(bytes);
        let data = StaticSoundData::from_cursor(cursor).expect("fixture must decode");
        assert_eq!(data.sample_rate, 8000);
    }

    #[test]
    fn audio_source_loads_async_and_becomes_available() {
        use bevy_asset::{AssetApp, AssetServer, Assets};
        use bsengine_app::new_app;

        // Synthesized fixture (see `minimal_flac_silence`'s doc comment for
        // why), written to a real temp file — matches the
        // `texture_asset_loads_async_and_becomes_available` precedent in
        // `bsengine-asset`'s texture loader test, which also generates its
        // fixture at test time rather than relying on a checked-in binary.
        let path = std::env::temp_dir().join("bsengine_test_audio.flac");
        std::fs::write(&path, minimal_flac_silence()).unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.init_asset::<AudioSource>();
        app.register_asset_loader(AudioSourceLoader);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<AudioSource>(path.to_str().unwrap().to_owned())
        };

        let mut loaded = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<Assets<AudioSource>>()
                .get(&handle)
                .is_some()
            {
                loaded = true;
                break;
            }
        }
        assert!(
            loaded,
            "audio asset did not finish loading within 200 frames"
        );
    }
}
