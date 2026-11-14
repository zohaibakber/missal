use std::{
    borrow::Cow,
    hint::black_box,
    sync::{Arc, OnceLock},
};

use gpui_kit::{
    App, Font, FontFeatures, FontId, FontStyle, FontWeight, Global, Hsla, TextSystem as GpuiText,
    px,
};

use crate::{
    fonts::{Face, FaceId, UI_FAMILY, bundled_faces, face},
    layout::{AttributedText, ParagraphStyle, Rgba, TextEngine, TextLayout},
    synth::SynthCache,
};

pub struct TextSystem {
    engine: TextEngine,
    gpui: Arc<GpuiText>,
    font_ids: Vec<OnceLock<FontId>>,
    pub(crate) synth: SynthCache,
}

impl Global for TextSystem {}

pub fn init(cx: &mut App) {
    if cx.has_global::<TextSystem>() {
        return;
    }
    let faces = bundled_faces();
    let gpui = cx.text_system().clone();
    register(
        &gpui,
        faces
            .iter()
            .filter(|f| is_eager(f))
            .map(|f| f.data)
            .collect(),
    );
    let font_ids = faces
        .iter()
        .map(|face| {
            let slot = OnceLock::new();
            if is_eager(face) {
                slot.set(resolve(&gpui, face)).ok();
            }
            slot
        })
        .collect();
    let deferred: Vec<&'static [u8]> = faces
        .iter()
        .filter(|f| !is_eager(f))
        .map(|f| f.data)
        .collect();
    std::thread::Builder::new()
        .name("missal-font-prefetch".into())
        .spawn(move || {
            for data in deferred {
                black_box(data.iter().step_by(4096).fold(0u8, |a, &b| a ^ b));
            }
        })
        .ok();
    cx.set_global(TextSystem {
        engine: TextEngine::new(),
        gpui,
        font_ids,
        synth: SynthCache::default(),
    });
    crate::input::bind_keys(cx);
}

fn is_eager(face: &Face) -> bool {
    face.family == UI_FAMILY
}

fn register(gpui: &GpuiText, data: Vec<&'static [u8]>) {
    if let Err(error) = gpui.add_fonts(data.into_iter().map(Cow::Borrowed).collect()) {
        eprintln!("missal-text: registering bundled fonts with GPUI failed: {error:#}");
    }
}

fn resolve(gpui: &GpuiText, face: &Face) -> FontId {
    gpui.resolve_font(&gpui_font(face.family, face.weight, face.italic))
}

pub(crate) fn gpui_font(family: &'static str, weight: u16, italic: bool) -> Font {
    Font {
        family: family.into(),
        features: FontFeatures::default(),
        fallbacks: None,
        weight: FontWeight(weight as f32),
        style: if italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        },
    }
}

impl TextSystem {
    pub fn global(cx: &App) -> &Self {
        cx.global::<Self>()
    }

    pub fn global_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<Self>()
    }

    pub fn engine(&mut self) -> &mut TextEngine {
        &mut self.engine
    }

    pub fn font_id(&self, id: FaceId) -> FontId {
        *self.font_ids[id.0 as usize].get_or_init(|| {
            let face = face(id);
            register(&self.gpui, vec![face.data]);
            resolve(&self.gpui, face)
        })
    }

    pub fn layout(
        cx: &mut App,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        max_width: Option<f32>,
    ) -> Arc<TextLayout> {
        cx.global_mut::<Self>()
            .engine
            .layout(text, paragraph, max_width)
    }

    pub fn check_font_mapping(cx: &App) -> Result<(), String> {
        let system = cx.global::<Self>();
        let mut problems = Vec::new();
        for (index, face) in bundled_faces().iter().enumerate() {
            let font_id = system.font_id(FaceId(index as u16));
            let font = swash::FontRef::from_index(face.data, 0)
                .ok_or_else(|| format!("{} is not a font", face.file_name))?;
            let metrics = font.metrics(&[]);
            let glyph_metrics = font.glyph_metrics(&[]);
            let charmap = font.charmap();
            for ch in ['a', 'W', 'g', 'ب', 'ک'] {
                let glyph = charmap.map(ch);
                if glyph == 0 {
                    continue;
                }
                let expected =
                    glyph_metrics.advance_width(glyph) * 100.0 / metrics.units_per_em as f32;
                let actual = cx
                    .text_system()
                    .advance(font_id, px(100.0), ch)
                    .map(|size| f32::from(size.width))
                    .unwrap_or(f32::NAN);
                if (expected - actual).abs() > 0.05 {
                    problems.push(format!(
                        "{} ({} {}): '{ch}' advance {actual} in GPUI, {expected} in the bundled file",
                        face.file_name, face.family, face.weight
                    ));
                }
            }
        }
        if problems.is_empty() {
            Ok(())
        } else {
            Err(problems.join("\n"))
        }
    }
}

pub fn to_hsla(color: Rgba) -> Hsla {
    let [r, g, b, a] = color.to_f32();
    gpui_kit::Rgba { r, g, b, a }.into()
}

pub fn to_rgba(color: Hsla) -> Rgba {
    let c: gpui_kit::Rgba = color.to_rgb();
    Rgba::from_f32(c.r, c.g, c.b, c.a)
}
