use std::ops::Range;

use gpui_kit::{App, Bounds, GlyphId, Hsla, Pixels, Point, Window, fill, point, px, size};

use crate::{
    layout::{Rect, TextLayout, TextPosition},
    synth::SynthGlyph,
    system::{TextSystem, to_hsla},
};

#[derive(Clone, Debug, Default)]
pub struct TextPaint {
    pub color: Option<Hsla>,
    pub selection: Option<(Range<usize>, Hsla)>,
    pub caret: Option<(TextPosition, Hsla)>,
    pub marked: Option<Range<usize>>,
}

pub fn snap(value: f32, scale_factor: f32) -> f32 {
    (value * scale_factor).round() / scale_factor
}

pub fn snap_origin(origin: Point<Pixels>, window: &Window) -> Point<Pixels> {
    let sf = window.scale_factor();
    point(px(snap(origin.x.into(), sf)), px(snap(origin.y.into(), sf)))
}

fn rect_bounds(origin: Point<Pixels>, rect: Rect) -> Bounds<Pixels> {
    Bounds::new(
        point(origin.x + px(rect.x), origin.y + px(rect.y)),
        size(px(rect.width), px(rect.height)),
    )
}

fn snapped_band(origin_y: f32, y: f32, height: f32, sf: f32) -> (f32, f32) {
    let top = ((origin_y + y) * sf).round();
    let thickness = (height * sf).round().max(1.0);
    (top / sf - origin_y, thickness / sf)
}

pub fn paint_text(
    layout: &TextLayout,
    origin: Point<Pixels>,
    paint: &TextPaint,
    window: &mut Window,
    cx: &mut App,
) {
    let origin = snap_origin(origin, window);
    let sf = window.scale_factor();
    let visible = window.content_mask().bounds;
    let visible_top = f32::from(visible.origin.y - origin.y);
    let visible_bottom = visible_top + f32::from(visible.size.height);
    let line_visible = |line: usize| {
        layout.lines().get(line).is_none_or(|l| {
            let slack = (l.bottom - l.top).max(l.ascent + l.descent);
            l.bottom + slack >= visible_top && l.top - slack <= visible_bottom
        })
    };

    for background in layout.backgrounds() {
        window.paint_quad(fill(
            rect_bounds(origin, background.rect),
            to_hsla(background.color),
        ));
    }

    if let Some((range, color)) = &paint.selection {
        for rect in layout.selection_rects(range.clone()) {
            window.paint_quad(fill(rect_bounds(origin, rect), *color));
        }
    }

    let ox: f32 = origin.x.into();
    let oy: f32 = origin.y.into();
    let system = cx.global_mut::<TextSystem>();
    for run in layout.glyph_runs() {
        if !line_visible(run.line) {
            continue;
        }
        let color = paint.color.unwrap_or_else(|| to_hsla(run.color));
        let glyphs = layout.glyphs(run);
        if run.synthesis.is_none() {
            let font_id = system.font_id(run.face);
            let font_size = px(run.font_size);
            for g in glyphs {
                window
                    .paint_glyph(
                        point(px(ox + g.x), px(oy + g.y)),
                        font_id,
                        GlyphId(g.id),
                        font_size,
                        color,
                    )
                    .ok();
            }
        } else {
            let rgba = crate::system::to_rgba(color);
            for g in glyphs {
                system.synth.paint(
                    &SynthGlyph {
                        face: run.face,
                        glyph: g.id,
                        font_size: run.font_size,
                        synthesis: run.synthesis,
                        color: rgba,
                    },
                    ox + g.x,
                    oy + g.y,
                    sf,
                    window,
                );
            }
        }
    }

    for decoration in layout.decorations() {
        let (y, height) = snapped_band(oy, decoration.rect.y, decoration.rect.height, sf);
        let rect = Rect::new(decoration.rect.x, y, decoration.rect.width, height);
        let color = paint.color.unwrap_or_else(|| to_hsla(decoration.color));
        window.paint_quad(fill(rect_bounds(origin, rect), color));
    }

    if let Some(marked) = &paint.marked {
        let color = paint
            .color
            .unwrap_or_else(|| to_hsla(layout.source().style.color));
        for rect in layout.selection_rects(marked.clone()) {
            let line = layout.lines()[layout.line_at_y(rect.y + rect.height * 0.5)].clone();
            let (y, height) = snapped_band(oy, line.baseline + line.descent * 0.35, 1.0, sf);
            window.paint_quad(fill(
                rect_bounds(origin, Rect::new(rect.x, y, rect.width, height)),
                color,
            ));
        }
    }

    if let Some((position, color)) = &paint.caret {
        paint_caret(layout, origin, *position, *color, window);
    }
}

pub fn paint_caret(
    layout: &TextLayout,
    origin: Point<Pixels>,
    position: TextPosition,
    color: Hsla,
    window: &mut Window,
) {
    let sf = window.scale_factor();
    let width = (sf.round().max(1.0)) / sf;
    let caret = layout.caret(position);
    let x = snap(f32::from(origin.x) + caret.x, sf) - if caret.rtl { width } else { 0.0 };
    let top = snap(f32::from(origin.y) + caret.top, sf);
    let bottom = snap(f32::from(origin.y) + caret.bottom, sf);
    window.paint_quad(fill(
        Bounds::new(point(px(x), px(top)), size(px(width), px(bottom - top))),
        color,
    ));
}

pub fn caret_bounds(
    layout: &TextLayout,
    origin: Point<Pixels>,
    position: TextPosition,
) -> Bounds<Pixels> {
    let caret = layout.caret(position);
    Bounds::new(
        point(origin.x + px(caret.x), origin.y + px(caret.top)),
        size(px(1.0), px(caret.bottom - caret.top)),
    )
}
