use std::rc::Rc;

use gpui_kit::{App, Context, EventEmitter, Window};
use missal_core::{FirId, TemplateId};

use crate::ui::dialogs::confirm_discard;

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

pub struct Navigator {
    entries: Vec<Route>,
    index: usize,
    guard: Option<Guard>,
}

impl EventEmitter<NavEvent> for Navigator {}

impl Navigator {
    pub fn new(start: Route) -> Navigator {
        Navigator {
            entries: vec![start],
            index: 0,
            guard: None,
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
        confirm_discard(window, cx, move |_, cx| {
            navigator.update(cx, |navigator, cx| {
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
        cx.emit(NavEvent::Changed);
        cx.notify();
    }

    pub fn replace_unguarded(&mut self, route: Route, cx: &mut Context<Self>) {
        self.commit(Request::Replace(route), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::{AppContext, TestAppContext};

    fn fir(id: i64) -> Route {
        Route::Fir(FirId::new(id).unwrap())
    }

    #[gpui_kit::test]
    fn history_pushes_truncates_and_walks(cx: &mut TestAppContext) {
        let nav = cx.new(|_| Navigator::new(Route::Firs));
        cx.update(|cx| {
            nav.update(cx, |nav, cx| {
                nav.commit(Request::Push(fir(1)), cx);
                nav.commit(Request::Push(Route::Settings), cx);
                assert_eq!(nav.current(), Route::Settings);
                nav.commit(Request::Back, cx);
                assert_eq!(nav.current(), fir(1));
                assert!(nav.can_go_forward());
                nav.commit(Request::Push(Route::Templates), cx);
                assert!(!nav.can_go_forward());
                assert_eq!(nav.history().0, &[Route::Firs, fir(1), Route::Templates]);
                nav.commit(Request::Replace(Route::NewFir), cx);
                assert_eq!(nav.history().0, &[Route::Firs, fir(1), Route::NewFir]);
                nav.commit(Request::Back, cx);
                nav.commit(Request::Back, cx);
                nav.commit(Request::Back, cx);
                assert_eq!(nav.current(), Route::Firs);
                assert!(!nav.can_go_back());
            });
        });
    }
}
