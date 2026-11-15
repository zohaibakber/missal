use std::ops::Range;
use std::sync::Arc;

use crate::geom::Rect;
use crate::measure::{Flow, FlowItem, Node, collapse};
use crate::position::BlockPath;
use crate::style::{BreakValue, VerticalAlign};
use crate::table::TableBox;
use crate::text::TextBox;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Inner {
    Lines(usize),
    Table(Box<TableToken>),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct FlowToken {
    pub item: usize,
    pub inner: Option<Inner>,
}

impl FlowToken {
    pub(crate) fn start() -> FlowToken {
        FlowToken {
            item: 0,
            inner: None,
        }
    }

    pub(crate) fn at(item: usize) -> FlowToken {
        FlowToken { item, inner: None }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct TableToken {
    pub row: usize,
    pub conts: Vec<(usize, FlowToken)>,
}

#[derive(Clone, Debug)]
pub(crate) struct TextPiece {
    pub path: BlockPath,
    pub node: Arc<TextBox>,
    pub lines: Range<usize>,
    pub dx: f32,
    pub dy: f32,
    pub first: bool,
    pub last: bool,
}

impl TextPiece {
    pub(crate) fn box_top(&self) -> f32 {
        if self.first {
            0.0
        } else {
            self.node.lines[self.lines.start].top
        }
    }

    pub(crate) fn box_bottom(&self) -> f32 {
        if self.last {
            self.node.height()
        } else {
            self.node.lines[self.lines.end - 1].bottom
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CellPiece {
    pub cell: usize,
    pub rect: Rect,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RowPiece {
    pub row: usize,
    pub y: f32,
    pub height: f32,
    pub starts: bool,
    pub ends: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct TablePiece {
    pub path: BlockPath,
    pub node: Arc<TableBox>,
    pub dx: f32,
    pub rows: Vec<RowPiece>,
    pub cells: Vec<CellPiece>,
}

#[derive(Clone, Debug)]
pub(crate) enum Piece {
    Text(TextPiece),
    Table(TablePiece),
    Rule { path: BlockPath, rect: Rect },
}

impl Piece {
    pub(crate) fn path_mut(&mut self) -> &mut BlockPath {
        match self {
            Piece::Text(t) => &mut t.path,
            Piece::Table(t) => &mut t.path,
            Piece::Rule { path, .. } => path,
        }
    }
}

pub(crate) struct Outcome {
    pub next: Option<FlowToken>,
    pub forced: bool,
    pub placed: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct Region {
    pub x: f32,
    pub top: f32,
    pub limit: f32,
    pub page_empty: bool,
    pub keep_first_margin: bool,
    pub forced_breaks: bool,
    pub repeat_headers: bool,
}

pub(crate) fn token_y(flow_items: &[FlowItem], ys: Option<&[f32]>, token: &FlowToken) -> f32 {
    let Some(ys) = ys else { return 0.0 };
    let Some(&base) = ys.get(token.item) else {
        return ys.last().copied().unwrap_or(0.0)
            + flow_items.last().map(FlowItem::height).unwrap_or(0.0);
    };
    match (&token.inner, &flow_items[token.item].node) {
        (Some(Inner::Lines(line)), Node::Text(text)) => {
            base + text.lines.get(*line).map(|l| l.top).unwrap_or(0.0)
        }
        (Some(Inner::Table(t)), Node::Table(table)) => {
            base + table.rows.get(t.row).map(|r| r.y).unwrap_or(table.height)
        }
        _ => base,
    }
}

fn remaining_height(flow: &Flow, token: &FlowToken) -> f32 {
    (flow.height - token_y(&flow.items, Some(&flow.ys), token)).max(0.0)
}

fn lines_fit(text: &TextBox, start: usize, top_ref: f32, available: f32) -> usize {
    let mut count = 0;
    for line in &text.lines[start..] {
        if line.bottom - top_ref <= available + 0.01 {
            count += 1;
        } else {
            break;
        }
    }
    count
}

pub(crate) fn compose(prefix: &[u32], path: &BlockPath) -> BlockPath {
    if prefix.is_empty() {
        return path.clone();
    }
    let mut steps = Vec::with_capacity(prefix.len() + path.0.len());
    steps.extend_from_slice(prefix);
    steps.extend_from_slice(&path.0);
    BlockPath(steps)
}

pub(crate) fn fragment_flow(
    items: &[FlowItem],
    prefix: &[u32],
    start: &FlowToken,
    region: Region,
    out: &mut Vec<Piece>,
) -> Outcome {
    let mut y = region.top;
    let mut previous_margin: Option<f32> = None;
    let mut page_empty = region.page_empty;
    let mut placed = false;
    let mut index = start.item;
    let mut inner = start.inner.clone();
    let mut placed_marks: Vec<(usize, usize, f32, Option<f32>)> = Vec::new();
    while index < items.len() {
        let item = &items[index];
        let is_start = index == start.item && start.inner.is_none() && inner.is_none();
        let forced_before = region.forced_breaks
            && index > 0
            && (item.break_before == BreakValue::Page
                || items[index - 1].break_after == BreakValue::Page);
        if forced_before && !(is_start && !placed) {
            return Outcome {
                next: Some(FlowToken::at(index)),
                forced: true,
                placed,
            };
        }
        let gap = if inner.is_some() {
            0.0
        } else {
            match previous_margin {
                Some(mb) => collapse(mb, item.margin_top),
                None if region.keep_first_margin || !page_empty => item.margin_top,
                None => 0.0,
            }
        };
        let item_top = y + gap;
        let available = region.limit - item_top;
        let mark = (index, out.len(), y, previous_margin);
        match &item.node {
            Node::Text(text) => {
                let start_line = match &inner {
                    Some(Inner::Lines(l)) => *l,
                    _ => 0,
                };
                let first = start_line == 0;
                let top_ref = if first {
                    0.0
                } else {
                    text.lines[start_line].top
                };
                let total = text.lines.len();
                let remaining = text.height() - top_ref;
                if remaining <= available + 0.01 || total == 0 {
                    out.push(Piece::Text(TextPiece {
                        path: compose(prefix, &item.path),
                        node: text.clone(),
                        lines: start_line..total,
                        dx: region.x + item.x,
                        dy: item_top - top_ref,
                        first,
                        last: true,
                    }));
                    y = item_top + remaining;
                } else {
                    let avoid = item.break_inside == BreakValue::Avoid;
                    let left = total - start_line;
                    let mut fit = lines_fit(text, start_line, top_ref, available);
                    if avoid && !page_empty {
                        fit = 0;
                    }
                    if fit < left {
                        let widows = text.widows.max(1);
                        let orphans = text.orphans.max(1);
                        if left - fit < widows {
                            fit = left.saturating_sub(widows);
                        }
                        if fit < orphans {
                            fit = 0;
                        }
                    }
                    if fit == 0 {
                        if page_empty {
                            fit = lines_fit(text, start_line, top_ref, available)
                                .max(1)
                                .min(left);
                        } else {
                            if let Some(back) = avoid_backtrack(items, index, &placed_marks, out) {
                                return back;
                            }
                            return Outcome {
                                next: Some(FlowToken {
                                    item: index,
                                    inner: inner.clone(),
                                }),
                                forced: false,
                                placed,
                            };
                        }
                    }
                    let end = start_line + fit;
                    out.push(Piece::Text(TextPiece {
                        path: compose(prefix, &item.path),
                        node: text.clone(),
                        lines: start_line..end,
                        dx: region.x + item.x,
                        dy: item_top - top_ref,
                        first,
                        last: end == total,
                    }));
                    if end < total {
                        return Outcome {
                            next: Some(FlowToken {
                                item: index,
                                inner: Some(Inner::Lines(end)),
                            }),
                            forced: false,
                            placed: true,
                        };
                    }
                    y = item_top + (text.height() - top_ref);
                }
            }
            Node::Table(table) => {
                let token = match &inner {
                    Some(Inner::Table(t)) => (**t).clone(),
                    _ => TableToken {
                        row: 0,
                        conts: Vec::new(),
                    },
                };
                let continuing = inner.is_some();
                let table_path = compose(prefix, &item.path);
                let result = fragment_table(
                    table,
                    &table_path,
                    &token,
                    continuing,
                    Region {
                        x: region.x + item.x,
                        top: item_top,
                        page_empty,
                        ..region
                    },
                    out,
                );
                match result {
                    TableResult::Done(bottom) => y = bottom,
                    TableResult::Break { token, placed_rows } => {
                        if !placed_rows && !page_empty {
                            if let Some(back) = avoid_backtrack(items, index, &placed_marks, out) {
                                return back;
                            }
                            return Outcome {
                                next: Some(FlowToken {
                                    item: index,
                                    inner: inner.clone(),
                                }),
                                forced: false,
                                placed,
                            };
                        }
                        return Outcome {
                            next: Some(FlowToken {
                                item: index,
                                inner: Some(Inner::Table(Box::new(token))),
                            }),
                            forced: false,
                            placed: true,
                        };
                    }
                }
            }
            node => {
                let height = node.height();
                if height > available + 0.01 && !page_empty {
                    if let Some(back) = avoid_backtrack(items, index, &placed_marks, out) {
                        return back;
                    }
                    return Outcome {
                        next: Some(FlowToken::at(index)),
                        forced: false,
                        placed,
                    };
                }
                if let Node::Rule { width, height } = node {
                    out.push(Piece::Rule {
                        path: compose(prefix, &item.path),
                        rect: Rect::new(region.x + item.x, item_top, *width, *height),
                    });
                }
                y = item_top + height;
            }
        }
        placed_marks.push(mark);
        placed = true;
        page_empty = false;
        previous_margin = Some(item.margin_bottom);
        inner = None;
        index += 1;
    }
    Outcome {
        next: None,
        forced: false,
        placed,
    }
}

fn avoid_backtrack(
    items: &[FlowItem],
    index: usize,
    marks: &[(usize, usize, f32, Option<f32>)],
    out: &mut Vec<Piece>,
) -> Option<Outcome> {
    let avoid = items[index].break_before == BreakValue::Avoid
        || (index > 0 && items[index - 1].break_after == BreakValue::Avoid);
    if !avoid || marks.len() < 2 {
        return None;
    }
    let &(previous, pieces, _, _) = marks.last()?;
    if previous + 1 != index {
        return None;
    }
    out.truncate(pieces);
    Some(Outcome {
        next: Some(FlowToken::at(previous)),
        forced: false,
        placed: true,
    })
}

enum TableResult {
    Done(f32),
    Break {
        token: TableToken,
        placed_rows: bool,
    },
}

fn cont_of(conts: &[(usize, FlowToken)], cell: usize) -> Option<&FlowToken> {
    conts.iter().find(|(c, _)| *c == cell).map(|(_, t)| t)
}

fn valign_offset(table: &TableBox, cell: usize, box_height: f32) -> f32 {
    let c = &table.cells[cell];
    let inner = box_height - c.chrome_top() - c.chrome_bottom();
    let free = (inner - c.flow.height).max(0.0);
    match c.valign {
        VerticalAlign::Middle => free / 2.0,
        VerticalAlign::Bottom => free,
        _ => 0.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn place_cell_content(
    prefix: &[u32],
    table: &TableBox,
    cell: usize,
    token: &FlowToken,
    x: f32,
    top: f32,
    limit: f32,
    page_empty: bool,
    region: Region,
    out: &mut Vec<Piece>,
) -> Outcome {
    let c = &table.cells[cell];
    fragment_flow(
        &c.flow.items,
        prefix,
        token,
        Region {
            x: x + c.content_x(),
            top,
            limit,
            page_empty,
            keep_first_margin: true,
            forced_breaks: false,
            repeat_headers: region.repeat_headers,
        },
        out,
    )
}

fn fragment_table(
    table: &Arc<TableBox>,
    path: &BlockPath,
    token: &TableToken,
    continuing: bool,
    region: Region,
    out: &mut Vec<Piece>,
) -> TableResult {
    let x = region.x + table.x;
    let mut y = region.top;
    let table_piece_index = out.len();
    out.push(Piece::Table(TablePiece {
        path: path.clone(),
        node: table.clone(),
        dx: x,
        rows: Vec::new(),
        cells: Vec::new(),
    }));
    let mut rows: Vec<RowPiece> = Vec::new();
    let mut cells: Vec<CellPiece> = Vec::new();
    let mut page_empty = region.page_empty;
    let nrows = table.rows.len();
    if continuing
        && region.repeat_headers
        && table.header_rows > 0
        && token.row >= table.header_rows
    {
        let header_height: f32 = table.rows[..table.header_rows]
            .iter()
            .map(|r| r.height)
            .sum();
        if y + header_height <= region.limit {
            for r in 0..table.header_rows {
                let height = table.rows[r].height;
                place_full_row(path, table, r, x, y, height, &[], region, &mut cells, out);
                rows.push(RowPiece {
                    row: r,
                    y,
                    height,
                    starts: true,
                    ends: true,
                });
                y += height;
            }
        }
    }
    let mut r = token.row;
    let mut conts: Vec<(usize, FlowToken)> = token.conts.clone();
    let mut placed_rows = false;
    while r < nrows {
        let continuing_row = !conts.is_empty();
        let row_height = if continuing_row {
            let mut h: f32 = 0.0;
            for (cell, t) in &conts {
                let c = &table.cells[*cell];
                h = h.max(remaining_height(&c.flow, t) + c.chrome_bottom());
            }
            h
        } else {
            table.rows[r].height
        };
        if y + row_height <= region.limit + 0.01 {
            place_full_row(
                path, table, r, x, y, row_height, &conts, region, &mut cells, out,
            );
            rows.push(RowPiece {
                row: r,
                y,
                height: row_height,
                starts: !continuing_row,
                ends: true,
            });
            y += row_height;
            r += 1;
            conts.clear();
            placed_rows = true;
            page_empty = false;
            continue;
        }
        let avoid = table.rows[r].break_inside == BreakValue::Avoid;
        if avoid && !page_empty {
            break;
        }
        let available = region.limit - y;
        let mark = out.len();
        let mut next_conts = Vec::new();
        let mut any = false;
        let overlapping: Vec<usize> = table.cells_in_row(r).map(|(i, _)| i).collect();
        for cell in overlapping {
            let c = &table.cells[cell];
            let (start, content_top) = match cont_of(&conts, cell) {
                Some(t) => (t.clone(), y),
                None if c.row == r && !continuing_row => {
                    let offset = valign_offset(table, cell, row_box_height(table, cell));
                    (FlowToken::start(), y + c.chrome_top() + offset)
                }
                None => continue,
            };
            let outcome = place_cell_content(
                &path.0,
                table,
                cell,
                &start,
                x,
                content_top,
                region.limit,
                page_empty,
                region,
                out,
            );
            any |= outcome.placed;
            if let Some(next) = outcome.next {
                next_conts.push((cell, next));
            }
        }
        if !any && !page_empty {
            out.truncate(mark);
            break;
        }
        let height = available.max(0.0);
        push_cell_boxes(table, r, x, y, height, &mut cells);
        rows.push(RowPiece {
            row: r,
            y,
            height,
            starts: !continuing_row,
            ends: false,
        });
        placed_rows = true;
        if next_conts.is_empty() {
            for (cell, c) in table.cells_in_row(r) {
                if c.row == r && !c.flow.items.is_empty() {
                    next_conts.push((cell, FlowToken::at(c.flow.items.len())));
                }
            }
        }
        conts = next_conts;
        finish_table_piece(out, table_piece_index, rows, cells);
        return TableResult::Break {
            token: TableToken { row: r, conts },
            placed_rows,
        };
    }
    let done = r >= nrows;
    finish_table_piece(out, table_piece_index, rows, cells);
    if done {
        TableResult::Done(y)
    } else {
        TableResult::Break {
            token: TableToken { row: r, conts },
            placed_rows,
        }
    }
}

fn row_box_height(table: &TableBox, cell: usize) -> f32 {
    let c = &table.cells[cell];
    let end = (c.row + c.row_span).min(table.rows.len());
    table.rows[c.row..end].iter().map(|r| r.height).sum()
}

fn finish_table_piece(out: &mut [Piece], index: usize, rows: Vec<RowPiece>, cells: Vec<CellPiece>) {
    if let Some(Piece::Table(piece)) = out.get_mut(index) {
        piece.rows = rows;
        piece.cells = cells;
    }
}

fn push_cell_boxes(
    table: &TableBox,
    row: usize,
    x: f32,
    y: f32,
    height: f32,
    cells: &mut Vec<CellPiece>,
) {
    for (index, c) in table.cells_in_row(row) {
        if let Some(existing) = cells.iter_mut().find(|p| p.cell == index) {
            let bottom = y + height;
            existing.rect.height = bottom - existing.rect.y;
            continue;
        }
        cells.push(CellPiece {
            cell: index,
            rect: Rect::new(x + c.x0, y, c.x1 - c.x0, height),
        });
    }
}

#[allow(clippy::too_many_arguments)]
fn place_full_row(
    path: &BlockPath,
    table: &TableBox,
    row: usize,
    x: f32,
    y: f32,
    height: f32,
    conts: &[(usize, FlowToken)],
    region: Region,
    cells: &mut Vec<CellPiece>,
    out: &mut Vec<Piece>,
) {
    push_cell_boxes(table, row, x, y, height, cells);
    for (index, c) in table.cells_in_row(row) {
        if let Some(token) = cont_of(conts, index) {
            place_cell_content(
                &path.0,
                table,
                index,
                token,
                x,
                y,
                f32::INFINITY,
                true,
                region,
                out,
            );
        } else if c.row == row && conts.is_empty() {
            let full = row_box_height(table, index);
            let offset = valign_offset(table, index, full);
            place_cell_content(
                &path.0,
                table,
                index,
                &FlowToken::start(),
                x,
                y + c.chrome_top() + offset,
                f32::INFINITY,
                true,
                region,
                out,
            );
        }
    }
}
