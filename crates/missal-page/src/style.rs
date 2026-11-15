use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::sync::Arc;

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Weight {
    Fixed(u16),
    Bolder,
    Lighter,
}

impl Weight {
    fn resolve(self, parent: u16) -> u16 {
        match self {
            Weight::Fixed(w) => w,
            Weight::Bolder => {
                if parent < 350 {
                    400
                } else if parent < 550 {
                    700
                } else {
                    900
                }
            }
            Weight::Lighter => {
                if parent < 550 {
                    100
                } else if parent < 750 {
                    400
                } else {
                    700
                }
            }
        }
    }
}

fn weight(value: &str) -> Option<Weight> {
    let value = css::clean(value).to_ascii_lowercase();
    Some(match value.as_str() {
        "normal" => Weight::Fixed(400),
        "bold" => Weight::Fixed(700),
        "bolder" => Weight::Bolder,
        "lighter" => Weight::Lighter,
        other => Weight::Fixed(other.parse::<f32>().ok()?.clamp(1.0, 1000.0) as u16),
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FontSize {
    Px(f32),
    Smaller,
    Larger,
    Percent(f32),
    Em(f32),
}

impl FontSize {
    fn resolve(self, parent: f32) -> f32 {
        match self {
            FontSize::Px(v) => v,
            FontSize::Smaller => parent / 1.2,
            FontSize::Larger => parent * 1.2,
            FontSize::Percent(p) => parent * p / 100.0,
            FontSize::Em(e) => parent * e,
        }
    }
}

fn font_size(value: &str) -> Option<FontSize> {
    let lower = css::clean(value).to_ascii_lowercase();
    let keyword = match lower.as_str() {
        "xx-small" => 9.0,
        "x-small" => 10.0,
        "small" => 13.0,
        "medium" => 16.0,
        "large" => 18.0,
        "x-large" => 24.0,
        "xx-large" => 32.0,
        "smaller" => return Some(FontSize::Smaller),
        "larger" => return Some(FontSize::Larger),
        _ => {
            return match css::length(&lower)? {
                Length::Auto => None,
                Length::Percent(p) => Some(FontSize::Percent(p)),
                Length::Em(e) => Some(FontSize::Em(e)),
                other => Some(FontSize::Px(other.resolve(0.0, 0.0))),
            };
        }
    };
    Some(FontSize::Px(keyword))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum LineHeightValue {
    Spec(LineHeightSpec),
    Percent(f32),
    Length(Length),
}

impl LineHeightValue {
    fn resolve(self, size: f32) -> LineHeightSpec {
        match self {
            LineHeightValue::Spec(spec) => spec,
            LineHeightValue::Percent(p) => LineHeightSpec::Px(size * p / 100.0),
            LineHeightValue::Length(l) => LineHeightSpec::Px(l.resolve(size, 0.0).max(0.0)),
        }
    }
}

fn line_height(value: &str) -> Option<LineHeightValue> {
    let lower = css::clean(value).to_ascii_lowercase();
    if lower == "normal" {
        return Some(LineHeightValue::Spec(LineHeightSpec::Normal));
    }
    if let Ok(factor) = lower.parse::<f32>() {
        return Some(LineHeightValue::Spec(LineHeightSpec::Factor(
            factor.max(0.0),
        )));
    }
    match css::length(&lower)? {
        Length::Auto => None,
        Length::Percent(p) => Some(LineHeightValue::Percent(p)),
        other => Some(LineHeightValue::Length(other)),
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

pub(crate) fn border_shorthand(value: &str) -> BorderSide {
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DecorationLine {
    underline: bool,
    overline: bool,
    strike: bool,
    dashed: Option<bool>,
    color: Option<Rgba>,
}

impl DecorationLine {
    fn apply(self, current: Decoration) -> Decoration {
        Decoration {
            underline: self.underline,
            overline: self.overline,
            strike: self.strike,
            dashed: self.dashed.unwrap_or(current.dashed),
            color: self.color.or(current.color),
        }
    }
}

fn decoration(value: &str) -> DecorationLine {
    let mut d = DecorationLine {
        underline: false,
        overline: false,
        strike: false,
        dashed: None,
        color: None,
    };
    for part in css::split_values(css::clean(value)) {
        match part.to_ascii_lowercase().as_str() {
            "none" => {}
            "underline" => d.underline = true,
            "overline" => d.overline = true,
            "line-through" => d.strike = true,
            "dashed" | "dotted" => d.dashed = Some(true),
            "solid" | "double" | "wavy" => d.dashed = Some(false),
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

fn absolute_width(value: &str) -> Option<f32> {
    let value = css::clean(value).to_ascii_lowercase();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let n: f32 = number.parse().ok()?;
    Some(n * css::unit_factor(unit)?)
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Op {
    FontSize(FontSize),
    Family(FontFamily),
    Weight(Weight),
    Italic(bool),
    Color(Rgba),
    Background(Option<Rgba>),
    LineHeight(LineHeightValue),
    LetterSpacing(Option<Length>),
    WordSpacing(Option<Length>),
    WhiteSpace(WhiteSpace),
    Decoration(DecorationLine),
    DecorationColor(Option<Rgba>),
    DecorationDashed(bool),
    VerticalAlign(VerticalAlign),
    Direction(Dir),
    UnicodeBidi(UnicodeBidi),
    Align(Align),
    Indent(Length),
    Orphans(u32),
    Widows(u32),
    BorderCollapse(bool),
    TableFixed(bool),
    Width(Option<Length>),
    MinWidth(Option<Length>),
    MaxWidth(Option<Length>),
    Height(Option<Length>),
    MinHeight(Option<Length>),
    Margin([Length; 4]),
    Padding([Length; 4]),
    MarginSide(usize, Length),
    PaddingSide(usize, Length),
    MarginInline(usize, Option<Length>),
    PaddingInline(usize, Option<Length>),
    Border(BorderSide),
    BorderWidth([f32; 4]),
    BorderStyle([BorderStyle; 4]),
    BorderColor([Rgba; 4]),
    BorderSide(usize, BorderSide),
    BorderSideWidth(usize, f32),
    BorderSideStyle(usize, BorderStyle),
    BorderSideColor(usize, Option<Rgba>),
    BorderInlineStart(BorderSide),
    BreakBefore(BreakValue),
    BreakAfter(BreakValue),
    BreakInside(BreakValue),
}

impl Op {
    pub(crate) fn parse(property: &str, value: &str) -> Option<Op> {
        if property.eq_ignore_ascii_case("font-size") {
            return font_size(value).map(Op::FontSize);
        }
        let property = property.trim().to_ascii_lowercase();
        let value = css::clean(value);
        let lower = value.to_ascii_lowercase();
        if lower.contains("var(") || lower == "inherit" || lower == "initial" || lower == "unset" {
            return None;
        }
        let len = || css::length(value);
        Some(match property.as_str() {
            "font-family" => Op::Family(fonts::family_for(&css::first_family(value)?)),
            "font-weight" => Op::Weight(weight(value)?),
            "font-style" => Op::Italic(matches!(lower.as_str(), "italic" | "oblique")),
            "color" => Op::Color(css::color(value)?),
            "background-color" | "background" => {
                if let Some(c) = css::split_values(value).into_iter().find_map(css::color) {
                    Op::Background((c.alpha() > 0).then_some(c))
                } else if lower == "none" || lower == "transparent" {
                    Op::Background(None)
                } else {
                    return None;
                }
            }
            "line-height" => Op::LineHeight(line_height(value)?),
            "letter-spacing" => Op::LetterSpacing(if lower == "normal" {
                None
            } else {
                Some(len()?)
            }),
            "word-spacing" => Op::WordSpacing(if lower == "normal" {
                None
            } else {
                Some(len()?)
            }),
            "white-space" => Op::WhiteSpace(match lower.as_str() {
                "normal" => WhiteSpace::Normal,
                "nowrap" => WhiteSpace::NoWrap,
                "pre" => WhiteSpace::Pre,
                "pre-line" => WhiteSpace::PreLine,
                "pre-wrap" | "break-spaces" => WhiteSpace::PreWrap,
                _ => return None,
            }),
            "text-decoration" | "text-decoration-line" => Op::Decoration(decoration(value)),
            "text-decoration-color" => Op::DecorationColor(css::color(value)),
            "text-decoration-style" => {
                Op::DecorationDashed(matches!(lower.as_str(), "dashed" | "dotted"))
            }
            "vertical-align" => Op::VerticalAlign(vertical_align(value)?),
            "direction" => Op::Direction(match lower.as_str() {
                "rtl" => Dir::Rtl,
                "ltr" => Dir::Ltr,
                _ => return None,
            }),
            "unicode-bidi" => Op::UnicodeBidi(match lower.as_str() {
                "embed" => UnicodeBidi::Embed,
                "isolate" | "-webkit-isolate" => UnicodeBidi::Isolate,
                "bidi-override" => UnicodeBidi::Override,
                "isolate-override" => UnicodeBidi::IsolateOverride,
                "plaintext" => UnicodeBidi::Plaintext,
                _ => UnicodeBidi::Normal,
            }),
            "text-align" => Op::Align(align(value)?),
            "text-indent" => Op::Indent(len()?),
            "orphans" => Op::Orphans(css::number(value)?.max(1.0) as u32),
            "widows" => Op::Widows(css::number(value)?.max(1.0) as u32),
            "border-collapse" => Op::BorderCollapse(lower == "collapse"),
            "table-layout" => Op::TableFixed(lower == "fixed"),
            "width" => Op::Width(len()),
            "min-width" => Op::MinWidth(len()),
            "max-width" => Op::MaxWidth(if lower == "none" { None } else { len() }),
            "height" => Op::Height(len()),
            "min-height" => Op::MinHeight(len()),
            "margin" => Op::Margin(css::box_values(value, css::length)?),
            "padding" => Op::Padding(css::box_values(value, css::length)?),
            "border" => Op::Border(border_shorthand(value)),
            "border-width" => Op::BorderWidth(css::box_values(value, border_width)?),
            "border-style" => Op::BorderStyle(css::box_values(value, BorderStyle::parse)?),
            "border-color" => Op::BorderColor(css::box_values(value, css::color)?),
            "break-before" | "page-break-before" => Op::BreakBefore(BreakValue::parse(value)?),
            "break-after" | "page-break-after" => Op::BreakAfter(BreakValue::parse(value)?),
            "break-inside" | "page-break-inside" => {
                let b = BreakValue::parse(value)?;
                Op::BreakInside(if b == BreakValue::Page {
                    BreakValue::Auto
                } else {
                    b
                })
            }
            other => side_op(other, value)?,
        })
    }

    fn apply(&self, parent: &TextProps, text: &mut TextProps, boxp: &mut BoxProps) {
        match self {
            Op::FontSize(_) => {}
            Op::Family(family) => text.family = family.clone(),
            Op::Weight(w) => text.weight = w.resolve(parent.weight),
            Op::Italic(italic) => text.italic = *italic,
            Op::Color(c) => text.color = *c,
            Op::Background(c) => boxp.background = *c,
            Op::LineHeight(lh) => text.line_height = lh.resolve(text.size),
            Op::LetterSpacing(l) => {
                text.letter_spacing = l.map_or(0.0, |l| l.resolve(text.size, 0.0))
            }
            Op::WordSpacing(l) => text.word_spacing = l.map_or(0.0, |l| l.resolve(text.size, 0.0)),
            Op::WhiteSpace(w) => text.white_space = *w,
            Op::Decoration(d) => boxp.decoration = d.apply(boxp.decoration),
            Op::DecorationColor(c) => boxp.decoration.color = *c,
            Op::DecorationDashed(dashed) => boxp.decoration.dashed = *dashed,
            Op::VerticalAlign(v) => boxp.vertical_align = *v,
            Op::Direction(d) => text.direction = *d,
            Op::UnicodeBidi(u) => boxp.unicode_bidi = *u,
            Op::Align(a) => text.align = *a,
            Op::Indent(l) => text.indent = *l,
            Op::Orphans(n) => text.orphans = *n,
            Op::Widows(n) => text.widows = *n,
            Op::BorderCollapse(b) => text.border_collapse = *b,
            Op::TableFixed(b) => boxp.table_fixed = *b,
            Op::Width(l) => boxp.width = *l,
            Op::MinWidth(l) => boxp.min_width = *l,
            Op::MaxWidth(l) => boxp.max_width = *l,
            Op::Height(l) => boxp.height = *l,
            Op::MinHeight(l) => boxp.min_height = *l,
            Op::Margin(m) => {
                boxp.margin = *m;
                boxp.margin_inline = [None; 2];
            }
            Op::Padding(p) => {
                boxp.padding = *p;
                boxp.padding_inline = [None; 2];
            }
            Op::MarginSide(i, l) => {
                boxp.margin[*i] = *l;
                if *i == 1 || *i == 3 {
                    boxp.margin_inline = [None; 2];
                }
            }
            Op::PaddingSide(i, l) => {
                boxp.padding[*i] = *l;
                if *i == 1 || *i == 3 {
                    boxp.padding_inline = [None; 2];
                }
            }
            Op::MarginInline(i, l) => boxp.margin_inline[*i] = *l,
            Op::PaddingInline(i, l) => boxp.padding_inline[*i] = *l,
            Op::Border(side) => {
                boxp.border = [*side; 4];
                boxp.border_inline_start = None;
            }
            Op::BorderWidth(w) => {
                for (side, w) in boxp.border.iter_mut().zip(w) {
                    side.width = *w;
                }
            }
            Op::BorderStyle(s) => {
                for (side, s) in boxp.border.iter_mut().zip(s) {
                    side.style = *s;
                }
            }
            Op::BorderColor(c) => {
                for (side, c) in boxp.border.iter_mut().zip(c) {
                    side.color = Some(*c);
                }
            }
            Op::BorderSide(i, side) => boxp.border[*i] = *side,
            Op::BorderSideWidth(i, w) => boxp.border[*i].width = *w,
            Op::BorderSideStyle(i, s) => boxp.border[*i].style = *s,
            Op::BorderSideColor(i, c) => boxp.border[*i].color = *c,
            Op::BorderInlineStart(side) => boxp.border_inline_start = Some(*side),
            Op::BreakBefore(b) => boxp.break_before = *b,
            Op::BreakAfter(b) => boxp.break_after = *b,
            Op::BreakInside(b) => boxp.break_inside = *b,
        }
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

fn side_op(property: &str, value: &str) -> Option<Op> {
    if let Some(rest) = property.strip_prefix("margin-") {
        return Some(match rest {
            "inline-start" => Op::MarginInline(0, css::length(value)),
            "inline-end" => Op::MarginInline(1, css::length(value)),
            "block-start" => Op::MarginSide(0, css::length(value)?),
            "block-end" => Op::MarginSide(2, css::length(value)?),
            side => Op::MarginSide(side_index(side)?, css::length(value)?),
        });
    }
    if let Some(rest) = property.strip_prefix("padding-") {
        return Some(match rest {
            "inline-start" => Op::PaddingInline(0, css::length(value)),
            "inline-end" => Op::PaddingInline(1, css::length(value)),
            "block-start" => Op::PaddingSide(0, css::length(value)?),
            "block-end" => Op::PaddingSide(2, css::length(value)?),
            side => Op::PaddingSide(side_index(side)?, css::length(value)?),
        });
    }
    let rest = property.strip_prefix("border-")?;
    let (side, part) = match rest.split_once('-') {
        Some((side, part)) => (side, Some(part)),
        None => (rest, None),
    };
    if side == "inline" && part == Some("start") {
        return Some(Op::BorderInlineStart(border_shorthand(value)));
    }
    let i = side_index(side)?;
    Some(match part {
        None => Op::BorderSide(i, border_shorthand(value)),
        Some("width") => Op::BorderSideWidth(i, border_width(value)?),
        Some("style") => Op::BorderSideStyle(i, BorderStyle::parse(value)?),
        Some("color") => Op::BorderSideColor(i, css::color(value)),
        _ => return None,
    })
}

#[derive(Debug, Default)]
pub(crate) struct Parsed {
    font_size: Option<FontSize>,
    ops: Vec<Op>,
    pub direction: bool,
    pub width: Option<f32>,
}

impl Parsed {
    fn new(style: &Style) -> Parsed {
        let mut parsed = Parsed::default();
        let mut width = None;
        for (property, value) in style.declarations() {
            if property.eq_ignore_ascii_case("direction") {
                parsed.direction = true;
            }
            if property.eq_ignore_ascii_case("width") {
                width = Some(value);
            }
            match Op::parse(property, value) {
                Some(Op::FontSize(size)) => parsed.font_size = Some(size),
                Some(op) => parsed.ops.push(op),
                None => {}
            }
        }
        parsed.width = width.and_then(absolute_width);
        parsed
    }
}

#[derive(Default)]
struct AddressHasher(u64);

impl Hasher for AddressHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ b as u64).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn write_usize(&mut self, n: usize) {
        let h = (n as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15);
        self.0 = h ^ (h >> 29);
    }
}

#[derive(Default)]
pub(crate) struct StyleCache {
    map: HashMap<usize, (Style, Arc<Parsed>), BuildHasherDefault<AddressHasher>>,
}

impl StyleCache {
    pub(crate) fn get(&mut self, style: &Style) -> Arc<Parsed> {
        let key = style.as_str().as_ptr() as usize;
        self.map
            .entry(key)
            .or_insert_with(|| (style.clone(), Arc::new(Parsed::new(style))))
            .1
            .clone()
    }

    pub(crate) fn parsed(&mut self, style: Option<&Style>) -> Option<Arc<Parsed>> {
        style.map(|s| self.get(s))
    }

    pub(crate) fn trim(&mut self) {
        if self.map.len() > 8192 {
            self.map.clear();
        }
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }
}

pub(crate) fn compute<'a>(
    parent: &TextProps,
    base: BoxProps,
    extra: impl Iterator<Item = &'a Op> + Clone,
    styles: &[Option<&Parsed>],
) -> Computed {
    let mut text = parent.clone();
    let mut boxp = base;
    let mut size = None;
    for op in extra.clone() {
        if let Op::FontSize(s) = op {
            size = Some(*s);
        }
    }
    for parsed in styles.iter().flatten() {
        if parsed.font_size.is_some() {
            size = parsed.font_size;
        }
    }
    if let Some(size) = size {
        text.size = size.resolve(parent.size).max(0.0);
    }
    for op in extra {
        op.apply(parent, &mut text, &mut boxp);
    }
    for parsed in styles.iter().flatten() {
        for op in &parsed.ops {
            op.apply(parent, &mut text, &mut boxp);
        }
    }
    Computed { text, boxp }
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
