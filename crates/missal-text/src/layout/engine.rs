use std::{borrow::Cow, sync::Arc};

use parley::{
    BaseDirection, CHROMIUM_LINE_BREAK_OVERRIDE, FontContext, FontFamily as ParleyFamily,
    FontFamilyName, FontStyle, FontWeight, InlineBox, InlineBoxKind, LayoutContext,
    LineHeight as ParleyLineHeight, StyleProperty, VerticalAlign,
};

use super::{
    cache::{LayoutCache, Lookup},
    style::{Attr, AttributedText, LineHeight, ParagraphStyle, Rgba},
    text_layout::TextLayout,
};
use crate::fonts::{FontFamily, UI_FAMILY, URDU_FAMILY, font_context};

pub struct TextEngine {
    fcx: FontContext,
    lcx: LayoutContext<Rgba>,
    cache: LayoutCache,
    counts: LayoutCounts,
}

impl Default for TextEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LayoutCounts {
    pub hits: u64,
    pub rebreaks: u64,
    pub builds: u64,
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
        Attr::WordSpacing(spacing) => StyleProperty::WordSpacing(*spacing),
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
        Self::with_cache(LayoutCache::default())
    }

    pub fn with_cache(cache: LayoutCache) -> Self {
        Self {
            fcx: font_context(),
            lcx: LayoutContext::new(),
            cache,
            counts: LayoutCounts::default(),
        }
    }

    pub fn cache(&self) -> &LayoutCache {
        &self.cache
    }

    pub fn counts(&self) -> LayoutCounts {
        self.counts
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
        match self.cache.find(text, paragraph, max_width) {
            Lookup::Hit(found) => {
                self.counts.hits += 1;
                found
            }
            Lookup::Rebreak(probe, template) => {
                self.counts.rebreaks += 1;
                self.cache
                    .insert(probe, Arc::new(template.with_max_width(max_width)))
            }
            Lookup::Miss(probe) => {
                self.counts.builds += 1;
                let built = Arc::new(self.layout_uncached(text, paragraph, max_width));
                self.cache.insert(probe, built)
            }
        }
    }
}
