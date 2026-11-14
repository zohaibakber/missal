use std::collections::{HashMap, HashSet};

use super::{Block, Document, Field, Inline, List, ListItemChild};
use crate::field::{FieldReference, field_lookup_key, fir_field_by_key};
use crate::ids::PlaceholderId;

pub const PREVIEW_TEXT_LIMIT: usize = 280;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Projections {
    pub plain_text: String,
    pub preview_text: String,
    pub field_references: Vec<FieldReference>,
    pub field_count: usize,
}

#[derive(Default)]
struct Collector {
    text: String,
    references: Vec<FieldReference>,
    seen: HashSet<FieldReference>,
}

impl Collector {
    fn inline(&mut self, inline: &Inline) {
        match inline {
            Inline::Text(text) => self.text.push_str(&text.text),
            Inline::LineBreak(_) => self.text.push('\n'),
            Inline::Field(field) => {
                if self.seen.insert(field.reference.clone()) {
                    self.references.push(field.reference.clone());
                }
            }
            Inline::Link(link) => link.children.iter().for_each(|child| self.inline(child)),
        }
    }

    fn list(&mut self, list: &List) {
        for item in &list.items {
            for child in &item.children {
                match child {
                    ListItemChild::Inline(inline) => self.inline(inline),
                    ListItemChild::List(list) => self.list(list),
                }
            }
        }
    }

    fn block(&mut self, block: &Block) {
        match block {
            Block::Paragraph(paragraph) => paragraph.children.iter().for_each(|c| self.inline(c)),
            Block::Heading(heading) => heading.children.iter().for_each(|c| self.inline(c)),
            Block::Quote(quote) => quote.children.iter().for_each(|c| self.inline(c)),
            Block::List(list) => self.list(list),
            Block::Table(table) => {
                for cell in table.rows.iter().flat_map(|row| &row.cells) {
                    cell.blocks.iter().for_each(|block| self.block(block));
                }
            }
            Block::PageBreak(_) | Block::HorizontalRule(_) => {}
        }
    }

    fn finish(self) -> Projections {
        let plain_text = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let preview_text = match plain_text.char_indices().nth(PREVIEW_TEXT_LIMIT) {
            Some((end, _)) => plain_text[..end].to_owned(),
            None => plain_text.clone(),
        };
        Projections {
            preview_text,
            plain_text,
            field_count: self.references.len(),
            field_references: self.references,
        }
    }
}

pub(super) fn project(document: &Document) -> Projections {
    let mut collector = Collector::default();
    document
        .blocks
        .iter()
        .for_each(|block| collector.block(block));
    collector.finish()
}

fn inline_fields(inline: &mut Inline, visit: &mut dyn FnMut(&mut Field)) {
    match inline {
        Inline::Field(field) => visit(field),
        Inline::Link(link) => link
            .children
            .iter_mut()
            .for_each(|child| inline_fields(child, visit)),
        Inline::Text(_) | Inline::LineBreak(_) => {}
    }
}

fn list_fields(list: &mut List, visit: &mut dyn FnMut(&mut Field)) {
    for child in list.items.iter_mut().flat_map(|item| &mut item.children) {
        match child {
            ListItemChild::Inline(inline) => inline_fields(inline, visit),
            ListItemChild::List(list) => list_fields(list, visit),
        }
    }
}

fn block_fields(block: &mut Block, visit: &mut dyn FnMut(&mut Field)) {
    match block {
        Block::Paragraph(paragraph) => paragraph
            .children
            .iter_mut()
            .for_each(|c| inline_fields(c, visit)),
        Block::Heading(heading) => heading
            .children
            .iter_mut()
            .for_each(|c| inline_fields(c, visit)),
        Block::Quote(quote) => quote
            .children
            .iter_mut()
            .for_each(|c| inline_fields(c, visit)),
        Block::List(list) => list_fields(list, visit),
        Block::Table(table) => {
            for cell in table.rows.iter_mut().flat_map(|row| &mut row.cells) {
                cell.blocks
                    .iter_mut()
                    .for_each(|block| block_fields(block, visit));
            }
        }
        Block::PageBreak(_) | Block::HorizontalRule(_) => {}
    }
}

pub(super) fn for_each_field_mut(document: &mut Document, visit: &mut dyn FnMut(&mut Field)) {
    document
        .blocks
        .iter_mut()
        .for_each(|block| block_fields(block, visit));
}

pub(super) fn bind(document: &mut Document, ids: &HashMap<String, PlaceholderId>) {
    for_each_field_mut(document, &mut |field| {
        if let FieldReference::Unresolved(text) = &field.reference {
            let key = field_lookup_key(text);
            if let Some(reference) = fir_field_by_key(&key)
                .map(FieldReference::Fir)
                .or_else(|| ids.get(&key).copied().map(FieldReference::Catalog))
            {
                field.reference = reference;
            }
        }
    });
}

pub(super) fn unbind(document: &mut Document, label_of: &dyn Fn(PlaceholderId) -> Option<String>) {
    for_each_field_mut(document, &mut |field| {
        if let FieldReference::Catalog(id) = field.reference
            && let Some(label) = label_of(id).filter(|label| !label.is_empty())
        {
            field.reference = FieldReference::Unresolved(label);
        }
    });
}
