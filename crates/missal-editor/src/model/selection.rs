use std::cmp::Ordering;

use missal_core::Document;
use missal_page::{Affinity, BlockPath, Position};

use super::inline::{len, snap};
use super::tree::{self, all_text_paths, cell_chain, container, first_text, inlines, last_text};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub anchor: Position,
    pub head: Position,
}

pub fn pos(path: BlockPath, offset: usize) -> Position {
    Position::new(0, path, offset)
}

pub fn order(a: &Position, b: &Position) -> Ordering {
    a.path.cmp(&b.path).then(a.offset.cmp(&b.offset))
}

impl Selection {
    pub fn caret(position: Position) -> Selection {
        Selection {
            anchor: position.clone(),
            head: position,
        }
    }

    pub fn new(anchor: Position, head: Position) -> Selection {
        Selection { anchor, head }
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor.path == self.head.path && self.anchor.offset == self.head.offset
    }

    pub fn ordered(&self) -> (Position, Position) {
        if order(&self.anchor, &self.head) == Ordering::Greater {
            (self.head.clone(), self.anchor.clone())
        } else {
            (self.anchor.clone(), self.head.clone())
        }
    }

    pub fn is_reversed(&self) -> bool {
        order(&self.head, &self.anchor) == Ordering::Less
    }
}

pub fn block_len(doc: &Document, path: &BlockPath) -> Option<usize> {
    inlines(doc, path).map(|inlines| len(&inlines))
}

pub fn start_of(path: BlockPath) -> Position {
    pos(path, 0)
}

pub fn end_of(doc: &Document, path: BlockPath) -> Position {
    let length = block_len(doc, &path).unwrap_or(0);
    pos(path, length).with_affinity(Affinity::Upstream)
}

pub fn document_start(doc: &Document) -> Option<Position> {
    first_text(doc).map(start_of)
}

pub fn document_end(doc: &Document) -> Option<Position> {
    last_text(doc).map(|path| end_of(doc, path))
}

pub fn is_valid(doc: &Document, position: &Position) -> bool {
    match inlines(doc, &position.path) {
        Some(found) => {
            position.offset <= len(&found) && snap(&found, position.offset) == position.offset
        }
        None => false,
    }
}

pub fn clamp(doc: &Document, position: &Position) -> Position {
    if let Some(found) = inlines(doc, &position.path) {
        let offset = snap(&found, position.offset.min(len(&found)));
        return Position {
            document: 0,
            path: position.path.clone(),
            offset,
            affinity: position.affinity,
        };
    }
    let paths = all_text_paths(doc);
    let index = paths.partition_point(|p| p < &position.path);
    let path = paths
        .get(index)
        .or_else(|| paths.last())
        .cloned()
        .unwrap_or_else(|| BlockPath::top(0));
    pos(path, 0)
}

pub fn clamp_selection(doc: &Document, selection: &Selection) -> Selection {
    Selection {
        anchor: clamp(doc, &selection.anchor),
        head: clamp(doc, &selection.head),
    }
}

fn cell_bounds(doc: &Document, cell: &[u32]) -> Option<(Position, Position)> {
    let blocks = container(doc, cell)?;
    let paths = tree::text_paths_in(blocks, cell);
    let first = paths.first()?.clone();
    let last = paths.last()?.clone();
    Some((start_of(first), end_of(doc, last)))
}

pub fn effective_range(doc: &Document, selection: &Selection) -> (Position, Position) {
    let (mut start, mut end) = selection.ordered();
    if start.path == end.path {
        return (start, end);
    }
    let a = cell_chain(doc, &start.path);
    let b = cell_chain(doc, &end.path);
    let common = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    if a.len() > common
        && let Some((first, _)) = cell_bounds(doc, &a[common])
    {
        start = first;
    }
    if b.len() > common
        && let Some((_, last)) = cell_bounds(doc, &b[common])
    {
        end = last;
    }
    (start, end)
}

pub fn spans_cells(doc: &Document, selection: &Selection) -> bool {
    let (start, end) = selection.ordered();
    cell_chain(doc, &start.path) != cell_chain(doc, &end.path)
}

pub fn covered_cells(doc: &Document, selection: &Selection) -> Vec<Vec<u32>> {
    if !spans_cells(doc, selection) {
        return Vec::new();
    }
    let (start, end) = effective_range(doc, selection);
    let mut cells: Vec<Vec<u32>> = Vec::new();
    for path in all_text_paths(doc) {
        if path < start.path || path > end.path {
            continue;
        }
        if let Some(cell) = tree::cell_of(doc, &path)
            && cells.last() != Some(&cell)
            && !cells.contains(&cell)
        {
            cells.push(cell);
        }
    }
    cells
}
