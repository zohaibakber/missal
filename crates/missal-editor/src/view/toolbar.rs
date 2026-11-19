use std::rc::Rc;
use std::time::Duration;

use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{Icon, Selectable, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, AnyView, App, AppContext, ClickEvent, Context, ElementId, FontWeight,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_core::DisplayMode;
use missal_core::document::{Align, ListType, TextFormat};
use missal_text::label;

use super::Editor;
use super::theme::EditorTheme;

pub const TOOLTIP_DELAY: Duration = Duration::from_millis(400);

fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

fn caps(keys: &str) -> Vec<String> {
    keys.split('+')
        .map(|key| match key {
            "Mod" => modifier().to_owned(),
            other => other.to_owned(),
        })
        .collect()
}

pub struct ToolTip {
    label: SharedString,
    keys: Option<SharedString>,
}

impl Render for ToolTip {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = EditorTheme::get(cx);
        div().p(px(6.)).child(
            h_flex()
                .items_center()
                .gap(px(8.))
                .py(px(4.))
                .px(px(8.))
                .rounded(px(8.))
                .bg(theme.raised)
                .shadow(theme.bar_shadows().to_vec())
                .text_color(theme.ink)
                .font_family(missal_text::UI_FAMILY)
                .text_size(px(12.))
                .line_height(px(16.))
                .child(self.label.clone())
                .children(self.keys.as_ref().map(|keys| {
                    h_flex()
                        .gap(px(3.))
                        .children(caps(keys).into_iter().map(|cap| {
                            div()
                                .h(px(20.))
                                .min_w(px(20.))
                                .px(px(4.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(4.))
                                .bg(theme.hover)
                                .text_color(theme.ink_muted)
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(cap)
                        }))
                })),
        )
    }
}

pub fn tooltip(
    label: impl Into<SharedString>,
    keys: Option<&str>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label: SharedString = label.into();
    let keys: Option<SharedString> = keys.map(|keys| SharedString::from(keys.to_owned()));
    move |_, cx| {
        cx.new(|_| ToolTip {
            label: label.clone(),
            keys: keys.clone(),
        })
        .into()
    }
}

type Handler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub struct ToolButton {
    id: ElementId,
    icon: IconName,
    label: SharedString,
    keys: Option<SharedString>,
    dense: bool,
    toggled: bool,
    disabled: bool,
    on_click: Option<Handler>,
}

pub fn tool(
    id: impl Into<ElementId>,
    icon: IconName,
    label: impl Into<SharedString>,
) -> ToolButton {
    ToolButton {
        id: id.into(),
        icon,
        label: label.into(),
        keys: None,
        dense: false,
        toggled: false,
        disabled: false,
        on_click: None,
    }
}

impl ToolButton {
    pub fn keys(mut self, keys: impl Into<SharedString>) -> Self {
        self.keys = Some(keys.into());
        self
    }

    pub fn dense(mut self) -> Self {
        self.dense = true;
        self
    }

    pub fn toggled(mut self, toggled: bool) -> Self {
        self.toggled = toggled;
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
}

impl Selectable for ToolButton {
    fn selected(mut self, selected: bool) -> Self {
        self.toggled = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.toggled
    }
}

impl RenderOnce for ToolButton {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = EditorTheme::get(cx);
        let size = if self.dense { 24. } else { 28. };
        let glyph = if self.dense { 14. } else { 16. };
        let color = theme.ink.opacity(0.8);
        let disabled = self.disabled;
        let hover = theme.hover;
        div()
            .id(self.id)
            .flex_none()
            .size(px(size))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(if self.dense { 6. } else { 8. }))
            .text_color(color)
            .cursor_pointer()
            .when(self.toggled, |this| {
                this.bg(theme.pressed).text_color(theme.ink)
            })
            .when(disabled, |this| this.opacity(0.64))
            .when(!disabled && !self.toggled, |this| {
                this.hover(move |s| s.bg(hover))
            })
            .role(gpui_kit::accesskit::Role::Button)
            .aria_label(self.label.clone())
            .when(self.toggled, |this| {
                this.aria_toggled(gpui_kit::accesskit::Toggled::True)
            })
            .tooltip(tooltip(self.label.clone(), self.keys.as_deref()))
            .tooltip_show_delay(TOOLTIP_DELAY)
            .when_some(self.on_click.filter(|_| !disabled), |this, handler| {
                this.on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event, window, cx)
                })
            })
            .child(Icon::new(self.icon).size(px(glyph)).text_color(color))
            .test_support()
    }
}

fn separator(line: gpui_kit::Hsla) -> AnyElement {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(16.))
        .mx(px(6.))
        .bg(line)
        .into_any_element()
}

impl Editor {
    fn field_menu(&self, theme: EditorTheme, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity();
        let tip = format!(
            "Insert field (or type {}name{})",
            self.fields.markers.open(),
            self.fields.markers.close()
        );
        let trigger = tool("insert-field-trigger", IconName::Braces, tip).keys("Mod+Shift+F");
        let menu_entity = entity.clone();
        let search = self.field_search.clone();
        let query_empty = self.field_query.is_empty();
        Popover::new("insert-field")
            .trigger(trigger)
            .open(self.field_menu_open)
            .on_open_change({
                let entity = entity.clone();
                move |open, window, cx| {
                    let open = *open;
                    entity.update(cx, |editor, cx| editor.open_field_menu(open, window, cx));
                }
            })
            .p_0()
            .content(move |_, _, cx| {
                let editor = menu_entity.read(cx);
                let groups = editor.field_choices();
                let entity = menu_entity.clone();
                let mut index = 0usize;
                let mut list = v_flex()
                    .id("field-choices")
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .p(px(4.));
                if groups.is_empty() {
                    list = list.child(
                        div()
                            .p(px(12.))
                            .text_size(px(13.))
                            .text_color(theme.ink_muted)
                            .child("No fields found"),
                    );
                }
                for (heading, items) in groups {
                    list = list.child(
                        div()
                            .px(px(8.))
                            .pt(px(8.))
                            .pb(px(4.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.ink_muted)
                            .child(heading),
                    );
                    for (name, reference) in items {
                        let entity = entity.clone();
                        list = list.child(
                            h_flex()
                                .id(("field-choice", index))
                                .test_support()
                                .h(px(32.))
                                .px(px(8.))
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(theme.hover))
                                .on_click(move |_, window, cx| {
                                    let reference = reference.clone();
                                    entity.update(cx, |editor, cx| {
                                        editor.choose_field(reference, window, cx)
                                    });
                                })
                                .child(
                                    div().flex_1().min_w_0().text_size(px(16.)).child(
                                        label(name)
                                            .rtl()
                                            .leading(missal_text::LineHeight::Px(30.))
                                            .truncate(),
                                    ),
                                ),
                        );
                        index += 1;
                    }
                }
                let clear = {
                    let entity = menu_entity.clone();
                    (!query_empty).then(|| {
                        tool("field-search-clear", IconName::X, "Clear search")
                            .dense()
                            .on_click(move |_, window, cx| {
                                entity.update(cx, |editor, cx| {
                                    editor.field_search.update(cx, |input, cx| {
                                        input.set_text("", cx);
                                        input.focus(window, cx);
                                    })
                                });
                            })
                    })
                };
                let close = menu_entity.clone();
                v_flex()
                    .w(px(288.))
                    .on_key_down(move |event: &KeyDownEvent, window, cx| {
                        if event.keystroke.key == "escape" {
                            cx.stop_propagation();
                            close
                                .update(cx, |editor, cx| editor.open_field_menu(false, window, cx));
                        }
                    })
                    .child(
                        div().p(px(8.)).child(
                            h_flex()
                                .h(px(36.))
                                .px(px(11.))
                                .gap(px(8.))
                                .items_center()
                                .rounded(px(10.))
                                .border_1()
                                .border_color(theme.input)
                                .bg(theme.surface)
                                .children(clear)
                                .child(div().flex_1().min_w_0().mt(px(-1.)).child(search.clone()))
                                .child(
                                    Icon::new(IconName::Search)
                                        .size(px(16.))
                                        .text_color(theme.ink_muted.opacity(0.8)),
                                ),
                        ),
                    )
                    .child(div().h(px(1.)).bg(theme.line))
                    .child(list)
            })
            .into_any_element()
    }

    fn mode_toggle(&self, _theme: EditorTheme, cx: &mut Context<Self>) -> AnyElement {
        let entity = cx.entity();
        let names = self.fields.mode == DisplayMode::Labels;
        tool(
            "editor-mode-names",
            IconName::Tag,
            if names {
                "Show values"
            } else {
                "Show field names"
            },
        )
        .keys("Mod+Shift+L")
        .toggled(names)
        .on_click(move |_, _, cx| {
            let mode = if names {
                DisplayMode::Values
            } else {
                DisplayMode::Labels
            };
            entity.update(cx, |editor, cx| editor.request_mode(mode, cx));
        })
        .into_any_element()
    }

    pub(crate) fn render_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = EditorTheme::get(cx);
        let entity = cx.entity();
        let core = &self.core;
        let bold = core.flag_state(TextFormat::BOLD);
        let italic = core.flag_state(TextFormat::ITALIC);
        let underline = core.flag_state(TextFormat::UNDERLINE);
        let align = core.align();
        let list = core.list_state();
        let can_undo = core.can_undo();
        let can_redo = core.can_redo();
        let act = |f: fn(&mut Editor, &mut Window, &mut Context<Editor>)| {
            let entity = entity.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                entity.update(cx, |editor, cx| f(editor, window, cx));
            }
        };
        let group = |id: &'static str| h_flex().id(id).flex_none().gap(px(1.)).items_center();
        let history = group("editor-history")
            .child(
                tool("editor-undo", IconName::Undo2, "Undo")
                    .keys("Mod+Z")
                    .disabled(!can_undo)
                    .on_click(act(Editor::undo)),
            )
            .child(
                tool("editor-redo", IconName::Redo2, "Redo")
                    .keys("Mod+Y")
                    .disabled(!can_redo)
                    .on_click(act(Editor::redo)),
            );
        let bar = group("editor-toolbar")
            .flex_shrink(1.)
            .min_w_0()
            .overflow_hidden()
            .child(
                tool("editor-bold", IconName::Bold, "Bold")
                    .keys("Mod+B")
                    .toggled(bold)
                    .on_click(act(|e, w, cx| e.toggle_format(TextFormat::BOLD, w, cx))),
            )
            .child(
                tool("editor-italic", IconName::Italic, "Italic")
                    .keys("Mod+I")
                    .toggled(italic)
                    .on_click(act(|e, w, cx| e.toggle_format(TextFormat::ITALIC, w, cx))),
            )
            .child(
                tool("editor-underline", IconName::Underline, "Underline")
                    .keys("Mod+U")
                    .toggled(underline)
                    .on_click(act(|e, w, cx| {
                        e.toggle_format(TextFormat::UNDERLINE, w, cx)
                    })),
            )
            .child(separator(theme.line))
            .child(
                tool("editor-align-left", IconName::TextAlignStart, "Align left")
                    .keys("Mod+L")
                    .toggled(align == Align::Left)
                    .on_click(act(|e, w, cx| e.align(Align::Left, w, cx))),
            )
            .child(
                tool("editor-align-center", IconName::TextAlignCenter, "Center")
                    .keys("Mod+E")
                    .toggled(align == Align::Center)
                    .on_click(act(|e, w, cx| e.align(Align::Center, w, cx))),
            )
            .child(
                tool("editor-align-right", IconName::TextAlignEnd, "Align right")
                    .keys("Mod+R")
                    .toggled(align == Align::Right)
                    .on_click(act(|e, w, cx| e.align(Align::Right, w, cx))),
            )
            .child(
                tool(
                    "editor-align-justify",
                    IconName::TextAlignJustify,
                    "Justify",
                )
                .keys("Mod+J")
                .toggled(align == Align::Justify)
                .on_click(act(|e, w, cx| e.align(Align::Justify, w, cx))),
            )
            .child(separator(theme.line))
            .child(
                tool("editor-bullets", IconName::List, "Bulleted list")
                    .toggled(list == Some(ListType::Bullet))
                    .on_click(act(|e, w, cx| e.list(ListType::Bullet, w, cx))),
            )
            .child(
                tool("editor-numbers", IconName::ListOrdered, "Numbered list")
                    .toggled(list == Some(ListType::Number))
                    .on_click(act(|e, w, cx| e.list(ListType::Number, w, cx))),
            )
            .child(separator(theme.line))
            .child(self.field_menu(theme, cx))
            .child(
                tool("editor-find-trigger", IconName::Search, "Find and replace")
                    .keys("Mod+F")
                    .toggled(self.find.open)
                    .on_click(act(|e, w, cx| {
                        if e.find.open {
                            e.close_find(w, cx)
                        } else {
                            e.open_find(false, w, cx)
                        }
                    })),
            );
        let mode = self.mode_toggle.then(|| self.mode_toggle(theme, cx));
        let end = self
            .toolbar_end
            .clone()
            .and_then(|slot| slot(window, cx))
            .map(|content| h_flex().id("editor-toolbar-end").flex_none().child(content));
        h_flex()
            .id("editor-bar")
            .flex_none()
            .w_full()
            .h(px(44.))
            .items_center()
            .gap(px(2.))
            .px(px(8.))
            .bg(theme.surface)
            .border_b_1()
            .border_color(theme.line)
            .overflow_hidden()
            .occlude()
            .font_family(missal_text::UI_FAMILY)
            .child(history)
            .child(separator(theme.line))
            .child(bar)
            .child(div().flex_1().min_w(px(8.)))
            .children(mode)
            .children(end.map(|end| {
                h_flex()
                    .flex_none()
                    .items_center()
                    .child(separator(theme.line))
                    .child(end)
            }))
            .into_any_element()
    }

    fn zoom_controls(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = EditorTheme::get(cx);
        let entity = cx.entity();
        let act = |f: fn(&mut Editor, &mut Window, &mut Context<Editor>)| {
            let entity = entity.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                entity.update(cx, |editor, cx| f(editor, window, cx));
            }
        };
        let zoom = self.placement.zoom;
        let fit = self.is_fit_zoom();
        h_flex()
            .id("editor-zoom")
            .flex_none()
            .gap(px(2.))
            .items_center()
            .child(
                tool("editor-zoom-out", IconName::Minus, "Zoom out")
                    .keys("Mod+-")
                    .dense()
                    .on_click(act(|e, _, cx| {
                        let z = e.placement.zoom / 1.1;
                        e.set_zoom(Some(z), cx)
                    })),
            )
            .child(
                div()
                    .id("editor-zoom-fit")
                    .flex_none()
                    .w(px(48.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(8.))
                    .cursor_pointer()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(theme.ink_muted)
                    .font_features(gpui_kit::FontFeatures(std::sync::Arc::new(vec![(
                        "tnum".into(),
                        1,
                    )])))
                    .hover(move |s| s.bg(theme.hover).text_color(theme.ink))
                    .role(gpui_kit::accesskit::Role::Button)
                    .aria_label("Fit width")
                    .tooltip(tooltip(
                        if fit {
                            "Fit width"
                        } else {
                            "Reset to fit width"
                        },
                        Some("Mod+0"),
                    ))
                    .tooltip_show_delay(TOOLTIP_DELAY)
                    .on_click(act(|e, _, cx| e.set_zoom(None, cx)))
                    .child(format!("{:.0}%", zoom * 100.0))
                    .test_support(),
            )
            .child(
                tool("editor-zoom-in", IconName::Plus, "Zoom in")
                    .keys("Mod+=")
                    .dense()
                    .on_click(act(|e, _, cx| {
                        let z = e.placement.zoom * 1.1;
                        e.set_zoom(Some(z), cx)
                    })),
            )
            .into_any_element()
    }

    pub(crate) fn render_zoom(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let theme = EditorTheme::get(cx);
        h_flex()
            .absolute()
            .right(px(12.))
            .bottom(px(12.))
            .h(px(32.))
            .px(px(2.))
            .items_center()
            .rounded(px(10.))
            .bg(theme.raised)
            .shadow(theme.bar_shadows().to_vec())
            .occlude()
            .font_family(missal_text::UI_FAMILY)
            .child(self.zoom_controls(cx))
            .into_any_element()
    }
}
