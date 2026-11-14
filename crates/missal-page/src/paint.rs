use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{
    App, Bounds, Corners, Global, GlyphId, Hsla, Pixels, Point as GPoint, RenderImage, Window,
    fill, get_gamma_correction_ratios, point, px, size,
};
use image::{Frame, RgbaImage};
use missal_text::{FaceId, Rgba, Synthesis, TextSystem, to_hsla};
use swash::scale::{Render, ScaleContext, Source, StrikeWith, image::Content};
use swash::zeno::{Angle, Format, Transform, Vector};

use crate::display::{DisplayItem, DisplayList, GlyphRun};
use crate::engine::Page;
use crate::geom::Rect;
use crate::image::ImageData;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PagePaint {
    pub zoom: f32,
    pub paper: Option<Hsla>,
    pub show_fields: bool,
    pub field_text: Option<Hsla>,
    pub field_background: Option<Hsla>,
    pub unresolved_underline: Option<Hsla>,
    pub field_radius: f32,
    pub empty_text: Option<Hsla>,
    pub empty_background: Option<Hsla>,
    pub empty_underline: Option<Hsla>,
    pub ink: Option<Hsla>,
}

impl PagePaint {
    pub fn print(zoom: f32) -> PagePaint {
        PagePaint {
            zoom,
            paper: Some(to_hsla(Rgba::rgb(0xffffff))),
            show_fields: false,
            field_text: None,
            field_background: None,
            unresolved_underline: None,
            field_radius: 0.0,
            empty_text: None,
            empty_background: None,
            empty_underline: None,
            ink: None,
        }
    }

    pub fn editor(zoom: f32) -> PagePaint {
        PagePaint {
            zoom,
            paper: Some(to_hsla(Rgba::rgb(0xffffff))),
            show_fields: true,
            field_text: Some(to_hsla(Rgba::rgb(0x1d4ed8))),
            field_background: Some(to_hsla(Rgba(0x1d4ed81a))),
            unresolved_underline: Some(to_hsla(Rgba(0x1d4ed88c))),
            field_radius: 0.0,
            empty_text: None,
            empty_background: None,
            empty_underline: None,
            ink: None,
        }
    }
}

#[derive(Default)]
struct PaintCache {
    context: ScaleContext,
    rasters: HashMap<RasterKey, Option<Raster>>,
    images: HashMap<usize, Arc<RenderImage>>,
}

impl Global for PaintCache {}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct RasterKey {
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

const SUBPIXEL_STEPS: f32 = 4.0;
const EMBOLDEN_DIVISOR: f32 = 48.0;

fn rasterize(
    context: &mut ScaleContext,
    key: &RasterKey,
    ppem: f32,
    synthesis: Synthesis,
    color: Rgba,
) -> Option<Raster> {
    let font = swash::FontRef::from_index(missal_text::face(key.face).data, 0)?;
    let mut scaler = context.builder(font).size(ppem).hint(true).build();
    let mut render = Render::new(&[Source::Bitmap(StrikeWith::ExactSize), Source::Outline]);
    render
        .format(Format::Alpha)
        .offset(Vector::new(key.subpixel as f32 / SUBPIXEL_STEPS, 0.0));
    if synthesis.embolden {
        render.embolden(ppem / EMBOLDEN_DIVISOR);
    }
    if synthesis.skew != 0.0 {
        render.transform(Some(Transform::skew(
            Angle::from_degrees(synthesis.skew),
            Angle::ZERO,
        )));
    }
    let image = render.render(&mut scaler, u16::try_from(key.glyph).ok()?)?;
    let placement = image.placement;
    if placement.width == 0 || placement.height == 0 || image.content != Content::Mask {
        return None;
    }
    let [r, g, b, a] = color.to_f32();
    let ratios = get_gamma_correction_ratios(1.8);
    let brightness = 0.30 * r + 0.59 * g + 0.11 * b;
    let contrast = (4.0 * (0.75 - brightness)).clamp(0.0, 1.0);
    let channel = |v: f32| (v * 255.0).round() as u8;
    let mut pixels = Vec::with_capacity(image.data.len() * 4);
    for &coverage in &image.data {
        let alpha = coverage as f32 / 255.0;
        let alpha = alpha * (contrast + 1.0) / (alpha * contrast + 1.0);
        let correction =
            (ratios[0] * brightness + ratios[1]) * alpha + (ratios[2] * brightness + ratios[3]);
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

fn render_image(data: &ImageData) -> Option<Arc<RenderImage>> {
    let decoded = data.decoded()?;
    let mut bgra = decoded.rgba.clone();
    for px in bgra.as_chunks_mut::<4>().0 {
        px.swap(0, 2);
    }
    let buffer = RgbaImage::from_raw(decoded.width, decoded.height, bgra)?;
    Some(Arc::new(RenderImage::new(vec![Frame::new(buffer)])))
}

fn to_bounds(origin: GPoint<Pixels>, rect: Rect, zoom: f32) -> Bounds<Pixels> {
    Bounds::new(
        point(origin.x + px(rect.x * zoom), origin.y + px(rect.y * zoom)),
        size(px(rect.width * zoom), px(rect.height * zoom)),
    )
}

pub fn page_bounds(page: &Page, origin: GPoint<Pixels>, zoom: f32) -> Bounds<Pixels> {
    to_bounds(
        origin,
        Rect::new(0.0, 0.0, page.size.width, page.size.height),
        zoom,
    )
}

pub fn paint_page(
    page: &Page,
    origin: GPoint<Pixels>,
    options: &PagePaint,
    window: &mut Window,
    cx: &mut App,
) {
    paint_display_list(
        page.display_list(),
        page.size.width,
        page.size.height,
        origin,
        options,
        window,
        cx,
    );
}

pub fn paint_display_list(
    list: &DisplayList,
    width: f32,
    height: f32,
    origin: GPoint<Pixels>,
    options: &PagePaint,
    window: &mut Window,
    cx: &mut App,
) {
    let zoom = options.zoom.max(0.01);
    if !cx.has_global::<PaintCache>() {
        cx.set_global(PaintCache::default());
    }
    if let Some(paper) = options.paper {
        window.paint_quad(fill(
            to_bounds(origin, Rect::new(0.0, 0.0, width, height), zoom),
            paper,
        ));
    }
    let mask = window.content_mask().bounds;
    let visible = Rect::new(
        f32::from(mask.origin.x - origin.x) / zoom,
        f32::from(mask.origin.y - origin.y) / zoom,
        f32::from(mask.size.width) / zoom,
        f32::from(mask.size.height) / zoom,
    );
    for item in &list.items {
        if !item.bounds().intersects(&visible) {
            continue;
        }
        match item {
            DisplayItem::Rect { rect, color, .. } => {
                let mut r = *rect;
                if r.height * zoom < 1.0 {
                    r.height = 1.0 / zoom;
                }
                if r.width * zoom < 1.0 {
                    r.width = 1.0 / zoom;
                }
                window.paint_quad(fill(to_bounds(origin, r, zoom), inked(options, *color)));
            }
            DisplayItem::FieldHighlight { rect, unresolved } => {
                if !options.show_fields {
                    continue;
                }
                let empty = *unresolved && options.empty_background.is_some();
                let background = if empty {
                    options.empty_background
                } else {
                    options.field_background
                };
                if let Some(bg) = background {
                    let radius = px(options.field_radius * zoom);
                    let corners = if empty {
                        Corners {
                            top_left: radius,
                            top_right: radius,
                            bottom_left: px(0.),
                            bottom_right: px(0.),
                        }
                    } else {
                        Corners::all(radius)
                    };
                    window
                        .paint_quad(fill(to_bounds(origin, *rect, zoom), bg).corner_radii(corners));
                }
                let underline = if empty {
                    options.empty_underline
                } else if *unresolved {
                    options.unresolved_underline
                } else {
                    None
                };
                if let Some(line) = underline {
                    let thickness = if empty { 1.5 } else { 1.0 };
                    let y = rect.bottom() - thickness;
                    let mut x = rect.x;
                    while x < rect.right() {
                        let w = 3.0f32.min(rect.right() - x);
                        window.paint_quad(fill(
                            to_bounds(origin, Rect::new(x, y, w, thickness), zoom),
                            line,
                        ));
                        x += if empty { 5.0 } else { 6.0 };
                    }
                }
            }
            DisplayItem::Image { rect, image } => {
                let key = Arc::as_ptr(&image.bytes) as *const u8 as usize;
                let cache = cx.global_mut::<PaintCache>();
                let render = match cache.images.get(&key) {
                    Some(r) => Some(r.clone()),
                    None => {
                        let r = render_image(image);
                        if let Some(r) = &r {
                            cache.images.insert(key, r.clone());
                        }
                        r
                    }
                };
                if let Some(render) = render {
                    let bounds = to_bounds(origin, *rect, zoom);
                    window
                        .paint_image(bounds, bounds, Corners::default(), render, 0, false)
                        .ok();
                }
            }
            DisplayItem::Glyphs(run) => paint_run(list, run, origin, options, zoom, window, cx),
        }
    }
}

fn inked(options: &PagePaint, color: Rgba) -> Hsla {
    let hsla = to_hsla(color);
    match options.ink {
        Some(ink) if hsla.l < 0.2 && hsla.s < 0.25 => ink.opacity(hsla.a),
        _ => hsla,
    }
}

fn paint_run(
    list: &DisplayList,
    run: &GlyphRun,
    origin: GPoint<Pixels>,
    options: &PagePaint,
    zoom: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let color = match (options.show_fields, run.field) {
        (true, Some(mark)) => if mark.unresolved {
            options.empty_text.or(options.field_text)
        } else {
            options.field_text
        }
        .unwrap_or_else(|| inked(options, run.color)),
        _ => inked(options, run.color),
    };
    let ox: f32 = origin.x.into();
    let oy: f32 = origin.y.into();
    let size = run.size * zoom;
    if run.synthesis.is_none() {
        let font_id = TextSystem::global(cx).font_id(run.face);
        for g in list.glyphs(run) {
            window
                .paint_glyph(
                    point(px(ox + g.x * zoom), px(oy + g.y * zoom)),
                    font_id,
                    GlyphId(g.id),
                    px(size),
                    color,
                )
                .ok();
        }
        return;
    }
    let sf = window.scale_factor();
    let rgba = missal_text::to_rgba(color);
    for g in list.glyphs(run) {
        let device_x = (ox + g.x * zoom) * sf;
        let device_y = ((oy + g.y * zoom) * sf).round();
        let whole_x = device_x.floor();
        let subpixel = (((device_x - whole_x) * SUBPIXEL_STEPS).round() as u8).min(3);
        let key = RasterKey {
            face: run.face,
            glyph: g.id,
            size: (size * sf).to_bits(),
            subpixel,
            embolden: run.synthesis.embolden,
            skew: run.synthesis.skew.to_bits(),
            color: rgba.0,
        };
        let cache = cx.global_mut::<PaintCache>();
        let PaintCache {
            context, rasters, ..
        } = cache;
        let raster = rasters
            .entry(key)
            .or_insert_with(|| rasterize(context, &key, size * sf, run.synthesis, rgba));
        let Some(raster) = raster else { continue };
        let bounds = Bounds::new(
            point(
                px((whole_x + raster.left as f32) / sf),
                px((device_y - raster.top as f32) / sf),
            ),
            size_px(raster.width as f32 / sf, raster.height as f32 / sf),
        );
        let image = raster.image.clone();
        window
            .paint_image(bounds, bounds, Corners::default(), image, 0, false)
            .ok();
    }
}

fn size_px(w: f32, h: f32) -> gpui_kit::Size<Pixels> {
    size(px(w), px(h))
}
