use std::{collections::HashMap, ops::Range, sync::Arc};

use parley::{Alignment, AlignmentOptions, PositionedInlineBox, PositionedLayoutItem};

use super::style::{Align, AttributedText, ParagraphStyle, Rgba};
use crate::fonts::FaceId;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn right(&self) -> f32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Affinity {
    Upstream,
    #[default]
    Downstream,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct TextPosition {
    pub index: usize,
    pub affinity: Affinity,
}

impl TextPosition {
    pub const fn new(index: usize, affinity: Affinity) -> Self {
        Self { index, affinity }
    }

    pub const fn downstream(index: usize) -> Self {
        Self::new(index, Affinity::Downstream)
    }

    pub const fn upstream(index: usize) -> Self {
        Self::new(index, Affinity::Upstream)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Synthesis {
    pub embolden: bool,
    pub skew: f32,
}

impl Synthesis {
    pub fn is_none(&self) -> bool {
        !self.embolden && self.skew == 0.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PositionedGlyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
    pub advance: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GlyphRun {
    pub face: FaceId,
    pub font_size: f32,
    pub synthesis: Synthesis,
    pub color: Rgba,
    pub line: usize,
    pub baseline: f32,
    pub rtl: bool,
    pub glyphs: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DecorationKind {
    Underline,
    Strikethrough,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decoration {
    pub kind: DecorationKind,
    pub rect: Rect,
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Background {
    pub rect: Rect,
    pub color: Rgba,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlacedObject {
    pub id: u64,
    pub index: usize,
    pub rect: Rect,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LineInfo {
    pub text_range: Range<usize>,
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    pub ascent: f32,
    pub descent: f32,
    pub left: f32,
    pub right: f32,
    pub start_x: f32,
    pub(crate) clusters: Range<usize>,
    pub(crate) nav: Range<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ClusterKind {
    Text,
    Space,
    Newline,
    Object(u64),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct LineDecoration {
    offset: f32,
    size: f32,
    color: Rgba,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct VisualCluster {
    pub(crate) range: Range<usize>,
    pub(crate) x0: f32,
    pub(crate) x1: f32,
    pub(crate) rtl: bool,
    pub(crate) kind: ClusterKind,
    pub(crate) line: u32,
    pub(crate) trailing: bool,
    underline: Option<LineDecoration>,
    strikethrough: Option<LineDecoration>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Edge {
    Leading,
    Trailing,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Geometry {
    pub(crate) lines: Vec<LineInfo>,
    pub(crate) clusters: Vec<VisualCluster>,
    pub(crate) logical: Vec<u32>,
    pub(crate) objects_by_index: Vec<(usize, u32)>,
    pub(crate) runs: Vec<GlyphRun>,
    pub(crate) glyphs: Vec<PositionedGlyph>,
    pub(crate) decorations: Vec<Decoration>,
    pub(crate) backgrounds: Vec<Background>,
    pub(crate) objects: Vec<PlacedObject>,
    pub(crate) width: f32,
    pub(crate) natural_width: f32,
    pub(crate) height: f32,
}

#[derive(Clone)]
pub struct TextLayout {
    pub(crate) source: AttributedText,
    pub(crate) paragraph: ParagraphStyle,
    pub(crate) rtl: bool,
    pub(crate) max_width: Option<f32>,
    pub(crate) inner: parley::Layout<Rgba>,
    pub(crate) faces: Arc<HashMap<u64, FaceId>>,
    pub(crate) geo: Geometry,
}

impl std::fmt::Debug for TextLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextLayout")
            .field("text", &self.source.text)
            .field("rtl", &self.rtl)
            .field("max_width", &self.max_width)
            .field("lines", &self.geo.lines.len())
            .field("width", &self.geo.width)
            .field("height", &self.geo.height)
            .finish()
    }
}

fn parley_alignment(align: Align) -> Alignment {
    match align {
        Align::Start => Alignment::Start,
        Align::End => Alignment::End,
        Align::Left => Alignment::Left,
        Align::Right => Alignment::Right,
        Align::Center => Alignment::Center,
        Align::Justify => Alignment::Justify,
    }
}

fn alignment_factor(align: Align, rtl: bool) -> f32 {
    match (align, rtl) {
        (Align::Left, _) | (Align::Start | Align::Justify, false) | (Align::End, true) => 0.0,
        (Align::Right, _) | (Align::Start | Align::Justify, true) | (Align::End, false) => 1.0,
        (Align::Center, _) => 0.5,
    }
}

impl TextLayout {
    pub(crate) fn from_parley(
        source: AttributedText,
        paragraph: ParagraphStyle,
        rtl: bool,
        inner: parley::Layout<Rgba>,
        faces: Arc<HashMap<u64, FaceId>>,
        max_width: Option<f32>,
    ) -> Self {
        let mut layout = Self {
            source,
            paragraph,
            rtl,
            max_width: None,
            inner,
            faces,
            geo: Geometry::default(),
        };
        layout.set_max_width(max_width);
        layout
    }

    pub fn set_max_width(&mut self, max_width: Option<f32>) {
        let max_width = max_width.filter(|w| w.is_finite()).map(|w| w.max(0.0));
        self.max_width = max_width;
        let options = AlignmentOptions::default();
        let alignment = parley_alignment(self.paragraph.align);
        let dx;
        if self.paragraph.wrap && max_width.is_some() {
            self.inner.break_all_lines(max_width);
            self.inner.align(alignment, options);
            dx = 0.0;
        } else {
            self.inner.break_all_lines(None);
            self.inner.align(alignment, options);
            dx = match max_width {
                Some(width) => {
                    alignment_factor(self.paragraph.align, self.rtl) * (width - self.inner.width())
                }
                None => 0.0,
            };
        }
        self.geo = self.extract(dx);
    }

    pub fn with_max_width(&self, max_width: Option<f32>) -> Self {
        let mut copy = self.clone();
        copy.set_max_width(max_width);
        copy
    }

    fn extract(&self, dx: f32) -> Geometry {
        let mut geo = Geometry::default();
        let text: &str = &self.source.text;
        let has_backgrounds = self.source.has_backgrounds();
        let object_index: HashMap<u64, usize> = self
            .source
            .objects
            .iter()
            .map(|o| (o.id, o.index))
            .collect();
        let mut object_y: HashMap<u64, (f32, f32)> = HashMap::new();
        for (line_index, line) in self.inner.lines().enumerate() {
            let metrics = *line.metrics();
            let start_x = metrics.offset + metrics.inline_min_coord + dx;
            let first = geo.clusters.len();
            let mut boxes: Vec<PositionedInlineBox> = Vec::new();
            for item in line.items() {
                if let PositionedLayoutItem::InlineBox(b) = item {
                    boxes.push(b);
                }
            }
            let mut boxes = boxes.into_iter();
            let mut runs = line.runs().peekable();
            let mut x = start_x;
            let mut item = 0usize;
            loop {
                if let Some(run) = runs.peek()
                    && run.index() == item
                {
                    let run = runs.next().expect("peeked");
                    let font_metrics = *run.font_metrics();
                    for cluster in run.visual_clusters() {
                        let advance = cluster.advance();
                        let range = cluster.text_range();
                        let kind = if cluster.is_hard_line_break() {
                            ClusterKind::Newline
                        } else if cluster.is_space_or_nbsp()
                            || text[range.clone()].chars().all(char::is_whitespace)
                        {
                            ClusterKind::Space
                        } else {
                            ClusterKind::Text
                        };
                        let style = cluster.style();
                        let underline = style.underline.as_ref().map(|d| {
                            let size = d.size.unwrap_or(font_metrics.underline_size).max(1.0);
                            LineDecoration {
                                offset: d.offset.unwrap_or(-(size * 0.5).ceil().max(1.0)),
                                size,
                                color: d.brush,
                            }
                        });
                        let strikethrough = style.strikethrough.as_ref().map(|d| LineDecoration {
                            offset: d.offset.unwrap_or(font_metrics.strikethrough_offset),
                            size: d.size.unwrap_or(font_metrics.strikethrough_size),
                            color: d.brush,
                        });
                        geo.clusters.push(VisualCluster {
                            range,
                            x0: x,
                            x1: x + advance,
                            rtl: cluster.is_rtl(),
                            kind,
                            line: line_index as u32,
                            trailing: false,
                            underline,
                            strikethrough,
                        });
                        x += advance;
                    }
                    item += 1;
                    continue;
                }
                if let Some(b) = boxes.next() {
                    let index = object_index.get(&b.id).copied().unwrap_or(0);
                    object_y.insert(b.id, (b.y, b.height));
                    geo.clusters.push(VisualCluster {
                        range: index..index,
                        x0: x,
                        x1: x + b.width,
                        rtl: self.rtl,
                        kind: ClusterKind::Object(b.id),
                        line: line_index as u32,
                        trailing: false,
                        underline: None,
                        strikethrough: None,
                    });
                    x += b.width;
                    item += 1;
                    continue;
                }
                if runs.peek().is_some() {
                    item += 1;
                    continue;
                }
                break;
            }
            let shifts = self.apply_trailing_whitespace_rule(&mut geo.clusters[first..], start_x);
            let end = geo.clusters.len();
            let line_clusters = &geo.clusters[first..end];
            let mut nav = first..end;
            if let Some(c) = line_clusters.first()
                && c.kind == ClusterKind::Newline
            {
                nav.start += 1;
            }
            if let Some(c) = line_clusters.last()
                && c.kind == ClusterKind::Newline
                && nav.end > nav.start
            {
                nav.end -= 1;
            }
            let mut left = f32::INFINITY;
            let mut right = f32::NEG_INFINITY;
            for c in line_clusters.iter().filter(|c| !c.trailing) {
                left = left.min(c.x0);
                right = right.max(c.x1);
            }
            if left > right {
                left = start_x;
                right = start_x;
            }
            let ascent = metrics.baseline - metrics.content_block_min_coord;
            let descent = metrics.content_block_max_coord - metrics.baseline;
            geo.lines.push(LineInfo {
                text_range: line.text_range(),
                top: metrics.block_min_coord,
                bottom: metrics.block_max_coord,
                baseline: metrics.baseline,
                ascent,
                descent,
                left,
                right,
                start_x,
                clusters: first..end,
                nav,
            });
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let Some(&face) = self.faces.get(&run.font().data.id()) else {
                    continue;
                };
                let synthesis = run.synthesis();
                let start = geo.glyphs.len();
                for g in glyph_run.positioned_glyphs() {
                    let mut gx = g.x + dx;
                    if !shifts.is_empty() {
                        gx += shift_for(&shifts, gx + g.advance * 0.5);
                    }
                    geo.glyphs.push(PositionedGlyph {
                        id: g.id,
                        x: gx,
                        y: g.y,
                        advance: g.advance,
                    });
                }
                geo.runs.push(GlyphRun {
                    face,
                    font_size: run.font_size(),
                    synthesis: Synthesis {
                        embolden: synthesis.embolden(),
                        skew: synthesis.skew().unwrap_or(0.0),
                    },
                    color: glyph_run.style().brush,
                    line: line_index,
                    baseline: glyph_run.baseline(),
                    rtl: run.is_rtl(),
                    glyphs: start..geo.glyphs.len(),
                });
            }
            let line_info = geo.lines.last().expect("pushed").clone();
            let line_clusters = &geo.clusters[first..end];
            for kind in [DecorationKind::Underline, DecorationKind::Strikethrough] {
                let mut current: Option<Decoration> = None;
                for c in line_clusters
                    .iter()
                    .filter(|c| !c.trailing && c.kind != ClusterKind::Newline)
                {
                    let deco = match kind {
                        DecorationKind::Underline => c.underline,
                        DecorationKind::Strikethrough => c.strikethrough,
                    };
                    let Some(deco) = deco else {
                        if let Some(d) = current.take() {
                            geo.decorations.push(d);
                        }
                        continue;
                    };
                    let rect = Rect::new(
                        c.x0,
                        line_info.baseline - deco.offset,
                        c.x1 - c.x0,
                        deco.size.max(0.5),
                    );
                    match &mut current {
                        Some(d)
                            if d.color == deco.color
                                && (d.rect.y - rect.y).abs() < 0.01
                                && (d.rect.height - rect.height).abs() < 0.01
                                && (d.rect.right() - rect.x).abs() < 0.01 =>
                        {
                            d.rect.width = rect.right() - d.rect.x;
                        }
                        _ => {
                            if let Some(d) = current.take() {
                                geo.decorations.push(d);
                            }
                            current = Some(Decoration {
                                kind,
                                rect,
                                color: deco.color,
                            });
                        }
                    }
                }
                if let Some(d) = current.take() {
                    geo.decorations.push(d);
                }
            }
            if has_backgrounds {
                let mut current: Option<Background> = None;
                for c in line_clusters.iter().filter(|c| {
                    !c.trailing && matches!(c.kind, ClusterKind::Text | ClusterKind::Space)
                }) {
                    let Some(color) = self.source.background_at(c.range.start) else {
                        if let Some(b) = current.take() {
                            geo.backgrounds.push(b);
                        }
                        continue;
                    };
                    let rect = Rect::new(
                        c.x0,
                        line_info.baseline - line_info.ascent,
                        c.x1 - c.x0,
                        line_info.ascent + line_info.descent,
                    );
                    match &mut current {
                        Some(b) if b.color == color && (b.rect.right() - rect.x).abs() < 0.01 => {
                            b.rect.width = rect.right() - b.rect.x;
                        }
                        _ => {
                            if let Some(b) = current.take() {
                                geo.backgrounds.push(b);
                            }
                            current = Some(Background { rect, color });
                        }
                    }
                }
                if let Some(b) = current.take() {
                    geo.backgrounds.push(b);
                }
            }
        }
        for (i, c) in geo.clusters.iter().enumerate() {
            if let ClusterKind::Object(id) = c.kind {
                let (y, height) = object_y.get(&id).copied().unwrap_or_default();
                geo.objects.push(PlacedObject {
                    id,
                    index: c.range.start,
                    rect: Rect::new(c.x0, y, c.x1 - c.x0, height),
                    line: c.line as usize,
                });
                geo.objects_by_index.push((c.range.start, i as u32));
            }
        }
        geo.objects_by_index.sort_by_key(|(index, _)| *index);
        let mut logical: Vec<u32> = geo
            .clusters
            .iter()
            .enumerate()
            .filter(|(_, c)| !matches!(c.kind, ClusterKind::Object(_)) && !c.range.is_empty())
            .map(|(i, _)| i as u32)
            .collect();
        logical.sort_by_key(|&i| geo.clusters[i as usize].range.start);
        geo.logical = logical;
        geo.natural_width = geo
            .lines
            .iter()
            .map(|l| l.right - l.left)
            .fold(0.0, f32::max);
        geo.height = self.inner.height();
        geo.width = self.max_width.unwrap_or(self.inner.width());
        geo
    }

    fn apply_trailing_whitespace_rule(
        &self,
        clusters: &mut [VisualCluster],
        start_x: f32,
    ) -> Vec<(f32, f32, f32)> {
        if clusters.is_empty() {
            return Vec::new();
        }
        let mut order: Vec<usize> = (0..clusters.len()).collect();
        order.sort_by_key(|&i| {
            let c = &clusters[i];
            (
                c.range.start,
                !matches!(c.kind, ClusterKind::Object(_)),
            )
        });
        let mut trailing = Vec::new();
        for &i in order.iter().rev() {
            match clusters[i].kind {
                ClusterKind::Space | ClusterKind::Newline => trailing.push(i),
                _ => break,
            }
        }
        if trailing.is_empty() {
            return Vec::new();
        }
        for &i in &trailing {
            clusters[i].trailing = true;
        }
        let others: Vec<usize> = (0..clusters.len())
            .filter(|i| !clusters[*i].trailing)
            .collect();
        let visual: Vec<usize> = if self.rtl {
            trailing.iter().copied().chain(others).collect()
        } else {
            others
                .into_iter()
                .chain(trailing.iter().rev().copied())
                .collect()
        };
        if visual.iter().copied().eq(0..clusters.len()) {
            return Vec::new();
        }
        let mut shifts = Vec::new();
        let mut reordered = Vec::with_capacity(clusters.len());
        let mut x = start_x;
        for &i in &visual {
            let mut c = clusters[i].clone();
            let width = c.x1 - c.x0;
            let delta = x - c.x0;
            if delta.abs() > 1e-4 && width > 1e-3 {
                shifts.push((c.x0, c.x1, delta));
            }
            c.x0 = x;
            c.x1 = x + width;
            x += width;
            reordered.push(c);
        }
        clusters.clone_from_slice(&reordered);
        shifts
    }

    pub fn text(&self) -> &str {
        &self.source.text
    }

    pub fn source(&self) -> &AttributedText {
        &self.source
    }

    pub fn paragraph(&self) -> &ParagraphStyle {
        &self.paragraph
    }

    pub fn is_rtl(&self) -> bool {
        self.rtl
    }

    pub fn max_width(&self) -> Option<f32> {
        self.max_width
    }

    pub fn width(&self) -> f32 {
        self.geo.width
    }

    pub fn natural_width(&self) -> f32 {
        self.geo.natural_width
    }

    pub fn height(&self) -> f32 {
        self.geo.height
    }

    pub fn min_content_width(&self) -> f32 {
        self.inner.calculate_content_widths().min
    }

    pub fn max_content_width(&self) -> f32 {
        self.inner.calculate_content_widths().max
    }

    pub fn lines(&self) -> &[LineInfo] {
        &self.geo.lines
    }

    pub fn first_baseline(&self) -> f32 {
        self.geo.lines.first().map(|l| l.baseline).unwrap_or(0.0)
    }

    pub fn glyph_runs(&self) -> &[GlyphRun] {
        &self.geo.runs
    }

    pub fn glyphs(&self, run: &GlyphRun) -> &[PositionedGlyph] {
        &self.geo.glyphs[run.glyphs.clone()]
    }

    pub fn decorations(&self) -> &[Decoration] {
        &self.geo.decorations
    }

    pub fn backgrounds(&self) -> &[Background] {
        &self.geo.backgrounds
    }

    pub fn objects(&self) -> &[PlacedObject] {
        &self.geo.objects
    }

    pub fn glyph_count(&self) -> usize {
        self.geo.glyphs.len()
    }

    pub(crate) fn cluster_starting_at(&self, index: usize) -> Option<usize> {
        let logical = &self.geo.logical;
        let clusters = &self.geo.clusters;
        let pos = logical.partition_point(|&i| clusters[i as usize].range.start < index);
        logical
            .get(pos)
            .filter(|&&i| clusters[i as usize].range.start == index)
            .map(|&i| i as usize)
    }

    pub(crate) fn cluster_ending_at(&self, index: usize) -> Option<usize> {
        let logical = &self.geo.logical;
        let clusters = &self.geo.clusters;
        let pos = logical.partition_point(|&i| clusters[i as usize].range.end < index);
        logical
            .get(pos)
            .filter(|&&i| clusters[i as usize].range.end == index)
            .map(|&i| i as usize)
    }

    pub(crate) fn object_at(&self, index: usize) -> Option<usize> {
        let objects = &self.geo.objects_by_index;
        let pos = objects.partition_point(|(i, _)| *i < index);
        objects
            .get(pos)
            .filter(|(i, _)| *i == index)
            .map(|(_, c)| *c as usize)
    }

    pub fn snap_index(&self, index: usize) -> usize {
        let text = self.text();
        let mut index = index.min(text.len());
        while !text.is_char_boundary(index) {
            index -= 1;
        }
        if self.cluster_starting_at(index).is_some() || self.cluster_ending_at(index).is_some() {
            return index;
        }
        let logical = &self.geo.logical;
        let clusters = &self.geo.clusters;
        let pos = logical.partition_point(|&i| clusters[i as usize].range.end <= index);
        match logical.get(pos) {
            Some(&i) if clusters[i as usize].range.start < index => {
                clusters[i as usize].range.start
            }
            _ => index,
        }
    }

    pub fn normalize(&self, pos: TextPosition) -> TextPosition {
        let len = self.text().len();
        let index = self.snap_index(pos.index);
        let mut affinity = pos.affinity;
        if index == 0 {
            affinity = Affinity::Downstream;
        }
        if affinity == Affinity::Upstream && self.text()[..index].ends_with('\n') {
            affinity = Affinity::Downstream;
        }
        if index == len
            && affinity == Affinity::Downstream
            && self.object_at(index).is_none()
            && !self.text().ends_with('\n')
            && index > 0
        {
            affinity = Affinity::Upstream;
        }
        TextPosition { index, affinity }
    }

    pub(crate) fn resolve(&self, pos: TextPosition) -> Option<(usize, Edge)> {
        let pos = self.normalize(pos);
        let index = pos.index;
        let down = || {
            self.cluster_starting_at(index)
                .map(|c| (c, Edge::Leading))
                .or_else(|| self.object_at(index).map(|c| (c, Edge::Trailing)))
        };
        let up = || {
            self.cluster_ending_at(index)
                .filter(|&c| self.geo.clusters[c].kind != ClusterKind::Newline)
                .map(|c| (c, Edge::Trailing))
                .or_else(|| self.object_at(index).map(|c| (c, Edge::Leading)))
        };
        match pos.affinity {
            Affinity::Downstream => down().or_else(up),
            Affinity::Upstream => up().or_else(down),
        }
    }

    pub(crate) fn edge_x(&self, cluster: usize, edge: Edge) -> f32 {
        let c = &self.geo.clusters[cluster];
        match (edge, c.rtl) {
            (Edge::Leading, false) | (Edge::Trailing, true) => c.x0,
            (Edge::Leading, true) | (Edge::Trailing, false) => c.x1,
        }
    }

    fn empty_line_for(&self, index: usize) -> usize {
        let lines = &self.geo.lines;
        lines
            .iter()
            .rposition(|l| l.text_range.start <= index)
            .unwrap_or(0)
            .min(lines.len().saturating_sub(1))
    }

    pub fn caret(&self, pos: TextPosition) -> Caret {
        let (line, x, rtl) = match self.resolve(pos) {
            Some((cluster, edge)) => {
                let c = &self.geo.clusters[cluster];
                (c.line as usize, self.edge_x(cluster, edge), c.rtl)
            }
            None => {
                let line = self.empty_line_for(pos.index);
                let x = self
                    .geo
                    .lines
                    .get(line)
                    .map(|l| l.start_x)
                    .unwrap_or_default();
                (line, x, self.rtl)
            }
        };
        let (top, bottom, baseline) = match self.geo.lines.get(line) {
            Some(l) => (l.baseline - l.ascent, l.baseline + l.descent, l.baseline),
            None => (0.0, self.geo.height, self.geo.height),
        };
        Caret {
            x,
            top,
            bottom,
            baseline,
            line,
            rtl,
        }
    }

    pub fn caret_rect(&self, pos: TextPosition, width: f32) -> Rect {
        let caret = self.caret(pos);
        let x = if caret.rtl { caret.x - width } else { caret.x };
        Rect::new(x, caret.top, width, caret.bottom - caret.top)
    }

    pub fn selection_rects(&self, range: Range<usize>) -> Vec<Rect> {
        let mut rects: Vec<Rect> = Vec::new();
        if range.is_empty() {
            return rects;
        }
        for line in &self.geo.lines {
            if line.text_range.end <= range.start && !line.text_range.is_empty() {
                continue;
            }
            if line.text_range.start >= range.end {
                break;
            }
            let mut line_rects: Vec<Rect> = Vec::new();
            for c in &self.geo.clusters[line.clusters.clone()] {
                let selected = match c.kind {
                    ClusterKind::Object(_) => {
                        range.start <= c.range.start && c.range.start < range.end
                    }
                    ClusterKind::Newline => false,
                    _ => c.range.start >= range.start && c.range.end <= range.end,
                };
                if !selected {
                    continue;
                }
                let rect = Rect::new(c.x0, line.top, c.x1 - c.x0, line.bottom - line.top);
                match line_rects.last_mut() {
                    Some(last) if (last.right() - rect.x).abs() < 0.01 => {
                        last.width = rect.right() - last.x;
                    }
                    _ => line_rects.push(rect),
                }
            }
            rects.extend(line_rects);
        }
        rects
    }

    pub fn line_at_y(&self, y: f32) -> usize {
        let lines = &self.geo.lines;
        if lines.is_empty() {
            return 0;
        }
        lines
            .iter()
            .position(|l| y < l.bottom)
            .unwrap_or(lines.len() - 1)
    }

    pub fn hit_test(&self, x: f32, y: f32) -> TextPosition {
        self.hit_test_line(self.line_at_y(y), x)
    }

    pub fn hit_test_line(&self, line: usize, x: f32) -> TextPosition {
        let Some(info) = self.geo.lines.get(line) else {
            return TextPosition::downstream(0);
        };
        let nav = &self.geo.clusters[info.nav.clone()];
        let (Some(first), Some(last)) = (nav.first(), nav.last()) else {
            return self.normalize(TextPosition::downstream(info.text_range.start));
        };
        let pos = if x <= first.x0 {
            self.left_edge_position(info.nav.start)
        } else if x >= last.x1 {
            self.right_edge_position(info.nav.end - 1)
        } else {
            let offset = nav.partition_point(|c| c.x1 <= x);
            let index = info.nav.start + offset.min(nav.len() - 1);
            let c = &self.geo.clusters[index];
            if x < (c.x0 + c.x1) * 0.5 {
                self.left_edge_position(index)
            } else {
                self.right_edge_position(index)
            }
        };
        self.normalize(pos)
    }

    pub(crate) fn left_edge_position(&self, cluster: usize) -> TextPosition {
        let c = &self.geo.clusters[cluster];
        match (c.kind, c.rtl) {
            (ClusterKind::Object(_), true) => TextPosition::downstream(c.range.start),
            (ClusterKind::Object(_), false) => TextPosition::upstream(c.range.start),
            (_, true) => TextPosition::upstream(c.range.end),
            (_, false) => TextPosition::downstream(c.range.start),
        }
    }

    pub(crate) fn right_edge_position(&self, cluster: usize) -> TextPosition {
        let c = &self.geo.clusters[cluster];
        match (c.kind, c.rtl) {
            (ClusterKind::Object(_), true) => TextPosition::upstream(c.range.start),
            (ClusterKind::Object(_), false) => TextPosition::downstream(c.range.start),
            (_, true) => TextPosition::downstream(c.range.start),
            (_, false) => TextPosition::upstream(c.range.end),
        }
    }

    pub fn end_position(&self) -> TextPosition {
        self.normalize(TextPosition::upstream(self.text().len()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub x: f32,
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    pub line: usize,
    pub rtl: bool,
}

fn shift_for(shifts: &[(f32, f32, f32)], x: f32) -> f32 {
    shifts
        .iter()
        .find(|(x0, x1, _)| x >= *x0 - 1e-3 && x <= *x1 + 1e-3)
        .map(|(_, _, d)| *d)
        .unwrap_or(0.0)
}
