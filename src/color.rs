pub(crate) fn rgb(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

pub(crate) const SWATCHES: [(u8, u8, u8); 16] = [
    (0xFF, 0x33, 0x33),
    (0xFF, 0x8C, 0x00),
    (0xFF, 0xD7, 0x00),
    (0x00, 0xCC, 0x66),
    (0x00, 0x88, 0xFF),
    (0x66, 0x33, 0xFF),
    (0xCC, 0x33, 0xCC),
    (0x00, 0xBC, 0xD4),
    (0xF1, 0xC4, 0x0F),
    (0x2E, 0xCC, 0x71),
    (0xE6, 0x7E, 0x22),
    (0x34, 0x98, 0xDB),
    (0xE7, 0x4C, 0x3C),
    (0x9B, 0x59, 0xB6),
    (0x00, 0x00, 0x00),
    (0xFF, 0xFF, 0xFF),
];

pub(crate) fn gdi_color(r: u8, g: u8, b: u8) -> u32 {
    (0xFFu32 << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub(crate) fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let hi = (h * 6.0).floor() as i32 % 6;
    let f = h * 6.0 - (h * 6.0).floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match hi {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

pub(crate) fn parse_hex(s: &[u16]) -> Option<(u8, u8, u8)> {
    fn hx(c: u16) -> Option<u8> {
        match c {
            0x30..=0x39 => Some((c - 0x30) as u8),
            0x41..=0x46 => Some((c - 0x41 + 10) as u8),
            0x61..=0x66 => Some((c - 0x61 + 10) as u8),
            _ => None,
        }
    }
    if s.len() < 6 {
        return None;
    }
    Some((
        hx(s[0])? << 4 | hx(s[1])?,
        hx(s[2])? << 4 | hx(s[3])?,
        hx(s[4])? << 4 | hx(s[5])?,
    ))
}

pub(crate) fn scale_by(v: i32, dpi: i32) -> i32 {
    (v * dpi + 48) / 96
}
