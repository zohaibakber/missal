use std::ops::Range;

use missal_core::document::{
    Align, Block, Document, ElementProps, Field, Inline, Leaf as LeafNode, ListItem, Paragraph,
    Text, TextFormat,
};
use missal_core::{FieldCatalog, FieldMarkers, FieldReference};
use missal_page::{Affinity, BlockPath, Position};

use super::boundary;
use super::inline::{self, len, split_off};
use super::lists;
use super::selection::{block_len, end_of, pos};
use super::tree::{
    self, HostKind, Leaf, anatomy, container, container_mut, edit_inlines, host_kind, inlines,
    leaves_of_block, props, props_mut, split_block_path,
};

pub fn empty_paragraph(like: Option<&ElementProps>) -> Block {
    let props = match like {
        Some(props) => props.clone(),
        None => ElementProps {
            direction: Some(missal_core::document::Direction::Rtl),
            ..ElementProps::default()
        },
    };
    Block::Paragraph(Paragraph {
        props,
        children: Vec::new(),
    })
}

pub fn template(doc: &Document, at: &Position, pending: Option<TextFormat>) -> Text {
    let found = inlines(doc, &at.path)
        .and_then(|found| inline::template_at(&found, at.offset))
        .unwrap_or_else(|| {
            let props = props(doc, &at.path);
            let format = props.and_then(|p| p.text_format).unwrap_or_default();
            let style = props
                .and_then(|p| p.text_style.clone())
                .filter(|s| !s.is_empty());
            inline::new_text("", format, style)
        });
    match pending {
        Some(format) => Text { format, ..found },
        None => found,
    }
}

pub fn insert_text(doc: &mut Document, at: &Position, text: &str, template: &Text) -> Position {
    edit_inlines(doc, &at.path, |v| {
        inline::insert_text(v, at.offset, text, template)
    });
    pos(at.path.clone(), at.offset + text.len()).with_affinity(Affinity::Upstream)
}

pub fn insert_inlines(doc: &mut Document, at: &Position, new: Vec<Inline>) -> Position {
    let added = len(&new);
    edit_inlines(doc, &at.path, |v| inline::insert(v, at.offset, new));
    pos(at.path.clone(), at.offset + added).with_affinity(Affinity::Upstream)
}

pub fn line_break(doc: &mut Document, at: &Position) -> Position {
    let node = Inline::LineBreak(LeafNode::default());
    insert_inlines(doc, at, vec![node])
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Mark {
    Host(BlockPath),
    Block(BlockPath),
}

fn between(path: &BlockPath, s: &Position, e: &Position) -> bool {
    path > &s.path && path < &e.path
}

fn collect_marks(
    block: &Block,
    path: BlockPath,
    s: &Position,
    e: &Position,
    marks: &mut Vec<Mark>,
) {
    let contains_s = s.path.starts_with(path.steps());
    let contains_e = e.path.starts_with(path.steps());
    match block {
        Block::Table(table) => {
            if !contains_s && !contains_e {
                if between(&path, s, e) {
                    marks.push(Mark::Block(path));
                }
                return;
            }
            for (r, row) in table.rows.iter().enumerate() {
                for (c, cell) in row.cells.iter().enumerate() {
                    let prefix = path.child(r as u32).child(c as u32);
                    for (b, inner) in cell.blocks.iter().enumerate() {
                        collect_marks(inner, prefix.child(b as u32), s, e, marks);
                    }
                }
            }
        }
        Block::List(_) => {
            if !contains_s && !contains_e {
                if between(&path, s, e) {
                    marks.push(Mark::Block(path));
                }
                return;
            }
            let mut leaves = Vec::new();
            leaves_of_block(block, path, &mut leaves);
            for leaf in leaves {
                if let Leaf::Text(item) = leaf
                    && between(&item, s, e)
                {
                    marks.push(Mark::Host(item));
                }
            }
        }
        Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) => {
            if between(&path, s, e) {
                marks.push(Mark::Host(path));
            }
        }
        _ => {
            if between(&path, s, e) {
                marks.push(Mark::Block(path));
            }
        }
    }
}

pub fn remove_host(doc: &mut Document, path: &BlockPath) -> Option<(Vec<u32>, ElementProps)> {
    let found = anatomy(doc, path)?;
    let removed_props = props(doc, path).cloned().unwrap_or_default();
    let blocks = container_mut(doc, &found.container)?;
    if found.tail.is_empty() {
        blocks.remove(found.block);
        return Some((found.container, removed_props));
    }
    let Block::List(list) = &mut blocks[found.block] else {
        return None;
    };
    let empty = lists::remove_item_text(list, &found.tail);
    if empty {
        blocks.remove(found.block);
    }
    Some((found.container, removed_props))
}

fn remove_block(doc: &mut Document, path: &BlockPath) -> Option<Vec<u32>> {
    let (prefix, index) = split_block_path(path);
    let prefix = prefix.to_vec();
    let blocks = container_mut(doc, &prefix)?;
    if index < blocks.len() {
        blocks.remove(index);
    }
    Some(prefix)
}

fn fill_empty_containers(doc: &mut Document, touched: Vec<(Vec<u32>, Option<ElementProps>)>) {
    for (prefix, props) in touched {
        if prefix.is_empty() {
            continue;
        }
        if let Some(blocks) = container_mut(doc, &prefix)
            && blocks.is_empty()
        {
            blocks.push(empty_paragraph(props.as_ref()));
        }
    }
}

pub fn delete_range(doc: &mut Document, s: &Position, e: &Position) -> Position {
    if s.path == e.path {
        let range = s.offset.min(e.offset)..s.offset.max(e.offset);
        edit_inlines(doc, &s.path, |v| inline::delete(v, range));
        return pos(s.path.clone(), s.offset.min(e.offset));
    }
    let mut marks = Vec::new();
    let last = e.path.top_index().min(doc.blocks.len().saturating_sub(1));
    for (k, block) in doc
        .blocks
        .iter()
        .enumerate()
        .take(last + 1)
        .skip(s.path.top_index())
    {
        collect_marks(block, BlockPath::top(k), s, e, &mut marks);
    }
    let merge = tree::same_container(doc, &s.path, &e.path);
    if let Some(length) = block_len(doc, &s.path) {
        edit_inlines(doc, &s.path, |v| inline::delete(v, s.offset..length));
    }
    let tail = edit_inlines(doc, &e.path, |v| {
        let tail = split_off(v, e.offset);
        let head = std::mem::replace(v, tail);
        drop(head);
        inline::normalize(v);
        v.clone()
    });
    if merge && let Some(tail) = tail {
        edit_inlines(doc, &s.path, |v| {
            v.extend(tail);
            inline::normalize(v);
        });
        marks.push(Mark::Host(e.path.clone()));
    }
    let mut touched: Vec<(Vec<u32>, Option<ElementProps>)> = Vec::new();
    for mark in marks.iter().rev() {
        match mark {
            Mark::Host(path) => {
                if let Some((prefix, removed)) = remove_host(doc, path) {
                    match touched.iter_mut().find(|(p, _)| *p == prefix) {
                        Some(entry) => entry.1 = Some(removed),
                        None => touched.push((prefix, Some(removed))),
                    }
                }
            }
            Mark::Block(path) => {
                if let Some(prefix) = remove_block(doc, path)
                    && !touched.iter().any(|(p, _)| *p == prefix)
                {
                    touched.push((prefix, None));
                }
            }
        }
    }
    fill_empty_containers(doc, touched);
    pos(s.path.clone(), s.offset)
}

fn incremented(path: &BlockPath) -> BlockPath {
    let mut steps = path.steps().to_vec();
    if let Some(last) = steps.last_mut() {
        *last += 1;
    }
    BlockPath(steps)
}

pub fn with_text_defaults(mut props: ElementProps, template: &Text) -> ElementProps {
    props.text_format = Some(template.format);
    props.text_style = template.style.clone();
    props
}

pub fn split(doc: &mut Document, at: &Position) -> Position {
    let Some(kind) = host_kind(doc, &at.path) else {
        return at.clone();
    };
    let template = template(doc, at, None);
    if kind == HostKind::Item {
        let empty = block_len(doc, &at.path) == Some(0);
        if empty {
            return lists::outdent(doc, &at.path);
        }
        return lists::split_item(doc, at, &template);
    }
    let Some(found) = anatomy(doc, &at.path) else {
        return at.clone();
    };
    let Some(tail) = edit_inlines(doc, &at.path, |v| split_off(v, at.offset)) else {
        return at.clone();
    };
    let head_empty = block_len(doc, &at.path) == Some(0);
    let Some(blocks) = container_mut(doc, &found.container) else {
        return at.clone();
    };
    let at_end = tail.is_empty();
    let new_block = match &mut blocks[found.block] {
        Block::Paragraph(p) => {
            if head_empty {
                p.props = with_text_defaults(p.props.clone(), &template);
            }
            Block::Paragraph(Paragraph {
                props: with_text_defaults(p.props.clone(), &template),
                children: tail,
            })
        }
        Block::Heading(h) if at_end => Block::Paragraph(Paragraph {
            props: with_text_defaults(h.props.clone(), &template),
            children: tail,
        }),
        Block::Heading(h) => Block::Heading(missal_core::document::Heading {
            props: h.props.clone(),
            level: h.level,
            children: tail,
        }),
        Block::Quote(q) => Block::Quote(missal_core::document::Quote {
            props: with_text_defaults(q.props.clone(), &template),
            children: tail,
        }),
        _ => return at.clone(),
    };
    blocks.insert(found.block + 1, new_block);
    pos(incremented(&at.path), 0)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Backward {
    Range(Position, Position),
    Merge { into: BlockPath, from: BlockPath },
    RemoveBlock(BlockPath),
    Outdent(BlockPath),
    ToParagraph(BlockPath),
    Move(Position),
    Nothing,
}

fn previous_sibling(doc: &Document, path: &BlockPath) -> Option<(BlockPath, Block)> {
    let found = anatomy(doc, path)?;
    if found.block == 0 {
        return None;
    }
    let blocks = container(doc, &found.container)?;
    let mut base = found.container.clone();
    base.push((found.block - 1) as u32);
    Some((BlockPath(base), blocks[found.block - 1].clone()))
}

fn next_sibling_path<'a>(doc: &'a Document, path: &BlockPath) -> Option<(BlockPath, &'a Block)> {
    let found = anatomy(doc, path)?;
    let blocks = container(doc, &found.container)?;
    let next = blocks.get(found.block + 1)?;
    let mut base = found.container.clone();
    base.push((found.block + 1) as u32);
    Some((BlockPath(base), next))
}

pub fn backward(doc: &Document, at: &Position, word: bool) -> Backward {
    let Some(found) = inlines(doc, &at.path) else {
        return Backward::Nothing;
    };
    if at.offset > 0 {
        let text = inline::text_of(&found);
        let start = if word {
            boundary::previous_word(&text, at.offset)
        } else {
            boundary::previous_delete(&text, at.offset)
        };
        return Backward::Range(pos(at.path.clone(), start), at.clone());
    }
    let kind = host_kind(doc, &at.path);
    if kind == Some(HostKind::Item) {
        return Backward::Outdent(at.path.clone());
    }
    let Some((prev_path, prev)) = previous_sibling(doc, &at.path) else {
        if matches!(kind, Some(HostKind::Heading | HostKind::Quote)) {
            return Backward::ToParagraph(at.path.clone());
        }
        return Backward::Nothing;
    };
    match &prev {
        Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) => Backward::Merge {
            into: prev_path,
            from: at.path.clone(),
        },
        Block::List(_) => match tree::text_paths_of_block(&prev, prev_path).pop() {
            Some(into) => Backward::Merge {
                into,
                from: at.path.clone(),
            },
            None => Backward::Nothing,
        },
        Block::Table(_) => match tree::text_paths_of_block(&prev, prev_path).pop() {
            Some(last) => Backward::Move(end_of(doc, last)),
            None => Backward::Nothing,
        },
        _ => Backward::RemoveBlock(prev_path),
    }
}

pub fn forward(doc: &Document, at: &Position, word: bool) -> Backward {
    let Some(found) = inlines(doc, &at.path) else {
        return Backward::Nothing;
    };
    let length = len(&found);
    if at.offset < length {
        let text = inline::text_of(&found);
        let end = if word {
            boundary::next_word(&text, at.offset)
        } else {
            boundary::next_delete(&text, at.offset)
        };
        return Backward::Range(
            at.clone(),
            pos(at.path.clone(), end.max(at.offset + 1).min(length)),
        );
    }
    let Some(info) = anatomy(doc, &at.path) else {
        return Backward::Nothing;
    };
    if !info.tail.is_empty() {
        let blocks = container(doc, &info.container);
        if let Some(list_block) = blocks.and_then(|b| b.get(info.block)) {
            let mut base = info.container.clone();
            base.push(info.block as u32);
            let paths = tree::text_paths_of_block(list_block, BlockPath(base));
            if let Some(index) = paths.iter().position(|p| p == &at.path)
                && let Some(next) = paths.get(index + 1)
            {
                return Backward::Merge {
                    into: at.path.clone(),
                    from: next.clone(),
                };
            }
        }
    }
    let Some((next_path, next)) = next_sibling_path(doc, &at.path) else {
        return Backward::Nothing;
    };
    match next {
        Block::Paragraph(_) | Block::Heading(_) | Block::Quote(_) => Backward::Merge {
            into: at.path.clone(),
            from: next_path,
        },
        Block::List(_) => match tree::text_paths_of_block(next, next_path)
            .into_iter()
            .next()
        {
            Some(from) => Backward::Merge {
                into: at.path.clone(),
                from,
            },
            None => Backward::Nothing,
        },
        Block::Table(_) => Backward::Nothing,
        _ => Backward::RemoveBlock(next_path),
    }
}

pub fn merge_hosts(doc: &mut Document, into: &BlockPath, from: &BlockPath) -> Position {
    let at = block_len(doc, into).unwrap_or(0);
    let moved = inlines(doc, from)
        .map(|found| found.into_owned())
        .unwrap_or_default();
    let into_empty = at == 0;
    let from_props = props(doc, from).cloned();
    edit_inlines(doc, into, |v| {
        v.extend(moved);
        inline::normalize(v);
    });
    if into_empty
        && let Some(from_props) = from_props
        && matches!(host_kind(doc, into), Some(HostKind::Paragraph))
        && matches!(host_kind(doc, from), Some(HostKind::Paragraph))
        && let Some(target) = props_mut(doc, into)
    {
        *target = from_props;
    }
    remove_host(doc, from);
    pos(into.clone(), at)
}

pub fn to_paragraph(doc: &mut Document, path: &BlockPath) -> Position {
    if let Some(found) = anatomy(doc, path)
        && found.tail.is_empty()
        && let Some(blocks) = container_mut(doc, &found.container)
    {
        let replacement = match &blocks[found.block] {
            Block::Heading(h) => Some(Paragraph {
                props: h.props.clone(),
                children: h.children.clone(),
            }),
            Block::Quote(q) => Some(Paragraph {
                props: q.props.clone(),
                children: q.children.clone(),
            }),
            _ => None,
        };
        if let Some(paragraph) = replacement {
            blocks[found.block] = Block::Paragraph(paragraph);
        }
    }
    pos(path.clone(), 0)
}

pub fn hosts_between(doc: &Document, s: &Position, e: &Position) -> Vec<(BlockPath, Range<usize>)> {
    let mut out = Vec::new();
    if s.path == e.path {
        out.push((
            s.path.clone(),
            s.offset.min(e.offset)..s.offset.max(e.offset),
        ));
        return out;
    }
    let last = e.path.top_index().min(doc.blocks.len().saturating_sub(1));
    for k in s.path.top_index()..=last {
        for path in tree::text_paths_of_block(&doc.blocks[k], BlockPath::top(k)) {
            if path < s.path || path > e.path {
                continue;
            }
            let length = block_len(doc, &path).unwrap_or(0);
            let start = if path == s.path { s.offset } else { 0 };
            let end = if path == e.path { e.offset } else { length };
            out.push((path, start..end.max(start)));
        }
    }
    out
}

pub fn flag_state(doc: &Document, s: &Position, e: &Position, flag: TextFormat) -> Option<bool> {
    let mut any = false;
    let mut all = true;
    for (path, range) in hosts_between(doc, s, e) {
        if range.is_empty() {
            continue;
        }
        if let Some(found) = inlines(doc, &path) {
            inline::for_each_text_in(&found, range, &mut |text| {
                if !text.text.is_empty() {
                    any = true;
                    if !inline::has_flag(text, flag) {
                        all = false;
                    }
                }
            });
        }
    }
    any.then_some(all)
}

pub fn set_flag(doc: &mut Document, s: &Position, e: &Position, flag: TextFormat, on: bool) {
    for (path, range) in hosts_between(doc, s, e) {
        if range.is_empty() {
            continue;
        }
        edit_inlines(doc, &path, |v| {
            inline::map_range(v, range, &mut |text| inline::set_flag_on(text, flag, on))
        });
    }
}

pub fn set_align(doc: &mut Document, s: &Position, e: &Position, align: Align) -> Vec<BlockPath> {
    let mut touched = Vec::new();
    for (path, _) in hosts_between(doc, s, e) {
        if let Some(props) = props_mut(doc, &path) {
            props.align = Some(align);
            touched.push(path);
        }
    }
    touched
}

pub fn effective_align(doc: &Document, path: &BlockPath) -> Align {
    let Some(props) = props(doc, path) else {
        return Align::Right;
    };
    let rtl = props.direction != Some(missal_core::document::Direction::Ltr);
    let align = props.align.or_else(|| {
        let css = props.imported_style.as_ref()?.get("text-align")?;
        Some(match css.trim().to_ascii_lowercase().as_str() {
            "left" => Align::Left,
            "right" => Align::Right,
            "center" => Align::Center,
            "justify" => Align::Justify,
            "end" => Align::End,
            _ => Align::Start,
        })
    });
    match align.unwrap_or(Align::Start) {
        Align::Start => {
            if rtl {
                Align::Right
            } else {
                Align::Left
            }
        }
        Align::End => {
            if rtl {
                Align::Left
            } else {
                Align::Right
            }
        }
        other => other,
    }
}

pub fn field_inline(reference: FieldReference, style_from: Option<&Text>) -> Inline {
    let mut field = Field::new(reference);
    field.imported_style = style_from.and_then(inline::css_for_field);
    Inline::Field(field)
}

pub struct Converted {
    pub changes: Vec<(usize, usize)>,
}

impl Converted {
    pub fn map(&self, offset: usize) -> usize {
        let mut delta: isize = 0;
        for &(start, length) in &self.changes {
            if offset >= start + length {
                delta += missal_page::OBJECT_LEN as isize - length as isize;
            } else if offset > start {
                return (start as isize + delta) as usize + missal_page::OBJECT_LEN;
            }
        }
        (offset as isize + delta) as usize
    }
}

fn convert_list(
    inlines: Vec<Inline>,
    base: usize,
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
    caret: Option<usize>,
    changes: &mut Vec<(usize, usize)>,
) -> Vec<Inline> {
    let mut out = Vec::with_capacity(inlines.len());
    let mut acc = base;
    for item in inlines {
        let l = inline::inline_len(&item);
        match item {
            Inline::Text(mut text) if text.text.contains(markers.open()) => {
                let mut origin = acc;
                let mut from = 0;
                while let Some(found) = catalog.find_token(&text.text, markers, from) {
                    let start = origin + found.range.start;
                    let end = origin + found.range.end;
                    if caret.is_some_and(|c| c > start && c < end) {
                        from = found.range.end;
                        continue;
                    }
                    let rest = text.text.split_off(found.range.end);
                    let _token = text.text.split_off(found.range.start);
                    let field = field_inline(found.reference, Some(&text));
                    if !text.text.is_empty() {
                        out.push(Inline::Text(text.clone()));
                    }
                    out.push(field);
                    changes.push((start, end - start));
                    text.text = rest;
                    origin = end;
                    from = 0;
                }
                if !text.text.is_empty() {
                    out.push(Inline::Text(text));
                }
            }
            Inline::Link(mut link) => {
                link.children = convert_list(link.children, acc, catalog, markers, caret, changes);
                out.push(Inline::Link(link));
            }
            other => out.push(other),
        }
        acc += l;
    }
    out
}

pub fn convert_tokens(
    doc: &mut Document,
    path: &BlockPath,
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
    caret: Option<usize>,
) -> Converted {
    let mut changes = Vec::new();
    let needs = inlines(doc, path).is_some_and(|found| has_token(&found, catalog, markers, caret));
    if needs {
        edit_inlines(doc, path, |v| {
            let taken = std::mem::take(v);
            *v = convert_list(taken, 0, catalog, markers, caret, &mut changes);
            inline::normalize(v);
        });
    }
    Converted { changes }
}

pub fn has_token(
    found: &[Inline],
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
    caret: Option<usize>,
) -> bool {
    let mut acc = 0;
    for item in found {
        let l = inline::inline_len(item);
        match item {
            Inline::Text(text) if text.text.contains(markers.open()) => {
                let mut from = 0;
                while let Some(m) = catalog.find_token(&text.text, markers, from) {
                    let start = acc + m.range.start;
                    let end = acc + m.range.end;
                    if caret.is_some_and(|c| c > start && c < end) {
                        from = m.range.end;
                        continue;
                    }
                    return true;
                }
            }
            Inline::Link(link)
                if has_token(
                    &link.children,
                    catalog,
                    markers,
                    caret.and_then(|c| c.checked_sub(acc)),
                ) =>
            {
                return true;
            }
            _ => {}
        }
        acc += l;
    }
    false
}

pub fn reveal_field(
    doc: &mut Document,
    path: &BlockPath,
    offset: usize,
    catalog: &FieldCatalog,
    markers: &FieldMarkers,
) -> Option<(Position, Position)> {
    let found = inlines(doc, path)?;
    let field = inline::field_at(&found, offset)?;
    let name = catalog.label(&field.reference)?;
    if name.is_empty() {
        return None;
    }
    let style = field.imported_style.clone();
    let token = markers.token(&name);
    let run = inline::new_text(&token, TextFormat(0), style);
    edit_inlines(doc, path, |v| {
        let tail = split_off(v, offset + missal_page::OBJECT_LEN);
        let _ = split_off(v, offset);
        v.push(Inline::Text(run));
        v.extend(tail);
        inline::normalize(v);
    });
    let start = offset + markers.open().len();
    Some((
        pos(path.clone(), start),
        pos(path.clone(), start + name.len()).with_affinity(Affinity::Upstream),
    ))
}

pub fn ensure_text_block(doc: &mut Document) -> bool {
    if tree::first_text(doc).is_some() {
        return false;
    }
    doc.blocks.push(empty_paragraph(None));
    true
}

pub fn new_item(props: ElementProps, children: Vec<Inline>) -> ListItem {
    ListItem {
        props,
        value: None,
        checked: None,
        children: children
            .into_iter()
            .map(missal_core::document::ListItemChild::Inline)
            .collect(),
    }
}
