//! IES light profiles (IESNA LM-63): a fixture's measured light, imported as
//! a light cookie.
//!
//! An `.ies` file is a table of luminous intensity (candela) over the
//! directions round a fixture. Unreal binds it beside the light as its own
//! texture; HDRP imports it *as a cookie*. This follows HDRP: it needs no
//! texture binding of its own, and the lighting shaders are at WebGPU's 16
//! sampled textures already (`bsengine_rhi_wgpu::cookie`). The profile becomes
//! an equirectangular panorama, the mapping point lights' cookies already
//! use, so a point light takes it as it takes any cookie; a spot light reads
//! an `.ies` cookie by direction instead of by projection
//! ([`is_profile_path`]).
//!
//! # Orientation
//!
//! The profile's nadir (vertical angle 0, straight down out of a ceiling
//! fixture) is the panorama's bottom row: a point light's local -Y, so an
//! unturned light hangs its profile the right way up, and a spot light's
//! direction. Horizontal angle C = 0 is the panorama's centre: a point
//! light's local -Z, a spot light's local +Y. C increases counter-clockwise
//! seen from above, as LM-63 measures it, so C = 90 is local -X.
//!
//! # Brightness
//!
//! Normalised to the profile's peak: the brightest direction is white, and
//! the light's own intensity sets how bright that is. Unreal's default (its
//! "Use IES Brightness" is off) and HDRP's both do this, so a profile shapes
//! a light without making a scene's lighting depend on a fixture's lumens.
//!
//! Only type C photometry is read -- the type architectural and most game
//! profiles use. Types A and B (vehicle lamps and floodlights) are refused
//! with an error rather than drawn wrong.

/// Whether a cookie path names an IES profile, which a spot light samples by
/// direction rather than projecting: an `.ies` extension, in any case.
pub fn is_profile_path(path: &str) -> bool {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ies"))
}

/// The side of the panorama an `.ies` file becomes: as wide as the cookie
/// atlas's layers, half as tall, so a texel spans the same angle both ways.
pub const PANORAMA_WIDTH: u32 = 256;
/// See [`PANORAMA_WIDTH`].
pub const PANORAMA_HEIGHT: u32 = 128;

/// A parsed type C profile: candela over vertical angles (0 = nadir, 180 =
/// zenith) and horizontal angles (C planes), both in degrees and ascending.
#[derive(Debug, Clone, PartialEq)]
pub struct IesProfile {
    /// The vertical angles, ascending.
    pub vertical: Vec<f32>,
    /// The horizontal angles (C planes), ascending.
    pub horizontal: Vec<f32>,
    /// Candela, one row of `vertical.len()` values per horizontal angle.
    pub candela: Vec<f32>,
}

impl IesProfile {
    /// Reads an LM-63 file (1986, 1991, 1995 or 2002: they differ only in
    /// the header lines before `TILT=`, which are skipped).
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        let tilt = lines
            .by_ref()
            .find(|line| line.trim_start().starts_with("TILT="))
            .ok_or("no TILT= line: not an IES file")?;
        // Everything after `TILT=` is numbers, split by whitespace or commas
        // and wrapped anywhere: writers break the lists at different widths.
        let mut numbers = lines.flat_map(|line| {
            line.split(|c: char| c.is_whitespace() || c == ',')
                .filter(|t| !t.is_empty())
        });
        let mut next = |what: &str| -> Result<f32, String> {
            let token = numbers
                .next()
                .ok_or(format!("the file ends before its {what}"))?;
            token
                .parse::<f32>()
                .map_err(|_| format!("{what}: {token:?} is not a number"))
        };
        // `TILT=INCLUDE` puts the tilt table inline, between the TILT line and
        // the data: the lamp's orientation, then a count, its angles and its
        // factors. It describes how output changes as the lamp is tilted,
        // which a fixed light does not do, so it is read past and not used.
        if tilt.trim().eq_ignore_ascii_case("TILT=INCLUDE") {
            next("lamp-to-luminaire geometry")?;
            let pairs = next("tilt angle count")? as usize;
            for _ in 0..pairs * 2 {
                next("tilt table")?;
            }
        }
        let _lamps = next("lamp count")?;
        let _lumens = next("lumens per lamp")?;
        let multiplier = next("candela multiplier")?;
        let vertical_count = next("vertical angle count")? as usize;
        let horizontal_count = next("horizontal angle count")? as usize;
        let photometric_type = next("photometric type")?;
        for what in [
            "units",
            "width",
            "length",
            "height",
            "ballast factor",
            "future use",
            "input watts",
        ] {
            next(what)?;
        }
        if photometric_type != 1.0 {
            return Err(format!(
                "photometric type {photometric_type}: only type C (1) profiles are supported"
            ));
        }
        if vertical_count == 0 || horizontal_count == 0 {
            return Err("a profile needs at least one vertical and one horizontal angle".into());
        }
        let mut read = |count: usize, what: &str| {
            (0..count)
                .map(|_| next(what))
                .collect::<Result<Vec<_>, _>>()
        };
        let vertical = read(vertical_count, "vertical angles")?;
        let horizontal = read(horizontal_count, "horizontal angles")?;
        let candela = read(vertical_count * horizontal_count, "candela values")?
            .into_iter()
            .map(|c| c * multiplier)
            .collect();
        let ascending = |angles: &[f32]| angles.windows(2).all(|w| w[0] < w[1]);
        if !ascending(&vertical) || !ascending(&horizontal) {
            return Err("the angles must ascend".into());
        }
        Ok(Self {
            vertical,
            horizontal,
            candela,
        })
    }

    /// The brightest value in the table.
    pub fn peak(&self) -> f32 {
        self.candela.iter().copied().fold(0.0, f32::max)
    }

    /// Candela toward vertical angle `gamma` and horizontal angle `c`
    /// (degrees), interpolated between the measured angles, with the
    /// profile's symmetry unfolded.
    ///
    /// A profile measures only as much as its symmetry needs, which its last
    /// horizontal angle says: 0 for the same in every direction, 90 for one
    /// quadrant mirrored into the rest, 180 for a mirror about the 0-180
    /// plane, 360 for none. Vertical angles outside the measured range --
    /// above 90 in a downlight that stops there -- get no light.
    pub fn candela_at(&self, gamma: f32, c: f32) -> f32 {
        let (v0, v1) = (self.vertical[0], *self.vertical.last().unwrap());
        if gamma < v0 - 1e-3 || gamma > v1 + 1e-3 {
            return 0.0;
        }
        let c = c.rem_euclid(360.0);
        let (h0, h1) = (self.horizontal[0], *self.horizontal.last().unwrap());
        let c = if self.horizontal.len() == 1 {
            h0
        } else if h0 == 0.0 && h1 == 90.0 {
            let c = if c > 180.0 { 360.0 - c } else { c };
            if c > 90.0 {
                180.0 - c
            } else {
                c
            }
        } else if h0 == 0.0 && h1 == 180.0 {
            if c > 180.0 {
                360.0 - c
            } else {
                c
            }
        } else if h0 == 90.0 && h1 == 270.0 {
            // Mirrored about the 90-270 plane: C and 180 - C are alike.
            if c < 90.0 {
                180.0 - c
            } else if c > 270.0 {
                540.0 - c
            } else {
                c
            }
        } else {
            c
        };
        let rows = self.vertical.len();
        let at = |h: usize, g: f32| {
            let row = &self.candela[h * rows..(h + 1) * rows];
            let (i, t) = bracket(&self.vertical, g);
            if i + 1 < rows {
                row[i] * (1.0 - t) + row[i + 1] * t
            } else {
                row[i]
            }
        };
        let n = self.horizontal.len();
        // A full 0-360 profile that stops short of 360 wraps from its last
        // plane back round to its first.
        if c > h1 && n > 1 {
            let t = (c - h1) / (h0 + 360.0 - h1);
            return at(n - 1, gamma) * (1.0 - t) + at(0, gamma) * t;
        }
        let (j, t) = bracket(&self.horizontal, c);
        if j + 1 < n {
            at(j, gamma) * (1.0 - t) + at(j + 1, gamma) * t
        } else {
            at(j, gamma)
        }
    }

    /// The profile as an equirectangular RGBA panorama, `width` by `height`,
    /// normalised to its peak (see the module docs for the orientation).
    /// `srgb` encodes the values as the texture will be decoded: an sRGB
    /// texture's bytes are not its light.
    pub fn panorama(&self, width: u32, height: u32, srgb: bool) -> Vec<u8> {
        let peak = self.peak().max(1e-6);
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            // Polar angle from straight up; the nadir is gamma = 0.
            let theta = (y as f32 + 0.5) / height as f32 * 180.0;
            let gamma = 180.0 - theta;
            for x in 0..width {
                // `point_cookie`'s u: 0.5 + atan2(x, -z) / 2pi, so the angle
                // from local -Z toward +X. C runs the other way round.
                let phi = ((x as f32 + 0.5) / width as f32 - 0.5) * 360.0;
                let value = (self.candela_at(gamma, -phi) / peak).clamp(0.0, 1.0);
                let encoded = if srgb { linear_to_srgb(value) } else { value };
                let byte = (encoded * 255.0).round() as u8;
                out.extend_from_slice(&[byte, byte, byte, 255]);
            }
        }
        out
    }
}

/// Where `x` falls among the ascending `angles`: the index below it and how
/// far toward the next, clamped to the ends.
fn bracket(angles: &[f32], x: f32) -> (usize, f32) {
    if x <= angles[0] {
        return (0, 0.0);
    }
    let last = angles.len() - 1;
    if x >= angles[last] {
        return (last, 0.0);
    }
    let i = angles.partition_point(|&a| a <= x) - 1;
    (i, (x - angles[i]) / (angles[i + 1] - angles[i]))
}

fn linear_to_srgb(v: f32) -> f32 {
    if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A downlight: 1000 cd straight down, falling to 0 at 90 degrees,
    /// the same all round (one horizontal angle).
    const DOWNLIGHT: &str = "IESNA:LM-63-2002
[TEST] downlight
[MANUFAC] none
TILT=NONE
1 1000 1 4 1 1 2 0 0 0
1 1 50
0 30 60 90
0
1000 800 300 0
";

    #[test]
    fn a_downlights_table_is_read_and_interpolated() {
        let p = IesProfile::parse(DOWNLIGHT).unwrap();
        assert_eq!(p.vertical, vec![0.0, 30.0, 60.0, 90.0]);
        assert_eq!(p.horizontal, vec![0.0]);
        assert_eq!(p.candela_at(0.0, 0.0), 1000.0);
        assert_eq!(
            p.candela_at(15.0, 123.0),
            900.0,
            "halfway between 1000 and 800, any C"
        );
        assert_eq!(
            p.candela_at(120.0, 0.0),
            0.0,
            "above the measured range: no light"
        );
    }

    /// The multiplier scales every value, numbers wrap across lines and
    /// commas separate as well as spaces, and an inline tilt table is read
    /// past.
    #[test]
    fn the_layout_variations_writers_use_are_read() {
        let text = "IESNA91
TILT=INCLUDE
1
2
0, 90
1, 0.5
1 1000 2.0
3 1 1 2 0 0 0 1 1
50 0 45
90 0 100
50 25
";
        let p = IesProfile::parse(text).unwrap();
        assert_eq!(p.vertical, vec![0.0, 45.0, 90.0]);
        assert_eq!(p.candela, vec![200.0, 100.0, 50.0]);
    }

    #[test]
    fn types_other_than_c_are_refused() {
        let text = DOWNLIGHT.replace("1 1000 1 4 1 1 2", "1 1000 1 4 1 2 2");
        assert!(IesProfile::parse(&text).unwrap_err().contains("type C"));
        assert!(IesProfile::parse("no tilt here").is_err());
    }

    /// A profile measured over a quarter (0-90) is mirrored into the other
    /// three; one measured over 0-180 is mirrored about that plane.
    #[test]
    fn symmetric_profiles_are_unfolded() {
        // Two vertical angles, horizontal 0 and 90: bright at C=0, dim at 90.
        let quad = IesProfile {
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0, 90.0],
            candela: vec![100.0, 100.0, 10.0, 10.0],
        };
        for (c, want) in [
            (0.0, 100.0),
            (90.0, 10.0),
            (180.0, 100.0),
            (270.0, 10.0),
            (135.0, 55.0),
        ] {
            assert_eq!(quad.candela_at(0.0, c), want, "quadrant symmetry at C={c}");
        }
        let half = IesProfile {
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0, 90.0, 180.0],
            candela: vec![100.0, 100.0, 50.0, 50.0, 0.0, 0.0],
        };
        assert_eq!(half.candela_at(0.0, 270.0), 50.0, "mirror of 90");
        assert_eq!(half.candela_at(0.0, 315.0), 75.0, "mirror of 45");
        // A full profile stopping at 270 wraps back to 0 across the gap.
        let full = IesProfile {
            vertical: vec![0.0, 90.0],
            horizontal: vec![0.0, 90.0, 180.0, 270.0],
            candela: vec![100.0, 100.0, 0.0, 0.0, 0.0, 0.0, 20.0, 20.0],
        };
        assert_eq!(
            full.candela_at(0.0, 315.0),
            60.0,
            "halfway from 270 back to 360"
        );
    }

    /// The panorama: the nadir at the bottom row, the zenith at the top, the
    /// peak white, C = 0 at the centre and C = 90 at a quarter (local -X).
    #[test]
    fn the_panorama_hangs_the_profile_the_right_way_up_and_round() {
        let p = IesProfile::parse(DOWNLIGHT).unwrap();
        let (w, h) = (16, 8);
        let px = p.panorama(w, h, false);
        let at = |x: u32, y: u32| px[((y * w + x) * 4) as usize];
        // The bottom row's centre is 11.25 degrees off the nadir: 925 cd.
        assert_eq!(at(8, h - 1), 236, "straight down is the brightest");
        assert_eq!(at(8, 0), 0, "straight up is dark");
        assert_eq!(at(8, h / 2 - 1), 0, "just above the horizon is dark");
        // Bright only along C = 0..90 (a quadrant), so its side is visible.
        let side = IesProfile {
            vertical: vec![0.0, 90.0, 180.0],
            horizontal: vec![0.0, 90.0, 180.0, 270.0],
            candela: vec![
                0.0, 0.0, 0.0, // C = 0
                100.0, 100.0, 100.0, // C = 90
                0.0, 0.0, 0.0, // C = 180
                0.0, 0.0, 0.0, // C = 270
            ],
        };
        let px = side.panorama(w, h, false);
        let at = |x: u32| px[(((h / 2) * w + x) * 4) as usize];
        assert!(
            at(w / 4) > 200,
            "C = 90 is at u = 0.25, local -X: {}",
            at(w / 4)
        );
        assert!(
            at(3 * w / 4) < 20,
            "and not at u = 0.75, +X: {}",
            at(3 * w / 4)
        );
    }

    #[test]
    fn srgb_encoding_brightens_the_mid_tones() {
        // The upper hemisphere at half the peak: the top row of a two-row
        // panorama (gamma = 135) reads it.
        let p = IesProfile {
            vertical: vec![0.0, 90.0, 180.0],
            horizontal: vec![0.0],
            candela: vec![100.0, 50.0, 50.0],
        };
        assert_eq!(p.panorama(1, 2, false)[0], 128, "half the peak, linear");
        assert_eq!(
            p.panorama(1, 2, true)[0],
            188,
            "half the peak, sRGB-encoded"
        );
    }

    #[test]
    fn profile_paths_are_known_by_extension() {
        assert!(is_profile_path("assets/lights/cone.ies"));
        assert!(is_profile_path("assets/lights/CONE.IES"));
        assert!(!is_profile_path("assets/lights/cone.png"));
        assert!(!is_profile_path(""));
    }
}
