use std::cmp::Ordering;
use std::ops::Range;

use missal_core::document::Document;
use missal_core::text::{Folded, fold_char};
use missal_page::{BlockPath, Edit, OBJECT, Position};

use super::inline;
use super::tree::{inlines, text_paths_of_block};

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Query {
    pub text: String,
    pub match_case: bool,
}

impl Query {
    pub fn new(text: impl Into<String>, match_case: bool) -> Query {
        Query {
            text: text.into(),
            match_case,
        }
    }

    pub fn needle(&self) -> String {
        let text: String = self
            .text
            .chars()
            .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
            .filter(|&c| c != OBJECT)
            .collect();
        if self.match_case {
            text
        } else {
            text.chars().filter_map(fold_char).collect()
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Match {
    pub path: BlockPath,
    pub range: Range<usize>,
}

impl Match {
    pub fn start(&self) -> Position {
        Position::new(0, self.path.clone(), self.range.start)
    }

    pub fn end(&self) -> Position {
        Position::new(0, self.path.clone(), self.range.end)
    }

    fn cmp_at(&self, path: &BlockPath, offset: usize) -> Ordering {
        self.path.cmp(path).then(self.range.start.cmp(&offset))
    }
}

struct Scanner<'a> {
    doc: &'a Document,
    needle: String,
    match_case: bool,
    folder: Folded,
    out: Vec<Match>,
}

impl<'a> Scanner<'a> {
    fn new(doc: &'a Document, query: &Query) -> Option<Scanner<'a>> {
        let needle = query.needle();
        (!needle.is_empty()).then(|| Scanner {
            doc,
            needle,
            match_case: query.match_case,
            folder: Folded::default(),
            out: Vec::new(),
        })
    }

    fn scan(&mut self, path: BlockPath) {
        let Some(found) = inlines(self.doc, &path) else {
            return;
        };
        let text = inline::text_of(&found);
        if text.len() < self.needle.len() && self.match_case {
            return;
        }
        if self.match_case {
            for (start, found) in text.match_indices(self.needle.as_str()) {
                self.out.push(Match {
                    path: path.clone(),
                    range: start..start + found.len(),
                });
            }
            return;
        }
        self.folder.fold(&text);
        for (start, found) in self.folder.text().match_indices(self.needle.as_str()) {
            let range = self.folder.source_range(start..start + found.len());
            self.out.push(Match {
                path: path.clone(),
                range,
            });
        }
    }
}

pub fn find_all(doc: &Document, query: &Query) -> Vec<Match> {
    find_in_blocks(doc, query, 0..doc.blocks.len())
}

pub fn find_in_blocks(doc: &Document, query: &Query, blocks: Range<usize>) -> Vec<Match> {
    let Some(mut scanner) = Scanner::new(doc, query) else {
        return Vec::new();
    };
    let end = blocks.end.min(doc.blocks.len());
    let start = blocks.start.min(end);
    for (index, block) in doc.blocks[start..end].iter().enumerate() {
        for path in text_paths_of_block(block, BlockPath::top(start + index)) {
            scanner.scan(path);
        }
    }
    scanner.out
}

pub fn find_in_paths(doc: &Document, query: &Query, paths: &[BlockPath]) -> Vec<Match> {
    let Some(mut scanner) = Scanner::new(doc, query) else {
        return Vec::new();
    };
    for path in paths {
        scanner.scan(path.clone());
    }
    scanner.out
}

pub fn narrows(previous: &Query, next: &Query) -> bool {
    previous.match_case == next.match_case && {
        let before = previous.needle();
        !before.is_empty() && next.needle().starts_with(&before)
    }
}

pub fn retarget(matches: &mut Vec<Match>, doc: &Document, query: &Query, edit: &Edit) -> bool {
    let (start, removed, inserted) = match edit {
        Edit::Block(path) => (path.top_index(), 1, 1),
        Edit::Splice {
            start,
            removed,
            inserted,
        } => (*start, *removed, *inserted),
        Edit::Document => return false,
    };
    let lo = matches.partition_point(|m| m.path.top_index() < start);
    let hi = matches.partition_point(|m| m.path.top_index() < start + removed);
    if inserted != removed {
        for found in &mut matches[hi..] {
            if let Some(top) = found.path.0.first_mut() {
                *top = (*top as usize + inserted - removed) as u32;
            }
        }
    }
    let fresh = find_in_blocks(doc, query, start..start + inserted);
    matches.splice(lo..hi, fresh);
    true
}

pub fn exact(matches: &[Match], start: &Position, end: &Position) -> Option<usize> {
    if start.path != end.path {
        return None;
    }
    let index = matches
        .binary_search_by(|m| m.cmp_at(&start.path, start.offset))
        .ok()?;
    (matches[index].range.end == end.offset).then_some(index)
}

pub fn at_or_after(matches: &[Match], at: &Position) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let index = matches.partition_point(|m| m.cmp_at(&at.path, at.offset) == Ordering::Less);
    Some(if index < matches.len() { index } else { 0 })
}

pub fn after(matches: &[Match], at: &Position) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let index = matches.partition_point(|m| m.cmp_at(&at.path, at.offset) != Ordering::Greater);
    Some(if index < matches.len() { index } else { 0 })
}

pub fn before(matches: &[Match], at: &Position) -> Option<usize> {
    if matches.is_empty() {
        return None;
    }
    let index = matches.partition_point(|m| m.cmp_at(&at.path, at.offset) == Ordering::Less);
    Some(if index == 0 {
        matches.len() - 1
    } else {
        index - 1
    })
}
