mod codec;
mod envelope;
mod project;

use std::fmt;
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::field::FieldReference;

pub use envelope::{
    DOCUMENT_FORMAT, DOCUMENT_FORMAT_VERSION, DocumentError, Envelope, PageLayout, expand_styles,
    normalize_styles,
};
pub use project::{Projections, project_state};

pub type Extra = Map<String, Value>;

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Style(Arc<str>);

impl Style {
    pub fn new(css: &str) -> Style {
        Style(Arc::from(css))
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
pub enum TextMode {
    Normal,
    Token,
    Segmented,
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
    pub extra: Extra,
}

impl ElementProps {
    pub fn rtl() -> ElementProps {
        ElementProps {
            direction: Some(Direction::Rtl),
            ..ElementProps::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawNode(pub Value);

impl RawNode {
    pub fn node_type(&self) -> Option<&str> {
        self.0.get("type").and_then(Value::as_str)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Child<T> {
    Node(T),
    Other(RawNode),
}

impl<T> Child<T> {
    pub fn node(&self) -> Option<&T> {
        match self {
            Child::Node(node) => Some(node),
            Child::Other(_) => None,
        }
    }

    pub fn node_mut(&mut self) -> Option<&mut T> {
        match self {
            Child::Node(node) => Some(node),
            Child::Other(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Leaf {
    pub imported_style: Option<Style>,
    pub extra: Extra,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Text {
    pub text: String,
    pub format: Option<TextFormat>,
    pub style: Option<Style>,
    pub detail: Option<u32>,
    pub mode: Option<TextMode>,
    pub imported_style: Option<Style>,
    pub extra: Extra,
}

impl Text {
    pub fn new(text: impl Into<String>) -> Text {
        Text {
            text: text.into(),
            ..Text::default()
        }
    }

    pub fn format(&self) -> TextFormat {
        self.format.unwrap_or_default()
    }

    pub fn css(&self) -> Option<&Style> {
        self.style.as_ref().filter(|style| !style.is_empty())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub reference: FieldReference,
    pub imported_style: Option<Style>,
    pub extra: Extra,
    pub(crate) reference_missing: bool,
}

impl Field {
    pub fn new(reference: FieldReference) -> Field {
        Field {
            reference,
            imported_style: None,
            extra: Extra::new(),
            reference_missing: false,
        }
    }

    pub fn stored_reference(&self) -> Option<FieldReference> {
        match self.extra.get("reference") {
            Some(raw) if self.raw_reference_applies(raw) => FieldReference::from_json(raw),
            None if self.reference_is_missing() => None,
            _ => Some(self.reference.clone()),
        }
    }

    pub(crate) fn reference_is_missing(&self) -> bool {
        self.reference_missing && self.reference == FieldReference::Unresolved(String::new())
    }

    pub(crate) fn raw_reference_applies(&self, raw: &Value) -> bool {
        match FieldReference::from_json(raw) {
            Some(decoded) => decoded == self.reference,
            None => self.reference == FieldReference::Unresolved(String::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Image {
    pub src: Option<String>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub imported_style: Option<Style>,
    pub extra: Extra,
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
    Image(Image),
    Other(RawNode),
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
    pub items: Vec<Child<ListItem>>,
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
    pub rows: Vec<Child<TableRow>>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRow {
    pub props: ElementProps,
    pub height: Option<f64>,
    pub cells: Vec<Child<TableCell>>,
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
    Image(Image),
    Other(RawNode),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub root: ElementProps,
    pub blocks: Vec<Block>,
    pub page_layout: Option<PageLayout>,
    pub(crate) state_extra: Extra,
    pub(crate) envelope_extra: Extra,
}

impl Document {
    pub fn empty() -> Document {
        let state = serde_json::json!({
            "root": {
                "children": [{
                    "children": [],
                    "direction": "rtl",
                    "format": "",
                    "indent": 0,
                    "textFormat": 0,
                    "textStyle": "",
                    "type": "paragraph",
                    "version": 1
                }],
                "direction": "rtl",
                "format": "",
                "indent": 0,
                "type": "root",
                "version": 1
            }
        });
        Document::from_envelope(&Envelope {
            styles: Vec::new(),
            state,
            page_layout: None,
            extra: Extra::new(),
        })
        .expect("the empty document is valid")
    }

    pub fn new(blocks: Vec<Block>) -> Document {
        Document {
            root: ElementProps::rtl(),
            blocks,
            page_layout: None,
            state_extra: Extra::new(),
            envelope_extra: Extra::new(),
        }
    }

    pub fn decode(json: &str) -> Result<Document, DocumentError> {
        Document::from_envelope(&Envelope::parse(json)?)
    }

    pub fn encode(&self) -> String {
        self.to_envelope().to_json()
    }

    pub fn from_envelope(envelope: &Envelope) -> Result<Document, DocumentError> {
        codec::decode(envelope)
    }

    pub fn to_envelope(&self) -> Envelope {
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
