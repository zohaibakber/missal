use std::ops::Range;

use missal_core::FieldReference;
use missal_core::document::{
    Block, Document, ElementProps, Heading, HeadingLevel, Inline, Leaf, ListType, Paragraph, Quote,
    Table, TableCell, TableRow,
};
use missal_page::{BlockPath, Position};

use super::edit;
use super::lists;
use super::selection::{block_len, pos, start_of};
use super::tree::{self, HostKind, anatomy, container, container_mut, host_kind};

pub const MAX_TABLE_SIDE: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextKind {
    Paragraph,
    Heading(HeadingLevel),
    Quote,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BlockCommand {
    Text(TextKind),
    List(ListType),
    Table { rows: u32, columns: u32 },
    Divider,
    PageBreak,
    Field(FieldReference),
}

impl BlockCommand {
    pub fn table(rows: u32, columns: u32) -> BlockCommand {
        BlockCommand::Table {
            rows: rows.clamp(1, MAX_TABLE_SIDE),
            columns: columns.clamp(1, MAX_TABLE_SIDE),
        }
    }

    pub fn inserts_block(&self) -> bool {
        matches!(
            self,
            BlockCommand::Table { .. } | BlockCommand::Divider | BlockCommand::PageBreak
        )
    }
}

pub fn text_kind(doc: &Document, path: &BlockPath) -> Option<TextKind> {
    let found = anatomy(doc, path)?;
    if !found.tail.is_empty() {
        return None;
    }
    match container(doc, &found.container)?.get(found.block)? {
        Block::Paragraph(_) => Some(TextKind::Paragraph),
        Block::Heading(h) => Some(TextKind::Heading(h.level.unwrap_or(HeadingLevel::H1))),
        Block::Quote(_) => Some(TextKind::Quote),
        _ => None,
    }
}

fn unlist(doc: &mut Document, at: &Position) -> Option<Position> {
    if host_kind(doc, &at.path) != Some(HostKind::Item) {
        return Some(at.clone());
    }
    let kind = lists::list_state(doc, &at.path)?;
    lists::toggle(doc, at, at, kind).map(|(caret, _)| caret)
}

fn retype(doc: &mut Document, path: &BlockPath, kind: TextKind) -> Option<()> {
    let found = anatomy(doc, path)?;
    let blocks = container_mut(doc, &found.container)?;
    let block = blocks.get_mut(found.block)?;
    let (props, children): (ElementProps, Vec<Inline>) = match block {
        Block::Paragraph(p) => (p.props.clone(), std::mem::take(&mut p.children)),
        Block::Heading(h) => (h.props.clone(), std::mem::take(&mut h.children)),
        Block::Quote(q) => (q.props.clone(), std::mem::take(&mut q.children)),
        _ => return None,
    };
    *block = match kind {
        TextKind::Paragraph => Block::Paragraph(Paragraph { props, children }),
        TextKind::Heading(level) => Block::Heading(Heading {
            props,
            level: Some(level),
            children,
        }),
        TextKind::Quote => Block::Quote(Quote { props, children }),
    };
    Some(())
}

fn host_props(doc: &Document, path: &BlockPath) -> ElementProps {
    let direction = tree::props(doc, path)
        .and_then(|props| props.direction)
        .or(doc.root.direction);
    ElementProps {
        direction,
        ..ElementProps::default()
    }
}

fn empty_table(rows: u32, columns: u32, props: &ElementProps) -> Block {
    let cell = || TableCell {
        props: props.clone(),
        blocks: vec![edit::empty_paragraph(Some(props))],
        ..TableCell::default()
    };
    Block::Table(Table {
        props: props.clone(),
        rows: (0..rows)
            .map(|_| TableRow {
                cells: (0..columns).map(|_| cell()).collect(),
                ..TableRow::default()
            })
            .collect(),
        ..Table::default()
    })
}

fn insert_block(doc: &mut Document, at: &Position, command: &BlockCommand) -> Option<Position> {
    let props = host_props(doc, &at.path);
    let found = anatomy(doc, &at.path)?;
    let replace = found.tail.is_empty() && block_len(doc, &at.path) == Some(0);
    let block = match command {
        BlockCommand::Table { rows, columns } => empty_table(*rows, *columns, &props),
        BlockCommand::Divider => Block::HorizontalRule(Leaf::default()),
        BlockCommand::PageBreak => Block::PageBreak(Leaf::default()),
        _ => return None,
    };
    let is_table = matches!(block, Block::Table(_));
    let blocks = container_mut(doc, &found.container)?;
    let index = if replace {
        blocks[found.block] = block;
        found.block
    } else {
        blocks.insert(found.block + 1, block);
        found.block + 1
    };
    let follows_text = matches!(
        blocks.get(index + 1),
        Some(Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) | Block::List(_))
    );
    if !follows_text {
        blocks.insert(index + 1, edit::empty_paragraph(Some(&props)));
    }
    let mut steps = found.container.clone();
    if is_table {
        steps.extend([index as u32, 0, 0, 0]);
        return Some(start_of(BlockPath(steps)));
    }
    steps.push(index as u32 + 1);
    let next = container(doc, &found.container)?.get(index + 1)?;
    let path = tree::text_paths_of_block(next, BlockPath(steps))
        .into_iter()
        .next()?;
    Some(start_of(path))
}

pub fn apply(
    doc: &mut Document,
    path: &BlockPath,
    trigger: Range<usize>,
    command: &BlockCommand,
) -> Option<Position> {
    let at = if trigger.is_empty() {
        pos(path.clone(), trigger.start)
    } else {
        edit::delete_range(
            doc,
            &pos(path.clone(), trigger.start),
            &pos(path.clone(), trigger.end),
        )
    };
    match command {
        BlockCommand::Text(kind) => {
            let at = unlist(doc, &at)?;
            retype(doc, &at.path, *kind)?;
            Some(at)
        }
        BlockCommand::List(kind) => {
            if lists::list_state(doc, &at.path) == Some(*kind) {
                return Some(at);
            }
            let at = if host_kind(doc, &at.path) == Some(HostKind::Item) {
                at
            } else {
                retype(doc, &at.path, TextKind::Paragraph)?;
                at
            };
            lists::toggle(doc, &at, &at, *kind).map(|(caret, _)| caret)
        }
        BlockCommand::Field(reference) => {
            let template = edit::template(doc, &at, None);
            Some(edit::insert_inlines(
                doc,
                &at,
                vec![edit::field_inline(reference.clone(), Some(&template))],
            ))
        }
        BlockCommand::Table { .. } | BlockCommand::Divider | BlockCommand::PageBreak => {
            insert_block(doc, &at, command)
        }
    }
}
