mod codec;
mod project;

use std::fmt;
use std::sync::Arc;

use crate::field::FieldReference;

pub use codec::DocumentError;
pub use project::Projections;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Style(Arc<str>);

impl Style {
    pub fn new(css: &str) -> Style {
        Style(Arc::from(css))
    }

    pub fn non_empty(css: &str) -> Option<Style> {
        (!css.trim().is_empty()).then(|| Style::new(css))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn declarations(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.split(';').filter_map(|declaration| {
            let (property, value) = declaration.split_once(':')?;
            let property = property.trim();
            (!property.is_empty()).then(|| (property, value.trim()))
        })
    }

    pub fn get(&self, property: &str) -> Option<&str> {
        self.declarations()
            .filter(|(name, _)| name.eq_ignore_ascii_case(property))
            .last()
            .map(|(_, value)| value)
    }
}

impl fmt::Display for Style {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct TextFormat(pub u32);

impl TextFormat {
    pub const BOLD: TextFormat = TextFormat(1);
    pub const ITALIC: TextFormat = TextFormat(1 << 1);
    pub const STRIKETHROUGH: TextFormat = TextFormat(1 << 2);
    pub const UNDERLINE: TextFormat = TextFormat(1 << 3);
    pub const CODE: TextFormat = TextFormat(1 << 4);
    pub const SUBSCRIPT: TextFormat = TextFormat(1 << 5);
    pub const SUPERSCRIPT: TextFormat = TextFormat(1 << 6);
    pub const HIGHLIGHT: TextFormat = TextFormat(1 << 7);
    pub const LOWERCASE: TextFormat = TextFormat(1 << 8);
    pub const UPPERCASE: TextFormat = TextFormat(1 << 9);
    pub const CAPITALIZE: TextFormat = TextFormat(1 << 10);

    pub const fn contains(self, other: TextFormat) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn set(&mut self, other: TextFormat, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }

    pub fn toggled(self, other: TextFormat) -> TextFormat {
        TextFormat(self.0 ^ other.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Ltr,
    Rtl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Align {
    Left,
    Start,
    Center,
    Right,
    End,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeadingLevel {
    H1,
    H2,
    H3,
    H4,
    H5,
    H6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListType {
    Bullet,
    Number,
    Check,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ListTag {
    Ul,
    Ol,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ElementProps {
    pub direction: Option<Direction>,
    pub align: Option<Align>,
    pub indent: Option<u32>,
    pub text_format: Option<TextFormat>,
    pub text_style: Option<Style>,
    pub imported_style: Option<Style>,
}

impl ElementProps {
    pub fn rtl() -> ElementProps {
        ElementProps {
            direction: Some(Direction::Rtl),
            ..ElementProps::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Leaf {
    pub imported_style: Option<Style>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Text {
    pub text: String,
    pub format: TextFormat,
    pub style: Option<Style>,
    pub imported_style: Option<Style>,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Text {
        Text {
            text: text.into(),
            ..Text::default()
        }
    }

    pub fn format(&self) -> TextFormat {
        self.format
    }

    pub fn css(&self) -> Option<&Style> {
        self.style.as_ref().filter(|style| !style.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub reference: FieldReference,
    pub imported_style: Option<Style>,
}

impl Field {
    pub fn new(reference: FieldReference) -> Field {
        Field {
            reference,
            imported_style: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Link {
    pub props: ElementProps,
    pub url: Option<String>,
    pub rel: Option<String>,
    pub target: Option<String>,
    pub title: Option<String>,
    pub children: Vec<Inline>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(Text),
    LineBreak(Leaf),
    Field(Field),
    Link(Link),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Paragraph {
    pub props: ElementProps,
    pub children: Vec<Inline>,
}

impl Paragraph {
    pub fn new(children: Vec<Inline>) -> Paragraph {
        Paragraph {
            props: ElementProps::rtl(),
            children,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Heading {
    pub props: ElementProps,
    pub level: Option<HeadingLevel>,
    pub children: Vec<Inline>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Quote {
    pub props: ElementProps,
    pub children: Vec<Inline>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct List {
    pub props: ElementProps,
    pub list_type: Option<ListType>,
    pub start: Option<i64>,
    pub tag: Option<ListTag>,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListItemChild {
    Inline(Inline),
    List(List),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ListItem {
    pub props: ElementProps,
    pub value: Option<i64>,
    pub checked: Option<bool>,
    pub children: Vec<ListItemChild>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Table {
    pub props: ElementProps,
    pub col_widths: Option<Vec<f64>>,
    pub row_striping: Option<bool>,
    pub frozen_column_count: Option<u32>,
    pub frozen_row_count: Option<u32>,
    pub rows: Vec<TableRow>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRow {
    pub props: ElementProps,
    pub height: Option<f64>,
    pub cells: Vec<TableCell>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableCell {
    pub props: ElementProps,
    pub col_span: Option<u32>,
    pub row_span: Option<u32>,
    pub header_state: Option<u32>,
    pub width: Option<f64>,
    pub background_color: Option<String>,
    pub vertical_align: Option<String>,
    pub blocks: Vec<Block>,
}

impl TableCell {
    pub fn col_span(&self) -> u32 {
        self.col_span.unwrap_or(1)
    }

    pub fn row_span(&self) -> u32 {
        self.row_span.unwrap_or(1)
    }

    pub fn is_header(&self) -> bool {
        self.header_state.unwrap_or(0) != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Paragraph),
    Heading(Heading),
    Quote(Quote),
    List(List),
    Table(Table),
    PageBreak(Leaf),
    HorizontalRule(Leaf),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageLayout {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_top_mm: f64,
    pub margin_right_mm: f64,
    pub margin_bottom_mm: f64,
    pub margin_left_mm: f64,
}

impl PageLayout {
    pub fn new(width_mm: f64, height_mm: f64, margins_mm: [f64; 4]) -> Option<PageLayout> {
        let [top, right, bottom, left] = margins_mm;
        let valid = width_mm > 0.0 && height_mm > 0.0 && margins_mm.iter().all(|m| *m >= 0.0);
        valid.then_some(PageLayout {
            width_mm,
            height_mm,
            margin_top_mm: top,
            margin_right_mm: right,
            margin_bottom_mm: bottom,
            margin_left_mm: left,
        })
    }

    pub fn a4() -> PageLayout {
        PageLayout::new(210.0, 297.0, [18.0; 4]).expect("A4 is a valid layout")
    }

    pub(crate) fn margins(&self) -> [f64; 4] {
        [
            self.margin_top_mm,
            self.margin_right_mm,
            self.margin_bottom_mm,
            self.margin_left_mm,
        ]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub root: ElementProps,
    pub blocks: Vec<Block>,
    pub page_layout: Option<PageLayout>,
}

impl Document {
    pub fn empty() -> Document {
        Document::new(vec![Block::Paragraph(Paragraph::new(Vec::new()))])
    }

    pub fn new(blocks: Vec<Block>) -> Document {
        Document {
            root: ElementProps::rtl(),
            blocks,
            page_layout: None,
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Document, DocumentError> {
        codec::decode(bytes)
    }

    pub fn encode(&self) -> Vec<u8> {
        codec::encode(self)
    }

    pub fn projections(&self) -> Projections {
        project::project(self)
    }

    pub fn field_references(&self) -> Vec<FieldReference> {
        self.projections().field_references
    }

    pub fn bind_catalog_fields(
        &mut self,
        ids: &std::collections::HashMap<String, crate::ids::PlaceholderId>,
    ) {
        project::bind(self, ids);
    }

    pub fn unbind_catalog_fields(
        &mut self,
        label_of: &dyn Fn(crate::ids::PlaceholderId) -> Option<String>,
    ) {
        project::unbind(self, label_of);
    }

    pub fn for_each_field_mut(&mut self, visit: &mut dyn FnMut(&mut Field)) {
        project::for_each_field_mut(self, visit);
    }
}

impl Default for Document {
    fn default() -> Document {
        Document::empty()
    }
}
