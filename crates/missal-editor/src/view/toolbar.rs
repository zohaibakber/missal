use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::popover::Popover;
use gpui_kit::component::{ActiveTheme, Disableable, Selectable, Sizable, h_flex, v_flex};
use gpui_kit::{
    AnyElement, Context, InteractiveElement, IntoElement, ParentElement,
    StatefulInteractiveElement, Styled, Window, div, px,
};
use missal_core::document::{Align, ListType, TextFormat};
use missal_text::label;

use super::Editor;

fn modifier() -> &'static str {
    if cfg!(target_os = "macos") {
        "Cmd"
    } else {
        "Ctrl"
    }
}

fn hint(text: &str, keys: Option<&str>) -> String {
    match keys {
        Some(keys) => format!("{text}  ({})", keys.replace("Mod", modifier())),
        None => text.to_owned(),
    }
}

fn separator(cx: &Context<Editor>) -> AnyElement {
    div()
        .mx_1()
        .w(px(1.))
        .h(px(16.))
        .bg(cx.theme().border)
        .into_any_element()
}

impl Editor {
    pub(crate) fn render_toolbar(
        &mut self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let core = &self.core;
        let bold = core.flag_state(TextFormat::BOLD);
        let italic = core.flag_state(TextFormat::ITALIC);
        let underline = core.flag_state(TextFormat::UNDERLINE);
        let align = core.align();
        let list = core.list_state();
        let can_undo = core.can_undo();
        let can_redo = core.can_redo();
        let button = |id: &'static str, icon: IconName, tip: String| {
            Button::new(id).icon(icon).ghost().small().tooltip(tip)
        };
        let act = |f: fn(&mut Editor, &mut Window, &mut Context<Editor>)| {
            let entity = entity.clone();
            move |_: &gpui_kit::ClickEvent, window: &mut Window, cx: &mut gpui_kit::App| {
                entity.update(cx, |editor, cx| f(editor, window, cx));
            }
        };
        let field_button = Button::new("insert-field-trigger")
            .icon(IconName::Braces)
            .label("Insert field")
            .ghost()
            .small()
            .tooltip(format!(
                "Or type {}name{}",
                self.fields.markers.open(),
                self.fields.markers.close()
            ));
        let menu_entity = entity.clone();
        let search = self.field_search.clone();
        let popover = Popover::new("insert-field")
            .trigger(field_button)
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
                let muted = cx.theme().muted_foreground;
                let accent = cx.theme().accent;
                let entity = menu_entity.clone();
                let mut index = 0usize;
                let mut list = v_flex()
                    .id("field-choices")
                    .max_h(px(320.))
                    .overflow_y_scroll()
                    .p_1();
                if groups.is_empty() {
                    list = list.child(
                        div()
                            .p_3()
                            .text_size(px(12.))
                            .text_color(muted)
                            .child("No fields found"),
                    );
                }
                for (heading, items) in groups {
                    list = list.child(
                        div()
                            .px_2()
                            .pt_2()
                            .pb_1()
                            .text_size(px(11.))
                            .text_color(muted)
                            .child(heading),
                    );
                    for (name, reference) in items {
                        let entity = entity.clone();
                        list = list.child(
                            h_flex()
                                .id(("field-choice", index))
                                .test_support()
                                .h(px(34.))
                                .px_2()
                                .rounded(px(6.))
                                .cursor_pointer()
                                .hover(move |style| style.bg(accent))
                                .on_click(move |_, window, cx| {
                                    let reference = reference.clone();
                                    entity.update(cx, |editor, cx| {
                                        editor.choose_field(reference, window, cx)
                                    });
                                })
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .text_size(px(15.))
                                        .child(label(name).rtl().truncate()),
                                ),
                        );
                        index += 1;
                    }
                }
                v_flex()
                    .w(px(288.))
                    .child(div().p_2().child(search.clone()))
                    .child(list)
            });
        h_flex()
            .id("editor-toolbar")
            .flex_shrink_0()
            .h(px(40.))
            .px_2()
            .gap_0p5()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                button("editor-undo", IconName::Undo2, hint("Undo", Some("Mod+Z")))
                    .disabled(!can_undo)
                    .on_click(act(Editor::undo)),
            )
            .child(
                button(
                    "editor-redo",
                    IconName::Redo2,
                    hint("Redo", Some("Mod+Shift+Z")),
                )
                .disabled(!can_redo)
                .on_click(act(Editor::redo)),
            )
            .child(separator(cx))
            .child(
                button("editor-bold", IconName::Bold, hint("Bold", Some("Mod+B")))
                    .selected(bold)
                    .on_click(act(|e, w, cx| e.toggle_format(TextFormat::BOLD, w, cx))),
            )
            .child(
                button(
                    "editor-italic",
                    IconName::Italic,
                    hint("Italic", Some("Mod+I")),
                )
                .selected(italic)
                .on_click(act(|e, w, cx| e.toggle_format(TextFormat::ITALIC, w, cx))),
            )
            .child(
                button(
                    "editor-underline",
                    IconName::Underline,
                    hint("Underline", Some("Mod+U")),
                )
                .selected(underline)
                .on_click(act(|e, w, cx| {
                    e.toggle_format(TextFormat::UNDERLINE, w, cx)
                })),
            )
            .child(separator(cx))
            .child(
                button(
                    "editor-align-left",
                    IconName::TextAlignStart,
                    hint("Align left", Some("Mod+L")),
                )
                .selected(align == Align::Left)
                .on_click(act(|e, w, cx| e.align(Align::Left, w, cx))),
            )
            .child(
                button(
                    "editor-align-center",
                    IconName::TextAlignCenter,
                    hint("Align center", Some("Mod+E")),
                )
                .selected(align == Align::Center)
                .on_click(act(|e, w, cx| e.align(Align::Center, w, cx))),
            )
            .child(
                button(
                    "editor-align-right",
                    IconName::TextAlignEnd,
                    hint("Align right", Some("Mod+R")),
                )
                .selected(align == Align::Right)
                .on_click(act(|e, w, cx| e.align(Align::Right, w, cx))),
            )
            .child(
                button(
                    "editor-align-justify",
                    IconName::TextAlignJustify,
                    hint("Justify", Some("Mod+J")),
                )
                .selected(align == Align::Justify)
                .on_click(act(|e, w, cx| e.align(Align::Justify, w, cx))),
            )
            .child(separator(cx))
            .child(
                button(
                    "editor-bullets",
                    IconName::List,
                    hint("Bulleted list", Some("Mod+Shift+8")),
                )
                .selected(list == Some(ListType::Bullet))
                .on_click(act(|e, w, cx| e.list(ListType::Bullet, w, cx))),
            )
            .child(
                button(
                    "editor-numbers",
                    IconName::ListOrdered,
                    hint("Numbered list", Some("Mod+Shift+7")),
                )
                .selected(list == Some(ListType::Number))
                .on_click(act(|e, w, cx| e.list(ListType::Number, w, cx))),
            )
            .child(separator(cx))
            .child(popover)
            .child(div().flex_1())
            .child(
                button(
                    "editor-zoom-out",
                    IconName::ZoomOut,
                    hint("Zoom out", Some("Mod+-")),
                )
                .on_click(act(|e, _, cx| {
                    let z = e.placement.zoom / 1.1;
                    e.set_zoom(Some(z), cx)
                })),
            )
            .child(
                div()
                    .w(px(44.))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .flex()
                    .justify_center()
                    .child(format!("{:.0}%", self.placement.zoom * 100.0)),
            )
            .child(
                button(
                    "editor-zoom-in",
                    IconName::ZoomIn,
                    hint("Zoom in", Some("Mod+=")),
                )
                .on_click(act(|e, _, cx| {
                    let z = e.placement.zoom * 1.1;
                    e.set_zoom(Some(z), cx)
                })),
            )
            .into_any_element()
    }
}
