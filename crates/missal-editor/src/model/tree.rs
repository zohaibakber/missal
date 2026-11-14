use std::borrow::Cow;

use missal_core::document::{
    Block, Document, ElementProps, Inline, List, ListItem, ListItemChild, Table, TableCell,
};
use missal_page::BlockPath;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostKind {
    Paragraph,
    Heading,
    Quote,
    Item,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Leaf {
    Text(BlockPath),
    Opaque(BlockPath),
}

impl Leaf {
    pub fn path(&self) -> &BlockPath {
        match self {
            Leaf::Text(path) | Leaf::Opaque(path) => path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anatomy {
    pub container: Vec<u32>,
    pub block: usize,
    pub tail: Vec<u32>,
}

pub fn cell(table: &Table, r: u32, c: u32) -> Option<&TableCell> {
    table.rows.get(r as usize)?.cells.get(c as usize)
}

pub fn cell_mut(table: &mut Table, r: u32, c: u32) -> Option<&mut TableCell> {
    table.rows.get_mut(r as usize)?.cells.get_mut(c as usize)
}

pub fn container<'a>(doc: &'a Document, prefix: &[u32]) -> Option<&'a Vec<Block>> {
    let mut blocks = &doc.blocks;
    let mut steps = prefix;
    while !steps.is_empty() {
        let [b, r, c, rest @ ..] = steps else {
            return None;
        };
        let Block::Table(table) = blocks.get(*b as usize)? else {
            return None;
        };
        blocks = &cell(table, *r, *c)?.blocks;
        steps = rest;
    }
    Some(blocks)
}

pub fn container_mut<'a>(doc: &'a mut Document, prefix: &[u32]) -> Option<&'a mut Vec<Block>> {
    let mut blocks = &mut doc.blocks;
    let mut steps = prefix;
    while !steps.is_empty() {
        let [b, r, c, rest @ ..] = steps else {
            return None;
        };
        let Block::Table(table) = blocks.get_mut(*b as usize)? else {
            return None;
        };
        blocks = &mut cell_mut(table, *r, *c)?.blocks;
        steps = rest;
    }
    Some(blocks)
}

pub fn anatomy(doc: &Document, path: &BlockPath) -> Option<Anatomy> {
    let steps = path.steps();
    let mut blocks = &doc.blocks;
    let mut i = 0;
    loop {
        let b = *steps.get(i)? as usize;
        match blocks.get(b)? {
            Block::Table(table) => {
                let (r, c) = (*steps.get(i + 1)?, *steps.get(i + 2)?);
                if steps.len() <= i + 3 {
                    return None;
                }
                blocks = &cell(table, r, c)?.blocks;
                i += 3;
            }
            Block::List(list) => {
                let tail = steps[i + 1..].to_vec();
                item(list, &tail)?;
                return Some(Anatomy {
                    container: steps[..i].to_vec(),
                    block: b,
                    tail,
                });
            }
            Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) if steps.len() == i + 1 => {
                return Some(Anatomy {
                    container: steps[..i].to_vec(),
                    block: b,
                    tail: Vec::new(),
                });
            }
            _ => return None,
        }
    }
}

pub fn cell_chain(doc: &Document, path: &BlockPath) -> Vec<Vec<u32>> {
    let steps = path.steps();
    let mut chain = Vec::new();
    let mut blocks = &doc.blocks;
    let mut i = 0;
    while i < steps.len() {
        let Some(Block::Table(table)) = blocks.get(steps[i] as usize) else {
            break;
        };
        let (Some(&r), Some(&c)) = (steps.get(i + 1), steps.get(i + 2)) else {
            break;
        };
        let Some(found) = cell(table, r, c) else {
            break;
        };
        chain.push(steps[..i + 3].to_vec());
        blocks = &found.blocks;
        i += 3;
    }
    chain
}

pub fn item<'a>(list: &'a List, tail: &[u32]) -> Option<&'a ListItem> {
    let found = list.items.get(*tail.first()? as usize)?;
    if tail.len() == 1 {
        return Some(found);
    }
    let ListItemChild::List(nested) = found.children.get(*tail.get(1)? as usize)? else {
        return None;
    };
    item(nested, &tail[2..])
}

pub fn item_mut<'a>(list: &'a mut List, tail: &[u32]) -> Option<&'a mut ListItem> {
    let found = list.items.get_mut(*tail.first()? as usize)?;
    if tail.len() == 1 {
        return Some(found);
    }
    let ListItemChild::List(nested) = found.children.get_mut(*tail.get(1)? as usize)? else {
        return None;
    };
    item_mut(nested, &tail[2..])
}

pub fn list_at_mut<'a>(list: &'a mut List, tail: &[u32]) -> Option<&'a mut List> {
    if tail.len() <= 1 {
        return Some(list);
    }
    let found = list.items.get_mut(tail[0] as usize)?;
    let ListItemChild::List(nested) = found.children.get_mut(tail[1] as usize)? else {
        return None;
    };
    list_at_mut(nested, &tail[2..])
}

pub fn list_at<'a>(list: &'a List, tail: &[u32]) -> Option<&'a List> {
    if tail.len() <= 1 {
        return Some(list);
    }
    let found = list.items.get(tail[0] as usize)?;
    let ListItemChild::List(nested) = found.children.get(tail[1] as usize)? else {
        return None;
    };
    list_at(nested, &tail[2..])
}

pub fn item_inlines(item: &ListItem) -> Vec<Inline> {
    item.children
        .iter()
        .filter_map(|child| match child {
            ListItemChild::Inline(inline) => Some(inline.clone()),
            ListItemChild::List(_) => None,
        })
        .collect()
}

pub fn item_has_text(item: &ListItem) -> bool {
    let has_inline = item
        .children
        .iter()
        .any(|c| matches!(c, ListItemChild::Inline(_)));
    let has_list = item
        .children
        .iter()
        .any(|c| matches!(c, ListItemChild::List(_)));
    has_inline || !has_list
}

pub fn with_item_inlines<R>(item: &mut ListItem, f: impl FnOnce(&mut Vec<Inline>) -> R) -> R {
    let children = std::mem::take(&mut item.children);
    let mut inlines = Vec::new();
    let mut lists = Vec::new();
    for child in children {
        match child {
            ListItemChild::Inline(inline) => inlines.push(inline),
            other => lists.push(other),
        }
    }
    let result = f(&mut inlines);
    item.children = inlines
        .into_iter()
        .map(ListItemChild::Inline)
        .chain(lists)
        .collect();
    result
}

pub fn host_kind(doc: &Document, path: &BlockPath) -> Option<HostKind> {
    let found = anatomy(doc, path)?;
    let block = container(doc, &found.container)?.get(found.block)?;
    Some(match block {
        Block::Paragraph(_) => HostKind::Paragraph,
        Block::Heading(_) => HostKind::Heading,
        Block::Quote(_) => HostKind::Quote,
        Block::List(_) => HostKind::Item,
        _ => return None,
    })
}

pub fn inlines<'a>(doc: &'a Document, path: &BlockPath) -> Option<Cow<'a, [Inline]>> {
    let found = anatomy(doc, path)?;
    let block = container(doc, &found.container)?.get(found.block)?;
    match block {
        Block::Paragraph(p) => Some(Cow::Borrowed(&p.children)),
        Block::Heading(h) => Some(Cow::Borrowed(&h.children)),
        Block::Quote(q) => Some(Cow::Borrowed(&q.children)),
        Block::List(list) => {
            let found = item(list, &found.tail)?;
            if found
                .children
                .iter()
                .all(|c| matches!(c, ListItemChild::Inline(_)))
            {
                let mut out = Vec::with_capacity(found.children.len());
                for child in &found.children {
                    if let ListItemChild::Inline(inline) = child {
                        out.push(inline.clone());
                    }
                }
                Some(Cow::Owned(out))
            } else {
                Some(Cow::Owned(item_inlines(found)))
            }
        }
        _ => None,
    }
}

pub fn edit_inlines<R>(
    doc: &mut Document,
    path: &BlockPath,
    f: impl FnOnce(&mut Vec<Inline>) -> R,
) -> Option<R> {
    let found = anatomy(doc, path)?;
    let block = container_mut(doc, &found.container)?.get_mut(found.block)?;
    match block {
        Block::Paragraph(p) => Some(f(&mut p.children)),
        Block::Heading(h) => Some(f(&mut h.children)),
        Block::Quote(q) => Some(f(&mut q.children)),
        Block::List(list) => Some(with_item_inlines(item_mut(list, &found.tail)?, f)),
        _ => None,
    }
}

pub fn props<'a>(doc: &'a Document, path: &BlockPath) -> Option<&'a ElementProps> {
    let found = anatomy(doc, path)?;
    let block = container(doc, &found.container)?.get(found.block)?;
    match block {
        Block::Paragraph(p) => Some(&p.props),
        Block::Heading(h) => Some(&h.props),
        Block::Quote(q) => Some(&q.props),
        Block::List(list) => Some(&item(list, &found.tail)?.props),
        _ => None,
    }
}

pub fn props_mut<'a>(doc: &'a mut Document, path: &BlockPath) -> Option<&'a mut ElementProps> {
    let found = anatomy(doc, path)?;
    let block = container_mut(doc, &found.container)?.get_mut(found.block)?;
    match block {
        Block::Paragraph(p) => Some(&mut p.props),
        Block::Heading(h) => Some(&mut h.props),
        Block::Quote(q) => Some(&mut q.props),
        Block::List(list) => Some(&mut item_mut(list, &found.tail)?.props),
        _ => None,
    }
}

pub fn block_props(block: &Block) -> Option<&ElementProps> {
    match block {
        Block::Paragraph(p) => Some(&p.props),
        Block::Heading(h) => Some(&h.props),
        Block::Quote(q) => Some(&q.props),
        Block::List(l) => Some(&l.props),
        Block::Table(t) => Some(&t.props),
        _ => None,
    }
}

pub fn leaves_of_block(block: &Block, path: BlockPath, out: &mut Vec<Leaf>) {
    match block {
        Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) => out.push(Leaf::Text(path)),
        Block::List(list) => leaves_of_list(list, &path, out),
        Block::Table(table) => {
            for (r, row) in table.rows.iter().enumerate() {
                for (c, cell) in row.cells.iter().enumerate() {
                    let cell = cell;
                    let prefix = path.child(r as u32).child(c as u32);
                    for (b, inner) in cell.blocks.iter().enumerate() {
                        leaves_of_block(inner, prefix.child(b as u32), out);
                    }
                }
            }
        }
        Block::PageBreak(_) | Block::HorizontalRule(_) | Block::Image(_) => {
            out.push(Leaf::Opaque(path))
        }
    }
}

fn leaves_of_list(list: &List, path: &BlockPath, out: &mut Vec<Leaf>) {
    for (index, child) in list.items.iter().enumerate() {
        let found = child;
        let item_path = path.child(index as u32);
        if item_has_text(found) {
            out.push(Leaf::Text(item_path.clone()));
        }
        for (child_index, child) in found.children.iter().enumerate() {
            if let ListItemChild::List(nested) = child {
                leaves_of_list(nested, &item_path.child(child_index as u32), out);
            }
        }
    }
}

pub fn text_paths_of_block(block: &Block, path: BlockPath) -> Vec<BlockPath> {
    let mut leaves = Vec::new();
    leaves_of_block(block, path, &mut leaves);
    leaves
        .into_iter()
        .filter_map(|leaf| match leaf {
            Leaf::Text(path) => Some(path),
            Leaf::Opaque(_) => None,
        })
        .collect()
}

pub fn text_paths_in(blocks: &[Block], prefix: &[u32]) -> Vec<BlockPath> {
    let base = BlockPath::new(prefix);
    let mut out = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        out.extend(text_paths_of_block(block, base.child(index as u32)));
    }
    out
}

pub fn all_text_paths(doc: &Document) -> Vec<BlockPath> {
    text_paths_in(&doc.blocks, &[])
}

pub fn first_text_in_block(doc: &Document, index: usize) -> Option<BlockPath> {
    text_paths_of_block(doc.blocks.get(index)?, BlockPath::top(index))
        .into_iter()
        .next()
}

pub fn first_text(doc: &Document) -> Option<BlockPath> {
    (0..doc.blocks.len()).find_map(|index| first_text_in_block(doc, index))
}

pub fn last_text(doc: &Document) -> Option<BlockPath> {
    (0..doc.blocks.len()).rev().find_map(|index| {
        text_paths_of_block(&doc.blocks[index], BlockPath::top(index))
            .into_iter()
            .next_back()
    })
}

pub fn next_text(doc: &Document, path: &BlockPath) -> Option<BlockPath> {
    let top = path.top_index();
    let here = text_paths_of_block(doc.blocks.get(top)?, BlockPath::top(top));
    if let Some(found) = here.iter().position(|p| p == path)
        && let Some(next) = here.get(found + 1)
    {
        return Some(next.clone());
    }
    (top + 1..doc.blocks.len()).find_map(|index| first_text_in_block(doc, index))
}

pub fn prev_text(doc: &Document, path: &BlockPath) -> Option<BlockPath> {
    let top = path.top_index();
    let here = text_paths_of_block(doc.blocks.get(top)?, BlockPath::top(top));
    if let Some(found) = here.iter().position(|p| p == path)
        && found > 0
    {
        return Some(here[found - 1].clone());
    }
    (0..top).rev().find_map(|index| {
        text_paths_of_block(&doc.blocks[index], BlockPath::top(index))
            .into_iter()
            .next_back()
    })
}

pub fn split_block_path(path: &BlockPath) -> (&[u32], usize) {
    let steps = path.steps();
    let (last, prefix) = steps.split_last().expect("block paths are never empty");
    (prefix, *last as usize)
}

pub fn same_container(doc: &Document, a: &BlockPath, b: &BlockPath) -> bool {
    match (anatomy(doc, a), anatomy(doc, b)) {
        (Some(x), Some(y)) => x.container == y.container,
        _ => false,
    }
}

pub fn cell_of(doc: &Document, path: &BlockPath) -> Option<Vec<u32>> {
    cell_chain(doc, path).pop()
}

pub fn cells_of_table(table: &Table, path: &BlockPath) -> Vec<BlockPath> {
    let mut out = Vec::new();
    for (r, row) in table.rows.iter().enumerate() {
        for c in 0..row.cells.len() {
            out.push(path.child(r as u32).child(c as u32));
        }
    }
    out
}

pub fn table_at<'a>(doc: &'a Document, table_path: &[u32]) -> Option<&'a Table> {
    let (last, prefix) = table_path.split_last()?;
    match container(doc, prefix)?.get(*last as usize)? {
        Block::Table(table) => Some(table),
        _ => None,
    }
}
