//! Barvy: stabilní barva pro libovolný klíč (aplikace, dokument) a barvy značek.

/// Barvy nabízené u značek.
pub const TAG_COLORS: [&str; 7] =
    ["#8a9a0a", "#7a6bc4", "#8c1c3f", "#1a8f9a", "#c47a1a", "#3a7bd5", "#b03a8c"];

fn hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for unit in s.encode_utf16() {
        h ^= unit as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> [u8; 3] {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    [r, g, b].map(|v| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Stabilní, dobře rozprostřená barva pro libovolný klíč.
pub fn color_for(key: &str) -> [u8; 3] {
    let h = hash(key);
    let hue = (h % 360) as f64;
    let sat = (55 + ((h >> 9) % 35)) as f64 / 100.0;
    let light = (40 + ((h >> 17) % 20)) as f64 / 100.0;
    hsl_to_rgb(hue, sat, light)
}

/// `#rrggbb` -> RGB; neplatná hodnota dá šedou.
pub fn parse_hex(s: &str) -> [u8; 3] {
    let s = s.trim().trim_start_matches('#');
    if s.len() == 6 {
        if let Ok(v) = u32::from_str_radix(s, 16) {
            return [(v >> 16) as u8, (v >> 8) as u8, v as u8];
        }
    }
    [0x88, 0x88, 0x88]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_is_stable_and_differs_by_key() {
        assert_eq!(color_for("app:1"), color_for("app:1"));
        assert_ne!(color_for("app:1"), color_for("app:2"));
    }

    #[test]
    fn hsl_primaries() {
        assert_eq!(hsl_to_rgb(0.0, 1.0, 0.5), [255, 0, 0]);
        assert_eq!(hsl_to_rgb(120.0, 1.0, 0.5), [0, 255, 0]);
        assert_eq!(hsl_to_rgb(240.0, 1.0, 0.5), [0, 0, 255]);
    }

    #[test]
    fn hex_parsing() {
        assert_eq!(parse_hex("#8a9a0a"), [0x8a, 0x9a, 0x0a]);
        assert_eq!(parse_hex("zzz"), [0x88, 0x88, 0x88]);
    }
}
