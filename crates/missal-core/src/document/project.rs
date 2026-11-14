use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{Block, Child, Document, Extra, Field, Inline, ListItemChild, RawNode};
use crate::field::{
    FieldReference, ReferenceMap, bind_reference_json, field_lookup_key, fir_field_by_key,
    map_reference_entries, map_references, unbind_reference_json,
};
use crate::ids::PlaceholderId;
use crate::text::{collapse_whitespace, js_trim, utf16_prefix};

pub const PREVIEW_TEXT_LIMIT: usize = 280;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projections {
    pub plain_text: String,
    pub preview_text: String,
    pub field_references: Vec<FieldReference>,
    pub field_count: usize,
}

impl Projections {
    pub fn field_references_json(&self) -> String {
        Value::Array(
            self.field_references
                .iter()
                .map(FieldReference::to_json)
                .collect(),
        )
        .to_string()
    }
}

#[derive(Default)]
struct Collector {
    text: String,
    references: Vec<FieldReference>,
    seen: HashSet<String>,
}

impl Collector {
    fn reference(&mut self, reference: FieldReference) {
        if self.seen.insert(reference.dedupe_key()) {
            self.references.push(reference);
        }
    }

    fn finish(self) -> Projections {
        let plain_text = js_trim(&collapse_whitespace(&self.text)).to_owned();
        Projections {
            preview_text: utf16_prefix(&plain_text, PREVIEW_TEXT_LIMIT),
            plain_text,
            field_count: self.references.len(),
            field_references: self.references,
        }
    }

    fn value(&mut self, value: &Value) {
        let Value::Object(node) = value else { return };
        let kind = node.get("type").and_then(Value::as_str);
        if kind == Some("text")
            && let Some(Value::String(text)) = node.get("text")
        {
            self.text.push_str(text);
        }
        if kind == Some("linebreak") {
            self.text.push('\n');
        }
        if kind == Some("field")
            && let Some(reference) = node.get("reference").and_then(FieldReference::from_json)
        {
            self.reference(reference);
        }
        self.rest(node);
    }

    fn rest(&mut self, node: &Extra) {
        if let Some(Value::Array(children)) = node.get("children") {
            children.iter().for_each(|child| self.value(child));
        }
        if let Some(root) = node.get("root") {
            self.value(root);
        }
    }

    fn element(&mut self, extra: &Extra, children: impl FnOnce(&mut Collector)) {
        children(self);
        if let Some(root) = extra.get("root") {
            self.value(root);
        }
    }

    fn raw(&mut self, raw: &RawNode) {
        self.value(&raw.0);
    }

    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Text(text) => {
                self.text.push_str(&text.text);
                self.rest(&text.extra);
            }
            Inline::LineBreak(leaf) => {
                self.text.push('\n');
                self.rest(&leaf.extra);
            }
            Inline::Field(field) => {
                if let Some(reference) = field.stored_reference() {
                    self.reference(reference);
                }
                self.rest(&field.extra);
            }
            Inline::Image(image) => self.rest(&image.extra),
            Inline::Link(link) => self.element(&link.props.extra, |this| {
                link.children.iter().for_each(|child| this.inline(child));
            }),
            Inline::Other(raw) => self.raw(raw),
        }
    }

    fn child<T>(&mut self, child: &Child<T>, visit: impl FnOnce(&mut Collector, &T)) {
        match child {
            Child::Node(node) => visit(self, node),
            Child::Other(raw) => self.raw(raw),
        }
    }

    fn list(&mut self, list: &super::List) {
        self.element(&list.props.extra, |this| {
            for item in &list.items {
                this.child(item, |this, item| {
                    this.element(&item.props.extra, |this| {
                        for child in &item.children {
                            match child {
                                ListItemChild::Inline(inline) => this.inline(inline),
                                ListItemChild::List(list) => this.list(list),
                            }
                        }
                    });
                });
            }
        });
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Paragraph(paragraph) => self.element(&paragraph.props.extra, |this| {
                paragraph
                    .children
                    .iter()
                    .for_each(|child| this.inline(child));
            }),
            Block::Heading(heading) => self.element(&heading.props.extra, |this| {
                heading.children.iter().for_each(|child| this.inline(child));
            }),
            Block::Quote(quote) => self.element(&quote.props.extra, |this| {
                quote.children.iter().for_each(|child| this.inline(child));
            }),
            Block::List(list) => self.list(list),
            Block::Table(table) => self.element(&table.props.extra, |this| {
                for row in &table.rows {
                    this.child(row, |this, row| {
                        this.element(&row.props.extra, |this| {
                            for cell in &row.cells {
                                this.child(cell, |this, cell| {
                                    this.element(&cell.props.extra, |this| {
                                        cell.blocks.iter().for_each(|block| this.block(block));
                                    });
                                });
                            }
                        });
                    });
                }
            }),
            Block::PageBreak(leaf) | Block::HorizontalRule(leaf) => self.rest(&leaf.extra),
            Block::Image(image) => self.rest(&image.extra),
            Block::Other(raw) => self.raw(raw),
        }
    }
}

pub(super) fn project(document: &Document) -> Projections {
    let mut collector = Collector::default();
    if let Some(Value::Array(children)) = document.state_extra.get("children") {
        children.iter().for_each(|child| collector.value(child));
    }
    collector.element(&document.root.extra, |this| {
        document.blocks.iter().for_each(|block| this.block(block));
    });
    collector.finish()
}

pub fn project_state(state: &Value) -> Projections {
    let mut collector = Collector::default();
    collector.value(state);
    collector.finish()
}

struct Walker<'a> {
    field: &'a mut dyn FnMut(&mut Field),
    references: &'a mut ReferenceMap<'a>,
}

impl Walker<'_> {
    fn extra(&mut self, extra: &mut Extra, skip_reference: bool) {
        if skip_reference {
            for (key, value) in extra.iter_mut() {
                if key != "reference" {
                    map_references(value, self.references);
                }
            }
        } else {
            map_reference_entries(extra, self.references);
        }
    }

    fn raw(&mut self, raw: &mut RawNode) {
        map_references(&mut raw.0, self.references);
    }

    fn inline(&mut self, inline: &mut Inline) {
        match inline {
            Inline::Text(text) => self.extra(&mut text.extra, false),
            Inline::LineBreak(leaf) => self.extra(&mut leaf.extra, false),
            Inline::Field(field) => {
                (self.field)(field);
                self.extra(&mut field.extra, true);
            }
            Inline::Image(image) => self.extra(&mut image.extra, false),
            Inline::Link(link) => {
                self.extra(&mut link.props.extra, false);
                link.children
                    .iter_mut()
                    .for_each(|child| self.inline(child));
            }
            Inline::Other(raw) => self.raw(raw),
        }
    }

    fn child<T>(&mut self, child: &mut Child<T>, visit: impl FnOnce(&mut Self, &mut T)) {
        match child {
            Child::Node(node) => visit(self, node),
            Child::Other(raw) => self.raw(raw),
        }
    }

    fn list(&mut self, list: &mut super::List) {
        self.extra(&mut list.props.extra, false);
        for item in &mut list.items {
            self.child(item, |this, item| {
                this.extra(&mut item.props.extra, false);
                for child in &mut item.children {
                    match child {
                        ListItemChild::Inline(inline) => this.inline(inline),
                        ListItemChild::List(list) => this.list(list),
                    }
                }
            });
        }
    }

    fn block(&mut self, block: &mut Block) {
        match block {
            Block::Paragraph(paragraph) => {
                self.extra(&mut paragraph.props.extra, false);
                paragraph
                    .children
                    .iter_mut()
                    .for_each(|child| self.inline(child));
            }
            Block::Heading(heading) => {
                self.extra(&mut heading.props.extra, false);
                heading
                    .children
                    .iter_mut()
                    .for_each(|child| self.inline(child));
            }
            Block::Quote(quote) => {
                self.extra(&mut quote.props.extra, false);
                quote
                    .children
                    .iter_mut()
                    .for_each(|child| self.inline(child));
            }
            Block::List(list) => self.list(list),
            Block::Table(table) => {
                self.extra(&mut table.props.extra, false);
                for row in &mut table.rows {
                    self.child(row, |this, row| {
                        this.extra(&mut row.props.extra, false);
                        for cell in &mut row.cells {
                            this.child(cell, |this, cell| {
                                this.extra(&mut cell.props.extra, false);
                                cell.blocks.iter_mut().for_each(|block| this.block(block));
                            });
                        }
                    });
                }
            }
            Block::PageBreak(leaf) | Block::HorizontalRule(leaf) => {
                self.extra(&mut leaf.extra, false)
            }
            Block::Image(image) => self.extra(&mut image.extra, false),
            Block::Other(raw) => self.raw(raw),
        }
    }

    fn document(&mut self, document: &mut Document) {
        self.extra(&mut document.state_extra, false);
        self.extra(&mut document.root.extra, false);
        document
            .blocks
            .iter_mut()
            .for_each(|block| self.block(block));
    }
}

fn walk<'a>(
    document: &mut Document,
    field: &'a mut dyn FnMut(&mut Field),
    references: &'a mut ReferenceMap<'a>,
) {
    Walker { field, references }.document(document);
}

pub(super) fn for_each_field_mut(document: &mut Document, visit: &mut dyn FnMut(&mut Field)) {
    walk(document, visit, &mut |_| None);
}

fn rewrite_field(field: &mut Field, replacement: Option<FieldReference>) {
    if let Some(reference) = replacement {
        field.reference = reference;
        field.extra.shift_remove("reference");
        field.reference_missing = false;
    }
}

fn applies_to(field: &Field) -> Option<&Extra> {
    match field.extra.get("reference") {
        Some(Value::Object(raw)) if field.raw_reference_applies(&Value::Object(raw.clone())) => {
            Some(raw)
        }
        _ => None,
    }
}

pub(super) fn bind(document: &mut Document, ids: &HashMap<String, PlaceholderId>) {
    walk(
        document,
        &mut |field| {
            let replacement = match applies_to(field) {
                Some(raw) => bind_reference_json(raw, ids)
                    .and_then(|value| FieldReference::from_json(&value)),
                None if field.reference_is_missing() => None,
                None => match &field.reference {
                    FieldReference::Unresolved(text) => {
                        let key = field_lookup_key(text);
                        fir_field_by_key(&key)
                            .map(FieldReference::Fir)
                            .or_else(|| ids.get(&key).copied().map(FieldReference::Catalog))
                    }
                    _ => None,
                },
            };
            rewrite_field(field, replacement);
        },
        &mut |reference| bind_reference_json(reference, ids),
    );
}

pub(super) fn unbind(document: &mut Document, label_of: &dyn Fn(PlaceholderId) -> Option<String>) {
    walk(
        document,
        &mut |field| {
            let replacement = match applies_to(field) {
                Some(raw) => unbind_reference_json(raw, label_of)
                    .and_then(|value| FieldReference::from_json(&value)),
                None if field.reference_is_missing() => None,
                None => match &field.reference {
                    FieldReference::Catalog(id) => label_of(*id)
                        .filter(|label| !label.is_empty())
                        .map(FieldReference::Unresolved),
                    _ => None,
                },
            };
            rewrite_field(field, replacement);
        },
        &mut |reference| unbind_reference_json(reference, label_of),
    );
}
