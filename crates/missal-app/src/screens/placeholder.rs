use gpui_kit::assets::IconName;
use gpui_kit::{
    App, Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Window, div,
};

use crate::app::navigate_back;
use crate::state::{Models, PrintTarget, Route};
use crate::theme::Palette;
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{button, empty_state};

pub struct PlaceholderScreen {
    models: Models,
    route: Route,
    focus: FocusHandle,
}

impl PlaceholderScreen {
    pub fn new(models: Models, route: Route, cx: &mut Context<Self>) -> PlaceholderScreen {
        PlaceholderScreen {
            models,
            route,
            focus: cx.focus_handle(),
        }
    }

    pub fn route(&self) -> Route {
        self.route
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    fn fir_number(&self, id: missal_core::FirId, cx: &App) -> String {
        self.models
            .firs
            .read(cx)
            .get(id)
            .map(|row| row.no.to_string())
            .unwrap_or_default()
    }

    fn title(&self, cx: &App) -> String {
        match self.route {
            Route::EditFir(id) => format!("Edit FIR {}", self.fir_number(id, cx)),
            Route::Print(PrintTarget::Fir(id)) => format!("Print FIR {}", self.fir_number(id, cx)),
            Route::Print(PrintTarget::Template(_)) => "Print template".to_owned(),
            _ => String::new(),
        }
    }
}

impl HeaderSource for PlaceholderScreen {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        HeaderContent::new(self.title(cx).trim().to_owned())
    }
}

impl Render for PlaceholderScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        div()
            .id("placeholder-screen")
            .track_focus(&self.focus)
            .size_full()
            .child(empty_state(
                &palette,
                IconName::Construction,
                "This page is not built yet",
                Some("Go back to continue where you were.".into()),
                Some(
                    button("placeholder-back", "Back")
                        .outline()
                        .sm()
                        .on_click(|_, window, cx| navigate_back(window, cx))
                        .into_any_element(),
                ),
            ))
    }
}
