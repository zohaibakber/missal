use std::sync::OnceLock;

use missal_text::{
    FaceId, FontFamily, URDU_FAMILY, fonts::UI_FAMILY_ALIASES, fonts::URDU_FAMILY_ALIASES,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaceMetrics {
    pub units_per_em: f32,
    pub ascent: f32,
    pub descent: f32,
    pub line_gap: f32,
    pub underline_offset: f32,
    pub underline_size: f32,
    pub strikeout_offset: f32,
    pub space_advance: f32,
}

impl FaceMetrics {
    pub fn ascent_px(&self, size: f32) -> f32 {
        (self.ascent * size / self.units_per_em).round()
    }

    pub fn descent_px(&self, size: f32) -> f32 {
        (self.descent * size / self.units_per_em).round()
    }

    pub fn normal_line_height(&self, size: f32) -> f32 {
        self.ascent_px(size)
            + self.descent_px(size)
            + (self.line_gap * size / self.units_per_em).round()
    }

    pub fn scaled(&self, value: f32, size: f32) -> f32 {
        value * size / self.units_per_em
    }
}

fn read_u16(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes([*data.get(at)?, *data.get(at + 1)?]))
}

fn read_i16(data: &[u8], at: usize) -> Option<i16> {
    read_u16(data, at).map(|v| v as i16)
}

fn read_u32(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes([
        *data.get(at)?,
        *data.get(at + 1)?,
        *data.get(at + 2)?,
        *data.get(at + 3)?,
    ]))
}

pub(crate) fn table<'a>(data: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = read_u16(data, 4)? as usize;
    for i in 0..count {
        let record = 12 + i * 16;
        if data.get(record..record + 4)? == tag {
            let offset = read_u32(data, record + 8)? as usize;
            let length = read_u32(data, record + 12)? as usize;
            return data.get(offset..offset + length);
        }
    }
    None
}

fn measure(data: &[u8]) -> FaceMetrics {
    let head = table(data, b"head").unwrap_or_default();
    let hhea = table(data, b"hhea").unwrap_or_default();
    let units_per_em = read_u16(head, 18).unwrap_or(1000).max(16) as f32;
    let ascent = read_i16(hhea, 4).unwrap_or(800) as f32;
    let descent = -(read_i16(hhea, 6).unwrap_or(-200) as f32);
    let line_gap = read_i16(hhea, 8).unwrap_or(0).max(0) as f32;
    let font = swash::FontRef::from_index(data, 0);
    let (underline_offset, underline_size, strikeout_offset, space_advance) = match font {
        Some(font) => {
            let metrics = font.metrics(&[]);
            let scale = units_per_em / metrics.units_per_em.max(1) as f32;
            let space = font.charmap().map(' ');
            let advance = font.glyph_metrics(&[]).advance_width(space) * scale;
            (
                metrics.underline_offset * scale,
                metrics.stroke_size * scale,
                metrics.strikeout_offset * scale,
                advance,
            )
        }
        None => (
            -units_per_em * 0.1,
            units_per_em * 0.05,
            units_per_em * 0.3,
            units_per_em * 0.25,
        ),
    };
    FaceMetrics {
        units_per_em,
        ascent,
        descent,
        line_gap,
        underline_offset,
        underline_size: if underline_size > 0.0 {
            underline_size
        } else {
            units_per_em / 20.0
        },
        strikeout_offset,
        space_advance: if space_advance > 0.0 {
            space_advance
        } else {
            units_per_em * 0.25
        },
    }
}

pub fn metrics(face: FaceId) -> &'static FaceMetrics {
    static METRICS: OnceLock<Vec<FaceMetrics>> = OnceLock::new();
    let all = METRICS.get_or_init(|| {
        missal_text::fonts::bundled_faces()
            .iter()
            .map(|face| measure(face.data))
            .collect()
    });
    &all[(face.0 as usize).min(all.len() - 1)]
}

pub fn is_urdu_name(name: &str) -> bool {
    URDU_FAMILY_ALIASES
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(name))
}

pub fn is_ui_name(name: &str) -> bool {
    UI_FAMILY_ALIASES
        .iter()
        .any(|alias| alias.eq_ignore_ascii_case(name))
}

pub fn family_for(name: &str) -> FontFamily {
    let lower = name.to_ascii_lowercase();
    if is_urdu_name(name) || matches!(lower.as_str(), "serif" | "cursive" | "fantasy") {
        FontFamily::Document
    } else if is_ui_name(name)
        || matches!(lower.as_str(), "sans-serif" | "system-ui" | "ui-sans-serif")
    {
        FontFamily::Interface
    } else {
        FontFamily::named(name)
    }
}

pub fn primary_face(family: &FontFamily, weight: u16, italic: bool) -> FaceId {
    let interface = match family {
        FontFamily::Interface => true,
        FontFamily::Document => false,
        FontFamily::Named(name) => is_ui_name(name) && !is_urdu_name(name),
    };
    if !interface {
        return FaceId(0);
    }
    let faces = missal_text::fonts::bundled_faces();
    let mut best = 1usize;
    let mut best_score = u32::MAX;
    for (index, face) in faces.iter().enumerate() {
        if face.family == URDU_FAMILY {
            continue;
        }
        let score = (face.weight as i32 - weight as i32).unsigned_abs()
            + if face.italic == italic { 0 } else { 10_000 };
        if score < best_score {
            best = index;
            best_score = score;
        }
    }
    FaceId(best as u16)
}
