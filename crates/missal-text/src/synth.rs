use std::{collections::HashMap, sync::Arc};

use gpui_kit::{
    Bounds, Corners, RenderImage, Window, get_gamma_correction_ratios, point, px, size,
};
use image::{Frame, RgbaImage};
use swash::{
    FontRef,
    scale::{Render, ScaleContext, Source, StrikeWith, image::Content},
    zeno::{Angle, Format, Transform, Vector},
};

use crate::{
    fonts::{FaceId, face},
    layout::{Rgba, Synthesis},
};

const SUBPIXEL_STEPS: f32 = 4.0;
const GAMMA: f32 = 1.8;
const ENHANCED_CONTRAST: f32 = 1.0;
const EMBOLDEN_DIVISOR: f32 = 48.0;
const RASTER_BUDGET: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key {
    face: FaceId,
    glyph: u32,
    size: u32,
    subpixel: u8,
    embolden: bool,
    skew: u32,
    color: u32,
}

struct Raster {
    image: Arc<RenderImage>,
    left: i32,
    top: i32,
    width: u32,
    height: u32,
}

#[derive(Default)]
pub(crate) struct SynthCache {
    context: ScaleContext,
    rasters: HashMap<Key, Option<Raster>>,
    bytes: usize,
}

pub(crate) struct SynthGlyph {
    pub face: FaceId,
    pub glyph: u32,
    pub font_size: f32,
    pub synthesis: Synthesis,
    pub color: Rgba,
}

impl SynthCache {
    pub(crate) fn paint(
        &mut self,
        glyph: &SynthGlyph,
        x: f32,
        y: f32,
        scale_factor: f32,
        window: &mut Window,
    ) {
        let device_x = x * scale_factor;
        let device_y = (y * scale_factor).round();
        let whole_x = device_x.floor();
        let subpixel = (((device_x - whole_x) * SUBPIXEL_STEPS).round() as u8).min(3);
        let key = Key {
            face: glyph.face,
            glyph: glyph.glyph,
            size: (glyph.font_size * scale_factor).to_bits(),
            subpixel,
            embolden: glyph.synthesis.embolden,
            skew: glyph.synthesis.skew.to_bits(),
            color: glyph.color.0,
        };
        if !self.rasters.contains_key(&key) {
            let raster = rasterize(&mut self.context, glyph, scale_factor, subpixel);
            let bytes = raster
                .as_ref()
                .map_or(0, |r| r.width as usize * r.height as usize * 4);
            if self.bytes + bytes > RASTER_BUDGET {
                for raster in std::mem::take(&mut self.rasters).into_values().flatten() {
                    window.drop_image(raster.image).ok();
                }
                self.bytes = 0;
            }
            self.bytes += bytes;
            self.rasters.insert(key, raster);
        }
        let raster = &self.rasters[&key];
        let Some(raster) = raster else {
            return;
        };
        let origin = point(
            px((whole_x + raster.left as f32) / scale_factor),
            px((device_y - raster.top as f32) / scale_factor),
        );
        let bounds = Bounds::new(
            origin,
            size(
                px(raster.width as f32 / scale_factor),
                px(raster.height as f32 / scale_factor),
            ),
        );
        window
            .paint_image(
                bounds,
                bounds,
                Corners::default(),
                raster.image.clone(),
                0,
                false,
            )
            .ok();
    }
}

fn rasterize(
    context: &mut ScaleContext,
    glyph: &SynthGlyph,
    scale_factor: f32,
    subpixel: u8,
) -> Option<Raster> {
    let font = FontRef::from_index(face(glyph.face).data, 0)?;
    let ppem = glyph.font_size * scale_factor;
    let mut scaler = context.builder(font).size(ppem).hint(true).build();
    let mut render = Render::new(&[Source::Bitmap(StrikeWith::ExactSize), Source::Outline]);
    render
        .format(Format::Alpha)
        .offset(Vector::new(subpixel as f32 / SUBPIXEL_STEPS, 0.0));
    if glyph.synthesis.embolden {
        render.embolden(ppem / EMBOLDEN_DIVISOR);
    }
    if glyph.synthesis.skew != 0.0 {
        render.transform(Some(Transform::skew(
            Angle::from_degrees(glyph.synthesis.skew),
            Angle::ZERO,
        )));
    }
    let image = render.render(&mut scaler, u16::try_from(glyph.glyph).ok()?)?;
    let placement = image.placement;
    if placement.width == 0 || placement.height == 0 || image.content != Content::Mask {
        return None;
    }
    let [r, g, b, a] = glyph.color.to_f32();
    let ratios = get_gamma_correction_ratios(GAMMA);
    let brightness = 0.30 * r + 0.59 * g + 0.11 * b;
    let contrast = ENHANCED_CONTRAST * (4.0 * (0.75 - brightness)).clamp(0.0, 1.0);
    let channel = |v: f32| (v * 255.0).round() as u8;
    let mut pixels = Vec::with_capacity(image.data.len() * 4);
    for &coverage in &image.data {
        let alpha = coverage as f32 / 255.0;
        let alpha = alpha * (contrast + 1.0) / (alpha * contrast + 1.0);
        let adjustment = ratios[0] * brightness + ratios[1];
        let correction = adjustment * alpha + (ratios[2] * brightness + ratios[3]);
        let alpha = (alpha + alpha * (1.0 - alpha) * correction).clamp(0.0, 1.0);
        pixels.extend_from_slice(&[channel(b), channel(g), channel(r), channel(alpha * a)]);
    }
    let buffer = RgbaImage::from_raw(placement.width, placement.height, pixels)?;
    Some(Raster {
        image: Arc::new(RenderImage::new(vec![Frame::new(buffer)])),
        left: placement.left,
        top: placement.top,
        width: placement.width,
        height: placement.height,
    })
}
