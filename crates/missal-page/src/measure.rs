use std::collections::HashMap;
use std::sync::Arc;

use missal_core::document::{
    Block, ElementProps, HeadingLevel, Inline, List, ListItemChild, ListTag, ListType,
};
use missal_text::{Rgba, TextEngine};

use crate::FieldSource;
use crate::css::Length;
use crate::position::BlockPath;
use crate::style::{
    self, BorderSide, BoxProps, BreakValue, Computed, Declarations, TextProps, doc_align, doc_dir,
};
use crate::table::{self, TableBox};
use crate::text::{self, MarkerKind, TextBlockInput, TextBox};

#[derive(Clone, Debug)]
pub(crate) enum Node {
    Text(Arc<TextBox>),
    Table(Arc<TableBox>),
    Rule { width: f32, height: f32 },
    PageBreak,
}

impl Node {
    pub(crate) fn height(&self) -> f32 {
        match self {
            Node::Text(t) => t.height(),
            Node::Table(t) => t.height,
            Node::Rule { height, .. } => *height,
            Node::PageBreak => 0.0,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FlowItem {
    pub node: Node,
    pub path: BlockPath,
    pub x: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub break_before: BreakValue,
    pub break_after: BreakValue,
    pub break_inside: BreakValue,
}

impl FlowItem {
    pub(crate) fn height(&self) -> f32 {
        self.node.height()
    }

    fn simple(
        node: Node,
        path: BlockPath,
        x: f32,
        margin_top: f32,
        margin_bottom: f32,
    ) -> FlowItem {
        FlowItem {
            node,
            path,
            x,
            margin_top,
            margin_bottom,
            break_before: BreakValue::Auto,
            break_after: BreakValue::Auto,
            break_inside: BreakValue::Auto,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Flow {
    pub items: Vec<FlowItem>,
    pub ys: Vec<f32>,
    pub height: f32,
}

pub(crate) fn collapse(a: f32, b: f32) -> f32 {
    a.max(b).max(0.0) + a.min(b).min(0.0)
}

impl Flow {
    pub(crate) fn stack(items: Vec<FlowItem>) -> Flow {
        let mut ys = Vec::with_capacity(items.len());
        let mut y = 0.0f32;
        let mut previous: Option<f32> = None;
        for item in &items {
            let gap = match previous {
                None => item.margin_top,
                Some(mb) => collapse(mb, item.margin_top),
            };
            y += gap;
            ys.push(y);
            y += item.height();
            previous = Some(item.margin_bottom);
        }
        let height = (y + previous.unwrap_or(0.0)).max(0.0);
        Flow { items, ys, height }
    }
}

pub(crate) struct TextCache {
    map: HashMap<u64, (Arc<TextBox>, u64)>,
    generation: u64,
    pub hits: u64,
    pub misses: u64,
}

impl TextCache {
    pub(crate) fn new() -> TextCache {
        TextCache {
            map: HashMap::new(),
            generation: 0,
            hits: 0,
            misses: 0,
        }
    }

    pub(crate) fn next_generation(&mut self) {
        self.generation += 1;
        if self.map.len() > 50_000 {
            let keep = self.generation.saturating_sub(2);
            self.map.retain(|_, (_, g)| *g >= keep);
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.map.len()
    }

    pub(crate) fn sync_generation(&mut self, other: &TextCache) {
        self.generation = other.generation;
    }

    pub(crate) fn absorb(&mut self, other: &mut TextCache) {
        let generation = self.generation;
        for (key, (value, _)) in other.map.drain() {
            self.map.entry(key).or_insert((value, generation));
        }
        self.hits += other.hits;
        self.misses += other.misses;
        other.hits = 0;
        other.misses = 0;
    }

    pub(crate) fn clear(&mut self) {
        self.map.clear();
    }
}

pub(crate) struct Ctx<'a> {
    pub engine: &'a mut TextEngine,
    pub cache: &'a mut TextCache,
    pub fields: &'a dyn FieldSource,
}

impl Ctx<'_> {
    pub(crate) fn text_box(
        &mut self,
        computed: &Computed,
        inlines: &[Inline],
        marker: Option<MarkerKind>,
        available: f32,
    ) -> Arc<TextBox> {
        let key = text::content_hash(computed, inlines, &marker, available, self.fields);
        let generation = self.cache.generation;
        if let Some((found, used)) = self.cache.map.get_mut(&key) {
            *used = generation;
            self.cache.hits += 1;
            return found.clone();
        }
        self.cache.misses += 1;
        let measured = Arc::new(text::measure_text_block(
            self.engine,
            TextBlockInput {
                computed,
                inlines,
                marker,
                available,
            },
            self.fields,
        ));
        self.cache.map.insert(key, (measured.clone(), generation));
        measured
    }
}

pub(crate) fn element_decls<'a>(props: &'a ElementProps, decls: &mut Declarations<'a>) {
    if let Some(direction) = props.direction {
        decls.push(
            "direction",
            match direction {
                missal_core::document::Direction::Rtl => "rtl",
                missal_core::document::Direction::Ltr => "ltr",
            },
        );
    }
    decls.push_style(props.imported_style.as_ref());
}

fn apply_element_overrides(computed: &mut Computed, props: &ElementProps) {
    if let Some(align) = props.align {
        computed.text.align = doc_align(align);
    }
    if let Some(direction) = props.direction
        && props
            .imported_style
            .as_ref()
            .is_none_or(|s| s.get("direction").is_none())
    {
        computed.text.direction = doc_dir(direction);
    }
    if let Some(indent) = props.indent.filter(|i| *i > 0) {
        computed.boxp.padding_inline[0] = Some(Length::Px(indent as f32 * 40.0));
    }
}

pub(crate) fn paragraph_defaults() -> BoxProps {
    BoxProps {
        margin: [
            Length::Px(0.0),
            Length::Px(0.0),
            Length::Px(8.0),
            Length::Px(0.0),
        ],
        ..BoxProps::default()
    }
}

fn heading_scale(level: Option<HeadingLevel>) -> f32 {
    match level.unwrap_or(HeadingLevel::H1) {
        HeadingLevel::H1 => 2.0,
        HeadingLevel::H2 => 1.5,
        HeadingLevel::H3 => 1.25,
        HeadingLevel::H4 => 1.0,
        HeadingLevel::H5 => 0.83,
        HeadingLevel::H6 => 0.67,
    }
}

pub(crate) fn compute_element(
    parent: &TextProps,
    base: BoxProps,
    props: &ElementProps,
    extra: &[(&str, &str)],
) -> Computed {
    let mut decls = Declarations::new();
    for (p, v) in extra {
        decls.push(p, v);
    }
    element_decls(props, &mut decls);
    let mut computed = style::compute(parent, base, &decls);
    apply_element_overrides(&mut computed, props);
    computed
}

pub(crate) fn measure_blocks(
    blocks: &[Block],
    parent: &TextProps,
    available: f32,
    prefix: &BlockPath,
    ctx: &mut Ctx<'_>,
) -> Vec<FlowItem> {
    let mut items = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        let path = prefix.child(index as u32);
        measure_block(block, parent, available, path, ctx, None, &mut items);
    }
    items
}

pub(crate) fn measure_block(
    block: &Block,
    parent: &TextProps,
    available: f32,
    path: BlockPath,
    ctx: &mut Ctx<'_>,
    previous: Option<(&[FlowItem], &[u32])>,
    out: &mut Vec<FlowItem>,
) {
    match block {
        Block::Paragraph(p) => {
            let computed = compute_element(parent, paragraph_defaults(), &p.props, &[]);
            push_text(out, ctx, &computed, &p.children, None, available, path, 0.0);
        }
        Block::Heading(h) => {
            let size = format!("{}em", heading_scale(h.level));
            let computed = compute_element(
                parent,
                BoxProps::default(),
                &h.props,
                &[("font-size", size.as_str()), ("font-weight", "bold")],
            );
            push_text(out, ctx, &computed, &h.children, None, available, path, 0.0);
        }
        Block::Quote(q) => {
            let base = BoxProps {
                border_inline_start: Some(BorderSide::solid(3.0, Rgba::BLACK)),
                padding_inline: [Some(Length::Px(16.0)), None],
                ..BoxProps::default()
            };
            let computed = compute_element(parent, base, &q.props, &[]);
            push_text(out, ctx, &computed, &q.children, None, available, path, 0.0);
        }
        Block::List(list) => measure_list(list, parent, available, &path, 0.0, ctx, out),
        Block::Table(t) => {
            let prev_table = previous.and_then(|(items, dirty)| {
                items.iter().find_map(|item| match &item.node {
                    Node::Table(table) => Some((table.clone(), dirty)),
                    _ => None,
                })
            });
            let measured = table::measure_table(
                t,
                parent,
                available,
                ctx,
                prev_table.as_ref().map(|(t, d)| (t.as_ref(), *d)),
            );
            out.push(FlowItem {
                margin_top: measured.margin_top,
                margin_bottom: measured.margin_bottom,
                break_before: measured.break_before,
                break_after: measured.break_after,
                break_inside: measured.break_inside,
                x: 0.0,
                path,
                node: Node::Table(Arc::new(measured)),
            });
        }
        Block::PageBreak(_) => out.push(FlowItem {
            break_before: BreakValue::Page,
            ..FlowItem::simple(Node::PageBreak, path, 0.0, 0.0, 0.0)
        }),
        Block::HorizontalRule(_) => {
            let margin = parent.size * 0.5;
            out.push(FlowItem::simple(
                Node::Rule {
                    width: available,
                    height: 2.0,
                },
                path,
                0.0,
                margin,
                margin,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn push_text(
    out: &mut Vec<FlowItem>,
    ctx: &mut Ctx<'_>,
    computed: &Computed,
    inlines: &[Inline],
    marker: Option<MarkerKind>,
    available: f32,
    path: BlockPath,
    x: f32,
) {
    let measured = ctx.text_box(computed, inlines, marker, available);
    out.push(FlowItem {
        margin_top: measured.margin_top,
        margin_bottom: measured.margin_bottom,
        break_before: measured.break_before,
        break_after: measured.break_after,
        break_inside: measured.break_inside,
        x,
        path,
        node: Node::Text(measured),
    });
}

fn measure_list(
    list: &List,
    parent: &TextProps,
    available: f32,
    path: &BlockPath,
    x: f32,
    ctx: &mut Ctx<'_>,
    out: &mut Vec<FlowItem>,
) {
    let base = BoxProps {
        padding_inline: [Some(Length::Px(24.0)), None],
        ..BoxProps::default()
    };
    let computed = compute_element(parent, base, &list.props, &[]);
    let paddings = computed.boxp.paddings(&computed.text, available);
    let margins = computed.boxp.margins(&computed.text, available);
    let inner = (available - paddings.horizontal() - margins.horizontal()).max(0.0);
    let item_x = x + margins.left + paddings.left;
    let ordered = list.tag == Some(ListTag::Ol)
        || (list.tag.is_none() && list.list_type == Some(ListType::Number));
    let mut counter = list.start.unwrap_or(1);
    for (index, child) in list.items.iter().enumerate() {
        let item = child;
        let item_path = path.child(index as u32);
        let inlines: Vec<Inline> = item
            .children
            .iter()
            .filter_map(|c| match c {
                ListItemChild::Inline(i) => Some(i.clone()),
                ListItemChild::List(_) => None,
            })
            .collect();
        let nested_only = inlines.is_empty()
            && item
                .children
                .iter()
                .any(|c| matches!(c, ListItemChild::List(_)));
        if !nested_only {
            let value = item.value.unwrap_or(counter);
            counter = value + 1;
            let item_computed =
                compute_element(&computed.text, BoxProps::default(), &item.props, &[]);
            let marker = if ordered {
                MarkerKind::Text(value.to_string())
            } else {
                MarkerKind::Bullet
            };
            push_text(
                out,
                ctx,
                &item_computed,
                &inlines,
                Some(marker),
                inner,
                item_path.clone(),
                item_x,
            );
        }
        for (child_index, child) in item.children.iter().enumerate() {
            if let ListItemChild::List(nested) = child {
                measure_list(
                    nested,
                    &computed.text,
                    inner,
                    &item_path.child(child_index as u32),
                    item_x,
                    ctx,
                    out,
                );
            }
        }
    }
}
