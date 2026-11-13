use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::{Sizable, StyledExt, WindowExt, h_flex};
use gpui_kit::component::Disableable as _;

use gpui_kit::{
    App, AppContext, Context, Entity, IntoElement, ParentElement, Render,
    Styled, Subscription, Window, div, px,
};
use missal_core::FirRecord;

use crate::screens::fir_form::{FirForm, FirFormEvent};
use crate::state::{Models, toast_success};
use crate::theme::Palette;
use crate::ui::dirty_dot;

pub struct EditFooter {
    form: Entity<FirForm>,
    _subscription: Subscription,
}

impl EditFooter {
    fn new(form: Entity<FirForm>, cx: &mut Context<Self>) -> EditFooter {
        let subscription = cx.observe(&form, |_, _, cx| cx.notify());
        EditFooter {
            form,
            _subscription: subscription,
        }
    }
}

impl Render for EditFooter {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let (dirty, saving) = {
            let form = self.form.read(cx);
            (form.is_dirty(), form.is_saving())
        };
        let reset = self.form.clone();
        let save = self.form.clone();
        h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                h_flex()
                    .items_center()
                    .gap_1p5()
                    .text_size(px(12.))
                    .text_color(palette.muted_foreground)
                    .child(dirty_dot(&palette, dirty))
                    .child(if dirty { "Unsaved changes" } else { "No changes" }),
            )
            .child(div().flex_1())
            .child(
                Button::new("edit-reset")
                    .outline()
                    .small()
                    .label("Reset")
                    .disabled(!dirty || saving)
                    .on_click(move |_, _, cx| reset.update(cx, |form, cx| form.reset(cx))),
            )
            .child(
                Button::new("edit-save")
                    .primary()
                    .small()
                    .loading(saving)
                    .disabled(!dirty && !saving)
                    .label("Save changes")
                    .on_click(move |_, window, cx| save.update(cx, |form, cx| form.submit(window, cx))),
            )
    }
}

pub fn open_edit_sheet(record: FirRecord, window: &mut Window, cx: &mut App) {
    let models = Models::get(cx);
    let title = format!("Edit FIR {}", record.fir.fir_no);
    let form = cx.new(|cx| FirForm::new(models, Some(record), true, window, cx));
    let footer = cx.new(|cx| EditFooter::new(form.clone(), cx));
    let window_handle = window.window_handle();
    let subscription = cx.subscribe(&form, move |_, event: &FirFormEvent, cx| {
        if let FirFormEvent::Saved { .. } = event {
            window_handle
                .update(cx, |_, window, cx| {
                    window.close_sheet(cx);
                    toast_success(window, cx, "FIR details saved");
                })
                .ok();
        }
    });
    subscription.detach();
    form.update(cx, |form, cx| form.focus_first(window, cx));
    window.open_sheet(cx, move |sheet, _, cx| {
        let palette = *Palette::get(cx);
        sheet
            .size(px(640.))
            .title(div().text_size(px(15.)).font_semibold().child(missal_text::label(title.clone())))
            .child(div().px_4().pb_4().text_color(palette.foreground).child(form.clone()))
            .footer(footer.clone())
    });
}
