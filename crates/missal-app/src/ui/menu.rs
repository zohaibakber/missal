use std::rc::Rc;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, ClickEvent, Div, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, RenderOnce, SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};

use crate::theme::{ICON_SM, MENU_ITEM, Palette, RADIUS_LG, RADIUS_SM};
use crate::ui::{ClickHandler, KeyTone, keycaps_toned, sized_icon};

type HoverHandler = Rc<dyn Fn(&mut Window, &mut App)>;

pub fn menu_panel(palette: &Palette) -> Div {
    v_flex()
        .p(px(4.))
        .rounded(px(RADIUS_LG))
        .bg(palette.raised)
        .text_color(palette.ink)
        .shadow(palette.shadow_md())
        .font_family(missal_text::UI_FAMILY)
}

pub fn menu_separator(palette: &Palette) -> Div {
    div().h(px(1.)).my(px(4.)).mx(px(-4.)).bg(palette.line)
}

pub enum Lead {
    None,
    Icon(IconName),
    Check(bool),
    Element(AnyElement),
}

pub enum Trail {
    None,
    Keys(SharedString),
    Text(SharedString),
    Chevron,
}

#[derive(IntoElement)]
pub struct MenuItem {
    id: ElementId,
    lead: Lead,
    content: AnyElement,
    trail: Trail,
    highlighted: bool,
    danger: bool,
    disabled: bool,
    on_click: Option<ClickHandler>,
    on_hover: Option<HoverHandler>,
}

pub fn menu_item(id: impl Into<ElementId>, text: impl Into<SharedString>) -> MenuItem {
    let text: SharedString = text.into();
    menu_item_element(id, div().child(text).into_any_element())
}

pub fn menu_item_element(id: impl Into<ElementId>, content: AnyElement) -> MenuItem {
    MenuItem {
        id: id.into(),
        lead: Lead::None,
        content,
        trail: Trail::None,
        highlighted: false,
        danger: false,
        disabled: false,
        on_click: None,
        on_hover: None,
    }
}

impl MenuItem {
    pub fn icon(mut self, icon: IconName) -> Self {
        self.lead = Lead::Icon(icon);
        self
    }

    pub fn check(mut self, checked: bool) -> Self {
        self.lead = Lead::Check(checked);
        self
    }

    pub fn lead(mut self, element: impl IntoElement) -> Self {
        self.lead = Lead::Element(element.into_any_element());
        self
    }

    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.trail = Trail::Keys(keys.into());
        self
    }

    pub fn trailing_text(mut self, text: impl Into<SharedString>) -> Self {
        self.trail = Trail::Text(text.into());
        self
    }

    pub fn submenu(mut self) -> Self {
        self.trail = Trail::Chevron;
        self
    }

    pub fn highlighted(mut self, highlighted: bool) -> Self {
        self.highlighted = highlighted;
        self
    }

    pub fn danger(mut self) -> Self {
        self.danger = true;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    pub fn on_hover(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for MenuItem {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let text = if self.danger {
            palette.danger_text
        } else {
            palette.ink
        };
        let icon_color = if self.danger {
            palette.danger_text
        } else {
            palette.ink_muted
        };
        let lead: Option<AnyElement> = match self.lead {
            Lead::None => None,
            Lead::Icon(name) => Some(sized_icon(name, ICON_SM, icon_color).into_any_element()),
            Lead::Check(checked) => Some(
                div()
                    .size(px(ICON_SM))
                    .flex_none()
                    .when(checked, |this| {
                        this.child(sized_icon(IconName::Check, ICON_SM, palette.ink))
                    })
                    .into_any_element(),
            ),
            Lead::Element(element) => Some(element),
        };
        let trail: Option<AnyElement> = match self.trail {
            Trail::None => None,
            Trail::Keys(keys) => {
                Some(keycaps_toned(&palette, &keys, KeyTone::Bare).into_any_element())
            }
            Trail::Text(value) => Some(
                div()
                    .text_size(px(12.))
                    .text_color(palette.ink_muted)
                    .child(value)
                    .into_any_element(),
            ),
            Trail::Chevron => Some(
                sized_icon(IconName::ChevronRight, ICON_SM, palette.ink_muted).into_any_element(),
            ),
        };
        let hover = palette.hover;
        let on_click = self.on_click.filter(|_| !self.disabled);
        let on_hover = self.on_hover;
        h_flex()
            .id(self.id)
            .h(px(MENU_ITEM))
            .flex_none()
            .px(px(8.))
            .gap(px(8.))
            .items_center()
            .rounded(px(RADIUS_SM))
            .text_size(px(13.))
            .font_weight(FontWeight::NORMAL)
            .text_color(text)
            .role(gpui_kit::accesskit::Role::MenuItem)
            .when(self.highlighted, |this| this.bg(hover))
            .when(self.disabled, |this| this.opacity(0.45))
            .when(!self.disabled, |this| {
                this.cursor_pointer().hover(move |s| s.bg(hover))
            })
            .when_some(on_hover, |this, handler| {
                this.on_hover(move |hovered, window, cx| {
                    if *hovered {
                        handler(window, cx)
                    }
                })
            })
            .when_some(on_click, |this, handler| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
            .children(lead)
            .child(div().flex_1().min_w_0().child(self.content))
            .children(trail)
            .test_support()
    }
}
