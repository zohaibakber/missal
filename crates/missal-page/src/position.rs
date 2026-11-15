use std::cmp::Ordering;

use missal_text::Affinity;

#[derive(Clone, Debug, PartialEq, Eq, Hash, Default, PartialOrd, Ord)]
pub struct BlockPath(pub Vec<u32>);

impl BlockPath {
    pub fn top(index: usize) -> BlockPath {
        BlockPath(vec![index as u32])
    }

    pub fn new(steps: &[u32]) -> BlockPath {
        BlockPath(steps.to_vec())
    }

    pub fn child(&self, index: u32) -> BlockPath {
        let mut steps = Vec::with_capacity(self.0.len() + 1);
        steps.extend_from_slice(&self.0);
        steps.push(index);
        BlockPath(steps)
    }

    pub fn top_index(&self) -> usize {
        self.0.first().copied().unwrap_or(0) as usize
    }

    pub fn steps(&self) -> &[u32] {
        &self.0
    }

    pub fn starts_with(&self, prefix: &[u32]) -> bool {
        self.0.starts_with(prefix)
    }

    pub(crate) fn shift_top(&mut self, delta: isize) {
        if let Some(first) = self.0.first_mut() {
            *first = (*first as isize + delta) as u32;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub document: usize,
    pub path: BlockPath,
    pub offset: usize,
    pub affinity: Affinity,
}

impl Position {
    pub fn new(document: usize, path: BlockPath, offset: usize) -> Position {
        Position {
            document,
            path,
            offset,
            affinity: Affinity::Downstream,
        }
    }

    pub fn with_affinity(mut self, affinity: Affinity) -> Position {
        self.affinity = affinity;
        self
    }

    pub fn document_order(&self, other: &Position) -> Ordering {
        self.document
            .cmp(&other.document)
            .then_with(|| self.path.cmp(&other.path))
            .then_with(|| self.offset.cmp(&other.offset))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerticalDirection {
    Up,
    Down,
}
