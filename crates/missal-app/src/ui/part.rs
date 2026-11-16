use gpui_kit::{
    A11ySubtreeBuilder, AnyElement, App, Bounds, Div, Element, ElementId, GlobalElementId,
    InspectorElementId, InteractiveElement, Interactivity, IntoElement, LayoutId, ParentElement,
    Pixels, Refineable, StyleRefinement, Styled, Window, accesskit, div,
};

use crate::theme::Palette;

type Theme = Box<dyn FnOnce(Div, &Palette) -> Div>;

pub struct Part {
    inner: Div,
    theme: Option<Theme>,
}

pub fn part(theme: impl FnOnce(Div, &Palette) -> Div + 'static) -> Part {
    Part {
        inner: div(),
        theme: Some(Box::new(theme)),
    }
}

pub(crate) fn underlay<T: Styled>(mut element: T, theme: impl FnOnce(T) -> T) -> T {
    let own = std::mem::take(element.style());
    let mut element = theme(element);
    element.style().refine(&own);
    element
}

impl Part {
    fn resolve(&mut self, cx: &App) {
        if let Some(theme) = self.theme.take() {
            let inner = std::mem::replace(&mut self.inner, div());
            let palette = *Palette::get(cx);
            self.inner = underlay(inner, |inner| theme(inner, &palette));
        }
    }
}

impl Styled for Part {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl ParentElement for Part {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements)
    }
}

impl InteractiveElement for Part {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl IntoElement for Part {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Part {
    type RequestLayoutState = <Div as Element>::RequestLayoutState;
    type PrepaintState = <Div as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        Element::source_location(&self.inner)
    }

    fn a11y_role(&self) -> Option<accesskit::Role> {
        self.inner.a11y_role()
    }

    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node)
    }

    fn a11y_synthetic_children(
        &mut self,
        prepaint: &mut Self::PrepaintState,
        builder: &mut A11ySubtreeBuilder,
    ) {
        self.inner.a11y_synthetic_children(prepaint, builder)
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.resolve(cx);
        self.inner.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        )
    }
}
