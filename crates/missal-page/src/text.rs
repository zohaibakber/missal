use std::hash::{DefaultHasher, Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;

use missal_core::FieldReference;
use missal_core::document::{Inline, Style, Text};
use missal_text::{
    Affinity, Attr, AttributedText, FaceId, FontFamily, InlineObject, LineHeight, ParagraphStyle,
    Rgba, TextEngine, TextLayout, TextPosition, TextStyle,
};

use crate::FieldSource;
use crate::css::Length;
use crate::fonts;
use crate::geom::Rect;
use crate::style::{self, BoxProps, Computed, Declarations, Decoration, TextProps, UnicodeBidi};

pub const OBJECT: char = '\u{FFFC}';
pub const OBJECT_LEN: usize = 3;

const RLI: char = '\u{2067}';
const LRI: char = '\u{2066}';
const FSI: char = '\u{2068}';
const PDI: char = '\u{2069}';
const RLE: char = '\u{202B}';
const LRE: char = '\u{202A}';
const RLO: char = '\u{202E}';
const LRO: char = '\u{202D}';
const PDF: char = '\u{202C}';

const INDENT_OBJECT: u64 = u64::MAX;

pub fn logical_text(inlines: &[Inline]) -> String {
    let mut out = String::new();
    fn walk(inlines: &[Inline], out: &mut String) {
        for inline in inlines {
            match inline {
                Inline::Text(text) => out.push_str(&text.text),
                Inline::LineBreak(_) => out.push('\n'),
                Inline::Field(_) => out.push(OBJECT),
                Inline::Link(link) => walk(&link.children, out),
            }
        }
    }
    walk(inlines, &mut out);
    out
}

pub fn logical_len(inlines: &[Inline]) -> usize {
    inlines
        .iter()
        .map(|inline| match inline {
            Inline::Text(text) => text.text.len(),
            Inline::LineBreak(_) => 1,
            Inline::Field(_) => OBJECT_LEN,
            Inline::Link(link) => logical_len(&link.children),
        })
        .sum()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InlineLocation {
    pub path: Vec<usize>,
    pub offset: usize,
}

pub fn locate(inlines: &[Inline], offset: usize) -> Option<InlineLocation> {
    let mut remaining = offset;
    for (index, inline) in inlines.iter().enumerate() {
        let len = match inline {
            Inline::Text(text) => text.text.len(),
            Inline::LineBreak(_) => 1,
            Inline::Field(_) => OBJECT_LEN,
            Inline::Link(link) => logical_len(&link.children),
        };
        if remaining < len || (remaining == len && index + 1 == inlines.len()) {
            if let Inline::Link(link) = inline
                && let Some(mut inner) = locate(&link.children, remaining)
            {
                inner.path.insert(0, index);
                return Some(inner);
            }
            return Some(InlineLocation {
                path: vec![index],
                offset: remaining,
            });
        }
        remaining -= len;
    }
    if offset == 0 {
        return Some(InlineLocation {
            path: Vec::new(),
            offset: 0,
        });
    }
    None
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SegKind {
    Text,
    Atomic,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Seg {
    pub logical: Range<usize>,
    pub layout: Range<usize>,
    pub kind: SegKind,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct OffsetMap {
    pub(crate) segs: Vec<Seg>,
    pub(crate) logical_len: usize,
    pub(crate) layout_len: usize,
}

impl OffsetMap {
    pub fn logical_len(&self) -> usize {
        self.logical_len
    }

    pub fn to_layout(&self, offset: usize, affinity: Affinity) -> TextPosition {
        let offset = offset.min(self.logical_len);
        let mut before: Option<&Seg> = None;
        let mut after: Option<&Seg> = None;
        for seg in &self.segs {
            if seg.kind == SegKind::Text && seg.logical.start < offset && offset < seg.logical.end {
                return TextPosition::new(
                    seg.layout.start + (offset - seg.logical.start),
                    affinity,
                );
            }
            if seg.kind == SegKind::Atomic && seg.logical.start < offset && offset < seg.logical.end
            {
                return TextPosition::new(seg.layout.end, Affinity::Upstream);
            }
            if seg.logical.end == offset {
                before = Some(seg);
            }
            if seg.logical.start == offset && after.is_none() {
                after = Some(seg);
            }
        }
        match (before, after, affinity) {
            (Some(b), _, Affinity::Upstream) | (Some(b), None, _) => {
                TextPosition::new(b.layout.end, Affinity::Upstream)
            }
            (_, Some(a), _) => TextPosition::new(a.layout.start, Affinity::Downstream),
            (None, None, _) => {
                TextPosition::new(if offset == 0 { 0 } else { self.layout_len }, affinity)
            }
        }
    }

    pub fn to_logical(&self, position: TextPosition) -> (usize, Affinity) {
        let index = position.index;
        let mut previous_end: Option<usize> = None;
        for seg in &self.segs {
            if index < seg.layout.start {
                return match previous_end {
                    Some(end) if position.affinity == Affinity::Upstream => {
                        (end, Affinity::Upstream)
                    }
                    _ => (seg.logical.start, Affinity::Downstream),
                };
            }
            if index <= seg.layout.end {
                return match seg.kind {
                    SegKind::Text => (
                        seg.logical.start + (index - seg.layout.start),
                        position.affinity,
                    ),
                    SegKind::Atomic => {
                        let middle = (seg.layout.start + seg.layout.end) / 2;
                        if index == seg.layout.start {
                            (seg.logical.start, Affinity::Downstream)
                        } else if index == seg.layout.end || index > middle {
                            (seg.logical.end, Affinity::Upstream)
                        } else {
                            (seg.logical.start, Affinity::Downstream)
                        }
                    }
                };
            }
            previous_end = Some(seg.logical.end);
        }
        (self.logical_len, Affinity::Upstream)
    }

    pub fn floor_logical(&self, index: usize) -> usize {
        for seg in &self.segs {
            if index < seg.layout.end && index >= seg.layout.start {
                return match seg.kind {
                    SegKind::Text => seg.logical.start + (index - seg.layout.start),
                    SegKind::Atomic => seg.logical.start,
                };
            }
            if index < seg.layout.start {
                return seg.logical.start;
            }
        }
        self.logical_len
    }

    pub(crate) fn layout_range(&self, range: Range<usize>) -> Range<usize> {
        let start = self.to_layout(range.start, Affinity::Downstream).index;
        let end = self.to_layout(range.end, Affinity::Upstream).index;
        start..end.max(start)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InlineBox {
    pub ascent: f32,
    pub descent: f32,
    pub line_height: f32,
}

impl InlineBox {
    fn of(text: &TextProps) -> InlineBox {
        let (ascent, descent) = text.ascent_descent();
        InlineBox {
            ascent,
            descent,
            line_height: text.line_height_px(),
        }
    }

    fn above(&self) -> f32 {
        self.ascent + (self.line_height - self.ascent - self.descent) / 2.0
    }

    fn below(&self) -> f32 {
        self.line_height - self.above()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DecoSpan {
    pub range: Range<usize>,
    pub decoration: Decoration,
    pub color: Rgba,
    pub face: FaceId,
    pub size: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct BackgroundSpan {
    pub range: Range<usize>,
    pub color: Rgba,
    pub ascent: f32,
    pub descent: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldSpan {
    pub logical: usize,
    pub layout: Range<usize>,
    pub unresolved: bool,
    pub ascent: f32,
    pub descent: f32,
}

pub(crate) struct InlineContent {
    pub text: AttributedText,
    pub map: OffsetMap,
    pub boxes: Vec<(Range<usize>, InlineBox)>,
    pub decorations: Vec<DecoSpan>,
    pub backgrounds: Vec<BackgroundSpan>,
    pub fields: Vec<FieldSpan>,
}

struct Builder<'a> {
    base: &'a TextProps,
    block_decoration: Decoration,
    fields: &'a dyn FieldSource,
    text: String,
    spans: Vec<(Range<usize>, Attr)>,
    objects: Vec<InlineObject>,
    segs: Vec<Seg>,
    logical: usize,
    boxes: Vec<(Range<usize>, InlineBox)>,
    decorations: Vec<DecoSpan>,
    backgrounds: Vec<BackgroundSpan>,
    field_spans: Vec<FieldSpan>,
    spaces: Vec<SpaceRun>,
}

struct SpaceRun {
    range: Range<usize>,
    face: FaceId,
    size: f32,
    word_spacing: f32,
}

const ARABIC_SPACE_UNITS: f32 = 5.0;

fn script_of(c: char) -> Option<bool> {
    let cp = c as u32;
    if matches!(cp, 0x0600..=0x06FF | 0x0750..=0x077F | 0x08A0..=0x08FF | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF)
    {
        let digit = matches!(cp, 0x0660..=0x0669 | 0x06F0..=0x06F9);
        let common = matches!(cp, 0x060C | 0x061B | 0x061F | 0x0640 | 0x06DD);
        return if digit || common { None } else { Some(true) };
    }
    if c.is_alphabetic() {
        return Some(false);
    }
    None
}

fn arabic_context(text: &str, range: Range<usize>) -> bool {
    if let Some(found) = text[..range.start].chars().rev().find_map(script_of) {
        return found;
    }
    text[range.end..]
        .chars()
        .find_map(script_of)
        .unwrap_or(true)
}

fn bidi_open(text: &TextProps, bidi: UnicodeBidi, auto: bool) -> (Vec<char>, Vec<char>) {
    let rtl = text.direction.is_rtl();
    let isolate = if auto {
        FSI
    } else if rtl {
        RLI
    } else {
        LRI
    };
    match bidi {
        UnicodeBidi::Normal => (Vec::new(), Vec::new()),
        UnicodeBidi::Isolate => (vec![isolate], vec![PDI]),
        UnicodeBidi::Plaintext => (vec![FSI], vec![PDI]),
        UnicodeBidi::Embed => (vec![if rtl { RLE } else { LRE }], vec![PDF]),
        UnicodeBidi::Override => (vec![if rtl { RLO } else { LRO }], vec![PDF]),
        UnicodeBidi::IsolateOverride => {
            (vec![isolate, if rtl { RLO } else { LRO }], vec![PDF, PDI])
        }
    }
}

impl Builder<'_> {
    fn push_controls(&mut self, chars: &[char]) {
        for c in chars {
            self.text.push(*c);
        }
    }

    fn style_attrs(&mut self, range: Range<usize>, props: &TextProps) {
        if range.is_empty() {
            return;
        }
        let base = self.base;
        if props.family != base.family {
            self.spans
                .push((range.clone(), Attr::Family(props.family.clone())));
        }
        if props.size != base.size {
            self.spans.push((range.clone(), Attr::Size(props.size)));
        }
        if props.weight != base.weight {
            self.spans.push((range.clone(), Attr::Weight(props.weight)));
        }
        if props.italic != base.italic {
            self.spans.push((range.clone(), Attr::Italic(props.italic)));
        }
        if props.color != base.color {
            self.spans.push((range, Attr::Color(props.color)));
        }
    }

    fn word_spacing(&mut self, start: usize, end: usize, props: &TextProps) {
        if props.word_spacing == 0.0 {
            return;
        }
        let face = props.primary_face();
        for (i, c) in self.text[start..end].char_indices() {
            if c == ' ' || c == '\u{A0}' {
                self.spaces.push(SpaceRun {
                    range: start + i..start + i + c.len_utf8(),
                    face,
                    size: props.size,
                    word_spacing: props.word_spacing,
                });
            }
        }
    }

    fn apply_spaces(&mut self) {
        let inter = fonts::metrics(FaceId(1));
        let inter_space = inter.space_advance / inter.units_per_em;
        let spaces = std::mem::take(&mut self.spaces);
        for space in spaces {
            let arabic = arabic_context(&self.text, space.range.clone());
            let metrics = fonts::metrics(space.face);
            let natural = if arabic && space.face == FaceId(0) {
                ARABIC_SPACE_UNITS / metrics.units_per_em * space.size
            } else {
                metrics.space_advance / metrics.units_per_em * space.size
            };
            let target = (natural + space.word_spacing).max(0.0);
            self.spans
                .push((space.range.clone(), Attr::Family(FontFamily::Interface)));
            self.spans
                .push((space.range, Attr::Size(target / inter_space)));
        }
    }

    fn run(
        &mut self,
        content: &str,
        logical_len: usize,
        props: &TextProps,
        boxp: &BoxProps,
        auto_dir: bool,
        atomic: bool,
    ) -> Range<usize> {
        let (open, close) = bidi_open(props, boxp.unicode_bidi, auto_dir);
        let outer_start = self.text.len();
        self.push_controls(&open);
        let start = self.text.len();
        self.text.push_str(content);
        let end = self.text.len();
        self.push_controls(&close);
        let outer_end = self.text.len();
        self.style_attrs(start..end, props);
        self.word_spacing(start, end, props);
        if atomic {
            self.segs.push(Seg {
                logical: self.logical..self.logical + logical_len,
                layout: outer_start..outer_end,
                kind: SegKind::Atomic,
            });
        } else if logical_len > 0 {
            self.segs.push(Seg {
                logical: self.logical..self.logical + logical_len,
                layout: start..end,
                kind: SegKind::Text,
            });
        }
        self.logical += logical_len;
        if end > start {
            let ibox = InlineBox::of(props);
            self.boxes.push((start..end, ibox));
            let decoration = merge_decoration(self.block_decoration, boxp.decoration);
            if decoration.any() {
                self.decorations.push(DecoSpan {
                    range: start..end,
                    decoration,
                    color: decoration.color.unwrap_or(props.color),
                    face: props.primary_face(),
                    size: props.size,
                });
            }
            if let Some(color) = boxp.background {
                self.backgrounds.push(BackgroundSpan {
                    range: start..end,
                    color,
                    ascent: ibox.ascent,
                    descent: ibox.descent,
                });
            }
        }
        outer_start..outer_end
    }

    fn text_node(&mut self, text: &Text, parent: &TextProps, inherited: &BoxProps) {
        let mut decls = Declarations::new();
        decls.push_style(text.imported_style.as_ref());
        decls.push_style(text.css());
        let base = BoxProps {
            decoration: inherited.decoration,
            ..BoxProps::default()
        };
        let mut computed = style::compute(parent, base, &decls);
        style::apply_format(&mut computed.text, &mut computed.boxp, text.format());
        if inherited.background.is_some() && computed.boxp.background.is_none() {
            computed.boxp.background = inherited.background;
        }
        self.run(
            &text.text,
            text.text.len(),
            &computed.text,
            &computed.boxp,
            false,
            false,
        );
    }

    fn inlines(&mut self, inlines: &[Inline], parent: &TextProps, inherited: &BoxProps) {
        for inline in inlines {
            match inline {
                Inline::Text(text) => self.text_node(text, parent, inherited),
                Inline::LineBreak(_) => {
                    let start = self.text.len();
                    self.text.push('\n');
                    self.segs.push(Seg {
                        logical: self.logical..self.logical + 1,
                        layout: start..start + 1,
                        kind: SegKind::Text,
                    });
                    self.logical += 1;
                }
                Inline::Field(field) => {
                    let display = self.fields.display(&field.reference);
                    let mut decls = Declarations::new();
                    decls.push_style(field.imported_style.as_ref());
                    let base = BoxProps {
                        unicode_bidi: UnicodeBidi::Isolate,
                        decoration: inherited.decoration,
                        ..BoxProps::default()
                    };
                    let has_direction = field
                        .imported_style
                        .as_ref()
                        .is_some_and(|s| s.get("direction").is_some());
                    let computed = style::compute(parent, base, &decls);
                    let logical = self.logical;
                    let range = self.run(
                        &display.text,
                        OBJECT_LEN,
                        &computed.text,
                        &computed.boxp,
                        !has_direction,
                        true,
                    );
                    let ibox = InlineBox::of(&computed.text);
                    self.field_spans.push(FieldSpan {
                        logical,
                        layout: range,
                        unresolved: display.unresolved,
                        ascent: ibox.ascent,
                        descent: ibox.descent,
                    });
                }
                Inline::Link(link) => {
                    let mut decls = Declarations::new();
                    decls.push("color", "#0000ee");
                    decls.push_style(link.props.imported_style.as_ref());
                    let mut base = inherited.clone();
                    base.decoration.underline = true;
                    let computed = style::compute(parent, base, &decls);
                    self.inlines(&link.children, &computed.text, &computed.boxp);
                }
            }
        }
    }
}

fn merge_decoration(a: Decoration, b: Decoration) -> Decoration {
    Decoration {
        underline: a.underline || b.underline,
        overline: a.overline || b.overline,
        strike: a.strike || b.strike,
        dashed: a.dashed || b.dashed,
        color: b.color.or(a.color),
    }
}

pub(crate) fn build_inline(
    block: &Computed,
    inlines: &[Inline],
    fields: &dyn FieldSource,
    indent: f32,
) -> InlineContent {
    let mut builder = Builder {
        base: &block.text,
        block_decoration: block.boxp.decoration,
        fields,
        text: String::new(),
        spans: Vec::new(),
        objects: Vec::new(),
        segs: Vec::new(),
        logical: 0,
        boxes: Vec::new(),
        decorations: Vec::new(),
        backgrounds: Vec::new(),
        field_spans: Vec::new(),
        spaces: Vec::new(),
    };
    let inherited = BoxProps {
        decoration: Decoration::default(),
        ..BoxProps::default()
    };
    builder.inlines(inlines, &block.text, &inherited);
    builder.apply_spaces();
    if indent > 0.0 {
        builder.objects.push(InlineObject {
            id: INDENT_OBJECT,
            index: 0,
            width: indent,
            height: 0.0,
            baseline: Some(0.0),
        });
    }
    let layout_len = builder.text.len();
    let base = &block.text;
    let mut style = TextStyle::new(base.family.clone(), base.size);
    style.weight = base.weight;
    style.italic = base.italic;
    style.color = base.color;
    let text = AttributedText {
        text: Arc::from(builder.text.as_str()),
        style,
        spans: builder.spans,
        objects: builder.objects,
    };
    InlineContent {
        text,
        map: OffsetMap {
            segs: builder.segs,
            logical_len: builder.logical,
            layout_len,
        },
        boxes: builder.boxes,
        decorations: builder.decorations,
        backgrounds: builder.backgrounds,
        fields: builder.field_spans,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineBox {
    pub top: f32,
    pub bottom: f32,
    pub baseline: f32,
    pub content_top: f32,
    pub content_bottom: f32,
    pub(crate) glyph_dy: f32,
}

impl LineBox {
    pub fn height(&self) -> f32 {
        self.bottom - self.top
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum MarkerKind {
    Bullet,
    Text(String),
}

#[derive(Clone, Debug)]
pub struct Marker {
    pub layout: Arc<TextLayout>,
    pub x: f32,
}

#[derive(Clone, Debug)]
pub struct TextBox {
    pub(crate) layout: Arc<TextLayout>,
    pub(crate) lines: Vec<LineBox>,
    pub(crate) map: OffsetMap,
    pub(crate) border_box: Rect,
    pub(crate) content: Rect,
    pub(crate) margin_top: f32,
    pub(crate) margin_bottom: f32,
    pub(crate) borders: [style::BorderSide; 4],
    pub(crate) background: Option<Rgba>,
    pub(crate) color: Rgba,
    pub(crate) decorations: Vec<DecoSpan>,
    pub(crate) backgrounds: Vec<BackgroundSpan>,
    pub(crate) fields: Vec<FieldSpan>,
    pub(crate) marker: Option<Marker>,
    pub(crate) break_before: style::BreakValue,
    pub(crate) break_after: style::BreakValue,
    pub(crate) break_inside: style::BreakValue,
    pub(crate) orphans: usize,
    pub(crate) widows: usize,
    pub(crate) layout_dx: f32,
}

impl TextBox {
    pub fn layout(&self) -> &Arc<TextLayout> {
        &self.layout
    }

    pub fn lines(&self) -> &[LineBox] {
        &self.lines
    }

    pub fn offsets(&self) -> &OffsetMap {
        &self.map
    }

    pub fn height(&self) -> f32 {
        self.border_box.height
    }

    pub fn fields(&self) -> &[FieldSpan] {
        &self.fields
    }

    pub(crate) fn layout_dx(&self) -> f32 {
        self.layout_dx
    }

    pub(crate) fn caret_local(&self, position: TextPosition) -> (usize, Rect) {
        if self.layout.lines().is_empty() {
            let line = self.lines.first().copied().unwrap_or(LineBox {
                top: 0.0,
                bottom: 0.0,
                baseline: 0.0,
                content_top: 0.0,
                content_bottom: 0.0,
                glyph_dy: 0.0,
            });
            let x = if self.layout.is_rtl() {
                self.content.right()
            } else {
                self.content.x
            };
            return (
                0,
                Rect::new(
                    x,
                    line.content_top,
                    0.0,
                    line.content_bottom - line.content_top,
                ),
            );
        }
        let caret = self.layout.caret(position);
        let line = caret.line.min(self.lines.len() - 1);
        let lb = self.lines[line];
        let x = self.content.x + caret.x + self.layout_dx();
        (
            line,
            Rect::new(x, lb.content_top, 0.0, lb.content_bottom - lb.content_top),
        )
    }

    pub(crate) fn line_rect(&self, line: usize) -> Rect {
        let lb = self.lines[line];
        Rect::new(
            self.content.x,
            lb.top,
            self.content.width,
            lb.bottom - lb.top,
        )
    }

    pub(crate) fn line_rect_full(&self, line: usize) -> Rect {
        let lb = self.lines[line];
        Rect::new(
            self.border_box.x,
            lb.top,
            self.border_box.width,
            lb.bottom - lb.top,
        )
    }

    pub(crate) fn selection_local(&self, range: Range<usize>) -> Vec<(usize, Rect)> {
        let layout_range = self.map.layout_range(range.clone());
        let mut rects = Vec::new();
        if self.layout.lines().is_empty() {
            return rects;
        }
        for rect in self.layout.selection_rects(layout_range.clone()) {
            let line = self
                .layout
                .line_at_y(rect.y + rect.height * 0.5)
                .min(self.lines.len() - 1);
            let lb = self.lines[line];
            rects.push((
                line,
                Rect::new(
                    self.content.x + rect.x + self.layout_dx(),
                    lb.top,
                    rect.width,
                    lb.bottom - lb.top,
                ),
            ));
        }
        for (index, info) in self.layout.lines().iter().enumerate() {
            let breaks = self
                .layout
                .text()
                .get(info.text_range.clone())
                .is_some_and(|t| t.ends_with('\n'));
            let line_end = info.text_range.end;
            let covered = if breaks {
                layout_range.start < line_end && layout_range.end >= line_end
            } else {
                false
            };
            if covered && index < self.lines.len() {
                let lb = self.lines[index];
                let width = 6.0;
                let x = if self.layout.is_rtl() {
                    self.content.x + info.left - width
                } else {
                    self.content.x + info.right
                };
                rects.push((index, Rect::new(x, lb.top, width, lb.bottom - lb.top)));
            }
        }
        rects
    }

    pub(crate) fn hit_line(&self, line: usize, x: f32) -> TextPosition {
        if self.layout.lines().is_empty() {
            return TextPosition::downstream(0);
        }
        let line = line.min(self.layout.lines().len() - 1);
        self.layout
            .hit_test_line(line, x - self.content.x - self.layout_dx())
    }
}

pub(crate) struct TextBlockInput<'a> {
    pub computed: &'a Computed,
    pub inlines: &'a [Inline],
    pub marker: Option<MarkerKind>,
    pub available: f32,
}

pub(crate) fn content_hash(
    computed: &Computed,
    inlines: &[Inline],
    marker: &Option<MarkerKind>,
    available: f32,
    fields: &dyn FieldSource,
) -> u64 {
    let mut hasher = DefaultHasher::new();
    computed.text.hash(&mut hasher);
    computed.boxp.hash(&mut hasher);
    available.to_bits().hash(&mut hasher);
    match marker {
        None => 0u8.hash(&mut hasher),
        Some(MarkerKind::Bullet) => 1u8.hash(&mut hasher),
        Some(MarkerKind::Text(t)) => (2u8, t).hash(&mut hasher),
    }
    hash_inlines(inlines, fields, &mut hasher);
    hasher.finish()
}

fn hash_style<H: Hasher>(style: &Option<Style>, state: &mut H) {
    match style {
        Some(s) => s.as_str().hash(state),
        None => 0u8.hash(state),
    }
}

pub(crate) fn hash_inlines<H: Hasher>(inlines: &[Inline], fields: &dyn FieldSource, state: &mut H) {
    inlines.len().hash(state);
    for inline in inlines {
        match inline {
            Inline::Text(text) => {
                0u8.hash(state);
                text.text.hash(state);
                text.format.0.hash(state);
                hash_style(&text.style, state);
                hash_style(&text.imported_style, state);
            }
            Inline::LineBreak(_) => 1u8.hash(state),
            Inline::Field(field) => {
                2u8.hash(state);
                let display = fields.display(&field.reference);
                display.text.hash(state);
                display.unresolved.hash(state);
                hash_style(&field.imported_style, state);
            }
            Inline::Link(link) => {
                3u8.hash(state);
                hash_style(&link.props.imported_style, state);
                hash_inlines(&link.children, fields, state);
            }
        }
    }
}

pub(crate) fn collect_field_refs(inlines: &[Inline], out: &mut Vec<FieldReference>) {
    for inline in inlines {
        match inline {
            Inline::Field(field) => out.push(field.reference.clone()),
            Inline::Link(link) => collect_field_refs(&link.children, out),
            _ => {}
        }
    }
}

fn layout_paragraph(
    engine: &mut TextEngine,
    text: &AttributedText,
    props: &TextProps,
    width: f32,
) -> Arc<TextLayout> {
    let paragraph = ParagraphStyle {
        direction: if props.direction.is_rtl() {
            missal_text::Direction::Rtl
        } else {
            missal_text::Direction::Ltr
        },
        align: props.align,
        line_height: LineHeight::Normal,
        wrap: props.white_space.wraps(),
    };
    engine.layout(text, &paragraph, Some(width.max(0.0)))
}

pub(crate) fn measure_text_block(
    engine: &mut TextEngine,
    input: TextBlockInput<'_>,
    fields: &dyn FieldSource,
) -> TextBox {
    let computed = input.computed;
    let text_props = &computed.text;
    let boxp = &computed.boxp;
    let available = input.available;
    let margins = boxp.margins(text_props, available);
    let paddings = boxp.paddings(text_props, available);
    let borders = boxp.borders(text_props);
    let border_widths = boxp.border_widths(text_props);
    let mut border_width = available - margins.horizontal();
    if let Some(width) = boxp.width.filter(|w| *w != Length::Auto) {
        border_width = width.resolve(text_props.size, available)
            + paddings.horizontal()
            + border_widths.horizontal();
    }
    if let Some(max) = boxp.max_width.filter(|w| *w != Length::Auto) {
        border_width = border_width.min(
            max.resolve(text_props.size, available)
                + paddings.horizontal()
                + border_widths.horizontal(),
        );
    }
    let content_width =
        (border_width - paddings.horizontal() - border_widths.horizontal()).max(0.0);
    let indent = text_props.indent.resolve(text_props.size, content_width);
    let mut content = build_inline(computed, input.inlines, fields, indent.max(0.0));
    let mut layout = layout_paragraph(engine, &content.text, text_props, content_width);
    let mut layout_dx = 0.0;
    if indent < 0.0 {
        let hang = -indent;
        let wide = content_width + hang;
        let base_objects = content.text.objects.clone();
        let mut starts: Vec<usize> = Vec::new();
        for _ in 0..6 {
            let mut text = content.text.clone();
            text.objects = base_objects.clone();
            for (k, start) in starts.iter().enumerate() {
                text.objects.push(InlineObject {
                    id: INDENT_OBJECT - 1 - k as u64,
                    index: *start,
                    width: hang,
                    height: 0.0,
                    baseline: Some(0.0),
                });
            }
            let candidate = layout_paragraph(engine, &text, text_props, wide);
            let next: Vec<usize> = candidate
                .lines()
                .iter()
                .skip(1)
                .map(|l| l.text_range.start)
                .collect();
            layout = candidate;
            content.text = text;
            if next == starts {
                break;
            }
            starts = next;
        }
        if !text_props.direction.is_rtl() {
            layout_dx = -hang;
        }
    }
    let strut = InlineBox::of(text_props);
    let mut lines = Vec::with_capacity(layout.lines().len().max(1));
    let content_top = border_widths.top + paddings.top;
    let mut y = content_top;
    let infos = layout.lines();
    let count = infos.len().max(1);
    for index in 0..count {
        let mut above = strut.above();
        let mut below = strut.below();
        let mut content_above = strut.ascent;
        let mut content_below = strut.descent;
        let mut parley_baseline = 0.0;
        if let Some(info) = infos.get(index) {
            parley_baseline = info.baseline;
            let range = info.text_range.clone();
            for (box_range, ibox) in &content.boxes {
                if box_range.start < range.end && range.start < box_range.end {
                    let visible = content
                        .text
                        .text
                        .get(box_range.start.max(range.start)..box_range.end.min(range.end))
                        .unwrap_or("");
                    if visible.chars().all(|c| c == '\n') {
                        continue;
                    }
                    above = above.max(ibox.above());
                    below = below.max(ibox.below());
                    content_above = content_above.max(ibox.ascent);
                    content_below = content_below.max(ibox.descent);
                }
            }
            for object in layout.objects() {
                if object.line == index && object.id < INDENT_OBJECT - 64 {
                    above = above.max(object.rect.height);
                    content_above = content_above.max(object.rect.height);
                }
            }
        }
        let baseline = y + above;
        lines.push(LineBox {
            top: y,
            bottom: baseline + below,
            baseline,
            content_top: baseline - content_above,
            content_bottom: baseline + content_below,
            glyph_dy: baseline - parley_baseline,
        });
        y = baseline + below;
    }
    let lines_height = y - content_top;
    let min_height = boxp
        .min_height
        .map(|m| m.resolve(text_props.size, 0.0))
        .unwrap_or(0.0);
    let mut content_height = lines_height.max(min_height);
    if let Some(height) = boxp
        .height
        .and_then(|h| h.resolve_absolute(text_props.size))
    {
        content_height = content_height.max(height);
    }
    let border_height = content_height + paddings.vertical() + border_widths.vertical();
    let border_box = Rect::new(margins.left, 0.0, border_width, border_height);
    let content_rect = Rect::new(
        margins.left + border_widths.left + paddings.left,
        content_top,
        content_width,
        content_height,
    );
    let marker = input.marker.map(|kind| {
        let label: String = match &kind {
            MarkerKind::Bullet => "\u{2022} ".to_owned(),
            MarkerKind::Text(t) => format!("{t}. "),
        };
        let mut style = TextStyle::new(text_props.family.clone(), text_props.size);
        style.color = text_props.color;
        style.weight = text_props.weight;
        let attributed = AttributedText::new(label.as_str(), style);
        let paragraph = ParagraphStyle {
            direction: if text_props.direction.is_rtl() {
                missal_text::Direction::Rtl
            } else {
                missal_text::Direction::Ltr
            },
            align: missal_text::Align::Start,
            line_height: LineHeight::Normal,
            wrap: false,
        };
        let marker_layout = engine.layout(&attributed, &paragraph, None);
        let width = marker_layout.natural_width().max(marker_layout.width());
        let x = if text_props.direction.is_rtl() {
            content_rect.right()
        } else {
            content_rect.x - width
        };
        Marker {
            layout: marker_layout,
            x,
        }
    });
    TextBox {
        layout,
        lines,
        map: content.map,
        border_box,
        content: content_rect,
        margin_top: margins.top,
        margin_bottom: margins.bottom,
        borders,
        background: boxp.background,
        color: text_props.color,
        decorations: content.decorations,
        backgrounds: content.backgrounds,
        fields: content.fields,
        marker,
        break_before: boxp.break_before,
        break_after: boxp.break_after,
        break_inside: boxp.break_inside,
        orphans: text_props.orphans as usize,
        widows: text_props.widows as usize,
        layout_dx,
    }
}
