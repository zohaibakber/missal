use std::ops::Range;

use unicode_general_category::{GeneralCategory, get_general_category};

use super::text_layout::{ClusterKind, Edge, TextLayout, TextPosition};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HorizontalDirection {
    Left,
    Right,
}

const SAME_X: f32 = 0.5;

impl TextLayout {
    fn boundary_of(&self, pos: TextPosition) -> (usize, usize) {
        match self.resolve(pos) {
            Some((cluster, edge)) => {
                let c = &self.geo.clusters[cluster];
                let line = c.line as usize;
                let info = &self.geo.lines[line];
                if c.kind == ClusterKind::Newline {
                    let k = if self.rtl { 0 } else { info.nav.len() };
                    return (line, k);
                }
                let v = cluster - info.nav.start;
                let k = match (edge, c.rtl) {
                    (Edge::Leading, false) | (Edge::Trailing, true) => v,
                    (Edge::Leading, true) | (Edge::Trailing, false) => v + 1,
                };
                (line, k)
            }
            None => {
                let line = self.caret(pos).line;
                (line, 0)
            }
        }
    }

    fn line_left_end(&self, line: usize) -> TextPosition {
        let info = &self.geo.lines[line];
        if info.nav.is_empty() {
            self.normalize(TextPosition::downstream(info.text_range.start))
        } else {
            self.normalize(self.left_edge_position(info.nav.start))
        }
    }

    fn line_right_end(&self, line: usize) -> TextPosition {
        let info = &self.geo.lines[line];
        if info.nav.is_empty() {
            self.normalize(TextPosition::downstream(info.text_range.start))
        } else {
            self.normalize(self.right_edge_position(info.nav.end - 1))
        }
    }

    pub fn move_visual(&self, pos: TextPosition, direction: HorizontalDirection) -> TextPosition {
        let pos = self.normalize(pos);
        if self.geo.lines.is_empty() {
            return pos;
        }
        let start_x = self.caret(pos).x;
        let (line, mut k) = self.boundary_of(pos);
        let info = self.geo.lines[line].clone();
        let n = info.nav.len();
        loop {
            let candidate = match direction {
                HorizontalDirection::Left if k > 0 => {
                    k -= 1;
                    self.normalize(self.left_edge_position(info.nav.start + k))
                }
                HorizontalDirection::Right if k < n => {
                    k += 1;
                    self.normalize(self.right_edge_position(info.nav.start + k - 1))
                }
                _ => {
                    let forward = (direction == HorizontalDirection::Left) == self.rtl;
                    let next = if forward {
                        line + 1
                    } else {
                        match line.checked_sub(1) {
                            Some(l) => l,
                            None => return pos,
                        }
                    };
                    if next >= self.geo.lines.len() {
                        return pos;
                    }
                    return match direction {
                        HorizontalDirection::Left => self.line_right_end(next),
                        HorizontalDirection::Right => self.line_left_end(next),
                    };
                }
            };
            let at_edge = k == 0 || k == n;
            let moved = (self.caret(candidate).x - start_x).abs() > SAME_X;
            if (moved || at_edge) && candidate != pos {
                return candidate;
            }
            if at_edge {
                if candidate == pos {
                    continue;
                }
                return candidate;
            }
        }
    }

    pub fn move_vertical(
        &self,
        pos: TextPosition,
        lines: isize,
        sticky_x: Option<f32>,
    ) -> (TextPosition, f32) {
        let caret = self.caret(pos);
        let x = sticky_x.unwrap_or(caret.x);
        let target = caret.line as isize + lines;
        if target < 0 {
            return (TextPosition::downstream(0), x);
        }
        if target as usize >= self.geo.lines.len() {
            return (self.end_position(), x);
        }
        (self.hit_test_line(target as usize, x), x)
    }

    pub fn line_start(&self, pos: TextPosition) -> TextPosition {
        let line = self.caret(pos).line;
        match self.geo.lines.get(line) {
            Some(info) => self.normalize(TextPosition::downstream(info.text_range.start)),
            None => TextPosition::downstream(0),
        }
    }

    pub fn line_end(&self, pos: TextPosition) -> TextPosition {
        let line = self.caret(pos).line;
        let Some(info) = self.geo.lines.get(line) else {
            return self.end_position();
        };
        let end = info.text_range.end;
        if end > info.text_range.start && self.text()[..end].ends_with('\n') {
            return self.normalize(TextPosition::downstream(end - 1));
        }
        self.normalize(TextPosition::upstream(end))
    }

    pub fn line_range(&self, pos: TextPosition) -> Range<usize> {
        let line = self.caret(pos).line;
        self.geo
            .lines
            .get(line)
            .map(|l| l.text_range.clone())
            .unwrap_or(0..0)
    }

    pub fn word_left(&self, pos: TextPosition) -> TextPosition {
        let index = if self.rtl {
            next_word_end(self.text(), pos.index)
        } else {
            previous_word_start(self.text(), pos.index)
        };
        self.normalize(TextPosition::downstream(index))
    }

    pub fn word_right(&self, pos: TextPosition) -> TextPosition {
        let index = if self.rtl {
            previous_word_start(self.text(), pos.index)
        } else {
            next_word_end(self.text(), pos.index)
        };
        self.normalize(TextPosition::downstream(index))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CharClass {
    Word,
    Space,
    Newline,
    Punct,
}

fn class_of(c: char) -> CharClass {
    if c == '\n' || c == '\r' || c == '\u{2029}' {
        return CharClass::Newline;
    }
    if c.is_whitespace() {
        return CharClass::Space;
    }
    if c == '\u{200C}' || c == '\u{200D}' || c.is_alphanumeric() {
        return CharClass::Word;
    }
    match get_general_category(c) {
        GeneralCategory::NonspacingMark
        | GeneralCategory::SpacingMark
        | GeneralCategory::EnclosingMark
        | GeneralCategory::Format
        | GeneralCategory::ConnectorPunctuation => CharClass::Word,
        _ => CharClass::Punct,
    }
}

fn floor_char(text: &str, index: usize) -> usize {
    let mut index = index.min(text.len());
    while !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

pub fn previous_word_start(text: &str, index: usize) -> usize {
    let index = floor_char(text, index);
    let mut chars = text[..index].char_indices().rev().peekable();
    while let Some(&(_, c)) = chars.peek() {
        if matches!(class_of(c), CharClass::Space | CharClass::Newline) {
            chars.next();
        } else {
            break;
        }
    }
    let Some(&(mut start, first)) = chars.peek() else {
        return 0;
    };
    let class = class_of(first);
    for (i, c) in chars {
        if class_of(c) != class {
            break;
        }
        start = i;
    }
    start
}

pub fn next_word_end(text: &str, index: usize) -> usize {
    let index = floor_char(text, index);
    let mut chars = text[index..].char_indices().peekable();
    while let Some(&(_, c)) = chars.peek() {
        if matches!(class_of(c), CharClass::Space | CharClass::Newline) {
            chars.next();
        } else {
            break;
        }
    }
    let Some(&(_, first)) = chars.peek() else {
        return text.len();
    };
    let class = class_of(first);
    let mut end = text.len();
    for (i, c) in chars {
        if class_of(c) != class {
            end = index + i;
            break;
        }
    }
    end
}

pub fn next_word_start(text: &str, index: usize) -> usize {
    let index = floor_char(text, index);
    let mut chars = text[index..].char_indices().peekable();
    if let Some(&(_, c)) = chars.peek() {
        let class = class_of(c);
        if !matches!(class, CharClass::Space | CharClass::Newline) {
            while let Some(&(_, c)) = chars.peek() {
                if class_of(c) != class {
                    break;
                }
                chars.next();
            }
        }
    }
    for (i, c) in chars {
        if !matches!(class_of(c), CharClass::Space | CharClass::Newline) {
            return index + i;
        }
    }
    text.len()
}

pub fn word_range_at(text: &str, index: usize) -> Range<usize> {
    let index = floor_char(text, index);
    let probe = text[index..]
        .chars()
        .next()
        .filter(|c| class_of(*c) != CharClass::Newline)
        .map(|c| (index, c))
        .or_else(|| text[..index].char_indices().next_back());
    let Some((at, c)) = probe else {
        return index..index;
    };
    let class = class_of(c);
    if class == CharClass::Newline {
        return at..at;
    }
    let mut start = at;
    for (i, c) in text[..at].char_indices().rev() {
        if class_of(c) != class {
            break;
        }
        start = i;
    }
    let mut end = text.len();
    for (i, c) in text[at..].char_indices() {
        if class_of(c) != class {
            end = at + i;
            break;
        }
    }
    start..end
}

pub fn paragraph_range_at(text: &str, index: usize) -> Range<usize> {
    let index = floor_char(text, index);
    let start = text[..index].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let end = text[index..]
        .find('\n')
        .map(|i| index + i)
        .unwrap_or(text.len());
    start..end
}

pub fn previous_char_boundary(text: &str, index: usize) -> usize {
    let index = floor_char(text, index);
    text[..index]
        .char_indices()
        .next_back()
        .map(|(i, _)| i)
        .unwrap_or(0)
}

pub fn next_cluster_boundary(layout: &TextLayout, index: usize) -> usize {
    let text = layout.text();
    let index = floor_char(text, index);
    if index >= text.len() {
        return text.len();
    }
    let next = match layout.cluster_starting_at(index) {
        Some(c) => layout.geo.clusters[c].end(),
        None => index + text[index..].chars().next().map_or(0, char::len_utf8),
    };
    next.max(index + text[index..].chars().next().map_or(0, char::len_utf8))
}
