use std::rc::Rc;

use gpui_kit::{App, Context, EventEmitter, Window};
use missal_core::{FirId, TemplateId};

use crate::ui::dialogs::{Choice, Tone, confirm_discard, open_choice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PrintTarget {
    Fir(FirId),
    Template(TemplateId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Firs,
    NewFir,
    Fir(FirId),
    EditFir(FirId),
    Print(PrintTarget),
    Settings,
    Templates,
    NewTemplate,
    Template(TemplateId),
    People,
    Publishing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    Push(Route),
    Replace(Route),
    Back,
    Forward,
}

pub enum NavEvent {
    Changed,
}

pub type Guard = Rc<dyn Fn(&App) -> bool>;
pub type Discarder = Rc<dyn Fn(&mut App)>;
pub type Saver = Rc<dyn Fn(&mut Window, &mut App, Box<dyn FnOnce(&mut Window, &mut App)>)>;

pub struct Navigator {
    entries: Vec<Route>,
    index: usize,
    guard: Option<Guard>,
    saver: Option<Saver>,
    discarder: Option<Discarder>,
}

impl EventEmitter<NavEvent> for Navigator {}

impl Navigator {
    pub fn new(start: Route) -> Navigator {
        Navigator {
            entries: vec![start],
            index: 0,
            guard: None,
            saver: None,
            discarder: None,
        }
    }

    pub fn current(&self) -> Route {
        self.entries[self.index]
    }

    pub fn can_go_back(&self) -> bool {
        self.index > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.index + 1 < self.entries.len()
    }

    pub fn history(&self) -> (&[Route], usize) {
        (&self.entries, self.index)
    }

    pub fn set_guard(&mut self, guard: Option<Guard>) {
        self.guard = guard;
    }

    pub fn set_saver(&mut self, saver: Option<Saver>) {
        self.saver = saver;
    }

    pub fn set_discarder(&mut self, discarder: Option<Discarder>) {
        self.discarder = discarder;
    }

    fn discard_changes(&mut self, cx: &mut App) {
        if let Some(discarder) = self.discarder.take() {
            discarder(cx);
        }
    }

    pub fn has_unsaved_changes(&self, cx: &App) -> bool {
        self.guard.as_ref().is_some_and(|guard| guard(cx))
    }

    fn target(&self, request: Request) -> Option<Route> {
        match request {
            Request::Push(route) | Request::Replace(route) => Some(route),
            Request::Back => self.can_go_back().then(|| self.entries[self.index - 1]),
            Request::Forward => self.can_go_forward().then(|| self.entries[self.index + 1]),
        }
    }

    pub fn request(&mut self, request: Request, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.target(request) else {
            return;
        };
        if target == self.current() && !matches!(request, Request::Replace(_)) {
            return;
        }
        if !self.has_unsaved_changes(cx) {
            self.commit(request, cx);
            return;
        }
        let navigator = cx.entity();
        if let Some(saver) = self.saver.clone() {
            let discard = navigator.clone();
            open_choice(
                window,
                cx,
                "Save changes first?",
                "This page has unsaved changes.",
                vec![
                    Choice::new("discard", "Don't save", Tone::Ghost, move |_, cx| {
                        discard.update(cx, |navigator, cx| {
                            navigator.discard_changes(cx);
                            navigator.guard = None;
                            navigator.commit(request, cx);
                        });
                    }),
                    Choice::new("cancel", "Cancel", Tone::Neutral, |_, _| {}),
                    Choice::new("save", "Save", Tone::Primary, move |window, cx| {
                        let navigator = navigator.clone();
                        saver(
                            window,
                            cx,
                            Box::new(move |_, cx| {
                                navigator.update(cx, |navigator, cx| {
                                    navigator.guard = None;
                                    navigator.commit(request, cx);
                                });
                            }),
                        );
                    }),
                ],
            );
            return;
        }
        confirm_discard(window, cx, move |_, cx| {
            navigator.update(cx, |navigator, cx| {
                navigator.discard_changes(cx);
                navigator.guard = None;
                navigator.commit(request, cx);
            });
        });
    }

    pub fn go(&mut self, route: Route, window: &mut Window, cx: &mut Context<Self>) {
        self.request(Request::Push(route), window, cx);
    }

    pub fn back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request(Request::Back, window, cx);
    }

    pub fn forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.request(Request::Forward, window, cx);
    }

    pub fn commit(&mut self, request: Request, cx: &mut Context<Self>) {
        match request {
            Request::Push(route) => {
                if route == self.current() {
                    return;
                }
                self.entries.truncate(self.index + 1);
                self.entries.push(route);
                self.index += 1;
            }
            Request::Replace(route) => self.entries[self.index] = route,
            Request::Back => {
                if self.can_go_back() {
                    self.index -= 1;
                }
            }
            Request::Forward => {
                if self.can_go_forward() {
                    self.index += 1;
                }
            }
        }
        self.guard = None;
        self.discarder = None;
        cx.emit(NavEvent::Changed);
        cx.notify();
    }

    pub fn replace_unguarded(&mut self, route: Route, cx: &mut Context<Self>) {
        self.commit(Request::Replace(route), cx);
    }
}
