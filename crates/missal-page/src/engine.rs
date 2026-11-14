use std::sync::Arc;
use std::time::Instant;

use missal_core::document::{Block, Child, Document, ListItemChild, PageLayout};
use missal_core::{FieldDisplayText, FieldReference};
use missal_text::TextEngine;

use crate::FieldSource;
use crate::display::{self, DisplayList};
use crate::geom::{Rect, Size, mm_to_px};
use crate::measure::{Ctx, FlowItem, TextCache, measure_block};
use crate::paginate::{FlowToken, Piece, Region, fragment_flow};
use crate::position::BlockPath;
use crate::style::TextProps;
use crate::text::collect_field_refs;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutOptions {
    pub repeat_header_rows: bool,
}

impl Default for LayoutOptions {
    fn default() -> LayoutOptions {
        LayoutOptions {
            repeat_header_rows: true,
        }
    }
}

#[derive(Clone, Copy)]
pub struct DocSource<'a> {
    pub document: &'a Document,
    pub fields: &'a dyn FieldSource,
}

impl<'a> DocSource<'a> {
    pub fn new(document: &'a Document, fields: &'a dyn FieldSource) -> DocSource<'a> {
        DocSource { document, fields }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Edit {
    Block(BlockPath),
    Splice {
        start: usize,
        removed: usize,
        inserted: usize,
    },
    Document,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RelayoutStats {
    pub blocks_measured: usize,
    pub pages_rebuilt: usize,
    pub pages_reused: usize,
    pub micros: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PageStart {
    pub token: FlowToken,
    pub forced: bool,
}

#[derive(Clone, Debug)]
pub struct Page {
    pub size: Size,
    pub document: usize,
    pub number: usize,
    pub content: Rect,
    pub(crate) pieces: Arc<Vec<Piece>>,
    pub(crate) start: PageStart,
    display: Arc<DisplayList>,
}

impl Page {
    pub fn display_list(&self) -> &Arc<DisplayList> {
        &self.display
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DocLayout {
    pub page_size: Size,
    pub content: Rect,
    pub items: Vec<FlowItem>,
    pub block_starts: Vec<usize>,
    pub field_refs: Vec<Vec<FieldReference>>,
    pub field_texts: Vec<Vec<FieldDisplayText>>,
    pub pages: Vec<Page>,
}

#[derive(Clone, Debug, Default)]
pub struct Packet {
    pub(crate) docs: Vec<DocLayout>,
    pub(crate) page_offsets: Vec<usize>,
}

impl Packet {
    pub fn page_count(&self) -> usize {
        self.docs.iter().map(|d| d.pages.len()).sum()
    }

    pub fn document_count(&self) -> usize {
        self.docs.len()
    }

    pub fn pages(&self) -> impl Iterator<Item = &Page> {
        self.docs.iter().flat_map(|d| d.pages.iter())
    }

    pub fn page(&self, index: usize) -> Option<&Page> {
        let (doc, local) = self.locate_page(index)?;
        self.docs[doc].pages.get(local)
    }

    pub fn document_pages(&self, document: usize) -> std::ops::Range<usize> {
        let start = self.page_offsets.get(document).copied().unwrap_or(0);
        let len = self.docs.get(document).map(|d| d.pages.len()).unwrap_or(0);
        start..start + len
    }

    pub(crate) fn locate_page(&self, index: usize) -> Option<(usize, usize)> {
        let doc = self
            .page_offsets
            .partition_point(|&o| o <= index)
            .checked_sub(1)?;
        let local = index - self.page_offsets[doc];
        (local < self.docs[doc].pages.len()).then_some((doc, local))
    }

    fn refresh_offsets(&mut self) {
        self.page_offsets.clear();
        let mut offset = 0;
        for doc in &self.docs {
            self.page_offsets.push(offset);
            offset += doc.pages.len();
        }
    }
}

pub struct PageEngine {
    text: TextEngine,
    cache: TextCache,
    workers: Vec<(TextEngine, TextCache)>,
    options: LayoutOptions,
    last: RelayoutStats,
}

impl Default for PageEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn page_geometry(layout: Option<&PageLayout>) -> (Size, Rect) {
    let layout = layout.cloned().unwrap_or_else(PageLayout::a4);
    let width = mm_to_px(layout.width_mm);
    let height = mm_to_px(layout.height_mm);
    let top = mm_to_px(layout.margin_top_mm);
    let bottom = mm_to_px(layout.margin_bottom_mm);
    let left = mm_to_px(layout.margin_left_mm);
    let right = mm_to_px(layout.margin_right_mm);
    (
        Size::new(width, height),
        Rect::new(
            left,
            top,
            (width - left - right).max(1.0),
            (height - top - bottom).max(1.0),
        ),
    )
}

fn block_field_refs(block: &Block, out: &mut Vec<FieldReference>) {
    match block {
        Block::Paragraph(p) => collect_field_refs(&p.children, out),
        Block::Heading(h) => collect_field_refs(&h.children, out),
        Block::Quote(q) => collect_field_refs(&q.children, out),
        Block::List(list) => list_field_refs(list, out),
        Block::Table(table) => {
            for row in table.rows.iter().filter_map(Child::node) {
                for cell in row.cells.iter().filter_map(Child::node) {
                    for block in &cell.blocks {
                        block_field_refs(block, out);
                    }
                }
            }
        }
        _ => {}
    }
}

fn list_field_refs(list: &missal_core::document::List, out: &mut Vec<FieldReference>) {
    for item in list.items.iter().filter_map(Child::node) {
        for child in &item.children {
            match child {
                ListItemChild::Inline(inline) => {
                    collect_field_refs(std::slice::from_ref(inline), out)
                }
                ListItemChild::List(nested) => list_field_refs(nested, out),
            }
        }
    }
}

fn shift_token(token: &mut FlowToken, delta: isize) {
    token.item = (token.item as isize + delta) as usize;
}

fn shift_page(page: &mut Page, item_delta: isize, block_delta: isize) {
    shift_token(&mut page.start.token, item_delta);
    if block_delta != 0 {
        let pieces = Arc::make_mut(&mut page.pieces);
        for piece in pieces.iter_mut() {
            piece.path_mut().shift_top(block_delta);
        }
    }
}

impl PageEngine {
    pub fn new() -> PageEngine {
        PageEngine {
            text: TextEngine::new(),
            cache: TextCache::new(),
            workers: Vec::new(),
            options: LayoutOptions::default(),
            last: RelayoutStats::default(),
        }
    }

    pub fn with_options(options: LayoutOptions) -> PageEngine {
        PageEngine {
            options,
            ..PageEngine::new()
        }
    }

    pub fn options(&self) -> LayoutOptions {
        self.options
    }

    pub fn text_engine(&mut self) -> &mut TextEngine {
        &mut self.text
    }

    pub fn last_stats(&self) -> RelayoutStats {
        self.last
    }

    pub fn cached_blocks(&self) -> usize {
        self.cache.len()
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
        self.text.clear_cache();
    }

    pub fn layout(&mut self, sources: &[DocSource<'_>]) -> Packet {
        let started = Instant::now();
        self.cache.next_generation();
        let mut packet = Packet::default();
        let mut stats = RelayoutStats::default();
        for (index, source) in sources.iter().enumerate() {
            let doc = self.layout_document(index, source, &mut stats);
            packet.docs.push(doc);
        }
        packet.refresh_offsets();
        stats.micros = started.elapsed().as_secs_f64() * 1e6;
        self.last = stats;
        packet
    }

    fn measure_top(
        &mut self,
        source: &DocSource<'_>,
        content_width: f32,
        index: usize,
        previous: Option<(&[FlowItem], &[u32])>,
    ) -> Vec<FlowItem> {
        let body = TextProps::body();
        let mut ctx = Ctx {
            engine: &mut self.text,
            cache: &mut self.cache,
            fields: source.fields,
        };
        let mut out = Vec::new();
        if let Some(block) = source.document.blocks.get(index) {
            measure_block(
                block,
                &body,
                content_width,
                BlockPath::top(index),
                &mut ctx,
                previous,
                &mut out,
            );
        }
        out
    }

    fn layout_document(
        &mut self,
        index: usize,
        source: &DocSource<'_>,
        stats: &mut RelayoutStats,
    ) -> DocLayout {
        let (page_size, content) = page_geometry(source.document.page_layout.as_ref());
        let mut items = Vec::new();
        let mut block_starts = Vec::with_capacity(source.document.blocks.len() + 1);
        let mut field_refs = Vec::with_capacity(source.document.blocks.len());
        let mut field_texts = Vec::with_capacity(source.document.blocks.len());
        let mut measured = self.measure_parallel(source, content.width);
        for (block_index, block) in source.document.blocks.iter().enumerate() {
            block_starts.push(items.len());
            match measured
                .as_mut()
                .and_then(|m| m.get_mut(block_index))
                .map(std::mem::take)
            {
                Some(found) => items.extend(found),
                None => items.extend(self.measure_top(source, content.width, block_index, None)),
            }
            let mut refs = Vec::new();
            block_field_refs(block, &mut refs);
            field_texts.push(refs.iter().map(|r| source.fields.display(r)).collect());
            field_refs.push(refs);
        }
        block_starts.push(items.len());
        stats.blocks_measured += source.document.blocks.len();
        let mut doc = DocLayout {
            page_size,
            content,
            items,
            block_starts,
            field_refs,
            field_texts,
            pages: Vec::new(),
        };
        let pages = self.paginate(
            &doc,
            index,
            0,
            PageStart {
                token: FlowToken::start(),
                forced: false,
            },
            None,
            stats,
        );
        doc.pages = pages;
        doc
    }

    fn measure_parallel(
        &mut self,
        source: &DocSource<'_>,
        width: f32,
    ) -> Option<Vec<Vec<FlowItem>>> {
        let all: Vec<usize> = (0..source.document.blocks.len()).collect();
        self.measure_set(source, width, &all, 48)
    }

    fn measure_set(
        &mut self,
        source: &DocSource<'_>,
        width: f32,
        wanted: &[usize],
        threshold: usize,
    ) -> Option<Vec<Vec<FlowItem>>> {
        let blocks = &source.document.blocks;
        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(8);
        if threads < 2 || wanted.len() < threshold {
            return None;
        }
        while self.workers.len() < threads {
            self.workers.push((TextEngine::new(), TextCache::new()));
        }
        let main_cache = &self.cache;
        let fields = source.fields;
        let results: Vec<Vec<(usize, Vec<FlowItem>)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = self
                .workers
                .iter_mut()
                .take(threads)
                .enumerate()
                .map(|(worker, (engine, cache))| {
                    cache.sync_generation(main_cache);
                    scope.spawn(move || {
                        let body = TextProps::body();
                        let mut ctx = Ctx {
                            engine,
                            cache,
                            fields,
                        };
                        let mut out = Vec::new();
                        let mut slot = worker;
                        while slot < wanted.len() {
                            let block = wanted[slot];
                            let mut items = Vec::new();
                            measure_block(
                                &blocks[block],
                                &body,
                                width,
                                BlockPath::top(block),
                                &mut ctx,
                                None,
                                &mut items,
                            );
                            out.push((block, items));
                            slot += threads;
                        }
                        out
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("measure worker"))
                .collect()
        });
        for (_, cache) in self.workers.iter_mut() {
            self.cache.absorb(cache);
        }
        let mut measured: Vec<Vec<FlowItem>> = vec![Vec::new(); blocks.len()];
        for chunk in results {
            for (block, items) in chunk {
                measured[block] = items;
            }
        }
        Some(measured)
    }

    fn region(&self, doc: &DocLayout, start: &PageStart, first_page: bool) -> Region {
        Region {
            x: doc.content.x,
            top: doc.content.y,
            limit: doc.content.bottom(),
            page_empty: true,
            keep_first_margin: start.forced || first_page,
            forced_breaks: true,
            repeat_headers: self.options.repeat_header_rows,
        }
    }

    fn paginate(
        &self,
        doc: &DocLayout,
        document: usize,
        first_number: usize,
        start: PageStart,
        mut realign: Option<Realign<'_>>,
        stats: &mut RelayoutStats,
    ) -> Vec<Page> {
        let mut pages = Vec::new();
        let mut state = start;
        let mut number = first_number;
        loop {
            let mut pieces = Vec::new();
            let region = self.region(doc, &state, number == 0);
            let outcome = fragment_flow(&doc.items, &[], &state.token, region, &mut pieces);
            let display = Arc::new(display::build(&pieces));
            stats.pages_rebuilt += 1;
            pages.push(Page {
                size: doc.page_size,
                document,
                number,
                content: doc.content,
                pieces: Arc::new(pieces),
                start: state.clone(),
                display,
            });
            number += 1;
            let Some(mut next) = outcome.next else { break };
            if next == state.token && !outcome.forced && !outcome.placed {
                next = FlowToken::at(next.item + 1);
            }
            let next_state = PageStart {
                token: next,
                forced: outcome.forced,
            };
            if let Some(realign) = realign.as_mut()
                && next_state.token.item >= realign.new_end_item
            {
                let mut old_token = next_state.token.clone();
                shift_token(&mut old_token, -realign.item_delta);
                let old_state = PageStart {
                    token: old_token,
                    forced: next_state.forced,
                };
                if let Some(found) = realign.old_pages.iter().position(|p| p.start == old_state) {
                    for old in &realign.old_pages[found..] {
                        let mut page = old.clone();
                        shift_page(&mut page, realign.item_delta, realign.block_delta);
                        page.number = number;
                        number += 1;
                        stats.pages_reused += 1;
                        pages.push(page);
                    }
                    break;
                }
            }
            state = next_state;
        }
        pages
    }

    pub fn update(
        &mut self,
        packet: &mut Packet,
        document: usize,
        source: &DocSource<'_>,
        edit: &Edit,
    ) {
        let started = Instant::now();
        let mut stats = RelayoutStats::default();
        self.cache.next_generation();
        let Some(doc) = packet.docs.get_mut(document) else {
            return;
        };
        let (page_size, content) = page_geometry(source.document.page_layout.as_ref());
        let geometry_changed = page_size != doc.page_size || content != doc.content;
        let edit = if geometry_changed
            || doc.block_starts.len() != source.document.blocks.len() + 1
                && !matches!(edit, Edit::Splice { .. })
        {
            Edit::Document
        } else {
            edit.clone()
        };
        match edit {
            Edit::Document => {
                let fresh = self.layout_document(document, source, &mut stats);
                packet.docs[document] = fresh;
            }
            Edit::Block(path) => {
                let k = path.top_index();
                if k >= source.document.blocks.len() {
                    return;
                }
                let doc = &packet.docs[document];
                let range = doc.block_starts[k]..doc.block_starts[k + 1];
                let previous: Vec<FlowItem> = doc.items[range.clone()].to_vec();
                let tail: Vec<u32> = path.steps()[1..].to_vec();
                let width = doc.content.width;
                let items = self.measure_top(source, width, k, Some((&previous, &tail)));
                stats.blocks_measured += 1;
                let mut refs = Vec::new();
                block_field_refs(&source.document.blocks[k], &mut refs);
                let texts = refs.iter().map(|r| source.fields.display(r)).collect();
                let doc = &mut packet.docs[document];
                doc.field_refs[k] = refs;
                doc.field_texts[k] = texts;
                self.replace_blocks(doc, document, k, 1, vec![items], 0, &mut stats);
            }
            Edit::Splice {
                start,
                removed,
                inserted,
            } => {
                let width = packet.docs[document].content.width;
                let mut new_items = Vec::with_capacity(inserted);
                let mut new_refs = Vec::with_capacity(inserted);
                let mut new_texts = Vec::with_capacity(inserted);
                for k in start..start + inserted {
                    new_items.push(self.measure_top(source, width, k, None));
                    let mut refs = Vec::new();
                    if let Some(block) = source.document.blocks.get(k) {
                        block_field_refs(block, &mut refs);
                    }
                    new_texts.push(refs.iter().map(|r| source.fields.display(r)).collect());
                    new_refs.push(refs);
                }
                stats.blocks_measured += inserted;
                let doc = &mut packet.docs[document];
                let removed = removed.min(doc.field_refs.len().saturating_sub(start));
                doc.field_refs.splice(start..start + removed, new_refs);
                doc.field_texts.splice(start..start + removed, new_texts);
                let block_delta = inserted as isize - removed as isize;
                self.replace_blocks(
                    doc,
                    document,
                    start,
                    removed,
                    new_items,
                    block_delta,
                    &mut stats,
                );
            }
        }
        packet.refresh_offsets();
        stats.micros = started.elapsed().as_secs_f64() * 1e6;
        self.last = stats;
    }

    #[allow(clippy::too_many_arguments)]
    fn replace_blocks(
        &mut self,
        doc: &mut DocLayout,
        document: usize,
        start: usize,
        removed: usize,
        new_blocks: Vec<Vec<FlowItem>>,
        block_delta: isize,
        stats: &mut RelayoutStats,
    ) {
        let old_first = doc.block_starts[start];
        let old_end = doc.block_starts[(start + removed).min(doc.block_starts.len() - 1)];
        let flat: Vec<FlowItem> = new_blocks.iter().flatten().cloned().collect();
        let inserted_items = flat.len();
        let item_delta = inserted_items as isize - (old_end - old_first) as isize;
        doc.items.splice(old_first..old_end, flat);
        if block_delta != 0 {
            for item in &mut doc.items[old_first + inserted_items..] {
                item.path.shift_top(block_delta);
            }
        }
        let mut starts = Vec::with_capacity(doc.block_starts.len());
        starts.extend_from_slice(&doc.block_starts[..start]);
        let mut cursor = old_first;
        for block in &new_blocks {
            starts.push(cursor);
            cursor += block.len();
        }
        for &s in &doc.block_starts[start + removed..] {
            starts.push((s as isize + item_delta) as usize);
        }
        doc.block_starts = starts;
        let new_end_item = old_first + inserted_items;
        self.repaginate(
            doc,
            document,
            old_first,
            new_end_item,
            item_delta,
            block_delta,
            stats,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn repaginate(
        &mut self,
        doc: &mut DocLayout,
        document: usize,
        first_item: usize,
        new_end_item: usize,
        item_delta: isize,
        block_delta: isize,
        stats: &mut RelayoutStats,
    ) {
        let p = doc
            .pages
            .iter()
            .rposition(|page| {
                page.start.token.item < first_item
                    || (page.start.token.item == first_item && page.start.token.inner.is_none())
            })
            .unwrap_or(0);
        let start = doc
            .pages
            .get(p)
            .map(|page| page.start.clone())
            .unwrap_or(PageStart {
                token: FlowToken::start(),
                forced: false,
            });
        let old_tail: Vec<Page> = doc.pages.split_off(p);
        let realign = Realign {
            old_pages: &old_tail[1.min(old_tail.len())..],
            new_end_item,
            item_delta,
            block_delta,
        };
        let pages = self.paginate(doc, document, p, start, Some(realign), stats);
        doc.pages.extend(pages);
    }

    pub fn refresh_fields(&mut self, packet: &mut Packet, sources: &[DocSource<'_>]) -> usize {
        let started = Instant::now();
        let mut stats = RelayoutStats::default();
        self.cache.next_generation();
        let mut total = 0;
        for (document, source) in sources.iter().enumerate() {
            let Some(doc) = packet.docs.get(document) else {
                continue;
            };
            let mut dirty = Vec::new();
            for (k, refs) in doc.field_refs.iter().enumerate() {
                if refs.is_empty() {
                    continue;
                }
                let texts: Vec<FieldDisplayText> =
                    refs.iter().map(|r| source.fields.display(r)).collect();
                if texts != doc.field_texts[k] {
                    dirty.push((k, texts));
                }
            }
            if dirty.is_empty() {
                continue;
            }
            total += dirty.len();
            let width = doc.content.width;
            let first_block = dirty[0].0;
            let last_block = dirty[dirty.len() - 1].0;
            let dirty_blocks: Vec<usize> = dirty.iter().map(|(k, _)| *k).collect();
            let mut parallel = self.measure_set(source, width, &dirty_blocks, 8);
            let mut measured = Vec::with_capacity(last_block - first_block + 1);
            let doc_items: Vec<Vec<FlowItem>> = (first_block..=last_block)
                .map(|k| {
                    let d = &packet.docs[document];
                    d.items[d.block_starts[k]..d.block_starts[k + 1]].to_vec()
                })
                .collect();
            for (offset, k) in (first_block..=last_block).enumerate() {
                if dirty.iter().any(|(d, _)| *d == k) {
                    match parallel.as_mut().map(|m| std::mem::take(&mut m[k])) {
                        Some(items) => measured.push(items),
                        None => measured.push(self.measure_top(source, width, k, None)),
                    }
                    stats.blocks_measured += 1;
                } else {
                    measured.push(doc_items[offset].clone());
                }
            }
            let doc = &mut packet.docs[document];
            for (k, texts) in dirty {
                doc.field_texts[k] = texts;
            }
            let count = last_block - first_block + 1;
            self.replace_blocks(doc, document, first_block, count, measured, 0, &mut stats);
        }
        packet.refresh_offsets();
        stats.micros = started.elapsed().as_secs_f64() * 1e6;
        self.last = stats;
        total
    }
}

pub(crate) struct Realign<'a> {
    old_pages: &'a [Page],
    new_end_item: usize,
    item_delta: isize,
    block_delta: isize,
}
