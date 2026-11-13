use std::{
    borrow::Cow,
    collections::HashMap,
    hash::{DefaultHasher, Hash, Hasher},
    sync::Arc,
};

use parley::{
    BaseDirection, CHROMIUM_LINE_BREAK_OVERRIDE, FontFamily as ParleyFamily, FontFamilyName,
    FontStyle, FontWeight, InlineBox, InlineBoxKind, LayoutContext, LineHeight as ParleyLineHeight,
    StyleProperty, VerticalAlign,
};

use super::{
    style::{Attr, AttributedText, LineHeight, ParagraphStyle, Rgba},
    text_layout::TextLayout,
};
use crate::fonts::{FaceId, FontFamily, FontRegistry};

pub struct TextEngine {
    fonts: FontRegistry,
    faces: Arc<HashMap<u64, FaceId>>,
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
}

fn family_property(family: &FontFamily) -> StyleProperty<'static, Rgba> {
    let names: Vec<FontFamilyName<'static>> = family
        .stack()
        .into_iter()
        .map(|name| FontFamilyName::Named(Cow::Owned(name.to_owned())))
        .collect();
    StyleProperty::FontFamily(ParleyFamily::List(Cow::Owned(names)))
}

fn attr_property(attr: &Attr) -> Option<StyleProperty<'static, Rgba>> {
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
        let fonts = FontRegistry::new();
        let faces = Arc::new(fonts.faces_by_blob().clone());
        Self {
            fonts,
            faces,
            lcx: LayoutContext::new(),
            cache: LayoutCache::new(4096),
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
        let mut builder = self
            .lcx
            .ranged_builder(&mut self.fonts.fcx, source, 1.0, false);
        builder.set_base_direction(if rtl {
            BaseDirection::Rtl
        } else {
            BaseDirection::Ltr
        });
        builder.set_line_break_override(Some(CHROMIUM_LINE_BREAK_OVERRIDE));
        let style = &text.style;
        builder.push_default(family_property(&style.family));
        builder.push_default(StyleProperty::FontSize(style.size));
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(style.weight as f32)));
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
        TextLayout::from_parley(
            text.clone(),
            *paragraph,
            rtl,
            inner,
            self.faces.clone(),
            max_width,
        )
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
            ..self.cache.stats
        }
    }

    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub fn set_cache_capacity(&mut self, capacity: usize) {
        self.cache.capacity = capacity.max(16);
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
    last_used: u64,
}

struct LayoutCache {
    map: HashMap<u64, Vec<CacheEntry>>,
    count: usize,
    capacity: usize,
    tick: u64,
    stats: CacheStats,
}

impl LayoutCache {
    fn new(capacity: usize) -> Self {
        Self {
            map: HashMap::new(),
            count: 0,
            capacity,
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
    }

    fn get(
        &mut self,
        key: u64,
        text: &AttributedText,
        paragraph: &ParagraphStyle,
        width: Option<u32>,
        tick: u64,
    ) -> Option<Arc<TextLayout>> {
        let entries = self.map.get_mut(&key)?;
        let entry = entries.iter_mut().find(|e| {
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
            .iter()
            .find(|e| e.layout.paragraph == *paragraph && e.layout.source == *text)
            .map(|e| e.layout.clone())
    }

    fn insert(&mut self, key: u64, width: Option<u32>, layout: Arc<TextLayout>, tick: u64) {
        if self.count >= self.capacity {
            self.evict();
        }
        self.map.entry(key).or_default().push(CacheEntry {
            width,
            layout,
            last_used: tick,
        });
        self.count += 1;
    }

    fn evict(&mut self) {
        let mut ages: Vec<u64> = self
            .map
            .values()
            .flat_map(|v| v.iter().map(|e| e.last_used))
            .collect();
        ages.sort_unstable();
        let cutoff = ages[ages.len() / 2];
        for entries in self.map.values_mut() {
            entries.retain(|e| e.last_used > cutoff);
        }
        self.map.retain(|_, v| !v.is_empty());
        self.count = self.map.values().map(Vec::len).sum();
    }
}
