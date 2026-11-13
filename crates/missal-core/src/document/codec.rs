use std::collections::HashMap;

use serde_json::Value;

use super::envelope::{normalize_styles, number_value};
use super::{
    Align, Block, Child, Direction, Document, DocumentError, ElementProps, Envelope, Extra, Field,
    Heading, HeadingLevel, Image, Inline, Leaf, Link, List, ListItem, ListItemChild, ListTag,
    ListType, Paragraph, Quote, RawNode, Style, Table, TableCell, TableRow, Text, TextFormat,
    TextMode,
};
use crate::field::{FieldReference, json_int};

pub(super) fn decode(envelope: &Envelope) -> Result<Document, DocumentError> {
    let Value::Object(mut state) = envelope.expanded_state() else {
        return Err(DocumentError::Invalid("state is not an object"));
    };
    let Some(Value::Object(mut root)) = state.shift_remove("root") else {
        return Err(DocumentError::Invalid("state has no root"));
    };
    let Some(Value::Array(children)) = root.shift_remove("children") else {
        return Err(DocumentError::Invalid("root has no children"));
    };
    if root.get("type").and_then(Value::as_str) == Some("root") {
        root.shift_remove("type");
    }
    let mut decoder = Decoder::default();
    let root = decoder.props(&mut root);
    let blocks = children
        .into_iter()
        .map(|child| decoder.block(child))
        .collect();
    Ok(Document {
        root,
        blocks,
        page_layout: envelope.page_layout.clone(),
        state_extra: state,
        envelope_extra: envelope.extra.clone(),
    })
}

pub(super) fn encode(document: &Document) -> Envelope {
    let mut root = Extra::new();
    if !document.root.extra.contains_key("type") {
        root.insert("type".into(), Value::from("root"));
    }
    let children = document.blocks.iter().map(block_value).collect();
    let root = element(root, &document.root, children);
    let mut state = Extra::new();
    state.insert("root".into(), root);
    merge(&mut state, &document.state_extra);
    let (styles, state) = normalize_styles(Value::Object(state));
    Envelope {
        styles,
        state,
        page_layout: document.page_layout.clone(),
        extra: document.envelope_extra.clone(),
    }
}

#[derive(Default)]
struct Decoder {
    styles: HashMap<String, Style>,
}

fn take<T>(map: &mut Extra, key: &str, parse: impl FnOnce(&Value) -> Option<T>) -> Option<T> {
    let parsed = parse(map.get(key)?)?;
    map.shift_remove(key);
    Some(parsed)
}

fn string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn unsigned(value: &Value) -> Option<u32> {
    u32::try_from(json_int(value)?).ok()
}

fn number(value: &Value) -> Option<f64> {
    value.as_f64()
}

fn node_type(value: &Value) -> Option<&str> {
    value.get("type")?.as_str()
}

fn has_children(map: &Extra) -> bool {
    map.get("children").is_some_and(Value::is_array)
}

fn take_children(map: &mut Extra) -> Vec<Value> {
    match map.shift_remove("children") {
        Some(Value::Array(children)) => children,
        _ => Vec::new(),
    }
}

impl Decoder {
    fn style(&mut self, css: &str) -> Style {
        self.styles
            .entry(css.to_owned())
            .or_insert_with(|| Style::new(css))
            .clone()
    }

    fn take_style(&mut self, map: &mut Extra, key: &str) -> Option<Style> {
        let css = take(map, key, string)?;
        Some(self.style(&css))
    }

    fn imported_style(&mut self, map: &mut Extra) -> Option<Style> {
        let Some(Value::Object(state)) = map.get_mut("$") else {
            return None;
        };
        let css = take(state, "importedStyle", string)?;
        if state.is_empty() {
            map.shift_remove("$");
        }
        Some(self.style(&css))
    }

    fn props(&mut self, map: &mut Extra) -> ElementProps {
        let direction = take(map, "direction", |value| match value.as_str()? {
            "ltr" => Some(Direction::Ltr),
            "rtl" => Some(Direction::Rtl),
            _ => None,
        });
        let align = take(map, "format", |value| {
            Some(match value.as_str()? {
                "left" => Align::Left,
                "start" => Align::Start,
                "center" => Align::Center,
                "right" => Align::Right,
                "end" => Align::End,
                "justify" => Align::Justify,
                _ => return None,
            })
        });
        let indent = take(map, "indent", unsigned);
        let text_format = take(map, "textFormat", unsigned).map(TextFormat);
        let text_style = self.take_style(map, "textStyle");
        let imported_style = self.imported_style(map);
        ElementProps {
            direction,
            align,
            indent,
            text_format,
            text_style,
            imported_style,
            extra: std::mem::take(map),
        }
    }

    fn leaf(&mut self, mut map: Extra) -> Leaf {
        map.shift_remove("type");
        Leaf {
            imported_style: self.imported_style(&mut map),
            extra: map,
        }
    }

    fn image(&mut self, mut map: Extra) -> Image {
        map.shift_remove("type");
        Image {
            src: take(&mut map, "src", string),
            width: take(&mut map, "width", number),
            height: take(&mut map, "height", number),
            imported_style: self.imported_style(&mut map),
            extra: map,
        }
    }

    fn inlines(&mut self, children: Vec<Value>) -> Vec<Inline> {
        children
            .into_iter()
            .map(|child| self.inline(child))
            .collect()
    }

    fn blocks(&mut self, children: Vec<Value>) -> Vec<Block> {
        children
            .into_iter()
            .map(|child| self.block(child))
            .collect()
    }

    fn inline(&mut self, value: Value) -> Inline {
        let kind = node_type(&value).map(str::to_owned);
        let Value::Object(mut map) = value else {
            return Inline::Other(RawNode(value));
        };
        match kind.as_deref() {
            Some("text") if map.get("text").is_some_and(Value::is_string) => {
                map.shift_remove("type");
                Inline::Text(Text {
                    text: take(&mut map, "text", string).unwrap_or_default(),
                    format: take(&mut map, "format", unsigned).map(TextFormat),
                    style: self.take_style(&mut map, "style"),
                    detail: take(&mut map, "detail", unsigned),
                    mode: take(&mut map, "mode", |value| {
                        Some(match value.as_str()? {
                            "normal" => TextMode::Normal,
                            "token" => TextMode::Token,
                            "segmented" => TextMode::Segmented,
                            _ => return None,
                        })
                    }),
                    imported_style: self.imported_style(&mut map),
                    extra: map,
                })
            }
            Some("linebreak") => Inline::LineBreak(self.leaf(map)),
            Some("image") => Inline::Image(self.image(map)),
            Some("field") => Inline::Field(self.field(map)),
            Some("link") if has_children(&map) => {
                map.shift_remove("type");
                let children = take_children(&mut map);
                Inline::Link(Link {
                    url: take(&mut map, "url", string),
                    rel: take(&mut map, "rel", string),
                    target: take(&mut map, "target", string),
                    title: take(&mut map, "title", string),
                    props: self.props(&mut map),
                    children: self.inlines(children),
                })
            }
            _ => Inline::Other(RawNode(Value::Object(map))),
        }
    }

    fn field(&mut self, mut map: Extra) -> Field {
        map.shift_remove("type");
        let raw = map.get("reference");
        let reference_missing = raw.is_none();
        let decoded = raw.and_then(FieldReference::from_json);
        if let (Some(raw), Some(decoded)) = (raw, &decoded)
            && *raw == decoded.to_json()
        {
            map.shift_remove("reference");
        }
        Field {
            reference: decoded.unwrap_or(FieldReference::Unresolved(String::new())),
            imported_style: self.imported_style(&mut map),
            extra: map,
            reference_missing,
        }
    }

    fn block(&mut self, value: Value) -> Block {
        let kind = node_type(&value).map(str::to_owned);
        let Value::Object(mut map) = value else {
            return Block::Other(RawNode(value));
        };
        let element = has_children(&map);
        match kind.as_deref() {
            Some("pagebreak") => Block::PageBreak(self.leaf(map)),
            Some("horizontalrule") => Block::HorizontalRule(self.leaf(map)),
            Some("image") => Block::Image(self.image(map)),
            Some("paragraph") if element => {
                map.shift_remove("type");
                let children = take_children(&mut map);
                Block::Paragraph(Paragraph {
                    props: self.props(&mut map),
                    children: self.inlines(children),
                })
            }
            Some("quote") if element => {
                map.shift_remove("type");
                let children = take_children(&mut map);
                Block::Quote(Quote {
                    props: self.props(&mut map),
                    children: self.inlines(children),
                })
            }
            Some("heading") if element => {
                map.shift_remove("type");
                let children = take_children(&mut map);
                Block::Heading(Heading {
                    level: take(&mut map, "tag", |value| {
                        Some(match value.as_str()? {
                            "h1" => HeadingLevel::H1,
                            "h2" => HeadingLevel::H2,
                            "h3" => HeadingLevel::H3,
                            "h4" => HeadingLevel::H4,
                            "h5" => HeadingLevel::H5,
                            "h6" => HeadingLevel::H6,
                            _ => return None,
                        })
                    }),
                    props: self.props(&mut map),
                    children: self.inlines(children),
                })
            }
            Some("list") if element => Block::List(self.list(map)),
            Some("table") if element => {
                map.shift_remove("type");
                let children = take_children(&mut map);
                Block::Table(Table {
                    col_widths: take(&mut map, "colWidths", |value| {
                        value.as_array()?.iter().map(Value::as_f64).collect()
                    }),
                    row_striping: take(&mut map, "rowStriping", Value::as_bool),
                    frozen_column_count: take(&mut map, "frozenColumnCount", unsigned),
                    frozen_row_count: take(&mut map, "frozenRowCount", unsigned),
                    props: self.props(&mut map),
                    rows: children
                        .into_iter()
                        .map(|child| self.child(child, "tablerow", Decoder::row))
                        .collect(),
                })
            }
            _ => Block::Other(RawNode(Value::Object(map))),
        }
    }

    fn child<T>(
        &mut self,
        value: Value,
        kind: &str,
        decode: fn(&mut Decoder, Extra) -> T,
    ) -> Child<T> {
        match value {
            Value::Object(map)
                if map.get("type").and_then(Value::as_str) == Some(kind) && has_children(&map) =>
            {
                Child::Node(decode(self, map))
            }
            other => Child::Other(RawNode(other)),
        }
    }

    fn list(&mut self, mut map: Extra) -> List {
        map.shift_remove("type");
        let children = take_children(&mut map);
        List {
            list_type: take(&mut map, "listType", |value| {
                Some(match value.as_str()? {
                    "bullet" => ListType::Bullet,
                    "number" => ListType::Number,
                    "check" => ListType::Check,
                    _ => return None,
                })
            }),
            start: take(&mut map, "start", json_int),
            tag: take(&mut map, "tag", |value| match value.as_str()? {
                "ul" => Some(ListTag::Ul),
                "ol" => Some(ListTag::Ol),
                _ => None,
            }),
            props: self.props(&mut map),
            items: children
                .into_iter()
                .map(|child| self.child(child, "listitem", Decoder::list_item))
                .collect(),
        }
    }

    fn list_item(&mut self, mut map: Extra) -> ListItem {
        map.shift_remove("type");
        let children = take_children(&mut map);
        ListItem {
            value: take(&mut map, "value", json_int),
            checked: take(&mut map, "checked", Value::as_bool),
            props: self.props(&mut map),
            children: children
                .into_iter()
                .map(|child| match child {
                    Value::Object(map)
                        if map.get("type").and_then(Value::as_str) == Some("list")
                            && has_children(&map) =>
                    {
                        ListItemChild::List(self.list(map))
                    }
                    other => ListItemChild::Inline(self.inline(other)),
                })
                .collect(),
        }
    }

    fn row(&mut self, mut map: Extra) -> TableRow {
        map.shift_remove("type");
        let children = take_children(&mut map);
        TableRow {
            height: take(&mut map, "height", number),
            props: self.props(&mut map),
            cells: children
                .into_iter()
                .map(|child| self.child(child, "tablecell", Decoder::cell))
                .collect(),
        }
    }

    fn cell(&mut self, mut map: Extra) -> TableCell {
        map.shift_remove("type");
        let children = take_children(&mut map);
        TableCell {
            col_span: take(&mut map, "colSpan", unsigned),
            row_span: take(&mut map, "rowSpan", unsigned),
            header_state: take(&mut map, "headerState", unsigned),
            width: take(&mut map, "width", number),
            background_color: take(&mut map, "backgroundColor", string),
            vertical_align: take(&mut map, "verticalAlign", string),
            props: self.props(&mut map),
            blocks: self.blocks(children),
        }
    }
}

fn merge(target: &mut Extra, extra: &Extra) {
    for (key, value) in extra {
        target.entry(key.clone()).or_insert_with(|| value.clone());
    }
}

fn put<T>(map: &mut Extra, key: &str, value: Option<T>, encode: impl FnOnce(T) -> Value) {
    if let Some(value) = value {
        map.insert(key.to_owned(), encode(value));
    }
}

fn style_value(style: &Style) -> Value {
    Value::from(style.as_str())
}

fn put_imported_style(map: &mut Extra, style: &Option<Style>, extra: &Extra) {
    let Some(style) = style else { return };
    let mut state = match extra.get("$") {
        Some(Value::Object(state)) => state.clone(),
        _ => Extra::new(),
    };
    state.insert("importedStyle".into(), style_value(style));
    map.insert("$".into(), Value::Object(state));
}

fn typed(kind: &str) -> Extra {
    let mut map = Extra::new();
    map.insert("type".into(), Value::from(kind));
    map
}

fn element(mut map: Extra, props: &ElementProps, children: Vec<Value>) -> Value {
    put(&mut map, "direction", props.direction, |direction| {
        Value::from(match direction {
            Direction::Ltr => "ltr",
            Direction::Rtl => "rtl",
        })
    });
    put(&mut map, "format", props.align, |align| {
        Value::from(match align {
            Align::Left => "left",
            Align::Start => "start",
            Align::Center => "center",
            Align::Right => "right",
            Align::End => "end",
            Align::Justify => "justify",
        })
    });
    put(&mut map, "indent", props.indent, Value::from);
    put(&mut map, "textFormat", props.text_format, |format| {
        Value::from(format.0)
    });
    put(
        &mut map,
        "textStyle",
        props.text_style.as_ref(),
        style_value,
    );
    put_imported_style(&mut map, &props.imported_style, &props.extra);
    merge(&mut map, &props.extra);
    map.insert("children".into(), Value::Array(children));
    Value::Object(map)
}

fn leaf_value(kind: &str, leaf: &Leaf) -> Value {
    let mut map = typed(kind);
    put_imported_style(&mut map, &leaf.imported_style, &leaf.extra);
    merge(&mut map, &leaf.extra);
    Value::Object(map)
}

fn image_value(image: &Image) -> Value {
    let mut map = typed("image");
    put(&mut map, "src", image.src.as_deref(), Value::from);
    put(&mut map, "width", image.width, number_value);
    put(&mut map, "height", image.height, number_value);
    put_imported_style(&mut map, &image.imported_style, &image.extra);
    merge(&mut map, &image.extra);
    Value::Object(map)
}

fn field_value(field: &Field) -> Value {
    let mut map = typed("field");
    match field.extra.get("reference") {
        Some(raw) if field.raw_reference_applies(raw) => {}
        None if field.reference_is_missing() => {}
        _ => {
            map.insert("reference".into(), field.reference.to_json());
        }
    }
    put_imported_style(&mut map, &field.imported_style, &field.extra);
    merge(&mut map, &field.extra);
    Value::Object(map)
}

fn inline_value(inline: &Inline) -> Value {
    match inline {
        Inline::Text(text) => {
            let mut map = typed("text");
            map.insert("text".into(), Value::from(text.text.as_str()));
            put(&mut map, "format", text.format, |format| {
                Value::from(format.0)
            });
            put(&mut map, "style", text.style.as_ref(), style_value);
            put(&mut map, "detail", text.detail, Value::from);
            put(&mut map, "mode", text.mode, |mode| {
                Value::from(match mode {
                    TextMode::Normal => "normal",
                    TextMode::Token => "token",
                    TextMode::Segmented => "segmented",
                })
            });
            put_imported_style(&mut map, &text.imported_style, &text.extra);
            merge(&mut map, &text.extra);
            Value::Object(map)
        }
        Inline::LineBreak(leaf) => leaf_value("linebreak", leaf),
        Inline::Field(field) => field_value(field),
        Inline::Image(image) => image_value(image),
        Inline::Link(link) => {
            let mut map = typed("link");
            put(&mut map, "url", link.url.as_deref(), Value::from);
            put(&mut map, "rel", link.rel.as_deref(), Value::from);
            put(&mut map, "target", link.target.as_deref(), Value::from);
            put(&mut map, "title", link.title.as_deref(), Value::from);
            element(
                map,
                &link.props,
                link.children.iter().map(inline_value).collect(),
            )
        }
        Inline::Other(raw) => raw.0.clone(),
    }
}

fn child_value<T>(child: &Child<T>, encode: fn(&T) -> Value) -> Value {
    match child {
        Child::Node(node) => encode(node),
        Child::Other(raw) => raw.0.clone(),
    }
}

fn list_value(list: &List) -> Value {
    let mut map = typed("list");
    put(&mut map, "listType", list.list_type, |list_type| {
        Value::from(match list_type {
            ListType::Bullet => "bullet",
            ListType::Number => "number",
            ListType::Check => "check",
        })
    });
    put(&mut map, "start", list.start, Value::from);
    put(&mut map, "tag", list.tag, |tag| {
        Value::from(match tag {
            ListTag::Ul => "ul",
            ListTag::Ol => "ol",
        })
    });
    let items = list
        .items
        .iter()
        .map(|item| child_value(item, list_item_value))
        .collect();
    element(map, &list.props, items)
}

fn list_item_value(item: &ListItem) -> Value {
    let mut map = typed("listitem");
    put(&mut map, "value", item.value, Value::from);
    put(&mut map, "checked", item.checked, Value::from);
    let children = item
        .children
        .iter()
        .map(|child| match child {
            ListItemChild::Inline(inline) => inline_value(inline),
            ListItemChild::List(list) => list_value(list),
        })
        .collect();
    element(map, &item.props, children)
}

fn row_value(row: &TableRow) -> Value {
    let mut map = typed("tablerow");
    put(&mut map, "height", row.height, number_value);
    let cells = row
        .cells
        .iter()
        .map(|cell| child_value(cell, cell_value))
        .collect();
    element(map, &row.props, cells)
}

fn cell_value(cell: &TableCell) -> Value {
    let mut map = typed("tablecell");
    put(&mut map, "colSpan", cell.col_span, Value::from);
    put(&mut map, "rowSpan", cell.row_span, Value::from);
    put(&mut map, "headerState", cell.header_state, Value::from);
    put(&mut map, "width", cell.width, number_value);
    put(
        &mut map,
        "backgroundColor",
        cell.background_color.as_deref(),
        Value::from,
    );
    put(
        &mut map,
        "verticalAlign",
        cell.vertical_align.as_deref(),
        Value::from,
    );
    element(
        map,
        &cell.props,
        cell.blocks.iter().map(block_value).collect(),
    )
}

pub(super) fn block_value(block: &Block) -> Value {
    match block {
        Block::Paragraph(paragraph) => element(
            typed("paragraph"),
            &paragraph.props,
            paragraph.children.iter().map(inline_value).collect(),
        ),
        Block::Quote(quote) => element(
            typed("quote"),
            &quote.props,
            quote.children.iter().map(inline_value).collect(),
        ),
        Block::Heading(heading) => {
            let mut map = typed("heading");
            put(&mut map, "tag", heading.level, |level| {
                Value::from(match level {
                    HeadingLevel::H1 => "h1",
                    HeadingLevel::H2 => "h2",
                    HeadingLevel::H3 => "h3",
                    HeadingLevel::H4 => "h4",
                    HeadingLevel::H5 => "h5",
                    HeadingLevel::H6 => "h6",
                })
            });
            element(
                map,
                &heading.props,
                heading.children.iter().map(inline_value).collect(),
            )
        }
        Block::List(list) => list_value(list),
        Block::Table(table) => {
            let mut map = typed("table");
            put(&mut map, "colWidths", table.col_widths.as_ref(), |widths| {
                Value::Array(widths.iter().copied().map(number_value).collect())
            });
            put(&mut map, "rowStriping", table.row_striping, Value::from);
            put(
                &mut map,
                "frozenColumnCount",
                table.frozen_column_count,
                Value::from,
            );
            put(
                &mut map,
                "frozenRowCount",
                table.frozen_row_count,
                Value::from,
            );
            let rows = table
                .rows
                .iter()
                .map(|row| child_value(row, row_value))
                .collect();
            element(map, &table.props, rows)
        }
        Block::PageBreak(leaf) => leaf_value("pagebreak", leaf),
        Block::HorizontalRule(leaf) => leaf_value("horizontalrule", leaf),
        Block::Image(image) => image_value(image),
        Block::Other(raw) => raw.0.clone(),
    }
}
