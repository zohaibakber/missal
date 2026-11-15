use std::{
    borrow::Cow,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use parley::{
    BaseDirection, CHROMIUM_LINE_BREAK_OVERRIDE, FontContext, FontFamily as ParleyFamily,
    FontFamilyName, FontStyle, FontWeight, InlineBox, InlineBoxKind, LayoutContext,
    LineHeight as ParleyLineHeight, StyleProperty, VerticalAlign,
};

use super::{
    style::{Attr, AttributedText, LineHeight, ParagraphStyle, Rgba},
    text_layout::TextLayout,
};
use crate::fonts::{FontFamily, UI_FAMILY, URDU_FAMILY, font_context};

const DEFAULT_CACHE_ENTRIES: usize = 4096;
const DEFAULT_CACHE_BYTES: usize = 32 * 1024 * 1024;

pub struct TextEngine {
    fcx: FontContext,
    lcx: LayoutContext<Rgba>,
    cache: LayoutCache,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CacheStats {
    pub hits: u64,
    pub rebreaks: u64,
    pub builds: u64,
    pub entries: usize,
    pub bytes: usize,
}

const INTERFACE_STACK: [FontFamilyName<'static>; 2] = [
    FontFamilyName::Named(Cow::Borrowed(UI_FAMILY)),
    FontFamilyName::Named(Cow::Borrowed(URDU_FAMILY)),
];

const DOCUMENT_STACK: [FontFamilyName<'static>; 2] = [
    FontFamilyName::Named(Cow::Borrowed(URDU_FAMILY)),
    FontFamilyName::Named(Cow::Borrowed(UI_FAMILY)),
];

fn family_property(family: &FontFamily) -> StyleProperty<'_, Rgba> {
    StyleProperty::FontFamily(ParleyFamily::List(match family {
        FontFamily::Interface => Cow::Borrowed(&INTERFACE_STACK),
        FontFamily::Document => Cow::Borrowed(&DOCUMENT_STACK),
        FontFamily::Named(_) => Cow::Owned(
            family
                .stack()
                .into_iter()
                .map(|name| FontFamilyName::Named(Cow::Borrowed(name)))
                .collect(),
        ),
    }))
}

fn attr_property(attr: &Attr) -> Option<StyleProperty<'_, Rgba>> {
    Some(match attr {
        Attr::Family(family) => family_property(family),
        Attr::Size(size) => StyleProperty::FontSize(*size),
        Attr::Weight(weight) => StyleProperty::FontWeight(FontWeight::new(*weight as f32)),
        Attr::Italic(italic) => StyleProperty::FontStyle(if *italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }),
        Attr::Underline(on) => StyleProperty::Underline(*on),
        Attr::Strikethrough(on) => StyleProperty::Strikethrough(*on),
        Attr::Color(color) => StyleProperty::Brush(*color),
        Attr::Background(_) => return None,
    })
}

impl TextEngine {
    pub fn new() -> Self {
        Self {
            fcx: font_context(),
            lcx: LayoutContext::new(),
            cache: LayoutCache::new(DEFAULT_CACHE_ENTRIES, DEFAULT_CACHE_BYTES),
        }
    }

    pub fn layout_uncached(
        &mut self,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        max_width: Option<f32>,
    ) -> TextLayout {
        let source: &str = &text.text;
        let rtl = paragraph.direction.resolve_rtl(source);
        let mut builder = self.lcx.ranged_builder(&mut self.fcx, source, 1.0, false);
        builder.set_base_direction(if rtl {
            BaseDirection::Rtl
        } else {
            BaseDirection::Ltr
        });
        builder.set_line_break_override(Some(CHROMIUM_LINE_BREAK_OVERRIDE));
        let style = &text.style;
        builder.push_default(family_property(&style.family));
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(
            style.weight as f32,
        )));
        builder.push_default(StyleProperty::FontStyle(if style.italic {
            FontStyle::Italic
        } else {
            FontStyle::Normal
        }));
        builder.push_default(StyleProperty::Brush(style.color));
        builder.push_default(StyleProperty::Underline(style.underline));
        builder.push_default(StyleProperty::Strikethrough(style.strikethrough));
        builder.push_default(StyleProperty::LineHeight(match paragraph.line_height {
            LineHeight::Normal => ParleyLineHeight::MetricsRelative(1.0),
            LineHeight::Relative(factor) => ParleyLineHeight::FontSizeRelative(factor),
            LineHeight::Px(px) => ParleyLineHeight::Absolute(px),
        }));
        for (range, attr) in &text.spans {
            if range.is_empty()
                || !source.is_char_boundary(range.start)
                || !source.is_char_boundary(range.end)
            {
                continue;
            }
            if let Some(property) = attr_property(attr) {
                builder.push(property, range.clone());
            }
        }
        for object in &text.objects {
            let mut index = object.index.min(source.len());
            while !source.is_char_boundary(index) {
                index -= 1;
            }
            builder.push_inline_box(InlineBox {
                id: object.id,
                kind: InlineBoxKind::InFlow,
                index,
                width: object.width,
                height: object.height,
                baseline: object.baseline,
                vertical_align: VerticalAlign::BASELINE,
            });
        }
        let inner = builder.build(source);
        TextLayout::from_parley(text.clone(), *paragraph, rtl, inner, max_width)
    }

    pub fn layout(
        &mut self,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        max_width: Option<f32>,
    ) -> Arc<TextLayout> {
        let key = content_key(text, paragraph);
        let width = max_width.map(f32::to_bits);
        let tick = self.cache.next_tick();
        if let Some(found) = self.cache.get(key, text, paragraph, width, tick) {
            return found;
        }
        let layout = match self.cache.any_width(key, text, paragraph) {
            Some(template) => {
                self.cache.stats.rebreaks += 1;
                Arc::new(template.with_max_width(max_width))
            }
            None => {
                self.cache.stats.builds += 1;
                Arc::new(self.layout_uncached(text, paragraph, max_width))
            }
        };
        self.cache.insert(key, width, layout.clone(), tick);
        layout
    }

    pub fn cache_stats(&self) -> CacheStats {
        CacheStats {
            entries: self.cache.len(),
            bytes: self.cache.bytes,
            ..self.cache.stats
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn set_cache_capacity(&mut self, capacity: usize) {
        self.cache.capacity = capacity.max(16);
        self.cache.trim();
    }

    pub fn set_cache_budget(&mut self, bytes: usize) {
        self.cache.budget = bytes;
        self.cache.trim();
    }
}

fn content_key(text: &AttributedText, paragraph: &ParagraphStyle) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    paragraph.hash(&mut hasher);
    hasher.finish()
}

struct CacheEntry {
    width: Option<u32>,
    layout: Arc<TextLayout>,
    bytes: usize,
    last_used: u64,
}

#[derive(Default)]
struct CacheSlot {
    shaping_bytes: usize,
    entries: Vec<CacheEntry>,
}

impl CacheSlot {
    fn bytes(&self) -> usize {
        self.shaping_bytes + self.entries.iter().map(|e| e.bytes).sum::<usize>()
    }
}

struct LayoutCache {
    map: HashMap<u64, CacheSlot>,
    count: usize,
    capacity: usize,
    bytes: usize,
    budget: usize,
    tick: u64,
    stats: CacheStats,
}

impl LayoutCache {
    fn new(capacity: usize, budget: usize) -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
            capacity,
            bytes: 0,
            budget,
            tick: 0,
            stats: CacheStats::default(),
        }
    }

    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    fn len(&self) -> usize {
        self.count
    }

    fn clear(&mut self) {
        self.map.clear();
        self.count = 0;
        self.bytes = 0;
    }

    fn get(
        &mut self,
        key: u64,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        width: Option<u32>,
        tick: u64,
    ) -> Option<Arc<TextLayout>> {
        let slot = self.map.get_mut(&key)?;
        let entry = slot.entries.iter_mut().find(|e| {
            e.width == width && e.layout.paragraph == *paragraph && e.layout.source == *text
        })?;
        entry.last_used = tick;
        self.stats.hits += 1;
        Some(entry.layout.clone())
    }

    fn any_width(
        &self,
        key: u64,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
    ) -> Option<Arc<TextLayout>> {
        self.map
            .get(&key)?
            .entries
            .iter()
            .find(|e| e.layout.paragraph == *paragraph && e.layout.source == *text)
            .map(|e| e.layout.clone())
    }

    fn insert(&mut self, key: u64, width: Option<u32>, layout: Arc<TextLayout>, tick: u64) {
        let bytes = layout.geometry_bytes();
        let slot = self.map.entry(key).or_default();
        if !slot
            .entries
            .iter()
            .any(|e| e.layout.shares_shaping_with(&layout))
        {
            let shaping = layout.shaping_bytes();
            slot.shaping_bytes += shaping;
            self.bytes += shaping;
        }
        slot.entries.push(CacheEntry {
            width,
            layout,
            bytes,
            last_used: tick,
        });
        self.count += 1;
        self.bytes += bytes;
        self.trim();
    }

    fn over(&self) -> bool {
        self.count > self.capacity || self.bytes > self.budget
    }

    fn trim(&mut self) {
        while self.over() && self.count > 1 {
            self.evict();
        }
    }

    fn evict(&mut self) {
        let mut ages: Vec<u64> = self
            .map
            .values()
            .flat_map(|slot| slot.entries.iter().map(|e| e.last_used))
            .collect();
        ages.sort_unstable();
        let cutoff = ages[(ages.len() - 1) / 2];
        for slot in self.map.values_mut() {
            slot.entries.retain(|e| e.last_used > cutoff);
            if !slot.entries.is_empty() {
                let mut kept: Vec<&CacheEntry> = Vec::new();
                for entry in &slot.entries {
                    if !kept
                        .iter()
                        .any(|k| k.layout.shares_shaping_with(&entry.layout))
                    {
                        kept.push(entry);
                    }
                }
                slot.shaping_bytes = kept.iter().map(|e| e.layout.shaping_bytes()).sum();
            }
        }
        self.map.retain(|_, slot| !slot.entries.is_empty());
        self.count = self.map.values().map(|slot| slot.entries.len()).sum();
        self.bytes = self.map.values().map(CacheSlot::bytes).sum();
    }
}
