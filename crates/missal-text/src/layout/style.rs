use std::{
    hash::{Hash, Hasher},
    ops::Range,
    sync::Arc,
};

use crate::fonts::FontFamily;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rgba(pub u32);

impl Rgba {
    pub const BLACK: Rgba = Rgba(0x000000ff);
    pub const TRANSPARENT: Rgba = Rgba(0);

    pub const fn rgb(hex: u32) -> Self {
        Self((hex << 8) | 0xff)
    }

    pub fn from_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        let c = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        Self((c(r) << 24) | (c(g) << 16) | (c(b) << 8) | c(a))
    }

    pub fn to_f32(self) -> [f32; 4] {
        let c = |shift: u32| ((self.0 >> shift) & 0xff) as f32 / 255.0;
        [c(24), c(16), c(8), c(0)]
    }

    pub fn alpha(self) -> u8 {
        (self.0 & 0xff) as u8
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TextStyle {
    pub family: FontFamily,
    pub size: f32,
    pub weight: u16,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    pub color: Rgba,
    pub background: Option<Rgba>,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            family: FontFamily::Interface,
            size: 14.0,
            weight: 400,
            italic: false,
            underline: false,
            strikethrough: false,
            color: Rgba::BLACK,
            background: None,
        }
    }
}

impl TextStyle {
    pub fn new(family: FontFamily, size: f32) -> Self {
        Self {
            family,
            size,
            ..Self::default()
        }
    }

    pub fn document(size: f32) -> Self {
        Self::new(FontFamily::Document, size)
    }

    pub fn interface(size: f32) -> Self {
        Self::new(FontFamily::Interface, size)
    }

    pub fn with_color(mut self, color: Rgba) -> Self {
        self.color = color;
        self
    }

    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }
}

impl Eq for TextStyle {}

impl Hash for TextStyle {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.family.hash(state);
        self.size.to_bits().hash(state);
        self.weight.hash(state);
        self.italic.hash(state);
        self.underline.hash(state);
        self.strikethrough.hash(state);
        self.color.hash(state);
        self.background.hash(state);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Attr {
    Family(FontFamily),
    Size(f32),
    Weight(u16),
    Italic(bool),
    Underline(bool),
    Strikethrough(bool),
    Color(Rgba),
    Background(Rgba),
}

impl Eq for Attr {}

impl Hash for Attr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Attr::Family(family) => family.hash(state),
            Attr::Size(size) => size.to_bits().hash(state),
            Attr::Weight(weight) => weight.hash(state),
            Attr::Italic(v) | Attr::Underline(v) | Attr::Strikethrough(v) => v.hash(state),
            Attr::Color(color) | Attr::Background(color) => color.hash(state),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct InlineObject {
    pub id: u64,
    pub index: usize,
    pub width: f32,
    pub height: f32,
    pub baseline: Option<f32>,
}

impl Eq for InlineObject {}

impl Hash for InlineObject {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
        self.index.hash(state);
        self.width.to_bits().hash(state);
        self.height.to_bits().hash(state);
        self.baseline.map(f32::to_bits).hash(state);
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttributedText {
    pub text: Arc<str>,
    pub style: TextStyle,
    pub spans: Vec<(Range<usize>, Attr)>,
    pub objects: Vec<InlineObject>,
}

impl AttributedText {
    pub fn new(text: impl Into<Arc<str>>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
            spans: Vec::new(),
            objects: Vec::new(),
        }
    }

    pub fn with(mut self, range: Range<usize>, attr: Attr) -> Self {
        self.push(range, attr);
        self
    }

    pub fn push(&mut self, range: Range<usize>, attr: Attr) {
        let len = self.text.len();
        let start = range.start.min(len);
        let end = range.end.clamp(start, len);
        self.spans.push((start..end, attr));
    }

    pub fn with_object(mut self, object: InlineObject) -> Self {
        self.objects.push(object);
        self
    }

    pub(crate) fn background_at(&self, index: usize) -> Option<Rgba> {
        let mut background = self.style.background;
        for (range, attr) in &self.spans {
            if let Attr::Background(color) = attr
                && range.contains(&index)
            {
                background = Some(*color);
            }
        }
        background
    }

    pub(crate) fn has_backgrounds(&self) -> bool {
        self.style.background.is_some()
            || self
                .spans
                .iter()
                .any(|(_, attr)| matches!(attr, Attr::Background(_)))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    Ltr,
    Rtl,
    #[default]
    Auto,
    AutoLtr,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Align {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub enum LineHeight {
    #[default]
    Normal,
    Relative(f32),
    Px(f32),
}

impl Eq for LineHeight {}

impl Hash for LineHeight {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            LineHeight::Normal => {}
            LineHeight::Relative(v) | LineHeight::Px(v) => v.to_bits().hash(state),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParagraphStyle {
    pub direction: Direction,
    pub align: Align,
    pub line_height: LineHeight,
    pub wrap: bool,
}

impl Default for ParagraphStyle {
    fn default() -> Self {
        Self {
            direction: Direction::Auto,
            align: Align::Start,
            line_height: LineHeight::Normal,
            wrap: true,
        }
    }
}

impl ParagraphStyle {
    pub fn rtl() -> Self {
        Self {
            direction: Direction::Rtl,
            ..Self::default()
        }
    }

    pub fn ltr() -> Self {
        Self {
            direction: Direction::Ltr,
            ..Self::default()
        }
    }

    pub fn with_align(mut self, align: Align) -> Self {
        self.align = align;
        self
    }

    pub fn with_line_height(mut self, line_height: LineHeight) -> Self {
        self.line_height = line_height;
        self
    }

    pub fn with_wrap(mut self, wrap: bool) -> Self {
        self.wrap = wrap;
        self
    }

    pub fn with_direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }
}

pub fn first_strong_is_rtl(text: &str) -> Option<bool> {
    text.chars().find_map(strong_direction)
}

pub(crate) fn strong_direction(c: char) -> Option<bool> {
    let cp = c as u32;
    let rtl = matches!(cp,
        0x0590..=0x05FF | 0x0600..=0x0608 | 0x060B | 0x060D | 0x061B..=0x064A | 0x066D..=0x066F
        | 0x0671..=0x06D5 | 0x06E5..=0x06E6 | 0x06EE..=0x06EF | 0x06FA..=0x0710 | 0x0712..=0x072F
        | 0x074D..=0x07A5 | 0x07B1..=0x07EA | 0x0800..=0x0815 | 0x0840..=0x0858 | 0x0860..=0x08C9
        | 0x200F | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFE | 0x10800..=0x10FFF | 0x1E800..=0x1EFFF);
    if rtl {
        Some(true)
    } else if c == '\u{200E}' || (c.is_alphabetic() && !(0x0600..=0x08FF).contains(&cp)) {
        Some(false)
    } else {
        None
    }
}

impl Direction {
    pub fn resolve_rtl(self, text: &str) -> bool {
        match self {
            Direction::Ltr => false,
            Direction::Rtl => true,
            Direction::Auto => first_strong_is_rtl(text).unwrap_or(true),
            Direction::AutoLtr => first_strong_is_rtl(text).unwrap_or(false),
        }
    }
}
