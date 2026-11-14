use std::cmp::Ordering;
use std::sync::Arc;

use missal_text::{Affinity, HorizontalDirection, TextPosition};

use crate::engine::Packet;
use crate::geom::{Point, Rect};
use crate::paginate::{Piece, TextPiece};
use crate::position::{BlockPath, Position, VerticalDirection};
use crate::text::TextBox;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Caret {
    pub page: usize,
    pub rect: Rect,
    pub rtl: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineHit {
    pub page: usize,
    pub rect: Rect,
}

pub(crate) struct Located<'a> {
    pub page: usize,
    pub piece: &'a TextPiece,
}

impl Packet {
    pub(crate) fn text_pieces(&self, document: usize) -> impl Iterator<Item = Located<'_>> {
        let offset = self.page_offsets.get(document).copied().unwrap_or(0);
        self.docs
            .get(document)
            .into_iter()
            .flat_map(|doc| doc.pages.iter().enumerate())
            .flat_map(move |(local, page)| {
                page.pieces.iter().filter_map(move |piece| match piece {
                    Piece::Text(t) => Some(Located {
                        page: offset + local,
                        piece: t,
                    }),
                    _ => None,
                })
            })
    }

    pub(crate) fn page_text_pieces(&self, page: usize) -> impl Iterator<Item = &TextPiece> {
        self.page(page)
            .into_iter()
            .flat_map(|p| p.pieces.iter())
            .filter_map(|piece| match piece {
                Piece::Text(t) => Some(t),
                _ => None,
            })
    }

    fn pieces_for<'a>(
        &'a self,
        document: usize,
        path: &'a BlockPath,
    ) -> impl Iterator<Item = Located<'a>> + 'a {
        let offset = self.page_offsets.get(document).copied().unwrap_or(0);
        self.docs
            .get(document)
            .into_iter()
            .flat_map(|doc| doc.pages.iter().enumerate())
            .filter(move |(_, page)| page_may_contain(page.pieces.as_slice(), path))
            .flat_map(move |(local, page)| {
                page.pieces.iter().filter_map(move |piece| match piece {
                    Piece::Text(t) if &t.path == path => Some(Located {
                        page: offset + local,
                        piece: t,
                    }),
                    _ => None,
                })
            })
    }

    pub fn text_block(&self, document: usize, path: &BlockPath) -> Option<&Arc<TextBox>> {
        let doc = self.docs.get(document)?;
        for page in &doc.pages {
            if !page_may_contain(page.pieces.as_slice(), path) {
                continue;
            }
            for piece in page.pieces.iter() {
                if let Piece::Text(t) = piece
                    && &t.path == path
                {
                    return Some(&t.node);
                }
            }
        }
        None
    }

    pub fn text_blocks(&self, document: usize) -> Vec<BlockPath> {
        let mut out: Vec<BlockPath> = Vec::new();
        for located in self.text_pieces(document) {
            if out.last() != Some(&located.piece.path) && !out.contains(&located.piece.path) {
                out.push(located.piece.path.clone());
            }
        }
        out
    }

    pub fn layout_position(&self, position: &Position) -> Option<TextPosition> {
        let node = self.text_block(position.document, &position.path)?;
        Some(node.map.to_layout(position.offset, position.affinity))
    }

    pub fn caret(&self, position: &Position) -> Option<Caret> {
        let node = self.text_block(position.document, &position.path)?;
        let layout_position = node.map.to_layout(position.offset, position.affinity);
        let (line, local) = node.caret_local(layout_position);
        let rtl = if node.layout.lines().is_empty() {
            node.layout.is_rtl()
        } else {
            node.layout.caret(layout_position).rtl
        };
        let located = self
            .pieces_for(position.document, &position.path)
            .find(|l| l.piece.lines.contains(&line) || (l.piece.lines.is_empty() && line == 0))?;
        Some(Caret {
            page: located.page,
            rect: local.translate(located.piece.dx, located.piece.dy),
            rtl,
        })
    }

    pub fn line_rect(&self, position: &Position) -> Option<LineHit> {
        let node = self.text_block(position.document, &position.path)?;
        let layout_position = node.map.to_layout(position.offset, position.affinity);
        let (line, _) = node.caret_local(layout_position);
        let located = self
            .pieces_for(position.document, &position.path)
            .find(|l| l.piece.lines.contains(&line))?;
        Some(LineHit {
            page: located.page,
            rect: node
                .line_rect(line)
                .translate(located.piece.dx, located.piece.dy),
        })
    }

    pub fn selection(&self, a: &Position, b: &Position) -> Vec<(usize, Rect)> {
        let (start, end) = if a.document_order(b) == Ordering::Greater {
            (b, a)
        } else {
            (a, b)
        };
        let mut rects = Vec::new();
        if start == end {
            return rects;
        }
        for document in start.document..=end.document {
            for located in self.text_pieces(document) {
                let piece = located.piece;
                let key = (document, &piece.path);
                if key < (start.document, &start.path) || key > (end.document, &end.path) {
                    continue;
                }
                let node = &piece.node;
                let len = node.map.logical_len();
                let from = if document == start.document && piece.path == start.path {
                    start.offset
                } else {
                    0
                };
                let to = if document == end.document && piece.path == end.path {
                    end.offset
                } else {
                    len
                };
                let continues = !(document == end.document && piece.path == end.path);
                if from < to {
                    for (line, rect) in node.selection_local(from..to) {
                        if piece.lines.contains(&line) {
                            rects.push((located.page, rect.translate(piece.dx, piece.dy)));
                        }
                    }
                }
                if continues && !node.lines.is_empty() {
                    let last = node.lines.len() - 1;
                    if piece.lines.contains(&last) {
                        let end_position = node.map.to_layout(len, Affinity::Upstream);
                        let (_, caret) = node.caret_local(end_position);
                        let lb = node.lines[last];
                        let width = 6.0;
                        let x = if node.layout.is_rtl() {
                            caret.x - width
                        } else {
                            caret.x
                        };
                        rects.push((
                            located.page,
                            Rect::new(x, lb.top, width, lb.bottom - lb.top)
                                .translate(piece.dx, piece.dy),
                        ));
                    }
                }
            }
        }
        rects
    }

    pub fn hit_test(&self, page: usize, point: Point) -> Option<Position> {
        let (document, _) = self.locate_page(page)?;
        let mut best: Option<(f32, f32, &TextPiece, usize)> = None;
        for piece in self.page_text_pieces(page) {
            let node = &piece.node;
            for line in piece.lines.clone() {
                if line >= node.lines.len() {
                    continue;
                }
                let rect = node.line_rect_full(line).translate(piece.dx, piece.dy);
                let (dx, dy) = rect.distance_to(point);
                let better = match best {
                    None => true,
                    Some((bx, by, _, _)) => (dx > 0.0, dy, dx) < (bx > 0.0, by, bx),
                };
                if better {
                    best = Some((dx, dy, piece, line));
                }
            }
        }
        let (_, _, piece, line) = best?;
        let position = piece.node.hit_line(line, point.x - piece.dx);
        let (offset, affinity) = piece.node.map.to_logical(position);
        Some(Position {
            document,
            path: piece.path.clone(),
            offset,
            affinity,
        })
    }

    pub fn move_vertical(
        &self,
        position: &Position,
        direction: VerticalDirection,
        goal_x: Option<f32>,
    ) -> Option<(Position, f32)> {
        let caret = self.caret(position)?;
        let node = self.text_block(position.document, &position.path)?;
        let layout_position = node.map.to_layout(position.offset, position.affinity);
        let (current_line, _) = node.caret_local(layout_position);
        let current = self
            .pieces_for(position.document, &position.path)
            .find(|l| l.piece.lines.contains(&current_line))?;
        let line_box = node
            .line_rect(current_line)
            .translate(current.piece.dx, current.piece.dy);
        let x = goal_x.unwrap_or(caret.rect.x);
        let pages = self.document_pages(position.document);
        let mut page = caret.page;
        let mut fallback: Option<(&TextPiece, usize)> = None;
        let found = loop {
            let mut best: Option<((bool, f32, f32), &TextPiece, usize)> = None;
            for piece in self.page_text_pieces(page) {
                let node = &piece.node;
                for line in piece.lines.clone() {
                    if line >= node.lines.len() {
                        continue;
                    }
                    if page == caret.page && piece.path == position.path && line == current_line {
                        continue;
                    }
                    let rect = node.line_rect(line).translate(piece.dx, piece.dy);
                    let eligible = match (direction, page == caret.page) {
                        (VerticalDirection::Down, true) => rect.y >= line_box.bottom() - 1.0,
                        (VerticalDirection::Up, true) => rect.bottom() <= line_box.y + 1.0,
                        _ => true,
                    };
                    if !eligible {
                        continue;
                    }
                    let outside = x < rect.x - 0.5 || x > rect.right() + 0.5;
                    let vertical = match direction {
                        VerticalDirection::Down => rect.y,
                        VerticalDirection::Up => -rect.bottom(),
                    };
                    let horizontal = rect.distance_to(Point::new(x, rect.y)).0;
                    let score = (outside, vertical, horizontal);
                    if best.as_ref().is_none_or(|(s, _, _)| score < *s) {
                        best = Some((score, piece, line));
                    }
                }
            }
            if let Some(((outside, _, _), piece, line)) = best {
                if !outside {
                    break Some((piece, line));
                }
                if fallback.is_none() {
                    fallback = Some((piece, line));
                }
            }
            match direction {
                VerticalDirection::Down if page + 1 < pages.end => page += 1,
                VerticalDirection::Up if page > pages.start => page -= 1,
                _ => break None,
            }
        };
        let (piece, line) = found.or(fallback)?;
        let target = piece.node.hit_line(line, x - piece.dx);
        let (offset, affinity) = piece.node.map.to_logical(target);
        Some((
            Position {
                document: position.document,
                path: piece.path.clone(),
                offset,
                affinity,
            },
            x,
        ))
    }

    pub fn move_horizontal(&self, position: &Position, direction: HorizontalDirection) -> Position {
        let Some(node) = self.text_block(position.document, &position.path) else {
            return position.clone();
        };
        let layout_position = node.map.to_layout(position.offset, position.affinity);
        if !node.layout.lines().is_empty() {
            let moved = node.layout.move_visual(layout_position, direction);
            if moved != node.layout.normalize(layout_position) {
                let (offset, affinity) = node.map.to_logical(moved);
                if offset != position.offset || moved.index != layout_position.index {
                    return Position {
                        document: position.document,
                        path: position.path.clone(),
                        offset,
                        affinity,
                    };
                }
            }
        }
        let forward = (direction == HorizontalDirection::Left) == node.layout.is_rtl();
        let blocks = self.text_blocks(position.document);
        let Some(index) = blocks.iter().position(|p| p == &position.path) else {
            return position.clone();
        };
        let target = if forward {
            blocks.get(index + 1)
        } else {
            index.checked_sub(1).and_then(|i| blocks.get(i))
        };
        match target {
            Some(path) => {
                let offset = if forward {
                    0
                } else {
                    self.text_block(position.document, path)
                        .map(|n| n.map.logical_len())
                        .unwrap_or(0)
                };
                Position::new(position.document, path.clone(), offset)
            }
            None => position.clone(),
        }
    }
}

fn page_may_contain(pieces: &[Piece], path: &BlockPath) -> bool {
    let mut texts = pieces.iter().filter_map(|piece| match piece {
        Piece::Text(t) => Some(&t.path),
        _ => None,
    });
    let Some(first) = texts.next() else {
        return false;
    };
    let last = texts.next_back().unwrap_or(first);
    path >= first && path <= last
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedLine {
    pub path: BlockPath,
    pub line: usize,
    pub page: usize,
    pub rect: Rect,
    pub baseline: f32,
    pub text: std::ops::Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlacedCell {
    pub path: BlockPath,
    pub page: usize,
    pub rect: Rect,
}

impl Packet {
    pub fn lines(&self, document: usize) -> Vec<PlacedLine> {
        let mut out = Vec::new();
        for located in self.text_pieces(document) {
            let piece = located.piece;
            let node = &piece.node;
            for line in piece.lines.clone() {
                let Some(lb) = node.lines.get(line) else {
                    continue;
                };
                let range = node
                    .layout
                    .lines()
                    .get(line)
                    .map(|info| info.text_range.clone())
                    .unwrap_or(0..0);
                let start = node.map.floor_logical(range.start);
                let end = node.map.to_logical(TextPosition::upstream(range.end)).0;
                out.push(PlacedLine {
                    path: piece.path.clone(),
                    line,
                    page: located.page,
                    rect: node.line_rect(line).translate(piece.dx, piece.dy),
                    baseline: lb.baseline + piece.dy,
                    text: start..end,
                });
            }
        }
        out
    }

    pub fn cells(&self, document: usize) -> Vec<PlacedCell> {
        let mut out = Vec::new();
        let offset = self.page_offsets.get(document).copied().unwrap_or(0);
        let Some(doc) = self.docs.get(document) else {
            return out;
        };
        for (local, page) in doc.pages.iter().enumerate() {
            for piece in page.pieces.iter() {
                if let Piece::Table(table) = piece {
                    for cell in &table.cells {
                        let (r, c) = table.node.cells[cell.cell].source;
                        out.push(PlacedCell {
                            path: table.path.child(r).child(c),
                            page: offset + local,
                            rect: cell.rect,
                        });
                    }
                }
            }
        }
        out
    }
}
