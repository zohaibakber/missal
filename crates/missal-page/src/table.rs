use std::sync::Arc;

use missal_core::document::{Child, Table, TableCell, TableRow};
use missal_text::Rgba;

use crate::css::{self, Length};
use crate::measure::{Ctx, Flow, compute_element, measure_blocks};
use crate::position::BlockPath;
use crate::style::{BorderSide, BorderStyle, BoxProps, BreakValue, Computed, Edges, TextProps, VerticalAlign};

#[derive(Clone, Debug)]
pub struct CellBox {
    pub row: usize,
    pub col: usize,
    pub row_span: usize,
    pub col_span: usize,
    pub source: (u32, u32),
    pub x0: f32,
    pub x1: f32,
    pub padding: Edges,
    pub border_half: Edges,
    pub valign: VerticalAlign,
    pub background: Option<Rgba>,
    pub(crate) flow: Arc<Flow>,
    pub content_width: f32,
}

impl CellBox {
    pub(crate) fn content_x(&self) -> f32 {
        self.x0 + self.border_half.left + self.padding.left
    }

    pub(crate) fn chrome_top(&self) -> f32 {
        self.border_half.top + self.padding.top
    }

    pub(crate) fn chrome_bottom(&self) -> f32 {
        self.border_half.bottom + self.padding.bottom
    }
}

#[derive(Clone, Debug)]
pub struct RowBox {
    pub y: f32,
    pub height: f32,
    pub min_height: f32,
    pub header: bool,
    pub break_inside: BreakValue,
}

#[derive(Clone, Debug)]
pub struct TableBox {
    pub x: f32,
    pub width: f32,
    pub height: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub break_before: BreakValue,
    pub break_after: BreakValue,
    pub break_inside: BreakValue,
    pub ncols: usize,
    pub cols: Vec<f32>,
    pub rows: Vec<RowBox>,
    pub cells: Vec<CellBox>,
    pub h_edges: Vec<BorderSide>,
    pub v_edges: Vec<BorderSide>,
    pub header_rows: usize,
    pub rtl: bool,
    pub(crate) cell_styles: Vec<Computed>,
    available: f32,
}

impl TableBox {
    pub(crate) fn h_edge(&self, row_line: usize, physical_col: usize) -> BorderSide {
        self.h_edges[row_line * self.ncols + physical_col]
    }

    pub(crate) fn v_edge(&self, row: usize, line: usize) -> BorderSide {
        self.v_edges[row * (self.ncols + 1) + line]
    }

    pub(crate) fn physical(&self, col: usize, span: usize) -> (usize, usize) {
        if self.rtl {
            (self.ncols - col - span, self.ncols - col)
        } else {
            (col, col + span)
        }
    }

    pub(crate) fn cells_in_row(&self, row: usize) -> impl Iterator<Item = (usize, &CellBox)> {
        self.cells
            .iter()
            .enumerate()
            .filter(move |(_, c)| c.row <= row && row < c.row + c.row_span)
    }

    pub(crate) fn row_bottom(&self, row: usize) -> f32 {
        self.rows[row].y + self.rows[row].height
    }
}

fn absolute_width(style: Option<&missal_core::document::Style>) -> Option<f32> {
    let value = style?.get("width")?;
    let value = css::clean(value).to_ascii_lowercase();
    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let n: f32 = number.parse().ok()?;
    Some(n * css::unit_factor(unit)?)
}

struct Grid {
    ncols: usize,
    placements: Vec<(usize, usize, usize, usize, u32, u32)>,
}

fn build_grid(rows: &[&TableRow]) -> Grid {
    let mut occupied: Vec<Vec<bool>> = Vec::new();
    let mut placements = Vec::new();
    let mut ncols = 0;
    for (r, row) in rows.iter().enumerate() {
        if occupied.len() <= r {
            occupied.resize(r + 1, Vec::new());
        }
        let mut col = 0;
        for (c, cell) in row.cells.iter().enumerate() {
            let Child::Node(cell) = cell else { continue };
            while occupied[r].get(col).copied().unwrap_or(false) {
                col += 1;
            }
            let row_span = cell.row_span().max(1) as usize;
            let col_span = cell.col_span().max(1) as usize;
            let row_span = row_span.min(rows.len() - r);
            for rr in r..r + row_span {
                if occupied.len() <= rr {
                    occupied.resize(rr + 1, Vec::new());
                }
                if occupied[rr].len() < col + col_span {
                    occupied[rr].resize(col + col_span, false);
                }
                for cc in col..col + col_span {
                    occupied[rr][cc] = true;
                }
            }
            placements.push((r, col, row_span, col_span, r as u32, c as u32));
            col += col_span;
            ncols = ncols.max(col);
        }
    }
    Grid { ncols, placements }
}

fn cell_style(table_text: &TextProps, row: &Computed, cell: &TableCell, row_total: f32) -> Computed {
    let base = BoxProps {
        padding: [
            Length::Px(4.0),
            Length::Px(5.6),
            Length::Px(4.0),
            Length::Px(5.6),
        ],
        border: [BorderSide::solid(1.0, Rgba::BLACK); 4],
        vertical_align: VerticalAlign::Top,
        ..BoxProps::default()
    };
    let _ = table_text;
    let mut extra: Vec<(&str, String)> = Vec::new();
    if let Some(bg) = &cell.background_color {
        extra.push(("background-color", bg.clone()));
    }
    extra.push(("border", "1px solid black".into()));
    if let Some(width) = cell.width.filter(|w| *w > 0.0) {
        extra.push(("width", format!("{width}px")));
    }
    extra.push((
        "vertical-align",
        cell.vertical_align.clone().unwrap_or_else(|| "top".into()),
    ));
    extra.push(("text-align", "start".into()));
    let extra_refs: Vec<(&str, &str)> = extra.iter().map(|(p, v)| (*p, v.as_str())).collect();
    let mut computed = compute_element(&row.text, base, &cell.props, &extra_refs);
    if let Some(width) = absolute_width(cell.props.imported_style.as_ref())
        && row_total > 0.0
    {
        let percent = ((width / row_total) * 100.0 * 1000.0).round() / 1000.0;
        computed.boxp.width = Some(Length::Percent(percent));
    }
    computed
}

fn wins(a: &(BorderSide, u8), b: &(BorderSide, u8)) -> bool {
    let (sa, pa) = a;
    let (sb, pb) = b;
    if sa.style == BorderStyle::Hidden {
        return true;
    }
    if sb.style == BorderStyle::Hidden {
        return false;
    }
    let wa = sa.used_width();
    let wb = sb.used_width();
    if wa != wb {
        return wa > wb;
    }
    let ra = sa.style.collapse_rank();
    let rb = sb.style.collapse_rank();
    if ra != rb {
        return ra > rb;
    }
    pa >= pb
}

fn resolve(candidates: &[(BorderSide, u8)]) -> BorderSide {
    let mut best: Option<(BorderSide, u8)> = None;
    for c in candidates {
        best = match best {
            None => Some(*c),
            Some(b) if wins(c, &b) => Some(*c),
            Some(b) => Some(b),
        };
    }
    match best {
        Some((side, _)) if side.style.visible() && side.width > 0.0 => side,
        _ => BorderSide::default(),
    }
}

pub(crate) fn measure_table(
    table: &Table,
    parent: &TextProps,
    available: f32,
    ctx: &mut Ctx<'_>,
    previous: Option<(&TableBox, &[u32])>,
) -> TableBox {
    let imported = table
        .props
        .imported_style
        .as_ref()
        .is_some_and(|s| !s.is_empty());
    let base = BoxProps {
        width: Some(Length::Percent(100.0)),
        max_width: Some(Length::Percent(100.0)),
        border: [BorderSide::default(); 4],
        ..BoxProps::default()
    };
    let mut computed = compute_element(parent, base, &table.props, &[]);
    if imported {
        computed.boxp.margin[1] = Length::Px(0.0);
        computed.boxp.margin[3] = Length::Px(0.0);
        computed.boxp.margin_inline = [None; 2];
        computed.boxp.max_width = Some(Length::Percent(100.0));
    }
    computed.text.align = missal_text::Align::Start;
    let table_text = computed.text.clone();
    let rtl = table_text.direction.is_rtl();
    let margins = computed.boxp.margins(&table_text, available);
    let container = available - margins.horizontal();
    let mut width = computed
        .boxp
        .width
        .filter(|w| *w != Length::Auto)
        .map(|w| w.resolve(table_text.size, container))
        .unwrap_or(container);
    if let Some(max) = computed.boxp.max_width {
        width = width.min(max.resolve(table_text.size, container));
    }
    width = width.max(0.0);
    let x = if rtl {
        margins.left + (container - width).max(0.0)
    } else {
        margins.left
    };
    let rows: Vec<&TableRow> = table.rows.iter().filter_map(Child::node).collect();
    let grid = build_grid(&rows);
    let ncols = grid.ncols.max(1);
    let nrows = rows.len();
    let row_styles: Vec<Computed> = rows
        .iter()
        .map(|row| {
            let mut extra = Vec::new();
            let height = row.height.map(|h| format!("{h}px"));
            if let Some(h) = &height {
                extra.push(("height", h.as_str()));
            }
            compute_element(&table_text, BoxProps::default(), &row.props, &extra)
        })
        .collect();
    let row_totals: Vec<f32> = rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .filter_map(Child::node)
                .map(|c| absolute_width(c.props.imported_style.as_ref()).unwrap_or(0.0))
                .sum()
        })
        .collect();
    let reuse = previous.filter(|(prev, _)| {
        prev.cells.len() == grid.placements.len()
            && prev.ncols == ncols
            && prev.rows.len() == nrows
            && prev.available == available
    });
    let cell_styles: Vec<Computed> = match reuse {
        Some((prev, _)) => prev.cell_styles.clone(),
        None => grid
            .placements
            .iter()
            .map(|&(r, _, _, _, sr, sc)| {
                let cell = rows[sr as usize].cells[sc as usize]
                    .node()
                    .expect("placed cells are typed");
                cell_style(&table_text, &row_styles[r], cell, row_totals[r])
            })
            .collect(),
    };
    let mut col_widths: Vec<Option<f32>> = vec![None; ncols];
    if let Some(widths) = &table.col_widths {
        for (i, w) in widths.iter().enumerate().take(ncols) {
            if *w > 0.0 {
                col_widths[i] = Some(*w as f32);
            }
        }
    }
    if col_widths.iter().all(Option::is_none) {
        let first_row: Vec<usize> = (0..grid.placements.len())
            .filter(|i| grid.placements[*i].0 == 0)
            .collect();
        for index in first_row {
            let (_, col, _, span, _, _) = grid.placements[index];
            let style = &cell_styles[index];
            if let Some(w) = style.boxp.width.filter(|w| *w != Length::Auto) {
                let padding = if span == 1 {
                    style.boxp.paddings(&style.text, width).horizontal()
                } else {
                    0.0
                };
                let resolved = w.resolve(style.text.size, width) + padding;
                for slot in col_widths.iter_mut().skip(col).take(span) {
                    *slot = Some(resolved / span as f32);
                }
            }
        }
    }
    let fixed: f32 = col_widths.iter().flatten().sum();
    let autos = col_widths.iter().filter(|w| w.is_none()).count();
    let mut widths: Vec<f32> = if fixed > width && fixed > 0.0 {
        let scale = width / fixed;
        col_widths.iter().map(|w| w.unwrap_or(0.0) * scale).collect()
    } else if autos > 0 {
        let share = ((width - fixed) / autos as f32).max(0.0);
        col_widths.iter().map(|w| w.unwrap_or(share)).collect()
    } else if fixed > 0.0 {
        let scale = width / fixed;
        col_widths.iter().map(|w| w.unwrap_or(0.0) * scale).collect()
    } else {
        vec![width / ncols as f32; ncols]
    };
    if widths.len() < ncols {
        widths.resize(ncols, 0.0);
    }
    let mut cols = Vec::with_capacity(ncols + 1);
    cols.push(0.0f32);
    let physical_widths: Vec<f32> = if rtl {
        widths.iter().rev().copied().collect()
    } else {
        widths.clone()
    };
    for w in &physical_widths {
        let last = *cols.last().expect("non-empty");
        cols.push(last + w);
    }
    let physical = |col: usize, span: usize| {
        if rtl {
            (ncols - col - span, ncols - col)
        } else {
            (col, col + span)
        }
    };
    let table_borders = computed.boxp.borders(&table_text);
    let row_borders: Vec<[BorderSide; 4]> = row_styles
        .iter()
        .map(|s| s.boxp.borders(&s.text))
        .collect();
    let cell_borders: Vec<[BorderSide; 4]> = cell_styles
        .iter()
        .map(|s| s.boxp.borders(&s.text))
        .collect();
    let mut owner: Vec<Option<usize>> = vec![None; nrows * ncols];
    for (index, &(r, col, rs, cs, _, _)) in grid.placements.iter().enumerate() {
        let (p0, p1) = physical(col, cs);
        for rr in r..r + rs {
            for p in p0..p1 {
                owner[rr * ncols + p] = Some(index);
            }
        }
    }
    let at = |r: usize, p: usize| -> Option<usize> {
        if r < nrows && p < ncols {
            owner[r * ncols + p]
        } else {
            None
        }
    };
    let mut h_edges = vec![BorderSide::default(); (nrows + 1) * ncols];
    for line in 0..=nrows {
        for p in 0..ncols {
            let above = if line > 0 { at(line - 1, p) } else { None };
            let below = at(line, p);
            if above.is_some() && above == below {
                continue;
            }
            let mut candidates = Vec::new();
            if let Some(a) = above {
                candidates.push((cell_borders[a][2], 3));
            }
            if let Some(b) = below {
                candidates.push((cell_borders[b][0], 3));
            }
            if line > 0 {
                candidates.push((row_borders[line - 1][2], 2));
            }
            if line < nrows {
                candidates.push((row_borders[line][0], 2));
            }
            if line == 0 {
                candidates.push((table_borders[0], 0));
            }
            if line == nrows {
                candidates.push((table_borders[2], 0));
            }
            h_edges[line * ncols + p] = resolve(&candidates);
        }
    }
    let mut v_edges = vec![BorderSide::default(); nrows * (ncols + 1)];
    for r in 0..nrows {
        for k in 0..=ncols {
            let left = if k > 0 { at(r, k - 1) } else { None };
            let right = if k < ncols { at(r, k) } else { None };
            if left.is_some() && left == right {
                continue;
            }
            let mut candidates = Vec::new();
            if let Some(l) = left {
                candidates.push((cell_borders[l][1], 3));
            }
            if let Some(rt) = right {
                candidates.push((cell_borders[rt][3], 3));
            }
            if k == 0 {
                candidates.push((row_borders[r][3], 2));
                candidates.push((table_borders[3], 0));
            }
            if k == ncols {
                candidates.push((row_borders[r][1], 2));
                candidates.push((table_borders[1], 0));
            }
            v_edges[r * (ncols + 1) + k] = resolve(&candidates);
        }
    }
    let dirty_cell = reuse.and_then(|(_, dirty)| match dirty {
        [r, c, ..] => Some((*r, *c)),
        _ => None,
    });
    let mut cells = Vec::with_capacity(grid.placements.len());
    for (index, &(r, col, rs, cs, sr, sc)) in grid.placements.iter().enumerate() {
        let (p0, p1) = physical(col, cs);
        let style = &cell_styles[index];
        let max_h = |line: usize| -> f32 {
            (p0..p1)
                .map(|p| h_edges[line * ncols + p].used_width())
                .fold(0.0, f32::max)
        };
        let max_v = |k: usize| -> f32 {
            (r..r + rs)
                .map(|rr| v_edges[rr * (ncols + 1) + k].used_width())
                .fold(0.0, f32::max)
        };
        let border_half = Edges {
            top: max_h(r) / 2.0,
            bottom: max_h(r + rs) / 2.0,
            left: max_v(p0) / 2.0,
            right: max_v(p1) / 2.0,
        };
        let padding = style.boxp.paddings(&style.text, width);
        let x0 = cols[p0];
        let x1 = cols[p1];
        let content_width = (x1 - x0 - border_half.horizontal() - padding.horizontal()).max(0.0);
        let cell = rows[sr as usize].cells[sc as usize]
            .node()
            .expect("placed cells are typed");
        let reused = reuse.and_then(|(prev, _)| {
            let prev_cell = prev.cells.get(index)?;
            (dirty_cell != Some((sr, sc))
                && prev_cell.source == (sr, sc)
                && prev_cell.content_width == content_width)
                .then(|| prev_cell.flow.clone())
        });
        let flow = match reused {
            Some(flow) => flow,
            None => {
                let cell_path = BlockPath::new(&[sr, sc]);
                let items = measure_blocks(&cell.blocks, &style.text, content_width, &cell_path, ctx);
                Arc::new(Flow::stack(items))
            }
        };
        cells.push(CellBox {
            row: r,
            col,
            row_span: rs,
            col_span: cs,
            source: (sr, sc),
            x0,
            x1,
            padding,
            border_half,
            valign: style.boxp.vertical_align,
            background: style.boxp.background.filter(|c| c.alpha() > 0),
            flow,
            content_width,
        });
    }
    let min_heights: Vec<f32> = row_styles
        .iter()
        .map(|s| {
            let h = s
                .boxp
                .height
                .and_then(|h| h.resolve_absolute(s.text.size))
                .unwrap_or(0.0);
            let m = s
                .boxp
                .min_height
                .and_then(|h| h.resolve_absolute(s.text.size))
                .unwrap_or(0.0);
            h.max(m)
        })
        .collect();
    let content: Vec<f32> = cells.iter().map(|c| c.flow.height).collect();
    let heights = row_heights(&min_heights, &cells, &content);
    let mut y = 0.0;
    let mut row_boxes = Vec::with_capacity(nrows);
    let header_cells = |r: usize| {
        rows[r]
            .cells
            .iter()
            .filter_map(Child::node)
            .all(|c| c.header_state.unwrap_or(0) & 1 != 0)
            && !rows[r].cells.is_empty()
    };
    let mut header_rows = 0;
    while header_rows < nrows && header_cells(header_rows) {
        header_rows += 1;
    }
    if header_rows == nrows {
        header_rows = 0;
    }
    for r in 0..nrows {
        row_boxes.push(RowBox {
            y,
            height: heights[r],
            min_height: min_heights[r],
            header: r < header_rows,
            break_inside: row_styles[r].boxp.break_inside,
        });
        y += heights[r];
    }
    TableBox {
        x,
        width,
        height: y,
        margin_top: margins.top,
        margin_bottom: margins.bottom,
        break_before: computed.boxp.break_before,
        break_after: computed.boxp.break_after,
        break_inside: computed.boxp.break_inside,
        ncols,
        cols,
        rows: row_boxes,
        cells,
        h_edges,
        v_edges,
        header_rows,
        rtl,
        cell_styles,
        available,
    }
}

pub(crate) fn row_heights(min_heights: &[f32], cells: &[CellBox], content: &[f32]) -> Vec<f32> {
    let mut heights: Vec<f32> = min_heights.to_vec();
    for (cell, h) in cells.iter().zip(content) {
        if cell.row_span == 1 && cell.row < heights.len() {
            let needed = h + cell.chrome_top() + cell.chrome_bottom();
            heights[cell.row] = heights[cell.row].max(needed);
        }
    }
    for (cell, h) in cells.iter().zip(content) {
        if cell.row_span > 1 {
            let end = (cell.row + cell.row_span).min(heights.len());
            let needed = h + cell.chrome_top() + cell.chrome_bottom();
            let have: f32 = heights[cell.row..end].iter().sum();
            if needed > have && end > cell.row {
                heights[end - 1] += needed - have;
            }
        }
    }
    heights
}
