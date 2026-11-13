use std::{cell::RefCell, rc::Rc, sync::Arc};

use gpui_kit::{
    AbsoluteLength, App, AvailableSpace, Bounds, DefiniteLength, Element, ElementId,
    GlobalElementId, InspectorElementId, IntoElement, LayoutId, Pixels, Refineable, SharedString,
    Size, Style,
    StyleRefinement, Styled, TextAlign, TextOverflow, WhiteSpace, Window, px, size,
};

use crate::{
    fonts::{FontFamily, UI_FAMILY_ALIASES, URDU_FAMILY_ALIASES},
    layout::{
        Align, AttributedText, Direction, LineHeight, ParagraphStyle, TextLayout, TextStyle,
    },
    paint::{TextPaint, paint_text},
    system::{TextSystem, to_rgba},
};

const ELLIPSIS: &str = "\u{2026}";

#[derive(Clone)]
enum Content {
    Plain(SharedString),
    Rich(AttributedText),
}

pub struct Label {
    content: Content,
    style: StyleRefinement,
    family: Option<FontFamily>,
    direction: Direction,
    align: Option<Align>,
    line_height: Option<LineHeight>,
}

pub fn label(text: impl Into<SharedString>) -> Label {
    Label::new(text)
}

impl Label {
    pub fn new(text: impl Into<SharedString>) -> Self {
        Self {
            content: Content::Plain(text.into()),
            style: StyleRefinement::default(),
            family: None,
            direction: Direction::Auto,
            align: None,
            line_height: None,
        }
    }

    pub fn rich(text: AttributedText) -> Self {
        Self {
            content: Content::Rich(text),
            ..Self::new("")
        }
    }

    pub fn family(mut self, family: FontFamily) -> Self {
        self.family = Some(family);
        self
    }

    pub fn document_font(self) -> Self {
        self.family(FontFamily::Document)
    }

    pub fn direction(mut self, direction: Direction) -> Self {
        self.direction = direction;
        self
    }

    pub fn rtl(self) -> Self {
        self.direction(Direction::Rtl)
    }

    pub fn ltr(self) -> Self {
        self.direction(Direction::Ltr)
    }

    pub fn align(mut self, align: Align) -> Self {
        self.align = Some(align);
        self
    }

    pub fn leading(mut self, line_height: LineHeight) -> Self {
        self.line_height = Some(line_height);
        self
    }

    fn resolve(&self, window: &mut Window) -> Spec {
        let refinement = self.style.text.clone();
        let text_style = window.with_text_style(Some(refinement.clone()), |w| w.text_style());
        let rem = window.rem_size();
        let font_size: f32 = text_style.font_size.to_pixels(rem).into();
        let family = self.family.clone().unwrap_or_else(|| {
            match refinement.font_family.as_ref().map(|f| f.as_ref()) {
                Some(name) if URDU_FAMILY_ALIASES.contains(&name) => FontFamily::Document,
                Some(name) if UI_FAMILY_ALIASES.contains(&name) => FontFamily::Interface,
                None => FontFamily::Interface,
                Some(other) => FontFamily::named(other),
            }
        });
        let style = TextStyle {
            family,
            size: font_size,
            weight: text_style.font_weight.0.round() as u16,
            italic: matches!(text_style.font_style, gpui_kit::FontStyle::Italic),
            underline: text_style.underline.is_some(),
            strikethrough: text_style.strikethrough.is_some(),
            color: to_rgba(text_style.color),
            background: text_style.background_color.map(to_rgba),
        };
        let text = match &self.content {
            Content::Plain(text) => AttributedText::new(text.as_ref(), style),
            Content::Rich(rich) => {
                let mut rich = rich.clone();
                if self.family.is_some() || refinement.font_family.is_some() {
                    rich.style.family = style.family.clone();
                }
                if refinement.font_size.is_some() {
                    rich.style.size = style.size;
                }
                if refinement.color.is_some() {
                    rich.style.color = style.color;
                }
                if refinement.font_weight.is_some() {
                    rich.style.weight = style.weight;
                }
                rich
            }
        };
        let line_height = self.line_height.unwrap_or_else(|| match refinement.line_height {
            Some(DefiniteLength::Absolute(AbsoluteLength::Pixels(px))) => {
                LineHeight::Px(px.into())
            }
            Some(DefiniteLength::Absolute(AbsoluteLength::Rems(rems))) => {
                LineHeight::Px(rems.to_pixels(rem).into())
            }
            Some(DefiniteLength::Fraction(f)) => LineHeight::Relative(f),
            _ => LineHeight::Normal,
        });
        let align = self.align.unwrap_or(match refinement.text_align {
            Some(TextAlign::Left) => Align::Left,
            Some(TextAlign::Center) => Align::Center,
            Some(TextAlign::Right) => Align::Right,
            None => Align::Start,
        });
        let truncate = text_style.text_overflow.is_some();
        let max_lines = text_style.line_clamp;
        let wrap = text_style.white_space == WhiteSpace::Normal && (!truncate || max_lines.is_some());
        Spec {
            text,
            paragraph: ParagraphStyle {
                direction: self.direction,
                align,
                line_height,
                wrap,
            },
            truncate: matches!(text_style.text_overflow, Some(TextOverflow::Truncate(_)))
                || truncate,
            max_lines,
        }
    }
}

impl Styled for Label {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

#[derive(Clone)]
struct Spec {
    text: AttributedText,
    paragraph: ParagraphStyle,
    truncate: bool,
    max_lines: Option<usize>,
}

impl Spec {
    fn layout(&self, width: Option<f32>, cx: &mut App) -> Arc<TextLayout> {
        let system = TextSystem::global_mut(cx);
        let engine = system.engine();
        let full = engine.layout(&self.text, &self.paragraph, width);
        let Some(width) = width else {
            return full;
        };
        if !self.truncate {
            return full;
        }
        match self.max_lines {
            Some(lines) if self.paragraph.wrap => {
                if full.lines().len() <= lines.max(1) {
                    return full;
                }
                let cut_line = &full.lines()[lines.max(1) - 1];
                let fits = |cut: usize, engine: &mut crate::layout::TextEngine| {
                    let candidate = with_ellipsis(&self.text, cut);
                    let layout = engine.layout(&candidate, &self.paragraph, Some(width));
                    (layout.lines().len() <= lines.max(1)).then_some(layout)
                };
                let boundaries = boundaries(&full, cut_line.text_range.start, cut_line.text_range.end);
                best_fit(&boundaries, |cut| fits(cut, engine)).unwrap_or(full)
            }
            _ => {
                if full.natural_width() <= width + 0.5 {
                    return full;
                }
                let fits = |cut: usize, engine: &mut crate::layout::TextEngine| {
                    let candidate = with_ellipsis(&self.text, cut);
                    let layout = engine.layout(&candidate, &self.paragraph, Some(width));
                    (layout.natural_width() <= width + 0.5).then_some(layout)
                };
                let boundaries = boundaries(&full, 0, full.text().len());
                best_fit(&boundaries, |cut| fits(cut, engine)).unwrap_or(full)
            }
        }
    }

    fn measure(&self, known: Size<Option<Pixels>>, available: Size<AvailableSpace>, cx: &mut App) -> (Size<Pixels>, Arc<TextLayout>) {
        let constraint = known.width.map(f32::from).or(match available.width {
            AvailableSpace::Definite(w) => Some(f32::from(w)),
            AvailableSpace::MinContent => Some(0.0),
            AvailableSpace::MaxContent => None,
        });
        let layout = self.layout(constraint, cx);
        let width = if self.truncate {
            match constraint {
                Some(c) => layout.natural_width().min(c),
                None => layout.natural_width(),
            }
        } else {
            layout.natural_width()
        };
        (size(px(width.ceil()), px(layout.height().ceil())), layout)
    }
}

fn with_ellipsis(text: &AttributedText, cut: usize) -> AttributedText {
    let head = text.text[..cut].trim_end();
    let mut truncated = AttributedText::new(format!("{head}{ELLIPSIS}"), text.style.clone());
    let end = head.len();
    truncated.spans = text
        .spans
        .iter()
        .filter(|(range, _)| range.start < end)
        .map(|(range, attr)| (range.start..range.end.min(end), attr.clone()))
        .collect();
    truncated.objects = text
        .objects
        .iter()
        .filter(|o| o.index <= end)
        .cloned()
        .collect();
    truncated
}

fn boundaries(layout: &TextLayout, start: usize, end: usize) -> Vec<usize> {
    let text = layout.text();
    let mut cuts: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .filter(|&i| i >= start && i <= end && layout.snap_index(i) == i)
        .collect();
    cuts.push(end.min(text.len()));
    cuts.dedup();
    cuts
}

fn best_fit(
    cuts: &[usize],
    mut fits: impl FnMut(usize) -> Option<Arc<TextLayout>>,
) -> Option<Arc<TextLayout>> {
    let mut lo = 0usize;
    let mut hi = cuts.len();
    let mut best = None;
    while lo < hi {
        let mid = (lo + hi) / 2;
        match fits(cuts[mid]) {
            Some(layout) => {
                best = Some(layout);
                lo = mid + 1;
            }
            None => hi = mid,
        }
    }
    best.or_else(|| fits(0))
}

pub struct LabelState {
    spec: Spec,
    measured: Rc<RefCell<Option<Arc<TextLayout>>>>,
}

impl IntoElement for Label {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Label {
    type RequestLayoutState = LabelState;
    type PrepaintState = Option<Arc<TextLayout>>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let spec = self.resolve(window);
        let measured = Rc::new(RefCell::new(None));
        let mut style = Style::default();
        style.refine(&self.style);
        let layout_id = window.request_measured_layout(style, {
            let spec = spec.clone();
            let measured = measured.clone();
            move |known, available, _window, cx| {
                let (size, layout) = spec.measure(known, available, cx);
                *measured.borrow_mut() = Some(layout);
                size
            }
        });
        (layout_id, LabelState { spec, measured })
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let width: f32 = bounds.size.width.into();
        if let Some(layout) = state.measured.borrow().as_ref()
            && layout.max_width() == Some(width)
        {
            return Some(layout.clone());
        }
        Some(state.spec.layout(Some(width), cx))
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut Self::RequestLayoutState,
        layout: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(layout) = layout {
            paint_text(layout, bounds.origin, &TextPaint::default(), window, cx);
        }
    }
}
