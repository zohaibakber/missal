use std::collections::HashMap;
use std::fmt;

use super::{
    Align, Block, Direction, Document, ElementProps, Field, Heading, HeadingLevel, Inline, Leaf,
    Link, List, ListItem, ListItemChild, ListTag, ListType, PageLayout, Paragraph, Quote, Style,
    Table, TableCell, TableRow, Text, TextFormat,
};
use crate::field::{FieldReference, FirField};
use crate::fir::FirProperty;
use crate::ids::PlaceholderId;

const MAGIC: &[u8; 4] = b"MSD\x01";
const MAX_DEPTH: u32 = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentError {
    NotADocument,
    Truncated,
    Corrupt(&'static str),
}

impl fmt::Display for DocumentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentError::NotADocument => f.write_str("not a Missal document"),
            DocumentError::Truncated => f.write_str("document data is truncated"),
            DocumentError::Corrupt(what) => write!(f, "document data is corrupt: {what}"),
        }
    }
}

impl std::error::Error for DocumentError {}

type Result<T> = std::result::Result<T, DocumentError>;

pub(super) fn encode(document: &Document) -> Vec<u8> {
    let mut body = Writer::default();
    match &document.page_layout {
        None => body.byte(0),
        Some(layout) => {
            body.byte(1);
            body.f64(layout.width_mm);
            body.f64(layout.height_mm);
            for margin in layout.margins() {
                body.f64(margin);
            }
        }
    }
    body.props(&document.root);
    body.blocks(&document.blocks);
    let mut out = Writer {
        bytes: Vec::with_capacity(body.bytes.len() + body.styles_len + 16),
        ..Writer::default()
    };
    out.bytes.extend_from_slice(MAGIC);
    out.uint(body.styles.len() as u64);
    for style in &body.styles {
        out.str(style.as_str());
    }
    out.bytes.extend_from_slice(&body.bytes);
    out.bytes
}

pub(super) fn decode(bytes: &[u8]) -> Result<Document> {
    let rest = bytes
        .strip_prefix(MAGIC.as_slice())
        .ok_or(DocumentError::NotADocument)?;
    let mut reader = Reader {
        bytes: rest,
        styles: Vec::new(),
        depth: 0,
    };
    let count = reader.len()?;
    let mut styles = Vec::with_capacity(count);
    for _ in 0..count {
        styles.push(Style::new(reader.str()?));
    }
    reader.styles = styles;
    let page_layout = match reader.byte()? {
        0 => None,
        1 => {
            let width = reader.f64()?;
            let height = reader.f64()?;
            let margins = [reader.f64()?, reader.f64()?, reader.f64()?, reader.f64()?];
            Some(
                PageLayout::new(width, height, margins)
                    .ok_or(DocumentError::Corrupt("page layout"))?,
            )
        }
        _ => return Err(DocumentError::Corrupt("page layout")),
    };
    let root = reader.props()?;
    let blocks = reader.blocks()?;
    if !reader.bytes.is_empty() {
        return Err(DocumentError::Corrupt("trailing data"));
    }
    Ok(Document {
        root,
        blocks,
        page_layout,
    })
}

#[derive(Default)]
struct Writer {
    bytes: Vec<u8>,
    styles: Vec<Style>,
    style_ids: HashMap<Style, u64>,
    styles_len: usize,
}

impl Writer {
    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn uint(&mut self, mut value: u64) {
        while value >= 0x80 {
            self.bytes.push(value as u8 | 0x80);
            value >>= 7;
        }
        self.bytes.push(value as u8);
    }

    fn int(&mut self, value: i64) {
        self.uint(((value << 1) ^ (value >> 63)) as u64);
    }

    fn f64(&mut self, value: f64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn str(&mut self, value: &str) {
        self.uint(value.len() as u64);
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn opt_uint(&mut self, value: Option<u64>) {
        self.uint(value.map_or(0, |value| value + 1));
    }

    fn opt_int(&mut self, value: Option<i64>) {
        match value {
            None => self.byte(0),
            Some(value) => {
                self.byte(1);
                self.int(value);
            }
        }
    }

    fn opt_f64(&mut self, value: Option<f64>) {
        match value {
            None => self.byte(0),
            Some(value) => {
                self.byte(1);
                self.f64(value);
            }
        }
    }

    fn opt_bool(&mut self, value: Option<bool>) {
        self.byte(match value {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        });
    }

    fn opt_str(&mut self, value: Option<&str>) {
        match value {
            None => self.uint(0),
            Some(value) => {
                self.uint(value.len() as u64 + 1);
                self.bytes.extend_from_slice(value.as_bytes());
            }
        }
    }

    fn style(&mut self, style: Option<&Style>) {
        let Some(style) = style.filter(|style| !style.is_empty()) else {
            self.uint(0);
            return;
        };
        let next = self.styles.len() as u64;
        let id = *self.style_ids.entry(style.clone()).or_insert_with(|| {
            self.styles_len += style.as_str().len() + 2;
            self.styles.push(style.clone());
            next
        });
        self.uint(id + 1);
    }

    fn props(&mut self, props: &ElementProps) {
        self.byte(match props.direction {
            None => 0,
            Some(Direction::Ltr) => 1,
            Some(Direction::Rtl) => 2,
        });
        self.byte(match props.align {
            None => 0,
            Some(Align::Left) => 1,
            Some(Align::Start) => 2,
            Some(Align::Center) => 3,
            Some(Align::Right) => 4,
            Some(Align::End) => 5,
            Some(Align::Justify) => 6,
        });
        self.opt_uint(props.indent.map(u64::from));
        self.opt_uint(props.text_format.map(|format| u64::from(format.0)));
        self.style(props.text_style.as_ref());
        self.style(props.imported_style.as_ref());
    }

    fn blocks(&mut self, blocks: &[Block]) {
        self.uint(blocks.len() as u64);
        for block in blocks {
            self.block(block);
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Paragraph(paragraph) => {
                self.byte(0);
                self.props(&paragraph.props);
                self.inlines(&paragraph.children);
            }
            Block::Heading(heading) => {
                self.byte(1);
                self.props(&heading.props);
                self.byte(heading.level.map_or(0, |level| level as u8 + 1));
                self.inlines(&heading.children);
            }
            Block::Quote(quote) => {
                self.byte(2);
                self.props(&quote.props);
                self.inlines(&quote.children);
            }
            Block::List(list) => {
                self.byte(3);
                self.list(list);
            }
            Block::Table(table) => {
                self.byte(4);
                self.table(table);
            }
            Block::PageBreak(leaf) => {
                self.byte(5);
                self.style(leaf.imported_style.as_ref());
            }
            Block::HorizontalRule(leaf) => {
                self.byte(6);
                self.style(leaf.imported_style.as_ref());
            }
        }
    }

    fn inlines(&mut self, inlines: &[Inline]) {
        self.uint(inlines.len() as u64);
        for inline in inlines {
            self.inline(inline);
        }
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Text(text) => {
                self.byte(0);
                self.str(&text.text);
                self.uint(u64::from(text.format.0));
                self.style(text.style.as_ref());
                self.style(text.imported_style.as_ref());
            }
            Inline::LineBreak(leaf) => {
                self.byte(1);
                self.style(leaf.imported_style.as_ref());
            }
            Inline::Field(field) => {
                self.byte(2);
                self.reference(&field.reference);
                self.style(field.imported_style.as_ref());
            }
            Inline::Link(link) => {
                self.byte(3);
                self.props(&link.props);
                self.opt_str(link.url.as_deref());
                self.opt_str(link.rel.as_deref());
                self.opt_str(link.target.as_deref());
                self.opt_str(link.title.as_deref());
                self.inlines(&link.children);
            }
        }
    }

    fn reference(&mut self, reference: &FieldReference) {
        match reference {
            FieldReference::Catalog(id) => {
                self.byte(0);
                self.uint(id.get() as u64);
            }
            FieldReference::Fir(field) => {
                self.byte(1);
                self.byte(field.property as u8);
                self.opt_uint(field.index.map(u64::from));
            }
            FieldReference::Unresolved(text) => {
                self.byte(2);
                self.str(text);
            }
        }
    }

    fn list(&mut self, list: &List) {
        self.props(&list.props);
        self.byte(match list.list_type {
            None => 0,
            Some(ListType::Bullet) => 1,
            Some(ListType::Number) => 2,
            Some(ListType::Check) => 3,
        });
        self.opt_int(list.start);
        self.byte(match list.tag {
            None => 0,
            Some(ListTag::Ul) => 1,
            Some(ListTag::Ol) => 2,
        });
        self.uint(list.items.len() as u64);
        for item in &list.items {
            self.props(&item.props);
            self.opt_int(item.value);
            self.opt_bool(item.checked);
            self.uint(item.children.len() as u64);
            for child in &item.children {
                match child {
                    ListItemChild::Inline(inline) => {
                        self.byte(0);
                        self.inline(inline);
                    }
                    ListItemChild::List(list) => {
                        self.byte(1);
                        self.list(list);
                    }
                }
            }
        }
    }

    fn table(&mut self, table: &Table) {
        self.props(&table.props);
        match &table.col_widths {
            None => self.uint(0),
            Some(widths) => {
                self.uint(widths.len() as u64 + 1);
                for width in widths {
                    self.f64(*width);
                }
            }
        }
        self.opt_bool(table.row_striping);
        self.opt_uint(table.frozen_column_count.map(u64::from));
        self.opt_uint(table.frozen_row_count.map(u64::from));
        self.uint(table.rows.len() as u64);
        for row in &table.rows {
            self.props(&row.props);
            self.opt_f64(row.height);
            self.uint(row.cells.len() as u64);
            for cell in &row.cells {
                self.props(&cell.props);
                self.opt_uint(cell.col_span.map(u64::from));
                self.opt_uint(cell.row_span.map(u64::from));
                self.opt_uint(cell.header_state.map(u64::from));
                self.opt_f64(cell.width);
                self.opt_str(cell.background_color.as_deref());
                self.opt_str(cell.vertical_align.as_deref());
                self.blocks(&cell.blocks);
            }
        }
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    styles: Vec<Style>,
    depth: u32,
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8]> {
        if self.bytes.len() < count {
            return Err(DocumentError::Truncated);
        }
        let (head, tail) = self.bytes.split_at(count);
        self.bytes = tail;
        Ok(head)
    }

    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn uint(&mut self) -> Result<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let byte = self.byte()?;
            value |= u64::from(byte & 0x7F) << shift;
            if byte & 0x80 == 0 {
                return Ok(value);
            }
        }
        Err(DocumentError::Corrupt("varint"))
    }

    fn int(&mut self) -> Result<i64> {
        let value = self.uint()?;
        Ok((value >> 1) as i64 ^ -((value & 1) as i64))
    }

    fn u32(&mut self) -> Result<u32> {
        u32::try_from(self.uint()?).map_err(|_| DocumentError::Corrupt("integer"))
    }

    fn len(&mut self) -> Result<usize> {
        let len = usize::try_from(self.uint()?).map_err(|_| DocumentError::Truncated)?;
        if len > self.bytes.len() {
            return Err(DocumentError::Truncated);
        }
        Ok(len)
    }

    fn f64(&mut self) -> Result<f64> {
        let bytes: [u8; 8] = self.take(8)?.try_into().expect("eight bytes");
        Ok(f64::from_le_bytes(bytes))
    }

    fn utf8(&mut self, len: usize) -> Result<&'a str> {
        std::str::from_utf8(self.take(len)?).map_err(|_| DocumentError::Corrupt("text"))
    }

    fn str(&mut self) -> Result<&'a str> {
        let len = self.len()?;
        self.utf8(len)
    }

    fn string(&mut self) -> Result<String> {
        self.str().map(str::to_owned)
    }

    fn opt_uint(&mut self) -> Result<Option<u64>> {
        Ok(self.uint()?.checked_sub(1))
    }

    fn opt_u32(&mut self) -> Result<Option<u32>> {
        self.opt_uint()?
            .map(|value| u32::try_from(value).map_err(|_| DocumentError::Corrupt("integer")))
            .transpose()
    }

    fn opt_int(&mut self) -> Result<Option<i64>> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.int().map(Some),
            _ => Err(DocumentError::Corrupt("optional integer")),
        }
    }

    fn opt_f64(&mut self) -> Result<Option<f64>> {
        match self.byte()? {
            0 => Ok(None),
            1 => self.f64().map(Some),
            _ => Err(DocumentError::Corrupt("optional number")),
        }
    }

    fn opt_bool(&mut self) -> Result<Option<bool>> {
        match self.byte()? {
            0 => Ok(None),
            1 => Ok(Some(false)),
            2 => Ok(Some(true)),
            _ => Err(DocumentError::Corrupt("optional flag")),
        }
    }

    fn opt_string(&mut self) -> Result<Option<String>> {
        match self.uint()? {
            0 => Ok(None),
            len => {
                let len = usize::try_from(len - 1).map_err(|_| DocumentError::Truncated)?;
                self.utf8(len).map(|text| Some(text.to_owned()))
            }
        }
    }

    fn style(&mut self) -> Result<Option<Style>> {
        match self.uint()? {
            0 => Ok(None),
            id => self
                .styles
                .get(usize::try_from(id - 1).unwrap_or(usize::MAX))
                .cloned()
                .map(Some)
                .ok_or(DocumentError::Corrupt("style index")),
        }
    }

    fn count(&mut self) -> Result<usize> {
        let count = usize::try_from(self.uint()?).map_err(|_| DocumentError::Truncated)?;
        if count > self.bytes.len() {
            return Err(DocumentError::Truncated);
        }
        Ok(count)
    }

    fn nested<T>(&mut self, read: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(DocumentError::Corrupt("nesting"));
        }
        let value = read(self);
        self.depth -= 1;
        value
    }

    fn props(&mut self) -> Result<ElementProps> {
        let direction = match self.byte()? {
            0 => None,
            1 => Some(Direction::Ltr),
            2 => Some(Direction::Rtl),
            _ => return Err(DocumentError::Corrupt("direction")),
        };
        let align = match self.byte()? {
            0 => None,
            1 => Some(Align::Left),
            2 => Some(Align::Start),
            3 => Some(Align::Center),
            4 => Some(Align::Right),
            5 => Some(Align::End),
            6 => Some(Align::Justify),
            _ => return Err(DocumentError::Corrupt("alignment")),
        };
        Ok(ElementProps {
            direction,
            align,
            indent: self.opt_u32()?,
            text_format: self.opt_u32()?.map(TextFormat),
            text_style: self.style()?,
            imported_style: self.style()?,
        })
    }

    fn blocks(&mut self) -> Result<Vec<Block>> {
        let count = self.count()?;
        let mut blocks = Vec::with_capacity(count);
        for _ in 0..count {
            blocks.push(self.block()?);
        }
        Ok(blocks)
    }

    fn block(&mut self) -> Result<Block> {
        Ok(match self.byte()? {
            0 => Block::Paragraph(Paragraph {
                props: self.props()?,
                children: self.inlines()?,
            }),
            1 => {
                let props = self.props()?;
                let level = match self.byte()? {
                    0 => None,
                    1 => Some(HeadingLevel::H1),
                    2 => Some(HeadingLevel::H2),
                    3 => Some(HeadingLevel::H3),
                    4 => Some(HeadingLevel::H4),
                    5 => Some(HeadingLevel::H5),
                    6 => Some(HeadingLevel::H6),
                    _ => return Err(DocumentError::Corrupt("heading level")),
                };
                Block::Heading(Heading {
                    props,
                    level,
                    children: self.inlines()?,
                })
            }
            2 => Block::Quote(Quote {
                props: self.props()?,
                children: self.inlines()?,
            }),
            3 => Block::List(self.nested(Reader::list)?),
            4 => Block::Table(self.nested(Reader::table)?),
            5 => Block::PageBreak(Leaf {
                imported_style: self.style()?,
            }),
            6 => Block::HorizontalRule(Leaf {
                imported_style: self.style()?,
            }),
            _ => return Err(DocumentError::Corrupt("block kind")),
        })
    }

    fn inlines(&mut self) -> Result<Vec<Inline>> {
        let count = self.count()?;
        let mut inlines = Vec::with_capacity(count);
        for _ in 0..count {
            inlines.push(self.inline()?);
        }
        Ok(inlines)
    }

    fn inline(&mut self) -> Result<Inline> {
        Ok(match self.byte()? {
            0 => Inline::Text(Text {
                text: self.string()?,
                format: TextFormat(self.u32()?),
                style: self.style()?,
                imported_style: self.style()?,
            }),
            1 => Inline::LineBreak(Leaf {
                imported_style: self.style()?,
            }),
            2 => Inline::Field(Field {
                reference: self.reference()?,
                imported_style: self.style()?,
            }),
            3 => {
                let props = self.props()?;
                let url = self.opt_string()?;
                let rel = self.opt_string()?;
                let target = self.opt_string()?;
                let title = self.opt_string()?;
                let children = self.nested(Reader::inlines)?;
                Inline::Link(Link {
                    props,
                    url,
                    rel,
                    target,
                    title,
                    children,
                })
            }
            _ => return Err(DocumentError::Corrupt("inline kind")),
        })
    }

    fn reference(&mut self) -> Result<FieldReference> {
        Ok(match self.byte()? {
            0 => FieldReference::Catalog(
                i64::try_from(self.uint()?)
                    .ok()
                    .and_then(PlaceholderId::new)
                    .ok_or(DocumentError::Corrupt("field id"))?,
            ),
            1 => {
                let property = *FirProperty::ALL
                    .get(usize::from(self.byte()?))
                    .ok_or(DocumentError::Corrupt("field property"))?;
                let index = self.opt_u32()?;
                if index == Some(0) {
                    return Err(DocumentError::Corrupt("field index"));
                }
                FieldReference::Fir(FirField { property, index })
            }
            2 => FieldReference::Unresolved(self.string()?),
            _ => return Err(DocumentError::Corrupt("field kind")),
        })
    }

    fn list(&mut self) -> Result<List> {
        let props = self.props()?;
        let list_type = match self.byte()? {
            0 => None,
            1 => Some(ListType::Bullet),
            2 => Some(ListType::Number),
            3 => Some(ListType::Check),
            _ => return Err(DocumentError::Corrupt("list type")),
        };
        let start = self.opt_int()?;
        let tag = match self.byte()? {
            0 => None,
            1 => Some(ListTag::Ul),
            2 => Some(ListTag::Ol),
            _ => return Err(DocumentError::Corrupt("list tag")),
        };
        let count = self.count()?;
        let mut items = Vec::with_capacity(count);
        for _ in 0..count {
            let props = self.props()?;
            let value = self.opt_int()?;
            let checked = self.opt_bool()?;
            let count = self.count()?;
            let mut children = Vec::with_capacity(count);
            for _ in 0..count {
                children.push(match self.byte()? {
                    0 => ListItemChild::Inline(self.inline()?),
                    1 => ListItemChild::List(self.nested(Reader::list)?),
                    _ => return Err(DocumentError::Corrupt("list item child")),
                });
            }
            items.push(ListItem {
                props,
                value,
                checked,
                children,
            });
        }
        Ok(List {
            props,
            list_type,
            start,
            tag,
            items,
        })
    }

    fn table(&mut self) -> Result<Table> {
        let props = self.props()?;
        let col_widths = match self.uint()? {
            0 => None,
            count => {
                let count = usize::try_from(count - 1).map_err(|_| DocumentError::Truncated)?;
                if count > self.bytes.len() / 8 {
                    return Err(DocumentError::Truncated);
                }
                Some((0..count).map(|_| self.f64()).collect::<Result<Vec<_>>>()?)
            }
        };
        let row_striping = self.opt_bool()?;
        let frozen_column_count = self.opt_u32()?;
        let frozen_row_count = self.opt_u32()?;
        let count = self.count()?;
        let mut rows = Vec::with_capacity(count);
        for _ in 0..count {
            let props = self.props()?;
            let height = self.opt_f64()?;
            let count = self.count()?;
            let mut cells = Vec::with_capacity(count);
            for _ in 0..count {
                cells.push(TableCell {
                    props: self.props()?,
                    col_span: self.opt_u32()?,
                    row_span: self.opt_u32()?,
                    header_state: self.opt_u32()?,
                    width: self.opt_f64()?,
                    background_color: self.opt_string()?,
                    vertical_align: self.opt_string()?,
                    blocks: self.nested(Reader::blocks)?,
                });
            }
            rows.push(TableRow {
                props,
                height,
                cells,
            });
        }
        Ok(Table {
            props,
            col_widths,
            row_striping,
            frozen_column_count,
            frozen_row_count,
            rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Document {
        let style = Style::new("font-size: 12pt; color: red");
        let text = |value: &str| {
            Inline::Text(Text {
                text: value.into(),
                format: TextFormat::BOLD,
                style: Some(style.clone()),
                imported_style: Some(Style::new("font-family: x")),
            })
        };
        let mut document = Document::new(vec![
            Block::Paragraph(Paragraph {
                props: ElementProps {
                    direction: Some(Direction::Rtl),
                    align: Some(Align::Justify),
                    indent: Some(2),
                    text_format: Some(TextFormat::ITALIC),
                    text_style: Some(style.clone()),
                    imported_style: None,
                },
                children: vec![
                    text("سلام"),
                    Inline::LineBreak(Leaf::default()),
                    Inline::Field(Field::new(FieldReference::Fir(FirField {
                        property: FirProperty::Zimni,
                        index: Some(3),
                    }))),
                    Inline::Field(Field::new(FieldReference::Catalog(
                        PlaceholderId::new(9).unwrap(),
                    ))),
                    Inline::Field(Field::new(FieldReference::Unresolved("بینک".into()))),
                    Inline::Link(Link {
                        url: Some("https://example.com".into()),
                        children: vec![text("link")],
                        ..Link::default()
                    }),
                ],
            }),
            Block::Heading(Heading {
                props: ElementProps::rtl(),
                level: Some(HeadingLevel::H2),
                children: vec![text("عنوان")],
            }),
            Block::Quote(Quote::default()),
            Block::List(List {
                list_type: Some(ListType::Number),
                start: Some(-3),
                tag: Some(ListTag::Ol),
                items: vec![ListItem {
                    value: Some(3),
                    checked: Some(true),
                    children: vec![
                        ListItemChild::Inline(text("one")),
                        ListItemChild::List(List {
                            items: vec![ListItem::default()],
                            ..List::default()
                        }),
                    ],
                    ..ListItem::default()
                }],
                ..List::default()
            }),
            Block::Table(Table {
                col_widths: Some(vec![100.0, 50.5]),
                row_striping: Some(false),
                frozen_row_count: Some(1),
                rows: vec![TableRow {
                    height: Some(12.0),
                    cells: vec![TableCell {
                        col_span: Some(2),
                        header_state: Some(1),
                        width: Some(75.0),
                        background_color: Some("#fff".into()),
                        blocks: vec![Block::Paragraph(Paragraph::new(vec![text("cell")]))],
                        ..TableCell::default()
                    }],
                    ..TableRow::default()
                }],
                ..Table::default()
            }),
            Block::PageBreak(Leaf {
                imported_style: Some(style.clone()),
            }),
            Block::HorizontalRule(Leaf::default()),
        ]);
        document.page_layout = PageLayout::new(215.9, 279.4, [12.0, 10.0, 8.0, 6.5]);
        document
    }

    #[test]
    fn documents_round_trip_and_share_styles() {
        let document = sample();
        let bytes = document.encode();
        let decoded = Document::decode(&bytes).unwrap();
        assert_eq!(decoded, document);
        assert_eq!(decoded.encode(), bytes);
        let occurrences = bytes
            .windows(b"color: red".len())
            .filter(|window| *window == b"color: red")
            .count();
        assert_eq!(occurrences, 1);
    }

    #[test]
    fn damaged_data_is_rejected_without_panicking() {
        let bytes = sample().encode();
        assert_eq!(Document::decode(b"{}"), Err(DocumentError::NotADocument));
        for end in 4..bytes.len() {
            assert!(Document::decode(&bytes[..end]).is_err(), "prefix {end}");
        }
        let mut extended = bytes.clone();
        extended.push(0);
        assert!(Document::decode(&extended).is_err());
        for index in 4..bytes.len() {
            let mut flipped = bytes.clone();
            flipped[index] ^= 0xA5;
            let _ = Document::decode(&flipped);
        }
    }
}
