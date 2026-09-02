//! Deterministic, accessible Office theme palette derivation in OKLCH.
//!
//! The conversion matrices are the 2021-01-25 linear-sRGB/Oklab matrices
//! published at <https://bottosson.github.io/posts/oklab/>. Contrast uses the
//! WCAG definition at <https://www.w3.org/TR/WCAG22/#dfn-contrast-ratio>.

use serde::Serialize;
use std::fmt;

const BODY_TEXT_CONTRAST: f64 = 4.5;
const LARGE_TEXT_CONTRAST: f64 = 3.0;
const CONTRAST_MARGIN: f64 = 0.02;
const GAMUT_SEARCH_STEPS: usize = 32;
const LIGHTNESS_SEARCH_STEPS: usize = 32;

/// An eight-bit, non-linear sRGB color.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Srgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Srgb {
    pub const BLACK: Self = Self::new(0, 0, 0);
    pub const WHITE: Self = Self::new(255, 255, 255);

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses `RRGGBB` or `#RRGGBB`, case-insensitively.
    pub fn from_hex(value: &str) -> Result<Self, PaletteError> {
        let digits = value.strip_prefix('#').unwrap_or(value);
        if digits.len() != 6 || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(PaletteError::InvalidHex(value.to_string()));
        }
        let channel = |start| {
            u8::from_str_radix(&digits[start..start + 2], 16)
                .map_err(|_| PaletteError::InvalidHex(value.to_string()))
        };
        Ok(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }

    /// Emits uppercase Office-compatible `RRGGBB` bytes without a leading `#`.
    pub fn to_hex(self) -> String {
        format!("{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }

    pub fn to_oklch(self) -> Oklch {
        let [r, g, b] = self.linear_channels();
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let l_root = l.cbrt();
        let m_root = m.cbrt();
        let s_root = s.cbrt();
        let lightness =
            0.210_454_255_3 * l_root + 0.793_617_785 * m_root - 0.004_072_046_8 * s_root;
        let a = 1.977_998_495_1 * l_root - 2.428_592_205 * m_root + 0.450_593_709_9 * s_root;
        let b = 0.025_904_037_1 * l_root + 0.782_771_766_2 * m_root - 0.808_675_766 * s_root;
        let chroma = a.hypot(b);
        let hue = if chroma <= 1e-12 {
            0.0
        } else {
            b.atan2(a).to_degrees().rem_euclid(360.0)
        };
        Oklch::new(lightness, chroma, hue)
    }

    /// WCAG relative luminance in the range `0.0..=1.0`.
    pub fn relative_luminance(self) -> f64 {
        let [r, g, b] = self.linear_channels();
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    pub fn contrast_ratio(self, other: Self) -> f64 {
        let a = self.relative_luminance();
        let b = other.relative_luminance();
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    fn linear_channels(self) -> [f64; 3] {
        [self.r, self.g, self.b].map(|channel| srgb_to_linear(f64::from(channel) / 255.0))
    }
}

impl Serialize for Srgb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

/// OKLCH with lightness/chroma in unit coordinates and hue in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Oklch {
    pub l: f64,
    pub c: f64,
    pub h: f64,
}

impl Oklch {
    pub fn new(l: f64, c: f64, h: f64) -> Self {
        Self {
            l: l.clamp(0.0, 1.0),
            c: c.max(0.0),
            h: h.rem_euclid(360.0),
        }
    }

    /// Converts to sRGB, reducing chroma at fixed lightness and hue when needed.
    pub fn to_srgb(self) -> Srgb {
        let color = self.gamut_mapped();
        let linear = color.linear_srgb();
        Srgb::new(
            encode_channel(linear[0]),
            encode_channel(linear[1]),
            encode_channel(linear[2]),
        )
    }

    pub fn is_in_srgb_gamut(self) -> bool {
        in_gamut(self.linear_srgb())
    }

    fn gamut_mapped(self) -> Self {
        if self.is_in_srgb_gamut() || self.c == 0.0 {
            return self;
        }
        let mut low = 0.0;
        let mut high = self.c;
        for _ in 0..GAMUT_SEARCH_STEPS {
            let mid = (low + high) / 2.0;
            if Self::new(self.l, mid, self.h).is_in_srgb_gamut() {
                low = mid;
            } else {
                high = mid;
            }
        }
        Self::new(self.l, low, self.h)
    }

    fn linear_srgb(self) -> [f64; 3] {
        let radians = self.h.to_radians();
        let a = self.c * radians.cos();
        let b = self.c * radians.sin();
        let l_root = self.l + 0.396_337_777_4 * a + 0.215_803_757_3 * b;
        let m_root = self.l - 0.105_561_345_8 * a - 0.063_854_172_8 * b;
        let s_root = self.l - 0.089_484_177_5 * a - 1.291_485_548 * b;
        let l = l_root.powi(3);
        let m = m_root.powi(3);
        let s = s_root.powi(3);
        [
            4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s,
            -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s,
            -0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701 * s,
        ]
    }
}

/// The twelve colors required by an OOXML `a:clrScheme`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemePalette {
    pub dk1: Srgb,
    pub lt1: Srgb,
    pub dk2: Srgb,
    pub lt2: Srgb,
    pub accent1: Srgb,
    pub accent2: Srgb,
    pub accent3: Srgb,
    pub accent4: Srgb,
    pub accent5: Srgb,
    pub accent6: Srgb,
    pub hlink: Srgb,
    #[serde(rename = "folHlink")]
    pub fol_hlink: Srgb,
}

impl ThemePalette {
    pub fn derive(seed: &str) -> Result<Self, PaletteError> {
        Ok(Self::from_seed(Srgb::from_hex(seed)?))
    }

    pub fn from_seed(seed: Srgb) -> Self {
        let seed_lch = seed.to_oklch();
        let hue = if seed_lch.c < 0.02 { 250.0 } else { seed_lch.h };
        let chroma = seed_lch.c.clamp(0.10, 0.19);

        let lt1 = Srgb::WHITE;
        let lt2 = Oklch::new(0.955, (chroma * 0.13).min(0.025), hue).to_srgb();
        let dk1 = enforce_contrast(
            Oklch::new(0.17, (chroma * 0.16).min(0.025), hue),
            lt2,
            BODY_TEXT_CONTRAST,
        );
        let dk2 = enforce_contrast(
            Oklch::new(0.31, (chroma * 0.45).min(0.08), hue),
            lt2,
            BODY_TEXT_CONTRAST,
        );

        let rotations = [0.0, 52.0, 112.0, 172.0, 232.0, 292.0];
        let lightness_offsets = [0.0, 0.03, -0.015, 0.02, -0.025, 0.01];
        let chroma_scales = [1.0, 0.90, 0.82, 0.88, 0.78, 0.86];
        let base_lightness = seed_lch.l.clamp(0.48, 0.62);
        let mut accents = [Srgb::BLACK; 6];
        for index in 0..accents.len() {
            let candidate = if index == 0 {
                seed_lch
            } else {
                Oklch::new(
                    base_lightness + lightness_offsets[index],
                    chroma * chroma_scales[index],
                    hue + rotations[index],
                )
            };
            accents[index] = enforce_contrast(candidate, lt1, LARGE_TEXT_CONTRAST);
        }

        let hlink = enforce_contrast(
            Oklch::new(0.50, chroma.min(0.17), hue + 210.0),
            lt1,
            BODY_TEXT_CONTRAST,
        );
        let fol_hlink = enforce_contrast(
            Oklch::new(0.48, (chroma * 0.78).min(0.14), hue + 292.0),
            lt1,
            BODY_TEXT_CONTRAST,
        );

        Self {
            dk1,
            lt1,
            dk2,
            lt2,
            accent1: accents[0],
            accent2: accents[1],
            accent3: accents[2],
            accent4: accents[3],
            accent5: accents[4],
            accent6: accents[5],
            hlink,
            fol_hlink,
        }
    }

    pub fn color(&self, color: ThemeColor) -> Srgb {
        match color {
            ThemeColor::Dk1 => self.dk1,
            ThemeColor::Lt1 => self.lt1,
            ThemeColor::Dk2 => self.dk2,
            ThemeColor::Lt2 => self.lt2,
            ThemeColor::Accent1 => self.accent1,
            ThemeColor::Accent2 => self.accent2,
            ThemeColor::Accent3 => self.accent3,
            ThemeColor::Accent4 => self.accent4,
            ThemeColor::Accent5 => self.accent5,
            ThemeColor::Accent6 => self.accent6,
            ThemeColor::Hlink => self.hlink,
            ThemeColor::FolHlink => self.fol_hlink,
        }
    }

    pub fn contrast_for(&self, pairing: MasterTextPairing) -> f64 {
        self.color(pairing.foreground)
            .contrast_ratio(self.color(pairing.background))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThemeColor {
    Dk1,
    Lt1,
    Dk2,
    Lt2,
    Accent1,
    Accent2,
    Accent3,
    Accent4,
    Accent5,
    Accent6,
    Hlink,
    FolHlink,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasterTextPairing {
    pub foreground: ThemeColor,
    pub background: ThemeColor,
    pub text_class: MasterTextClass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MasterTextClass {
    Body,
    Large,
}

/// The only theme pairings that built-in masters may use for text.
///
/// Body text and hyperlinks require WCAG AA 4.5:1. Large display text on
/// dark/accent fills requires 3:1. Contrast is symmetric, so the large-text
/// rows also cover accent-colored display text on `lt1`.
pub const BUILT_IN_MASTER_TEXT_PAIRINGS: &[MasterTextPairing] = &[
    body(ThemeColor::Dk1, ThemeColor::Lt1),
    body(ThemeColor::Dk1, ThemeColor::Lt2),
    body(ThemeColor::Dk2, ThemeColor::Lt1),
    body(ThemeColor::Dk2, ThemeColor::Lt2),
    body(ThemeColor::Hlink, ThemeColor::Lt1),
    body(ThemeColor::FolHlink, ThemeColor::Lt1),
    large(ThemeColor::Lt1, ThemeColor::Dk1),
    large(ThemeColor::Lt1, ThemeColor::Dk2),
    large(ThemeColor::Lt1, ThemeColor::Accent1),
    large(ThemeColor::Lt1, ThemeColor::Accent2),
    large(ThemeColor::Lt1, ThemeColor::Accent3),
    large(ThemeColor::Lt1, ThemeColor::Accent4),
    large(ThemeColor::Lt1, ThemeColor::Accent5),
    large(ThemeColor::Lt1, ThemeColor::Accent6),
];

const fn body(foreground: ThemeColor, background: ThemeColor) -> MasterTextPairing {
    MasterTextPairing {
        foreground,
        background,
        text_class: MasterTextClass::Body,
    }
}

const fn large(foreground: ThemeColor, background: ThemeColor) -> MasterTextPairing {
    MasterTextPairing {
        foreground,
        background,
        text_class: MasterTextClass::Large,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PaletteError {
    InvalidHex(String),
}

impl fmt::Display for PaletteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidHex(value) => write!(
                formatter,
                "invalid sRGB seed '{value}'; expected RRGGBB or #RRGGBB"
            ),
        }
    }
}

impl std::error::Error for PaletteError {}

fn enforce_contrast(candidate: Oklch, background: Srgb, minimum: f64) -> Srgb {
    let target = minimum + CONTRAST_MARGIN;
    let initial = candidate.to_srgb();
    if initial.contrast_ratio(background) >= target {
        return initial;
    }

    let dark = Oklch::new(0.0, candidate.c, candidate.h);
    let light = Oklch::new(1.0, candidate.c, candidate.h);
    let endpoint = [dark, light]
        .into_iter()
        .filter(|color| color.to_srgb().contrast_ratio(background) >= target)
        .min_by(|left, right| {
            (left.l - candidate.l)
                .abs()
                .total_cmp(&(right.l - candidate.l).abs())
        })
        .expect("black or white must satisfy WCAG contrast against an sRGB color");

    let mut failing = candidate.l;
    let mut passing = endpoint.l;
    for _ in 0..LIGHTNESS_SEARCH_STEPS {
        let middle = (failing + passing) / 2.0;
        let probe = Oklch::new(middle, candidate.c, candidate.h).to_srgb();
        if probe.contrast_ratio(background) >= target {
            passing = middle;
        } else {
            failing = middle;
        }
    }
    Oklch::new(passing, candidate.c, candidate.h).to_srgb()
}

fn in_gamut(channels: [f64; 3]) -> bool {
    const EPSILON: f64 = 1e-9;
    channels
        .into_iter()
        .all(|channel| (-EPSILON..=1.0 + EPSILON).contains(&channel))
}

fn srgb_to_linear(channel: f64) -> f64 {
    if channel <= 0.04045 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(channel: f64) -> f64 {
    if channel <= 0.003_130_8 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

fn encode_channel(channel: f64) -> u8 {
    (linear_to_srgb(channel.clamp(0.0, 1.0)) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(actual: f64, expected: f64, tolerance: f64) {
        assert!(
            (actual - expected).abs() <= tolerance,
            "expected {expected:.8}, got {actual:.8}"
        );
    }

    #[test]
    fn srgb_primaries_match_reference_oklch_values() {
        for (hex, expected) in [
            ("FF0000", (0.627_955, 0.257_683, 29.233_885)),
            ("00FF00", (0.866_440, 0.294_827, 142.495_339)),
            ("0000FF", (0.452_014, 0.313_214, 264.052_021)),
        ] {
            let color = Srgb::from_hex(hex).expect("reference hex").to_oklch();
            assert_close(color.l, expected.0, 0.000_002);
            assert_close(color.c, expected.1, 0.000_002);
            assert_close(color.h, expected.2, 0.000_01);
        }
    }

    #[test]
    fn reference_colors_roundtrip_through_oklch() {
        for hex in ["000000", "FFFFFF", "FF0000", "00FF00", "0000FF", "1F4E79"] {
            let original = Srgb::from_hex(hex).expect("reference hex");
            assert_eq!(original.to_oklch().to_srgb(), original, "roundtrip {hex}");
        }
    }

    #[test]
    fn out_of_gamut_oklch_reduces_chroma_without_clipping_lightness() {
        let requested = Oklch::new(0.70, 0.40, 40.0);
        assert!(!requested.is_in_srgb_gamut());
        let mapped = requested.gamut_mapped();
        assert!(mapped.is_in_srgb_gamut());
        assert_close(mapped.l, requested.l, 1e-12);
        assert_close(mapped.h, requested.h, 1e-12);
        assert!(mapped.c < requested.c);
    }

    #[test]
    fn contrast_enforcement_moves_lightness_to_the_nearest_passing_color() {
        let requested = Oklch::new(0.82, 0.14, 110.0);
        let adjusted = enforce_contrast(requested, Srgb::WHITE, BODY_TEXT_CONTRAST);
        assert!(adjusted.contrast_ratio(Srgb::WHITE) >= BODY_TEXT_CONTRAST);
        assert!(adjusted.to_oklch().l < requested.l);
    }

    #[test]
    fn contrast_uses_wcag_relative_luminance() {
        assert_close(Srgb::BLACK.contrast_ratio(Srgb::WHITE), 21.0, 1e-12);
        let blue = Srgb::from_hex("0000FF").expect("blue");
        assert_close(blue.contrast_ratio(Srgb::WHITE), 8.592_471, 0.000_001);
    }

    #[test]
    fn invalid_hex_is_rejected_without_guessing() {
        for value in ["", "12345", "GG0000", "0x112233", "#1234567"] {
            assert!(matches!(
                Srgb::from_hex(value),
                Err(PaletteError::InvalidHex(invalid)) if invalid == value
            ));
        }
    }
}
