use missal_core::document::{
    Block, Child, Direction, Document, ElementProps, Inline, List, ListItem, ListItemChild,
    ListTag, ListType, Paragraph, Text,
};
use missal_page::{BlockPath, Position};

use super::edit::{new_item, version_extra};
use super::inline::{self, split_off};
use super::selection::pos;
use super::tree::{
    self, anatomy, container, container_mut, item_has_text, item_inlines, item_mut, list_at,
    list_at_mut, text_paths_in, with_item_inlines,
};

pub fn renumber(list: &mut List) {
    let mut value = list.start.unwrap_or(1);
    for child in &mut list.items {
        let Child::Node(item) = child else { continue };
        if item_has_text(item) {
            item.value = Some(value);
            value += 1;
        }
        for inner in &mut item.children {
            if let ListItemChild::List(nested) = inner {
                renumber(nested);
            }
        }
    }
}

fn is_empty_item(item: &ListItem) -> bool {
    item.children.is_empty()
}

pub fn remove_item_text(list: &mut List, tail: &[u32]) -> bool {
    remove_in(list, tail);
    renumber(list);
    list.items.is_empty()
}

fn remove_in(list: &mut List, tail: &[u32]) {
    let index = tail[0] as usize;
    if tail.len() == 1 {
        if let Some(Child::Node(item)) = list.items.get_mut(index) {
            item.children
                .retain(|child| matches!(child, ListItemChild::List(_)));
            if is_empty_item(item) {
                list.items.remove(index);
            }
        }
        return;
    }
    let child_index = tail[1] as usize;
    let Some(Child::Node(holder)) = list.items.get_mut(index) else {
        return;
    };
    let mut drop_child = false;
    if let Some(ListItemChild::List(nested)) = holder.children.get_mut(child_index) {
        remove_in(nested, &tail[2..]);
        drop_child = nested.items.is_empty();
    }
    if drop_child {
        holder.children.remove(child_index);
        if is_empty_item(holder) {
            list.items.remove(index);
        }
    }
}

fn list_block_mut<'a>(
    doc: &'a mut Document,
    container: &[u32],
    block: usize,
) -> Option<&'a mut List> {
    match container_mut(doc, container)?.get_mut(block)? {
        Block::List(list) => Some(list),
        _ => None,
    }
}

fn with_defaults(mut props: ElementProps, template: &Text) -> ElementProps {
    props.text_format = Some(template.format());
    props.text_style = Some(
        template
            .style
            .clone()
            .unwrap_or_else(|| missal_core::document::Style::new("")),
    );
    props
}

pub fn split_item(doc: &mut Document, at: &Position, template: &Text) -> Position {
    let Some(found) = anatomy(doc, &at.path) else {
        return at.clone();
    };
    let Some(list) = list_block_mut(doc, &found.container, found.block) else {
        return at.clone();
    };
    let Some(current) = item_mut(list, &found.tail) else {
        return at.clone();
    };
    let tail = with_item_inlines(current, |v| split_off(v, at.offset));
    let mut fresh = new_item(with_defaults(current.props.clone(), template), tail);
    fresh.checked = current.checked.map(|_| false);
    let index = *found.tail.last().expect("item tails are never empty") as usize;
    let Some(parent) = list_at_mut(list, &found.tail) else {
        return at.clone();
    };
    parent.items.insert(index + 1, Child::Node(fresh));
    renumber(list);
    let mut steps = at.path.steps().to_vec();
    *steps.last_mut().expect("non-empty") += 1;
    pos(BlockPath(steps), 0)
}

fn paragraph_from(item: ListItem) -> (Paragraph, Vec<ListItemChild>) {
    let mut inlines = Vec::new();
    let mut nested = Vec::new();
    for child in item.children {
        match child {
            ListItemChild::Inline(inline) => inlines.push(inline),
            other => nested.push(other),
        }
    }
    (
        Paragraph {
            props: item.props,
            children: inlines,
        },
        nested,
    )
}

fn sibling_list(list: &List, items: Vec<Child<ListItem>>) -> List {
    List {
        props: list.props.clone(),
        list_type: list.list_type,
        start: list.start.map(|_| 1),
        tag: list.tag,
        items,
    }
}

pub fn outdent(doc: &mut Document, path: &BlockPath) -> Position {
    let Some(found) = anatomy(doc, path) else {
        return pos(path.clone(), 0);
    };
    if found.tail.is_empty() {
        return pos(path.clone(), 0);
    }
    let Some(blocks) = container_mut(doc, &found.container) else {
        return pos(path.clone(), 0);
    };
    let Block::List(list) = &mut blocks[found.block] else {
        return pos(path.clone(), 0);
    };
    if found.tail.len() == 1 {
        let index = found.tail[0] as usize;
        if index >= list.items.len() {
            return pos(path.clone(), 0);
        }
        let after: Vec<Child<ListItem>> = list.items.split_off(index + 1);
        let Child::Node(item) = list.items.pop().expect("index is in range") else {
            return pos(path.clone(), 0);
        };
        let before_empty = list.items.is_empty();
        let (paragraph, nested) = paragraph_from(item);
        let mut after = after;
        if !nested.is_empty() {
            after.insert(
                0,
                Child::Node(ListItem {
                    props: paragraph.props.clone(),
                    value: None,
                    checked: None,
                    children: nested,
                }),
            );
        }
        let mut replacement = Vec::new();
        let template = list.clone();
        if !before_empty {
            let mut kept = std::mem::take(&mut list.items);
            let mut head = sibling_list(&template, Vec::new());
            head.start = template.start;
            head.items.append(&mut kept);
            renumber(&mut head);
            replacement.push(Block::List(head));
        }
        let paragraph_index = found.block + replacement.len();
        replacement.push(Block::Paragraph(paragraph));
        if !after.is_empty() {
            let mut rest = sibling_list(&template, after);
            renumber(&mut rest);
            replacement.push(Block::List(rest));
        }
        blocks.splice(found.block..found.block + 1, replacement);
        let mut steps = found.container.clone();
        steps.push(paragraph_index as u32);
        return pos(BlockPath(steps), 0);
    }
    let tail = &found.tail;
    let j = tail[tail.len() - 1] as usize;
    let child_index = tail[tail.len() - 2] as usize;
    let p = tail[tail.len() - 3] as usize;
    let parent_tail = &tail[..tail.len() - 2];
    let Some(parent) = list_at_mut(list, parent_tail) else {
        return pos(path.clone(), 0);
    };
    let Some(Child::Node(holder)) = parent.items.get_mut(p) else {
        return pos(path.clone(), 0);
    };
    let Some(ListItemChild::List(nested)) = holder.children.get_mut(child_index) else {
        return pos(path.clone(), 0);
    };
    if j >= nested.items.len() {
        return pos(path.clone(), 0);
    }
    let after = nested.items.split_off(j + 1);
    let moved = nested.items.pop().expect("index is in range");
    let template = nested.clone();
    let nested_empty = nested.items.is_empty();
    let mut holder_removed = false;
    if nested_empty {
        holder.children.remove(child_index);
        if holder.children.is_empty() {
            parent.items.remove(p);
            holder_removed = true;
        }
    }
    let insert_at = if holder_removed { p } else { p + 1 };
    parent.items.insert(insert_at, moved);
    if !after.is_empty() {
        let rest = sibling_list(&template, after);
        parent.items.insert(
            insert_at + 1,
            Child::Node(ListItem {
                props: ElementProps::default(),
                value: None,
                checked: None,
                children: vec![ListItemChild::List(rest)],
            }),
        );
    }
    renumber(list);
    let mut steps = path.steps()[..path.steps().len() - 3].to_vec();
    steps.push(insert_at as u32);
    pos(BlockPath(steps), 0)
}

pub fn indent(doc: &mut Document, path: &BlockPath) -> Option<Position> {
    let found = anatomy(doc, path)?;
    if found.tail.is_empty() {
        return None;
    }
    let list = list_block_mut(doc, &found.container, found.block)?;
    let tail = found.tail.clone();
    let i = *tail.last()? as usize;
    if i == 0 {
        return None;
    }
    let parent = list_at_mut(list, &tail)?;
    let previous_holder = match parent.items.get(i - 1)? {
        Child::Node(previous) if !item_has_text(previous) => previous
            .children
            .iter()
            .rposition(|c| matches!(c, ListItemChild::List(_))),
        _ => None,
    };
    let moved = parent.items.remove(i);
    let template = sibling_list(parent, Vec::new());
    let new_tail_end: Vec<u32> = match previous_holder {
        Some(child_index) => {
            let Some(Child::Node(previous)) = parent.items.get_mut(i - 1) else {
                return None;
            };
            let ListItemChild::List(nested) = &mut previous.children[child_index] else {
                return None;
            };
            nested.items.push(moved);
            vec![
                (i - 1) as u32,
                child_index as u32,
                (nested.items.len() - 1) as u32,
            ]
        }
        None => {
            let direction = match &moved {
                Child::Node(item) => item.props.direction,
                Child::Other(_) => Some(Direction::Rtl),
            };
            let mut nested = template;
            nested.start = Some(1);
            nested.items.push(moved);
            let holder = ListItem {
                props: ElementProps {
                    direction,
                    extra: version_extra(),
                    ..ElementProps::default()
                },
                value: None,
                checked: None,
                children: vec![ListItemChild::List(nested)],
            };
            parent.items.insert(i, Child::Node(holder));
            vec![i as u32, 0, 0]
        }
    };
    renumber(list);
    let mut steps = path.steps()[..path.steps().len() - 1].to_vec();
    steps.extend(new_tail_end);
    Some(pos(BlockPath(steps), 0))
}

fn flatten_items(list: List, out: &mut Vec<Block>) {
    for child in list.items {
        let Child::Node(item) = child else { continue };
        let (paragraph, nested) = paragraph_from(item);
        let has_text = !paragraph.children.is_empty() || nested.is_empty();
        if has_text {
            out.push(Block::Paragraph(paragraph));
        }
        for inner in nested {
            if let ListItemChild::List(nested) = inner {
                flatten_items(nested, out);
            }
        }
    }
}

fn retype(list: &mut List, kind: ListType) {
    list.list_type = Some(kind);
    list.tag = Some(if kind == ListType::Number {
        ListTag::Ol
    } else {
        ListTag::Ul
    });
    for child in &mut list.items {
        if let Child::Node(item) = child {
            for inner in &mut item.children {
                if let ListItemChild::List(nested) = inner {
                    retype(nested, kind);
                }
            }
        }
    }
}

fn list_kind(list: &List) -> ListType {
    match (list.list_type, list.tag) {
        (Some(kind), _) => kind,
        (None, Some(ListTag::Ol)) => ListType::Number,
        _ => ListType::Bullet,
    }
}

fn new_list(kind: ListType, direction: Option<Direction>) -> List {
    List {
        props: ElementProps {
            direction: direction.or(Some(Direction::Rtl)),
            indent: Some(0),
            extra: version_extra(),
            ..ElementProps::default()
        },
        list_type: Some(kind),
        start: Some(1),
        tag: Some(if kind == ListType::Number {
            ListTag::Ol
        } else {
            ListTag::Ul
        }),
        items: Vec::new(),
    }
}

fn item_from(props: ElementProps, children: Vec<Inline>) -> Child<ListItem> {
    Child::Node(new_item(props, children))
}

pub fn list_state(doc: &Document, path: &BlockPath) -> Option<ListType> {
    let found = anatomy(doc, path)?;
    if found.tail.is_empty() {
        return None;
    }
    let Block::List(list) = container(doc, &found.container)?.get(found.block)? else {
        return None;
    };
    Some(list_kind(list_at(list, &found.tail)?))
}

pub fn toggle(
    doc: &mut Document,
    s: &Position,
    e: &Position,
    kind: ListType,
) -> Option<(Position, Position)> {
    let start = anatomy(doc, &s.path)?;
    let end = anatomy(doc, &e.path);
    let k = start.container.clone();
    let first = start.block;
    let last = match end {
        Some(end) if end.container == k => end.block,
        _ => first,
    };
    let blocks = container(doc, &k)?;
    let before_paths = text_paths_in(&blocks[first..=last], &[]);
    let rel = |p: &BlockPath| -> Option<usize> {
        let steps = p.steps();
        if !steps.starts_with(&k) {
            return None;
        }
        let mut local = steps[k.len()..].to_vec();
        local[0] -= first as u32;
        before_paths
            .iter()
            .position(|q| q.steps() == local.as_slice())
    };
    let s_index = rel(&s.path).unwrap_or(0);
    let e_index = rel(&e.path).unwrap_or(s_index);
    let all_same = blocks[first..=last]
        .iter()
        .all(|b| matches!(b, Block::List(l) if list_kind(l) == kind));
    let container_blocks = container_mut(doc, &k)?;
    let taken: Vec<Block> = container_blocks.drain(first..=last).collect();
    let mut produced: Vec<Block> = Vec::new();
    if all_same {
        for block in taken {
            match block {
                Block::List(list) => flatten_items(list, &mut produced),
                other => produced.push(other),
            }
        }
    } else {
        let mut current: Option<List> = None;
        for block in taken {
            match block {
                Block::Paragraph(p) => {
                    let list = current.get_or_insert_with(|| new_list(kind, p.props.direction));
                    list.items.push(item_from(p.props, p.children));
                }
                Block::Heading(h) => {
                    let list = current.get_or_insert_with(|| new_list(kind, h.props.direction));
                    list.items.push(item_from(h.props, h.children));
                }
                Block::Quote(q) => {
                    let list = current.get_or_insert_with(|| new_list(kind, q.props.direction));
                    list.items.push(item_from(q.props, q.children));
                }
                Block::List(mut list) => {
                    retype(&mut list, kind);
                    match current.as_mut() {
                        Some(open) => open.items.append(&mut list.items),
                        None => current = Some(list),
                    }
                }
                other => {
                    if let Some(mut list) = current.take() {
                        renumber(&mut list);
                        produced.push(Block::List(list));
                    }
                    produced.push(other);
                }
            }
        }
        if let Some(mut list) = current.take() {
            renumber(&mut list);
            produced.push(Block::List(list));
        }
    }
    let after_paths = text_paths_in(&produced, &[]);
    let count = produced.len();
    container_blocks.splice(first..first, produced);
    let absolute = |index: usize, fallback_offset: usize| -> Option<Position> {
        let local = after_paths.get(index).or(after_paths.last())?;
        let mut steps = k.clone();
        let mut rest = local.steps().to_vec();
        rest[0] += first as u32;
        steps.extend(rest);
        Some(pos(BlockPath(steps), fallback_offset))
    };
    let _ = count;
    let a = absolute(s_index, s.offset)?;
    let b = absolute(e_index, e.offset)?;
    Some((a, b))
}

pub fn indent_or_outdent(doc: &mut Document, path: &BlockPath, outward: bool) -> Option<Position> {
    if tree::host_kind(doc, path) != Some(tree::HostKind::Item) {
        return None;
    }
    if outward {
        Some(outdent(doc, path))
    } else {
        indent(doc, path)
    }
}

pub fn item_text(item: &ListItem) -> Vec<Inline> {
    item_inlines(item)
}

pub fn trim_head(inlines: &mut Vec<Inline>, offset: usize) {
    let tail = split_off(inlines, offset);
    *inlines = tail;
    inline::normalize(inlines);
}
