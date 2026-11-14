use std::ops::Range;

use missal_text::{FaceId, Rgba, Synthesis};

use crate::fonts;
use crate::geom::Rect;
use crate::paginate::{Piece, TablePiece, TextPiece};
use crate::style::BorderSide;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RectKind {
    Background,
    Border,
    Rule,
    Underline,
    Strikethrough,
    Overline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FieldMark {
    pub unresolved: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub face: FaceId,
    pub size: f32,
    pub synthesis: Synthesis,
    pub color: Rgba,
    pub glyphs: Range<u32>,
    pub line: u32,
    pub field: Option<FieldMark>,
    pub bounds: Rect,
}

#[derive(Clone, Debug)]
pub enum DisplayItem {
    Rect {
        rect: Rect,
        color: Rgba,
        kind: RectKind,
    },
    Glyphs(GlyphRun),
    FieldHighlight {
        rect: Rect,
        unresolved: bool,
    },
}

impl DisplayItem {
    pub fn bounds(&self) -> Rect {
        match self {
            DisplayItem::Rect { rect, .. } | DisplayItem::FieldHighlight { rect, .. } => *rect,
            DisplayItem::Glyphs(run) => run.bounds,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub text: String,
    pub rtl: bool,
    pub bounds: Rect,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayList {
    pub items: Vec<DisplayItem>,
    pub glyphs: Vec<Glyph>,
    pub lines: Vec<TextLine>,
}

impl DisplayList {
    pub fn glyphs(&self, run: &GlyphRun) -> &[Glyph] {
        &self.glyphs[run.glyphs.start as usize..run.glyphs.end as usize]
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs.len()
    }
}

struct Layers {
    backgrounds: Vec<DisplayItem>,
    highlights: Vec<DisplayItem>,
    content: Vec<DisplayItem>,
    decorations: Vec<DisplayItem>,
    borders: Vec<DisplayItem>,
    glyphs: Vec<Glyph>,
    lines: Vec<TextLine>,
}

fn is_control(c: char) -> bool {
    matches!(c, '\u{2066}'..='\u{2069}' | '\u{202A}'..='\u{202E}' | '\u{200E}' | '\u{200F}')
}

fn border_rects(
    rect: Rect,
    sides: &[BorderSide; 4],
    color: Rgba,
    top: bool,
    bottom: bool,
    out: &mut Vec<DisplayItem>,
) {
    let w = |s: &BorderSide| s.used_width();
    let push = |out: &mut Vec<DisplayItem>, r: Rect, side: &BorderSide| {
        if r.width > 0.0 && r.height > 0.0 {
            out.push(DisplayItem::Rect {
                rect: r,
                color: side.color.unwrap_or(color),
                kind: RectKind::Border,
            });
        }
    };
    if top && w(&sides[0]) > 0.0 {
        push(
            out,
            Rect::new(rect.x, rect.y, rect.width, w(&sides[0])),
            &sides[0],
        );
    }
    if bottom && w(&sides[2]) > 0.0 {
        push(
            out,
            Rect::new(
                rect.x,
                rect.bottom() - w(&sides[2]),
                rect.width,
                w(&sides[2]),
            ),
            &sides[2],
        );
    }
    if w(&sides[1]) > 0.0 {
        push(
            out,
            Rect::new(
                rect.right() - w(&sides[1]),
                rect.y,
                w(&sides[1]),
                rect.height,
            ),
            &sides[1],
        );
    }
    if w(&sides[3]) > 0.0 {
        push(
            out,
            Rect::new(rect.x, rect.y, w(&sides[3]), rect.height),
            &sides[3],
        );
    }
}

fn line_rects_by_line(piece: &TextPiece, range: Range<usize>) -> Vec<(usize, f32, f32)> {
    let node = &piece.node;
    let layout = &node.layout;
    if layout.lines().is_empty() || range.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for rect in layout.selection_rects(range) {
        let line = layout.line_at_y(rect.y + rect.height * 0.5);
        if line < piece.lines.start || line >= piece.lines.end || line >= node.lines.len() {
            continue;
        }
        let shift = node.layout_dx();
        let x0 = piece.dx + node.content.x + rect.x + shift;
        out.push((line, x0, x0 + rect.width));
    }
    out
}

fn text_piece(piece: &TextPiece, layers: &mut Layers) {
    let node = &piece.node;
    let dx = piece.dx;
    let dy = piece.dy;
    let box_rect = Rect::from_edges(
        dx + node.border_box.x,
        dy + piece.box_top(),
        dx + node.border_box.right(),
        dy + piece.box_bottom(),
    );
    if let Some(color) = node.background {
        layers.backgrounds.push(DisplayItem::Rect {
            rect: box_rect,
            color,
            kind: RectKind::Background,
        });
    }
    border_rects(
        box_rect,
        &node.borders,
        node.color,
        piece.first,
        piece.last,
        &mut layers.borders,
    );
    for span in &node.backgrounds {
        for (line, x0, x1) in line_rects_by_line(piece, span.range.clone()) {
            let lb = node.lines[line];
            layers.backgrounds.push(DisplayItem::Rect {
                rect: Rect::from_edges(
                    x0,
                    dy + lb.baseline - span.ascent,
                    x1,
                    dy + lb.baseline + span.descent,
                ),
                color: span.color,
                kind: RectKind::Background,
            });
        }
    }
    let mut field_rects: Vec<(usize, f32, f32, bool)> = Vec::new();
    for field in &node.fields {
        for (line, x0, x1) in line_rects_by_line(piece, field.layout.clone()) {
            let lb = node.lines[line];
            field_rects.push((line, x0, x1, field.unresolved));
            layers.highlights.push(DisplayItem::FieldHighlight {
                rect: Rect::from_edges(
                    x0,
                    dy + lb.baseline - field.ascent,
                    x1,
                    dy + lb.baseline + field.descent,
                ),
                unresolved: field.unresolved,
            });
        }
    }
    let layout = &node.layout;
    let text = layout.text();
    let line_base = layers.lines.len() as u32;
    for line in piece.lines.clone() {
        let Some(info) = layout.lines().get(line) else {
            continue;
        };
        let lb = node.lines[line];
        let content: String = text
            .get(info.text_range.clone())
            .unwrap_or("")
            .chars()
            .filter(|c| !is_control(*c) && *c != '\n')
            .collect();
        layers.lines.push(TextLine {
            text: content,
            rtl: layout.is_rtl(),
            bounds: Rect::from_edges(
                dx + node.content.x,
                dy + lb.top,
                dx + node.content.right(),
                dy + lb.bottom,
            ),
        });
    }
    let runs = layout.glyph_runs();
    let first_run = runs.partition_point(|r| r.line < piece.lines.start);
    for run in &runs[first_run..] {
        if run.line >= piece.lines.end {
            break;
        }
        let Some(lb) = node.lines.get(run.line) else {
            continue;
        };
        let shift = node.layout_dx();
        let gy = dy + lb.glyph_dy;
        let line_id = line_base + (run.line - piece.lines.start) as u32;
        let size = run.font_size;
        let metrics = fonts::metrics(run.face);
        let ascent = metrics.ascent * size / metrics.units_per_em;
        let descent = metrics.descent * size / metrics.units_per_em;
        let mut current: Option<(Option<FieldMark>, u32, f32, f32)> = None;
        let flush = |layers: &mut Layers,
                     current: &mut Option<(Option<FieldMark>, u32, f32, f32)>| {
            if let Some((field, start, x0, x1)) = current.take() {
                let end = layers.glyphs.len() as u32;
                if end > start {
                    layers.content.push(DisplayItem::Glyphs(GlyphRun {
                        face: run.face,
                        size,
                        synthesis: run.synthesis,
                        color: run.color,
                        glyphs: start..end,
                        line: line_id,
                        field,
                        bounds: Rect::from_edges(
                            x0,
                            gy + lb.baseline - ascent - size * 0.3,
                            x1,
                            gy + lb.baseline + descent + size * 0.3,
                        ),
                    }));
                }
            }
        };
        for g in layout.glyphs(run) {
            let x = dx + node.content.x + g.x + shift;
            let y = gy + g.y;
            let center = x + g.advance * 0.5;
            let field = field_rects
                .iter()
                .find(|(l, x0, x1, _)| *l == run.line && center >= *x0 && center <= *x1)
                .map(|(_, _, _, unresolved)| FieldMark {
                    unresolved: *unresolved,
                });
            match &mut current {
                Some((f, _, x0, x1)) if *f == field => {
                    *x0 = x0.min(x);
                    *x1 = x1.max(x + g.advance);
                }
                _ => {
                    flush(layers, &mut current);
                    current = Some((field, layers.glyphs.len() as u32, x, x + g.advance));
                }
            }
            layers.glyphs.push(crate::display::Glyph { id: g.id, x, y });
        }
        flush(layers, &mut current);
    }
    for deco in &node.decorations {
        let metrics = fonts::metrics(deco.face);
        let thickness = metrics.scaled(metrics.underline_size, deco.size).max(1.0);
        for (line, x0, x1) in line_rects_by_line(piece, deco.range.clone()) {
            let lb = node.lines[line];
            let baseline = dy + lb.baseline;
            let mut push = |y: f32, kind: RectKind| {
                layers.decorations.push(DisplayItem::Rect {
                    rect: Rect::new(x0, y, x1 - x0, thickness),
                    color: deco.color,
                    kind,
                });
            };
            if deco.decoration.underline {
                push(
                    baseline - metrics.scaled(metrics.underline_offset, deco.size),
                    RectKind::Underline,
                );
            }
            if deco.decoration.strike {
                push(
                    baseline
                        - metrics.scaled(metrics.strikeout_offset, deco.size)
                        - thickness / 2.0,
                    RectKind::Strikethrough,
                );
            }
            if deco.decoration.overline {
                push(baseline - metrics.ascent_px(deco.size), RectKind::Overline);
            }
        }
    }
    if piece.first
        && let Some(marker) = &node.marker
        && let (Some(first), Some(mline)) = (node.lines.first(), marker.layout.lines().first())
    {
        let line_id = layers.lines.len() as u32;
        let marker_text: String = marker.layout.text().trim().to_owned();
        layers.lines.push(TextLine {
            text: marker_text,
            rtl: marker.layout.is_rtl(),
            bounds: Rect::new(
                dx + marker.x,
                dy + first.top,
                marker.layout.width(),
                first.bottom - first.top,
            ),
        });
        for run in marker.layout.glyph_runs() {
            let start = layers.glyphs.len() as u32;
            for g in marker.layout.glyphs(run) {
                layers.glyphs.push(crate::display::Glyph {
                    id: g.id,
                    x: dx + marker.x + g.x,
                    y: dy + first.baseline + (g.y - mline.baseline),
                });
            }
            let end = layers.glyphs.len() as u32;
            layers.content.push(DisplayItem::Glyphs(GlyphRun {
                face: run.face,
                size: run.font_size,
                synthesis: run.synthesis,
                color: run.color,
                glyphs: start..end,
                line: line_id,
                field: None,
                bounds: Rect::new(
                    dx + marker.x,
                    dy + first.top,
                    marker.layout.width().max(1.0),
                    first.bottom - first.top,
                ),
            }));
        }
    }
}

fn table_piece(piece: &TablePiece, layers: &mut Layers) {
    let table = &piece.node;
    for cell in &piece.cells {
        if let Some(color) = table.cells[cell.cell].background {
            layers.backgrounds.push(DisplayItem::Rect {
                rect: cell.rect,
                color,
                kind: RectKind::Background,
            });
        }
    }
    let ncols = table.ncols;
    let x = piece.dx;
    let color = |side: &BorderSide| side.color.unwrap_or(Rgba::BLACK);
    let mut lines: Vec<(usize, f32)> = Vec::new();
    for row in &piece.rows {
        if row.starts
            && !lines
                .iter()
                .any(|(l, y)| *l == row.row && (y - row.y).abs() < 0.5)
        {
            lines.push((row.row, row.y));
        }
        if row.ends {
            lines.push((row.row + 1, row.y + row.height));
        }
        for k in 0..=ncols {
            let side = table.v_edge(row.row, k);
            let w = side.used_width();
            if w <= 0.0 {
                continue;
            }
            layers.borders.push(DisplayItem::Rect {
                rect: Rect::new(x + table.cols[k] - w / 2.0, row.y, w, row.height),
                color: color(&side),
                kind: RectKind::Border,
            });
        }
    }
    let nrows = table.rows.len();
    for (line, y) in lines {
        if line > nrows || nrows == 0 {
            continue;
        }
        let edge_row = line.min(nrows - 1);
        for p in 0..ncols {
            let side = table.h_edge(line, p);
            let w = side.used_width();
            if w <= 0.0 {
                continue;
            }
            let left_w = table.v_edge(edge_row, p).used_width();
            let right_w = table.v_edge(edge_row, p + 1).used_width();
            let x0 = x + table.cols[p] - left_w / 2.0;
            let x1 = x + table.cols[p + 1] + right_w / 2.0;
            layers.borders.push(DisplayItem::Rect {
                rect: Rect::from_edges(x0, y - w / 2.0, x1, y + w / 2.0),
                color: color(&side),
                kind: RectKind::Border,
            });
        }
    }
}

pub(crate) fn build(pieces: &[Piece]) -> DisplayList {
    let mut layers = Layers {
        backgrounds: Vec::new(),
        highlights: Vec::new(),
        content: Vec::new(),
        decorations: Vec::new(),
        borders: Vec::new(),
        glyphs: Vec::new(),
        lines: Vec::new(),
    };
    for piece in pieces {
        match piece {
            Piece::Text(t) => text_piece(t, &mut layers),
            Piece::Table(t) => table_piece(t, &mut layers),
            Piece::Rule { rect, .. } => {
                layers.borders.push(DisplayItem::Rect {
                    rect: Rect::new(rect.x, rect.y, rect.width, 1.0),
                    color: Rgba::rgb(0x9a9a9a),
                    kind: RectKind::Rule,
                });
                layers.borders.push(DisplayItem::Rect {
                    rect: Rect::new(rect.x, rect.y + 1.0, rect.width, 1.0),
                    color: Rgba::rgb(0xeeeeee),
                    kind: RectKind::Rule,
                });
            }
        }
    }
    let mut items = layers.backgrounds;
    items.extend(layers.highlights);
    items.extend(layers.content);
    items.extend(layers.decorations);
    items.extend(layers.borders);
    DisplayList {
        items,
        glyphs: layers.glyphs,
        lines: layers.lines,
    }
}
