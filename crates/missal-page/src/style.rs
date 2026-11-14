use std::hash::{Hash, Hasher};

use missal_core::document::{Align as DocAlign, Direction as DocDirection, Style, TextFormat};
use missal_text::{Align, FontFamily, Rgba};

use crate::css::{self, Length};
use crate::fonts;

pub const BODY_FONT_SIZE: f32 = 20.0;
pub const BODY_LINE_HEIGHT: f32 = 2.4;
pub const WORD_SPACING_EM: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dir {
    Ltr,
    Rtl,
}

impl Dir {
    pub fn is_rtl(self) -> bool {
        self == Dir::Rtl
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LineHeightSpec {
    Normal,
    Factor(f32),
    Px(f32),
}

impl LineHeightSpec {
    pub fn resolve(self, size: f32, normal: f32) -> f32 {
        match self {
            LineHeightSpec::Normal => normal,
            LineHeightSpec::Factor(f) => f * size,
            LineHeightSpec::Px(px) => px,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    #[default]
    PreWrap,
    Pre,
    PreLine,
}

impl WhiteSpace {
    pub fn wraps(self) -> bool {
        !matches!(self, WhiteSpace::NoWrap | WhiteSpace::Pre)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextProps {
    pub family: FontFamily,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    pub color: Rgba,
    pub line_height: LineHeightSpec,
    pub direction: Dir,
    pub align: Align,
    pub indent: Length,
    pub word_spacing: f32,
    pub letter_spacing: f32,
    pub white_space: WhiteSpace,
    pub orphans: u32,
    pub widows: u32,
    pub border_collapse: bool,
}

impl TextProps {
    pub fn body() -> TextProps {
        TextProps {
            family: FontFamily::Document,
            size: BODY_FONT_SIZE,
            weight: 400,
            italic: false,
            color: Rgba::BLACK,
            line_height: LineHeightSpec::Factor(BODY_LINE_HEIGHT),
            direction: Dir::Rtl,
            align: Align::Right,
            indent: Length::Px(0.0),
            word_spacing: BODY_FONT_SIZE * WORD_SPACING_EM,
            letter_spacing: 0.0,
            white_space: WhiteSpace::PreWrap,
            orphans: 2,
            widows: 2,
            border_collapse: true,
        }
    }

    pub fn primary_face(&self) -> missal_text::FaceId {
        fonts::primary_face(&self.family, self.weight, self.italic)
    }

    pub fn ascent_descent(&self) -> (f32, f32) {
        let metrics = fonts::metrics(self.primary_face());
        (metrics.ascent_px(self.size), metrics.descent_px(self.size))
    }

    pub fn line_height_px(&self) -> f32 {
        let metrics = fonts::metrics(self.primary_face());
        self.line_height
            .resolve(self.size, metrics.normal_line_height(self.size))
    }
}

impl Hash for TextProps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.family.hash(state);
        self.size.to_bits().hash(state);
        self.weight.hash(state);
        self.italic.hash(state);
        self.color.hash(state);
        match self.line_height {
            LineHeightSpec::Normal => 0u32.hash(state),
            LineHeightSpec::Factor(f) => (1u32, f.to_bits()).hash(state),
            LineHeightSpec::Px(p) => (2u32, p.to_bits()).hash(state),
        }
        self.direction.hash(state);
        self.align.hash(state);
        hash_length(self.indent, state);
        self.word_spacing.to_bits().hash(state);
        self.letter_spacing.to_bits().hash(state);
        self.white_space.hash(state);
        self.orphans.hash(state);
        self.widows.hash(state);
        self.border_collapse.hash(state);
    }
}

pub fn hash_length<H: Hasher>(length: Length, state: &mut H) {
    match length {
        Length::Px(v) => (0u8, v.to_bits()).hash(state),
        Length::Em(v) => (1u8, v.to_bits()).hash(state),
        Length::Rem(v) => (2u8, v.to_bits()).hash(state),
        Length::Percent(v) => (3u8, v.to_bits()).hash(state),
        Length::Auto => 4u8.hash(state),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub enum BorderStyle {
    #[default]
    None,
    Hidden,
    Dotted,
    Dashed,
    Solid,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl BorderStyle {
    pub fn parse(value: &str) -> Option<BorderStyle> {
        Some(match css::clean(value).to_ascii_lowercase().as_str() {
            "none" => BorderStyle::None,
            "hidden" => BorderStyle::Hidden,
            "dotted" => BorderStyle::Dotted,
            "dashed" => BorderStyle::Dashed,
            "solid" => BorderStyle::Solid,
            "double" => BorderStyle::Double,
            "groove" => BorderStyle::Groove,
            "ridge" => BorderStyle::Ridge,
            "inset" => BorderStyle::Inset,
            "outset" => BorderStyle::Outset,
            _ => return None,
        })
    }

    pub fn visible(self) -> bool {
        !matches!(self, BorderStyle::None | BorderStyle::Hidden)
    }

    pub fn collapse_rank(self) -> u8 {
        match self {
            BorderStyle::Hidden => 9,
            BorderStyle::Double => 8,
            BorderStyle::Solid => 7,
            BorderStyle::Dashed => 6,
            BorderStyle::Dotted => 5,
            BorderStyle::Ridge => 4,
            BorderStyle::Outset => 3,
            BorderStyle::Groove => 2,
            BorderStyle::Inset => 1,
            BorderStyle::None => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct BorderSide {
    pub width: f32,
    pub style: BorderStyle,
    pub color: Option<Rgba>,
}

impl BorderSide {
    pub fn used_width(&self) -> f32 {
        if self.style.visible() {
            self.width.max(0.0)
        } else {
            0.0
        }
    }

    pub fn solid(width: f32, color: Rgba) -> BorderSide {
        BorderSide {
            width,
            style: BorderStyle::Solid,
            color: Some(color),
        }
    }

    fn hash_into<H: Hasher>(&self, state: &mut H) {
        self.width.to_bits().hash(state);
        self.style.hash(state);
        self.color.hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum BreakValue {
    #[default]
    Auto,
    Page,
    Avoid,
}

impl BreakValue {
    fn parse(value: &str) -> Option<BreakValue> {
        Some(match css::clean(value).to_ascii_lowercase().as_str() {
            "auto" => BreakValue::Auto,
            "page" | "always" | "left" | "right" | "recto" | "verso" => BreakValue::Page,
            "avoid" | "avoid-page" => BreakValue::Avoid,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum VerticalAlign {
    #[default]
    Baseline,
    Top,
    Middle,
    Bottom,
    Sub,
    Super,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Decoration {
    pub underline: bool,
    pub overline: bool,
    pub strike: bool,
    pub dashed: bool,
    pub color: Option<Rgba>,
}

impl Decoration {
    pub fn any(&self) -> bool {
        self.underline || self.overline || self.strike
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum UnicodeBidi {
    #[default]
    Normal,
    Embed,
    Isolate,
    Override,
    IsolateOverride,
    Plaintext,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BoxProps {
    pub margin: [Length; 4],
    pub margin_inline: [Option<Length>; 2],
    pub padding: [Length; 4],
    pub padding_inline: [Option<Length>; 2],
    pub border: [BorderSide; 4],
    pub border_inline_start: Option<BorderSide>,
    pub background: Option<Rgba>,
    pub width: Option<Length>,
    pub min_width: Option<Length>,
    pub max_width: Option<Length>,
    pub height: Option<Length>,
    pub min_height: Option<Length>,
    pub break_before: BreakValue,
    pub break_after: BreakValue,
    pub break_inside: BreakValue,
    pub vertical_align: VerticalAlign,
    pub decoration: Decoration,
    pub unicode_bidi: UnicodeBidi,
    pub table_fixed: bool,
}

impl Default for BoxProps {
    fn default() -> BoxProps {
        BoxProps {
            margin: [Length::Px(0.0); 4],
            margin_inline: [None; 2],
            padding: [Length::Px(0.0); 4],
            padding_inline: [None; 2],
            border: [BorderSide::default(); 4],
            border_inline_start: None,
            background: None,
            width: None,
            min_width: None,
            max_width: None,
            height: None,
            min_height: None,
            break_before: BreakValue::Auto,
            break_after: BreakValue::Auto,
            break_inside: BreakValue::Auto,
            vertical_align: VerticalAlign::Baseline,
            decoration: Decoration::default(),
            unicode_bidi: UnicodeBidi::Normal,
            table_fixed: true,
        }
    }
}

impl Hash for BoxProps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for l in self.margin.iter().chain(self.padding.iter()) {
            hash_length(*l, state);
        }
        for l in self.margin_inline.iter().chain(self.padding_inline.iter()) {
            match l {
                Some(l) => hash_length(*l, state),
                None => 9u8.hash(state),
            }
        }
        for b in &self.border {
            b.hash_into(state);
        }
        if let Some(b) = &self.border_inline_start {
            b.hash_into(state);
        }
        self.background.hash(state);
        for l in [
            self.width,
            self.min_width,
            self.max_width,
            self.height,
            self.min_height,
        ] {
            match l {
                Some(l) => hash_length(l, state),
                None => 9u8.hash(state),
            }
        }
        self.break_before.hash(state);
        self.break_after.hash(state);
        self.break_inside.hash(state);
        self.vertical_align.hash(state);
        let d = self.decoration;
        (d.underline, d.overline, d.strike, d.dashed, d.color).hash(state);
        self.unicode_bidi.hash(state);
        self.table_fixed.hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Edges {
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl BoxProps {
    pub fn margins(&self, text: &TextProps, containing_width: f32) -> Edges {
        self.resolve_edges(self.margin, self.margin_inline, text, containing_width)
    }

    pub fn paddings(&self, text: &TextProps, containing_width: f32) -> Edges {
        let mut edges =
            self.resolve_edges(self.padding, self.padding_inline, text, containing_width);
        edges.top = edges.top.max(0.0);
        edges.right = edges.right.max(0.0);
        edges.bottom = edges.bottom.max(0.0);
        edges.left = edges.left.max(0.0);
        edges
    }

    pub fn borders(&self, text: &TextProps) -> [BorderSide; 4] {
        let mut sides = self.border;
        if let Some(start) = self.border_inline_start {
            if text.direction.is_rtl() {
                sides[1] = start;
            } else {
                sides[3] = start;
            }
        }
        sides
    }

    pub fn border_widths(&self, text: &TextProps) -> Edges {
        let sides = self.borders(text);
        Edges {
            top: sides[0].used_width(),
            right: sides[1].used_width(),
            bottom: sides[2].used_width(),
            left: sides[3].used_width(),
        }
    }

    fn resolve_edges(
        &self,
        physical: [Length; 4],
        inline: [Option<Length>; 2],
        text: &TextProps,
        containing_width: f32,
    ) -> Edges {
        let mut values = physical;
        let (start, end) = if text.direction.is_rtl() {
            (1, 3)
        } else {
            (3, 1)
        };
        if let Some(l) = inline[0] {
            values[start] = l;
        }
        if let Some(l) = inline[1] {
            values[end] = l;
        }
        let r = |l: Length| l.resolve(text.size, containing_width);
        Edges {
            top: r(values[0]),
            right: r(values[1]),
            bottom: r(values[2]),
            left: r(values[3]),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Computed {
    pub text: TextProps,
    pub boxp: BoxProps,
}

fn weight(value: &str, parent: u16) -> Option<u16> {
    let value = css::clean(value).to_ascii_lowercase();
    Some(match value.as_str() {
        "normal" => 400,
        "bold" => 700,
        "bolder" => {
            if parent < 350 {
                400
            } else if parent < 550 {
                700
            } else {
                900
            }
        }
        "lighter" => {
            if parent < 550 {
                100
            } else if parent < 750 {
                400
            } else {
                700
            }
        }
        other => other.parse::<f32>().ok()?.clamp(1.0, 1000.0) as u16,
    })
}

fn font_size(value: &str, parent: f32) -> Option<f32> {
    let lower = css::clean(value).to_ascii_lowercase();
    let keyword = match lower.as_str() {
        "xx-small" => Some(9.0),
        "x-small" => Some(10.0),
        "small" => Some(13.0),
        "medium" => Some(16.0),
        "large" => Some(18.0),
        "x-large" => Some(24.0),
        "xx-large" => Some(32.0),
        "smaller" => Some(parent / 1.2),
        "larger" => Some(parent * 1.2),
        _ => None,
    };
    if keyword.is_some() {
        return keyword;
    }
    match css::length(&lower)? {
        Length::Auto => None,
        Length::Percent(p) => Some(parent * p / 100.0),
        Length::Em(e) => Some(parent * e),
        other => Some(other.resolve(parent, parent)),
    }
}

fn line_height(value: &str, size: f32) -> Option<LineHeightSpec> {
    let lower = css::clean(value).to_ascii_lowercase();
    if lower == "normal" {
        return Some(LineHeightSpec::Normal);
    }
    if let Ok(factor) = lower.parse::<f32>() {
        return Some(LineHeightSpec::Factor(factor.max(0.0)));
    }
    match css::length(&lower)? {
        Length::Auto => None,
        Length::Percent(p) => Some(LineHeightSpec::Px(size * p / 100.0)),
        other => Some(LineHeightSpec::Px(other.resolve(size, 0.0).max(0.0))),
    }
}

fn align(value: &str) -> Option<Align> {
    Some(match css::clean(value).to_ascii_lowercase().as_str() {
        "start" => Align::Start,
        "end" => Align::End,
        "left" | "-webkit-left" => Align::Left,
        "right" | "-webkit-right" => Align::Right,
        "center" | "-webkit-center" => Align::Center,
        "justify" => Align::Justify,
        _ => return None,
    })
}

pub fn doc_align(align: DocAlign) -> Align {
    match align {
        DocAlign::Left => Align::Left,
        DocAlign::Start => Align::Start,
        DocAlign::Center => Align::Center,
        DocAlign::Right => Align::Right,
        DocAlign::End => Align::End,
        DocAlign::Justify => Align::Justify,
    }
}

pub fn doc_dir(direction: DocDirection) -> Dir {
    match direction {
        DocDirection::Ltr => Dir::Ltr,
        DocDirection::Rtl => Dir::Rtl,
    }
}

fn border_shorthand(value: &str) -> BorderSide {
    let mut side = BorderSide {
        width: 3.0,
        style: BorderStyle::None,
        color: None,
    };
    for part in css::split_values(css::clean(value)) {
        if let Some(style) = BorderStyle::parse(part) {
            side.style = style;
        } else if let Some(width) = border_width(part) {
            side.width = width;
        } else if let Some(color) = css::color(part) {
            side.color = Some(color);
        }
    }
    side
}

fn border_width(value: &str) -> Option<f32> {
    match css::clean(value).to_ascii_lowercase().as_str() {
        "thin" => Some(1.0),
        "medium" => Some(3.0),
        "thick" => Some(5.0),
        other => css::length(other)?
            .resolve_absolute(16.0)
            .map(|w| w.max(0.0)),
    }
}

fn decoration(value: &str, current: Decoration) -> Decoration {
    let mut d = Decoration {
        color: current.color,
        dashed: current.dashed,
        ..Decoration::default()
    };
    for part in css::split_values(css::clean(value)) {
        match part.to_ascii_lowercase().as_str() {
            "none" => {}
            "underline" => d.underline = true,
            "overline" => d.overline = true,
            "line-through" => d.strike = true,
            "dashed" | "dotted" => d.dashed = true,
            "solid" | "double" | "wavy" => d.dashed = false,
            other => {
                if let Some(color) = css::color(other) {
                    d.color = Some(color);
                }
            }
        }
    }
    d
}

fn vertical_align(value: &str) -> Option<VerticalAlign> {
    Some(match css::clean(value).to_ascii_lowercase().as_str() {
        "baseline" => VerticalAlign::Baseline,
        "top" | "text-top" => VerticalAlign::Top,
        "middle" => VerticalAlign::Middle,
        "bottom" | "text-bottom" => VerticalAlign::Bottom,
        "sub" => VerticalAlign::Sub,
        "super" => VerticalAlign::Super,
        _ => return None,
    })
}

pub struct Declarations<'a> {
    list: Vec<(&'a str, &'a str)>,
}

impl<'a> Declarations<'a> {
    pub fn new() -> Declarations<'a> {
        Declarations { list: Vec::new() }
    }

    pub fn push_style(&mut self, style: Option<&'a Style>) {
        if let Some(style) = style {
            self.list.extend(style.declarations());
        }
    }

    pub fn push(&mut self, property: &'a str, value: &'a str) {
        self.list.push((property, value));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&'a str, &'a str)> + '_ {
        self.list.iter().copied()
    }
}

impl Default for Declarations<'_> {
    fn default() -> Self {
        Self::new()
    }
}

pub fn compute(parent: &TextProps, base: BoxProps, decls: &Declarations<'_>) -> Computed {
    let mut text = parent.clone();
    let mut boxp = base;
    for (property, value) in decls.iter() {
        if property.eq_ignore_ascii_case("font-size")
            && let Some(size) = font_size(value, parent.size)
        {
            text.size = size.max(0.0);
        }
    }
    for (property, value) in decls.iter() {
        apply(property, value, parent, &mut text, &mut boxp);
    }
    Computed { text, boxp }
}

fn apply(
    property: &str,
    value: &str,
    parent: &TextProps,
    text: &mut TextProps,
    boxp: &mut BoxProps,
) {
    let property = property.trim().to_ascii_lowercase();
    let value = css::clean(value);
    if value.to_ascii_lowercase().contains("var(") {
        return;
    }
    let lower = value.to_ascii_lowercase();
    if lower == "inherit" || lower == "initial" || lower == "unset" {
        return;
    }
    let len = || css::length(value);
    match property.as_str() {
        "font-family" => {
            if let Some(name) = css::first_family(value) {
                text.family = fonts::family_for(&name);
            }
        }
        "font-weight" => {
            if let Some(w) = weight(value, parent.weight) {
                text.weight = w;
            }
        }
        "font-style" => text.italic = matches!(lower.as_str(), "italic" | "oblique"),
        "color" => {
            if let Some(c) = css::color(value) {
                text.color = c;
            }
        }
        "background-color" | "background" => {
            if let Some(c) = css::split_values(value).into_iter().find_map(css::color) {
                boxp.background = (c.alpha() > 0).then_some(c);
            } else if lower == "none" || lower == "transparent" {
                boxp.background = None;
            }
        }
        "line-height" => {
            if let Some(lh) = line_height(value, text.size) {
                text.line_height = lh;
            }
        }
        "letter-spacing" => {
            text.letter_spacing = if lower == "normal" {
                0.0
            } else {
                len()
                    .map(|l| l.resolve(text.size, 0.0))
                    .unwrap_or(text.letter_spacing)
            }
        }
        "word-spacing" => {
            text.word_spacing = if lower == "normal" {
                0.0
            } else {
                len()
                    .map(|l| l.resolve(text.size, 0.0))
                    .unwrap_or(text.word_spacing)
            }
        }
        "white-space" => {
            text.white_space = match lower.as_str() {
                "normal" => WhiteSpace::Normal,
                "nowrap" => WhiteSpace::NoWrap,
                "pre" => WhiteSpace::Pre,
                "pre-line" => WhiteSpace::PreLine,
                "pre-wrap" | "break-spaces" => WhiteSpace::PreWrap,
                _ => text.white_space,
            }
        }
        "text-decoration" | "text-decoration-line" => {
            boxp.decoration = decoration(value, boxp.decoration)
        }
        "text-decoration-color" => boxp.decoration.color = css::color(value),
        "text-decoration-style" => {
            boxp.decoration.dashed = matches!(lower.as_str(), "dashed" | "dotted")
        }
        "vertical-align" => {
            if let Some(v) = vertical_align(value) {
                boxp.vertical_align = v;
            }
        }
        "direction" => match lower.as_str() {
            "rtl" => text.direction = Dir::Rtl,
            "ltr" => text.direction = Dir::Ltr,
            _ => {}
        },
        "unicode-bidi" => {
            boxp.unicode_bidi = match lower.as_str() {
                "embed" => UnicodeBidi::Embed,
                "isolate" | "-webkit-isolate" => UnicodeBidi::Isolate,
                "bidi-override" => UnicodeBidi::Override,
                "isolate-override" => UnicodeBidi::IsolateOverride,
                "plaintext" => UnicodeBidi::Plaintext,
                _ => UnicodeBidi::Normal,
            }
        }
        "text-align" => {
            if let Some(a) = align(value) {
                text.align = a;
            }
        }
        "text-indent" => {
            if let Some(l) = len() {
                text.indent = l;
            }
        }
        "orphans" => {
            if let Some(n) = css::number(value) {
                text.orphans = n.max(1.0) as u32;
            }
        }
        "widows" => {
            if let Some(n) = css::number(value) {
                text.widows = n.max(1.0) as u32;
            }
        }
        "border-collapse" => text.border_collapse = lower == "collapse",
        "table-layout" => boxp.table_fixed = lower == "fixed",
        "width" => boxp.width = len(),
        "min-width" => boxp.min_width = len(),
        "max-width" => boxp.max_width = if lower == "none" { None } else { len() },
        "height" => boxp.height = len(),
        "min-height" => boxp.min_height = len(),
        "margin" => {
            if let Some(m) = css::box_values(value, css::length) {
                boxp.margin = m;
                boxp.margin_inline = [None; 2];
            }
        }
        "padding" => {
            if let Some(p) = css::box_values(value, css::length) {
                boxp.padding = p;
                boxp.padding_inline = [None; 2];
            }
        }
        "border" => {
            let side = border_shorthand(value);
            boxp.border = [side; 4];
            boxp.border_inline_start = None;
        }
        "border-width" => {
            if let Some(w) = css::box_values(value, border_width) {
                for (side, w) in boxp.border.iter_mut().zip(w) {
                    side.width = w;
                }
            }
        }
        "border-style" => {
            if let Some(s) = css::box_values(value, BorderStyle::parse) {
                for (side, s) in boxp.border.iter_mut().zip(s) {
                    side.style = s;
                }
            }
        }
        "border-color" => {
            if let Some(c) = css::box_values(value, css::color) {
                for (side, c) in boxp.border.iter_mut().zip(c) {
                    side.color = Some(c);
                }
            }
        }
        "break-before" | "page-break-before" => {
            if let Some(b) = BreakValue::parse(value) {
                boxp.break_before = b;
            }
        }
        "break-after" | "page-break-after" => {
            if let Some(b) = BreakValue::parse(value) {
                boxp.break_after = b;
            }
        }
        "break-inside" | "page-break-inside" => {
            if let Some(b) = BreakValue::parse(value) {
                boxp.break_inside = if b == BreakValue::Page {
                    BreakValue::Auto
                } else {
                    b
                };
            }
        }
        other => apply_side(other, value, boxp),
    }
}

fn side_index(name: &str) -> Option<usize> {
    Some(match name {
        "top" => 0,
        "right" => 1,
        "bottom" => 2,
        "left" => 3,
        _ => return None,
    })
}

fn apply_side(property: &str, value: &str, boxp: &mut BoxProps) {
    if let Some(rest) = property.strip_prefix("margin-") {
        match rest {
            "inline-start" => boxp.margin_inline[0] = css::length(value),
            "inline-end" => boxp.margin_inline[1] = css::length(value),
            "block-start" => {
                if let Some(l) = css::length(value) {
                    boxp.margin[0] = l;
                }
            }
            "block-end" => {
                if let Some(l) = css::length(value) {
                    boxp.margin[2] = l;
                }
            }
            side => {
                if let (Some(i), Some(l)) = (side_index(side), css::length(value)) {
                    boxp.margin[i] = l;
                    if i == 1 || i == 3 {
                        boxp.margin_inline = [None; 2];
                    }
                }
            }
        }
        return;
    }
    if let Some(rest) = property.strip_prefix("padding-") {
        match rest {
            "inline-start" => boxp.padding_inline[0] = css::length(value),
            "inline-end" => boxp.padding_inline[1] = css::length(value),
            "block-start" => {
                if let Some(l) = css::length(value) {
                    boxp.padding[0] = l;
                }
            }
            "block-end" => {
                if let Some(l) = css::length(value) {
                    boxp.padding[2] = l;
                }
            }
            side => {
                if let (Some(i), Some(l)) = (side_index(side), css::length(value)) {
                    boxp.padding[i] = l;
                    if i == 1 || i == 3 {
                        boxp.padding_inline = [None; 2];
                    }
                }
            }
        }
        return;
    }
    if let Some(rest) = property.strip_prefix("border-") {
        let (side, part) = match rest.split_once('-') {
            Some((side, part)) => (side, Some(part)),
            None => (rest, None),
        };
        if side == "inline" && part == Some("start") {
            boxp.border_inline_start = Some(border_shorthand(value));
            return;
        }
        let Some(i) = side_index(side) else { return };
        match part {
            None => boxp.border[i] = border_shorthand(value),
            Some("width") => {
                if let Some(w) = border_width(value) {
                    boxp.border[i].width = w;
                }
            }
            Some("style") => {
                if let Some(s) = BorderStyle::parse(value) {
                    boxp.border[i].style = s;
                }
            }
            Some("color") => boxp.border[i].color = css::color(value),
            _ => {}
        }
    }
}

pub fn apply_format(text: &mut TextProps, boxp: &mut BoxProps, format: TextFormat) {
    if format.contains(TextFormat::BOLD) {
        text.weight = text.weight.max(700);
    }
    if format.contains(TextFormat::ITALIC) {
        text.italic = true;
    }
    if format.contains(TextFormat::UNDERLINE) {
        boxp.decoration.underline = true;
    }
    if format.contains(TextFormat::STRIKETHROUGH) {
        boxp.decoration.strike = true;
    }
    if format.contains(TextFormat::SUBSCRIPT) {
        text.size /= 1.2;
        boxp.vertical_align = VerticalAlign::Sub;
    }
    if format.contains(TextFormat::SUPERSCRIPT) {
        text.size /= 1.2;
        boxp.vertical_align = VerticalAlign::Super;
    }
    if format.contains(TextFormat::HIGHLIGHT) {
        boxp.background = Some(Rgba::rgb(0xffff00));
    }
}

pub const SUPPORTED_PROPERTIES: &[&str] = &[
    "font-family",
    "font-size",
    "font-weight",
    "font-style",
    "color",
    "background-color",
    "background",
    "line-height",
    "letter-spacing",
    "word-spacing",
    "white-space",
    "text-decoration",
    "text-decoration-line",
    "text-decoration-color",
    "text-decoration-style",
    "vertical-align",
    "direction",
    "unicode-bidi",
    "text-align",
    "text-indent",
    "orphans",
    "widows",
    "border-collapse",
    "table-layout",
    "width",
    "min-width",
    "max-width",
    "height",
    "min-height",
    "margin",
    "padding",
    "border",
    "border-width",
    "border-style",
    "border-color",
    "break-before",
    "break-after",
    "break-inside",
    "page-break-before",
    "page-break-after",
    "page-break-inside",
];
