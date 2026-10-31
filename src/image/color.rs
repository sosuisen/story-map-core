//! The colors of the map, computed in the same way as the CSS of the HTML output

use std::fmt;

// The base colors of level 1 to 4. Level 4 and below all use the last one
const LEVEL_COLORS: [Rgb; 4] = [
    Rgb(0xe0, 0xef, 0xff),
    Rgb(0xe0, 0xff, 0xee),
    Rgb(0xff, 0xe0, 0xe9),
    Rgb(0xff, 0xf3, 0xe0),
];
// The card background is the base color with a lower lightness, and the card border is darker still
pub(super) const CARD_SHADE: f32 = 0.93;
pub(super) const BORDER_SHADE: f32 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Rgb(u8, u8, u8);

impl fmt::Display for Rgb {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// The base color of a level, starting at 0
pub(super) fn level_color(level: usize) -> Rgb {
    LEVEL_COLORS[level.min(LEVEL_COLORS.len() - 1)]
}

/// The color with its HSL lightness multiplied by the factor, like `hsl(from color h s calc(l * factor))` in CSS
pub(super) fn shaded(color: Rgb, factor: f32) -> Rgb {
    let (hue, saturation, lightness) = to_hsl(color);
    from_hsl(hue, saturation, (lightness * factor).clamp(0.0, 1.0))
}

fn to_hsl(Rgb(r, g, b): Rgb) -> (f32, f32, f32) {
    let [r, g, b] = [r, g, b].map(|channel| f32::from(channel) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } * 60.0;
    (hue, saturation, lightness)
}

fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Rgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - ((hue / 60.0).rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match hue {
        h if h < 60.0 => (chroma, x, 0.0),
        h if h < 120.0 => (x, chroma, 0.0),
        h if h < 180.0 => (0.0, chroma, x),
        h if h < 240.0 => (0.0, x, chroma),
        h if h < 300.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    let [r, g, b] = [r, g, b].map(|channel| ((channel + m) * 255.0).round() as u8);
    Rgb(r, g, b)
}
