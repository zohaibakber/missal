use crate::ui::ScrollArea as _;
use gpui_kit::TestSupportExt as _;
use gpui_kit::assets::IconName;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Render, ScrollHandle, SharedString, Styled, Subscription, Window,
    div, px,
};
use missal_core::TemplateId;
use missal_library::TemplateVersion;
use missal_text::{LineHeight, label};

use crate::app::navigate;
use crate::library::{LibraryModel, PublishState, PublishingRow, VersionHistory};
use crate::shortcuts::BlurFocus;
use crate::state::{Load, Models, Route};
use crate::theme::{PAGE_GUTTER, Palette, RADIUS};
use crate::ui::admin::{
    ACTIONS_WIDTH, Confirm, FilterOption, PANE_PAD, admins_only, cell, cell_text, confirm_card,
    data_row, fact, fact_text, filter_menu, head_cell, is_admin, page_toolbar, pane, pane_header,
    pane_section,
};
use crate::ui::dialogs::Tone;
use crate::ui::library::day_month_year;
use crate::ui::popup_menu::{MenuEntry, menu};
use crate::ui::titlebar::{HeaderContent, HeaderSource};
use crate::ui::{
    BadgeVariant, Intent, UrRole, badge, button, empty_state, ghost_sm, icon_button, isolate_urdu,
    primary_sm, skeleton_bar, table, table_body, table_footer, table_header, ur_inline,
};

const STATUS_WIDTH: f32 = 196.;
const VERSION_WIDTH: f32 = 96.;
const COMPUTER_WIDTH: f32 = 160.;
const UPDATED_WIDTH: f32 = 128.;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    NeedsAction,
    Published,
    NotInstalled,
}

const FILTERS: [(Filter, &str); 4] = [
    (Filter::All, "All"),
    (Filter::NeedsAction, "Needs action"),
    (Filter::Published, "Up to date"),
    (Filter::NotInstalled, "Not on this computer"),
];

impl Filter {
    fn admits(self, state: &PublishState) -> bool {
        match self {
            Filter::All => true,
            Filter::NeedsAction => matches!(
                state,
                PublishState::NotPublished
                    | PublishState::NameInLibrary
                    | PublishState::Changed
                    | PublishState::Behind { .. }
            ),
            Filter::Published => *state == PublishState::Current,
            Filter::NotInstalled => *state == PublishState::NotInstalled,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Selection {
    Local(TemplateId),
    Remote(String),
}

impl Selection {
    fn of(row: &PublishingRow) -> Option<Selection> {
        row.local
            .as_ref()
            .map(|local| Selection::Local(local.id))
            .or_else(|| {
                row.remote
                    .as_ref()
                    .map(|remote| Selection::Remote(remote.id.clone()))
            })
    }

    fn matches(&self, row: &PublishingRow) -> bool {
        match self {
            Selection::Local(id) => row.local.as_ref().is_some_and(|local| local.id == *id),
            Selection::Remote(id) => row.remote.as_ref().is_some_and(|remote| remote.id == *id),
        }
    }
}

fn state_label(state: &PublishState) -> &'static str {
    match state {
        PublishState::NotPublished => "Not published",
        PublishState::NameInLibrary => "Same name in library",
        PublishState::Current => "Up to date",
        PublishState::Changed => "Unpublished changes",
        PublishState::Behind { edited: false } => "Newer in library",
        PublishState::Behind { edited: true } => "Edited, newer in library",
        PublishState::NotInstalled => "Not on this computer",
    }
}

fn state_intent(state: &PublishState) -> Intent {
    match state {
        PublishState::Current => Intent::Success,
        PublishState::Changed | PublishState::NotPublished => Intent::Info,
        PublishState::NameInLibrary | PublishState::Behind { .. } => Intent::Warning,
        PublishState::NotInstalled => Intent::Neutral,
    }
}

fn state_badge(state: &PublishState) -> AnyElement {
    badge(BadgeVariant::Intent(state_intent(state)))
        .px(px(6.))
        .child(state_label(state))
        .into_any_element()
}

fn size_text(bytes: i64) -> String {
    const KB: f64 = 1024.;
    let value = bytes as f64;
    if value < KB {
        format!("{bytes} B")
    } else if value < KB * KB {
        format!("{:.0} KB", value / KB)
    } else {
        format!("{:.1} MB", value / (KB * KB))
    }
}

fn fields_text(count: i64) -> String {
    if count == 1 {
        "1 field".to_owned()
    } else {
        format!("{count} fields")
    }
}

fn person(name: Option<&str>, email: Option<&str>) -> String {
    let name = name.map(str::trim).filter(|name| !name.is_empty());
    match (name, email) {
        (Some(name), Some(email)) => format!("{name} · {email}"),
        (Some(name), None) => name.to_owned(),
        (None, Some(email)) => email.to_owned(),
        (None, None) => "Unknown".to_owned(),
    }
}

fn short_person(name: Option<&str>, email: Option<&str>) -> String {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .or(email)
        .unwrap_or("Unknown")
        .to_owned()
}

fn date_text(millis: i64) -> String {
    day_month_year(millis).unwrap_or_else(|| "Unknown".to_owned())
}

fn computer_text(row: &PublishingRow) -> String {
    match (&row.local, &row.link, &row.state) {
        (None, _, _) => "Not added".to_owned(),
        (Some(_), None, _) => "Local only".to_owned(),
        (Some(_), Some(link), PublishState::Changed | PublishState::Behind { edited: true }) => {
            format!("Version {} with edits", link.version)
        }
        (Some(_), Some(link), _) => format!("Version {}", link.version),
    }
}

fn publishable(row: &PublishingRow) -> Option<TemplateId> {
    matches!(
        row.state,
        PublishState::NotPublished | PublishState::Changed
    )
    .then(|| row.local.as_ref().map(|local| local.id))
    .flatten()
}

fn key_of(selection: &Selection) -> SharedString {
    match selection {
        Selection::Local(id) => format!("local-{id:?}").into(),
        Selection::Remote(id) => format!("remote-{id}").into(),
    }
}

const BULK: &str = "bulk";

pub struct PublishingScreen {
    focus: FocusHandle,
    table_scroll: ScrollHandle,
    pane_scroll: ScrollHandle,
    model: Option<Entity<LibraryModel>>,
    filter: Filter,
    selected: Option<Selection>,
    confirm: Option<Confirm>,
    _subscriptions: Vec<Subscription>,
}

impl PublishingScreen {
    pub fn new(cx: &mut Context<Self>) -> PublishingScreen {
        let model = LibraryModel::get(cx).filter(|_| is_admin(cx));
        let templates = Models::get(cx).templates;
        let mut subscriptions = vec![cx.observe(&templates, |_, _, cx| cx.notify())];
        if let Some(model) = &model {
            subscriptions.push(cx.observe(model, |this, _, cx| {
                this.ensure_history(cx);
                cx.notify();
            }));
            model.update(cx, |model, cx| {
                model.refresh(cx);
                model.refresh_managed(cx);
            });
        }
        if let Some(auth) = crate::auth::AuthModel::get(cx) {
            subscriptions.push(cx.observe(&auth, |_, _, cx| cx.notify()));
        }
        PublishingScreen {
            focus: cx.focus_handle(),
            table_scroll: ScrollHandle::new(),
            pane_scroll: ScrollHandle::new(),
            model,
            filter: Filter::All,
            selected: None,
            confirm: None,
            _subscriptions: subscriptions,
        }
    }

    pub fn focus(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.focus, cx);
    }

    pub fn is_dirty(&self, _: &App) -> bool {
        false
    }

    fn rows(&self, cx: &App) -> Vec<PublishingRow> {
        self.model
            .as_ref()
            .map_or_default(|model| model.read(cx).rows(cx))
    }

    fn visible(&self, cx: &App) -> Vec<PublishingRow> {
        self.rows(cx)
            .into_iter()
            .filter(|row| self.filter.admits(&row.state))
            .collect()
    }

    fn chosen(&self, cx: &App) -> Option<PublishingRow> {
        let selected = self.selected.as_ref()?;
        self.rows(cx).into_iter().find(|row| selected.matches(row))
    }

    fn ensure_history(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self.model.clone() else {
            return;
        };
        let remote = self
            .chosen(cx)
            .and_then(|row| row.remote.as_ref().map(|remote| remote.id.clone()));
        if let Some(remote) = remote {
            model.update(cx, |model, cx| model.load_versions(&remote, cx));
        }
    }

    fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        let key = selection.as_ref().map(key_of);
        if self
            .confirm
            .as_ref()
            .is_some_and(|confirm| Some(&confirm.key) != key.as_ref() && confirm.key != BULK)
        {
            self.confirm = None;
        }
        self.selected = selection;
        self.ensure_history(cx);
        cx.notify();
    }

    fn ask(&mut self, selection: Option<Selection>, confirm: Confirm, cx: &mut Context<Self>) {
        if selection.is_some() {
            self.select(selection, cx);
        }
        self.confirm = Some(confirm);
        cx.notify();
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.confirm = None;
        cx.notify();
    }

    fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.filter = filter;
        cx.notify();
    }

    fn refresh(&self, cx: &mut App) {
        if let Some(model) = &self.model {
            model.update(cx, |model, cx| {
                model.refresh(cx);
                model.refresh_managed(cx);
            });
        }
    }

    fn publish_all(&mut self, templates: Vec<TemplateId>, cx: &mut Context<Self>) {
        let Some(handle) = self.model.clone() else {
            return;
        };
        let count = templates.len();
        let title = if count == 1 {
            "Publish 1 template?".to_owned()
        } else {
            format!("Publish {count} templates?")
        };
        let confirm = Confirm::new(
            BULK,
            "publish-confirm",
            title,
            "Everyone who uses Missal will see these versions. Only saved changes are published.",
            "Publish",
            Tone::Primary,
            move |window, cx| {
                let templates = templates.clone();
                handle.update(cx, |model, cx| model.publish_many(templates, window, cx));
            },
        );
        self.ask(None, confirm, cx);
    }

    fn primary_action(
        &self,
        row: &PublishingRow,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let handle = self.model.clone()?;
        let screen = cx.entity().downgrade();
        let selection = Selection::of(row);
        let key = selection.as_ref().map_or_default(key_of);
        let local = row.local.as_ref().map(|local| local.id);
        let remote = row.remote.as_ref().map(|remote| remote.id.clone());
        let name = row.name.clone();
        let element = match (&row.state, local, remote) {
            (PublishState::NotPublished, Some(template), _) => {
                primary_sm("publishing-publish", "Publish")
                    .icon(IconName::CloudUpload)
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |model, cx| model.publish(template, window, cx));
                    })
                    .into_any_element()
            }
            (PublishState::Changed, Some(template), _) => {
                primary_sm("publishing-changes", "Publish changes")
                    .icon(IconName::CloudUpload)
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |model, cx| model.publish(template, window, cx));
                    })
                    .into_any_element()
            }
            (PublishState::NameInLibrary, Some(template), Some(remote)) => {
                primary_sm("publishing-replace", "Publish as new version")
                    .icon(IconName::CloudUpload)
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        let confirm = Confirm::new(
                            key.clone(),
                            "replace-confirm",
                            format!("Publish over «{}»?", isolate_urdu(&name)),
                            "Your copy becomes the next version of the library template with this name. The current version stays in its history.",
                            "Publish",
                            Tone::Primary,
                            move |window, cx| {
                                let remote = remote.clone();
                                handle.update(cx, |model, cx| {
                                    model.publish_to(template, Some(remote), window, cx);
                                });
                            },
                        );
                        let selection = selection.clone();
                        screen
                            .update(cx, |this, cx| this.ask(selection, confirm, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            (PublishState::Behind { edited }, _, Some(remote)) => {
                let edited = *edited;
                primary_sm("publishing-update", "Update this computer")
                    .icon(IconName::Download)
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        if !edited {
                            handle.update(cx, |model, cx| model.install_entry(&remote, window, cx));
                            return;
                        }
                        let confirm = Confirm::new(
                            key.clone(),
                            "replace",
                            format!("Replace your changes to «{}»?", isolate_urdu(&name)),
                            "Your edits to this template will be replaced by the library version. Documents already added to FIRs are not changed.",
                            "Replace",
                            Tone::Primary,
                            move |window, cx| {
                                handle.update(cx, |model, cx| {
                                    model.install_entry(&remote, window, cx);
                                });
                            },
                        );
                        let selection = selection.clone();
                        screen
                            .update(cx, |this, cx| this.ask(selection, confirm, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            (PublishState::NotInstalled, _, Some(remote)) => {
                primary_sm("publishing-add", "Add to this computer")
                    .icon(IconName::Download)
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |model, cx| model.install_entry(&remote, window, cx));
                    })
                    .into_any_element()
            }
            _ => return None,
        };
        Some(element)
    }

    fn remove_confirm(&self, row: &PublishingRow) -> Option<Confirm> {
        let handle = self.model.clone()?;
        let remote = row.remote.as_ref()?.id.clone();
        let key = Selection::of(row).as_ref().map(key_of)?;
        Some(Confirm::new(
            key,
            "remove-confirm",
            format!("Remove «{}» from the library?", isolate_urdu(&row.name)),
            "Its whole version history is deleted and cannot be restored. Copies people already added stay on their computers.",
            "Remove",
            Tone::Danger,
            move |window, cx| {
                handle.update(cx, |model, cx| model.unpublish(&remote, window, cx));
            },
        ))
    }

    fn row_menu(&self, index: usize, row: &PublishingRow, cx: &mut Context<Self>) -> AnyElement {
        let screen = cx.entity().downgrade();
        let selection = Selection::of(row);
        let template = row.local.as_ref().map(|local| local.id);
        let remove = self.remove_confirm(row);
        let target = Selection::of(row);
        menu(format!("publishing-row-menu-{index}"))
            .trigger(
                icon_button(
                    ("publishing-row-more", index),
                    IconName::Ellipsis,
                    "Actions",
                )
                .dense(),
            )
            .width(220.)
            .content(move |content, _, _| {
                let open = screen.clone();
                let selection = selection.clone();
                let content = content.item(
                    MenuEntry::new("publishing-row-details", "Show details", move |_, cx| {
                        let selection = selection.clone();
                        open.update(cx, |this, cx| this.select(selection, cx)).ok();
                    })
                    .icon(IconName::PanelRight),
                );
                let content = match template {
                    Some(template) => content.item(
                        MenuEntry::new(
                            "publishing-row-open",
                            "Open template",
                            move |window, cx| {
                                navigate(Route::Template(template), window, cx);
                            },
                        )
                        .icon(IconName::ExternalLink),
                    ),
                    None => content,
                };
                match remove.clone() {
                    Some(confirm) => {
                        let screen = screen.clone();
                        let selection = target.clone();
                        content.separator().item(
                            MenuEntry::new(
                                "publishing-row-remove",
                                "Remove from library…",
                                move |_, cx| {
                                    let confirm = confirm.clone();
                                    let selection = selection.clone();
                                    screen
                                        .update(cx, |this, cx| this.ask(selection, confirm, cx))
                                        .ok();
                                },
                            )
                            .icon(IconName::Trash)
                            .danger(),
                        )
                    }
                    None => content,
                }
            })
            .into_any_element()
    }

    fn head(&self, wide: bool) -> AnyElement {
        table_header()
            .id("publishing-table-head")
            .child(head_cell("Template").flex_1().min_w_0())
            .child(head_cell("Status").w(px(STATUS_WIDTH)).flex_none())
            .child(head_cell("Library").w(px(VERSION_WIDTH)).flex_none())
            .when(wide, |this| {
                this.child(head_cell("This computer").w(px(COMPUTER_WIDTH)).flex_none())
                    .child(head_cell("Last published").w(px(UPDATED_WIDTH)).flex_none())
            })
            .child(div().w(px(ACTIONS_WIDTH)).flex_none())
            .into_any_element()
    }

    fn row(
        &self,
        index: usize,
        row: &PublishingRow,
        last: bool,
        wide: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selection = Selection::of(row);
        let selected = self
            .selected
            .as_ref()
            .is_some_and(|selected| selected.matches(row));
        let fields = row
            .remote
            .as_ref()
            .map(|remote| remote.field_count)
            .or(row.local.as_ref().map(|local| local.field_count))
            .map_or_default(fields_text);
        let version = row
            .remote
            .as_ref()
            .map_or_else(|| "—".to_owned(), |remote| format!("v{}", remote.version));
        let updated = row
            .remote
            .as_ref()
            .map_or_else(|| "—".to_owned(), |remote| date_text(remote.updated_at));
        let menu = self.row_menu(index, row, cx);
        data_row(
            ("publishing-row", index),
            "publishing-row",
            selected,
            last,
            palette,
        )
        .aria_label(SharedString::from(row.name.clone()))
        .on_click(cx.listener(move |this, _, _, cx| this.select(selection.clone(), cx)))
        .child(
            cell()
                .flex_1()
                .gap(px(10.))
                .child(
                    div()
                        .size(px(28.))
                        .flex_none()
                        .rounded(px(RADIUS))
                        .border_1()
                        .border_color(palette.line)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::small_icon(IconName::FileText, palette.ink_muted)),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .child(
                            div()
                                .min_w_0()
                                .text_size(px(14.))
                                .text_color(palette.ink)
                                .child(ur_inline(row.name.clone(), UrRole::Label).truncate()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(palette.ink_faint)
                                .font_features(crate::ui::tabular())
                                .child(fields),
                        ),
                ),
        )
        .child(
            cell()
                .w(px(STATUS_WIDTH))
                .flex_none()
                .child(state_badge(&row.state)),
        )
        .child(
            cell()
                .w(px(VERSION_WIDTH))
                .flex_none()
                .child(cell_text(version, palette)),
        )
        .when(wide, |this| {
            this.child(
                cell()
                    .w(px(COMPUTER_WIDTH))
                    .flex_none()
                    .child(cell_text(computer_text(row), palette)),
            )
            .child(
                cell()
                    .w(px(UPDATED_WIDTH))
                    .flex_none()
                    .child(cell_text(updated, palette)),
            )
        })
        .child(
            div()
                .w(px(ACTIONS_WIDTH))
                .h_full()
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().child(menu)),
        )
        .test_support()
        .into_any_element()
    }

    fn facts(&self, row: &PublishingRow, palette: &Palette) -> AnyElement {
        let mut facts = Vec::new();
        match &row.remote {
            Some(remote) => {
                facts.push(("Library version", format!("Version {}", remote.version)));
                facts.push(("Versions kept", remote.version_count.to_string()));
                facts.push(("This computer", computer_text(row)));
                facts.push(("Fields", remote.field_count.to_string()));
                facts.push(("Size", size_text(remote.size)));
                facts.push((
                    "Published by",
                    person(
                        remote.publisher_name.as_deref(),
                        remote.publisher_email.as_deref(),
                    ),
                ));
                facts.push(("First published", date_text(remote.created_at)));
                facts.push(("Last published", date_text(remote.updated_at)));
            }
            None => {
                facts.push(("Library version", "Not published".to_owned()));
                facts.push(("This computer", computer_text(row)));
                if let Some(local) = &row.local {
                    facts.push(("Fields", local.field_count.to_string()));
                }
            }
        }
        v_flex()
            .children(
                facts
                    .into_iter()
                    .map(|(name, value)| fact(name, fact_text(value), palette)),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn version_row(
        &self,
        selection: Option<Selection>,
        remote: &str,
        name: &str,
        version: &TemplateVersion,
        current: i64,
        busy: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut details = vec![
            date_text(version.published_at),
            short_person(
                version.publisher_name.as_deref(),
                version.publisher_email.as_deref(),
            ),
            fields_text(version.field_count),
        ];
        if let Some(source) = version.restored_from {
            details.push(format!("from v{source}"));
        }
        let number = version.version;
        let handle = self.model.clone();
        let screen = cx.entity().downgrade();
        let remote = remote.to_owned();
        let name = name.to_owned();
        let key = selection.as_ref().map_or_default(key_of);
        h_flex()
            .id(("publishing-version", number as usize))
            .min_h(px(40.))
            .px(px(8.))
            .py(px(4.))
            .mx(px(-8.))
            .gap(px(10.))
            .items_center()
            .rounded(px(RADIUS))
            .hover(|style| style.bg(palette.hover))
            .child(
                div()
                    .w(px(32.))
                    .flex_none()
                    .text_size(px(13.))
                    .text_color(palette.ink)
                    .font_features(crate::ui::tabular())
                    .child(format!("v{number}")),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(12.))
                    .line_height(px(16.))
                    .text_color(palette.ink_muted)
                    .child(label(details.join(" · ")).ltr().truncate()),
            )
            .child(if number == current {
                badge(BadgeVariant::Intent(Intent::Success))
                    .px(px(6.))
                    .child("Live")
                    .into_any_element()
            } else {
                ghost_sm(("publishing-restore", number as usize), "Restore")
                    .disabled(busy)
                    .on_click(move |_, _, cx| {
                        let Some(handle) = handle.clone() else {
                            return;
                        };
                        let remote = remote.clone();
                        let confirm = Confirm::new(
                            key.clone(),
                            "restore-confirm",
                            format!("Restore version {number} of «{}»?", isolate_urdu(&name)),
                            format!(
                                "It is published again as version {}. People with this template are offered the update, and no version is deleted.",
                                current + 1
                            ),
                            "Restore",
                            Tone::Primary,
                            move |window, cx| {
                                handle.update(cx, |model, cx| {
                                    model.restore_version(&remote, number, window, cx);
                                });
                            },
                        );
                        let selection = selection.clone();
                        screen
                            .update(cx, |this, cx| this.ask(selection, confirm, cx))
                            .ok();
                    })
                    .into_any_element()
            })
            .into_any_element()
    }

    fn history(
        &self,
        row: &PublishingRow,
        busy: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let remote = row.remote.as_ref()?;
        let model = self.model.clone()?;
        let id = remote.id.clone();
        let history = model.read(cx).history(&remote.id).cloned();
        let body = match history {
            None | Some(VersionHistory::Loading) => v_flex()
                .gap(px(6.))
                .children((0..3).map(|_| skeleton_bar(300., 28.)))
                .into_any_element(),
            Some(VersionHistory::Failed(message)) => h_flex()
                .gap(px(8.))
                .items_center()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_size(px(13.))
                        .text_color(palette.ink_muted)
                        .child(label(message).ltr()),
                )
                .child(ghost_sm("publishing-history-retry", "Try again").on_click(
                    move |_, _, cx| {
                        model.update(cx, |model, cx| model.retry_versions(&id, cx));
                    },
                ))
                .into_any_element(),
            Some(VersionHistory::Ready(versions)) => {
                let selection = Selection::of(row);
                v_flex()
                    .children(versions.iter().map(|version| {
                        self.version_row(
                            selection.clone(),
                            &remote.id,
                            &row.name,
                            version,
                            remote.version,
                            busy,
                            palette,
                            cx,
                        )
                    }))
                    .into_any_element()
            }
        };
        Some(
            pane_section("Version history")
                .child(body)
                .into_any_element(),
        )
    }

    fn details(
        &self,
        row: &PublishingRow,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let busy = self
            .model
            .as_ref()
            .is_some_and(|model| model.read(cx).row_is_busy(row));
        let primary = self.primary_action(row, busy, cx);
        let open = row.local.as_ref().map(|local| local.id).map(|template| {
            icon_button("publishing-open", IconName::ExternalLink, "Open template")
                .on_click(move |_, window, cx| navigate(Route::Template(template), window, cx))
                .into_any_element()
        });
        let remove = self.remove_confirm(row).map(|confirm| {
            let screen = cx.entity().downgrade();
            let selection = Selection::of(row);
            menu("publishing-more")
                .trigger(
                    icon_button("publishing-more", IconName::Ellipsis, "More actions")
                        .disabled(busy),
                )
                .width(220.)
                .content(move |content, _, _| {
                    let screen = screen.clone();
                    let confirm = confirm.clone();
                    let selection = selection.clone();
                    content.item(
                        MenuEntry::new(
                            "publishing-remove",
                            "Remove from library…",
                            move |_, cx| {
                                let confirm = confirm.clone();
                                let selection = selection.clone();
                                screen
                                    .update(cx, |this, cx| this.ask(selection, confirm, cx))
                                    .ok();
                            },
                        )
                        .icon(IconName::Trash)
                        .danger(),
                    )
                })
                .into_any_element()
        });
        let key = Selection::of(row).as_ref().map(key_of);
        let confirm = self
            .confirm
            .as_ref()
            .filter(|confirm| Some(&confirm.key) == key.as_ref())
            .map(|confirm| {
                let screen = cx.entity().downgrade();
                confirm_card(confirm, move |_, cx| {
                    screen.update(cx, |this, cx| this.dismiss(cx)).ok();
                })
            });
        let history = self.history(row, busy, palette, cx);
        let close = cx.entity().downgrade();
        let title = div()
            .w_full()
            .text_size(px(18.))
            .text_color(palette.ink)
            .child(
                label(row.name.clone())
                    .rtl()
                    .leading(LineHeight::Px(30.))
                    .truncate(),
            );
        pane()
            .id("publishing-details")
            .child(pane_header(
                title,
                Some(
                    h_flex()
                        .justify_end()
                        .child(state_badge(&row.state))
                        .into_any_element(),
                ),
                "publishing-close",
                move |_, cx| {
                    close.update(cx, |this, cx| this.select(None, cx)).ok();
                },
            ))
            .child(
                v_flex()
                    .id("publishing-details-body")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_flex()
                            .px(px(PANE_PAD))
                            .pb(px(16.))
                            .gap(px(12.))
                            .child(
                                h_flex()
                                    .gap(px(6.))
                                    .items_center()
                                    .children(primary)
                                    .child(div().flex_1())
                                    .children(open)
                                    .children(remove),
                            )
                            .children(confirm),
                    )
                    .child(pane_section("Details").child(self.facts(row, palette)))
                    .children(history)
                    .scroll_area_y(&self.pane_scroll),
            )
            .into_any_element()
    }

    fn skeleton(&self, wide: bool) -> AnyElement {
        table()
            .child(self.head(wide))
            .child(
                table_body().child(v_flex().children((0..8usize).map(|index| {
                    h_flex()
                        .id(("publishing-skeleton", index))
                        .h(px(crate::ui::admin::ROW_HEIGHT))
                        .px(px(12.))
                        .gap(px(12.))
                        .items_center()
                        .child(skeleton_bar(28., 28.))
                        .child(skeleton_bar(200., 12.))
                }))),
            )
            .into_any_element()
    }

    fn body(&self, wide: bool, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let Some(model) = self.model.clone() else {
            return admins_only();
        };
        let load = model.read(cx).managed_load.clone();
        match load {
            Load::Loading => self.skeleton(wide),
            Load::Failed(message) => empty_state(
                IconName::TriangleAlert,
                "Could not load publishing details",
                Some(SharedString::from(message)),
                Some(
                    button("publishing-retry", "Try again")
                        .outline()
                        .sm()
                        .on_click(move |_, _, cx| {
                            model.update(cx, |model, cx| model.refresh_managed(cx));
                        })
                        .into_any_element(),
                ),
            )
            .into_any_element(),
            Load::Ready => {
                let rows = self.visible(cx);
                if rows.is_empty() {
                    return empty_state(
                        IconName::Library,
                        "Nothing here",
                        Some("No templates match this filter.".into()),
                        None,
                    )
                    .into_any_element();
                }
                let count = rows.len();
                let total = self.rows(cx).len();
                let items: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| self.row(index, row, index + 1 == count, wide, palette, cx))
                    .collect();
                let summary = if count == total {
                    format!(
                        "{total} {}",
                        if total == 1 { "template" } else { "templates" }
                    )
                } else {
                    format!("Showing {count} of {total} templates")
                };
                table()
                    .child(self.head(wide))
                    .child(
                        table_body().child(
                            v_flex()
                                .id("publishing-rows")
                                .size_full()
                                .children(items)
                                .scroll_area_y(&self.table_scroll),
                        ),
                    )
                    .child(table_footer().child(summary))
                    .into_any_element()
            }
        }
    }

    fn toolbar(&self, rows: &[PublishingRow], cx: &mut Context<Self>) -> AnyElement {
        let selected_filter = FILTERS
            .iter()
            .position(|(filter, _)| *filter == self.filter)
            .unwrap_or(0);
        let items = FILTERS
            .iter()
            .map(|(filter, title)| {
                let filter = *filter;
                let count = rows.iter().filter(|row| filter.admits(&row.state)).count();
                let handle = cx.entity().downgrade();
                FilterOption {
                    title,
                    count,
                    action: std::rc::Rc::new(move |_, cx| {
                        handle
                            .update(cx, |this, cx| this.set_filter(filter, cx))
                            .ok();
                    }),
                }
            })
            .collect();
        page_toolbar()
            .child(
                div()
                    .text_size(px(13.))
                    .text_color(Palette::get(cx).ink_muted)
                    .child("Templates on this computer and in the shared library"),
            )
            .child(div().flex_1())
            .child(filter_menu("publishing-filter", selected_filter, items))
            .into_any_element()
    }
}

impl Focusable for PublishingScreen {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for PublishingScreen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let root = v_flex()
            .id("publishing")
            .track_focus(&self.focus)
            .size_full()
            .bg(palette.surface);
        if self.model.is_none() || !is_admin(cx) {
            return root.child(admins_only()).into_any_element();
        }
        let rows = self.rows(cx);
        let side = self.chosen(cx).map(|row| self.details(&row, &palette, cx));
        let wide = side.is_none();
        let bulk = self
            .confirm
            .as_ref()
            .filter(|confirm| confirm.key == BULK)
            .map(|confirm| {
                let screen = cx.entity().downgrade();
                div()
                    .px(px(PAGE_GUTTER))
                    .pb(px(12.))
                    .child(confirm_card(confirm, move |_, cx| {
                        screen.update(cx, |this, cx| this.dismiss(cx)).ok();
                    }))
            });
        root.on_action(cx.listener(|this, _: &BlurFocus, _, cx| {
            if this.confirm.is_some() {
                this.dismiss(cx);
            } else if this.selected.is_some() {
                this.select(None, cx);
            } else {
                cx.propagate();
            }
        }))
        .child(
            h_flex()
                .flex_1()
                .min_h_0()
                .w_full()
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .child(self.toolbar(&rows, cx))
                        .children(bulk)
                        .child(
                            div()
                                .flex_1()
                                .min_h_0()
                                .px(px(PAGE_GUTTER))
                                .pb(px(PAGE_GUTTER))
                                .child(self.body(wide, &palette, cx)),
                        ),
                )
                .children(side),
        )
        .into_any_element()
    }
}

impl HeaderSource for PublishingScreen {
    fn header(&mut self, _: &mut Window, cx: &mut Context<Self>) -> HeaderContent {
        let content = HeaderContent::new("Publishing");
        let Some(model) = self.model.clone().filter(|_| is_admin(cx)) else {
            return content;
        };
        let model_ref = model.read(cx);
        let fetching = model_ref.is_fetching();
        let rows = model_ref.rows(cx);
        let pending: Vec<TemplateId> = rows
            .iter()
            .filter(|row| !model_ref.row_is_busy(row))
            .filter_map(publishable)
            .collect();
        let total = rows.len();
        let publish_label = if pending.is_empty() {
            "Publish all".to_owned()
        } else {
            format!("Publish all ({})", pending.len())
        };
        let can_publish_all = !pending.is_empty();
        content
            .meta(format!(
                "{total} {}",
                if total == 1 { "template" } else { "templates" }
            ))
            .action(
                icon_button("publishing-refresh", IconName::RefreshCw, "Refresh")
                    .disabled(fetching)
                    .on_click(cx.listener(|this, _, _, cx| this.refresh(cx))),
            )
            .action(
                primary_sm("publishing-all", publish_label)
                    .icon(IconName::CloudUpload)
                    .disabled(!can_publish_all)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.publish_all(pending.clone(), cx);
                    })),
            )
    }
}
