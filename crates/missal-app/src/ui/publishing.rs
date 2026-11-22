use gpui_kit::assets::IconName;
use gpui_kit::component::dialog::Dialog;
use gpui_kit::component::{WindowExt, h_flex, v_flex};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, IntoElement, ParentElement, Render, SharedString,
    Styled, Subscription, Window, div, px,
};
use missal_core::TemplateId;
use missal_library::TemplateVersion;
use missal_text::{LineHeight, label};

use crate::app::navigate;
use crate::library::{LibraryModel, PublishState, PublishingRow, VersionHistory};
use crate::state::{Load, Models, Route};
use crate::theme::{Palette, RADIUS};
use crate::ui::dialogs::{
    Choice, Tone, confirm_delete, dialog_footer, dialog_surface, dialog_title, open_choice,
    optical_top,
};
use crate::ui::library::day_month_year;
use crate::ui::{
    BadgeVariant, Intent, SegmentItem, badge, button, empty_state_body, ghost_sm, icon_button,
    isolate_urdu, primary_sm, section_label, segmented, sidebar_menu_button, skeleton_bar,
};

const WIDTH: f32 = 960.;
const MARGIN: f32 = 48.;
const LIST_WIDTH: f32 = 340.;
const BODY_HEIGHT: f32 = 500.;
const MIN_BODY_HEIGHT: f32 = 320.;
const DIALOG_CHROME: f32 = 260.;
const FACT_LABEL: f32 = 112.;
const ROW_HEIGHT: f32 = 66.;

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

#[derive(Clone, PartialEq, Eq)]
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

fn body_height(window: &Window) -> f32 {
    (f32::from(window.viewport_size().height) - DIALOG_CHROME).clamp(MIN_BODY_HEIGHT, BODY_HEIGHT)
}

pub fn open_publishing(window: &mut Window, cx: &mut App) {
    let Some(model) = LibraryModel::get(cx) else {
        return;
    };
    model.update(cx, |model, cx| model.refresh(cx));
    let panel = cx.new(|cx| PublishingPanel::new(model, cx));
    window.open_dialog(cx, move |dialog: Dialog, window, cx| {
        let palette = *Palette::get(cx);
        let room = f32::from(window.viewport_size().width) - MARGIN;
        dialog_surface(dialog, &palette, WIDTH.min(room))
            .margin_top(optical_top(window, body_height(window) + 180.))
            .child(panel.clone())
    });
}

struct PublishingPanel {
    model: Entity<LibraryModel>,
    filter: Filter,
    selected: Option<Selection>,
    _subscriptions: Vec<Subscription>,
}

impl PublishingPanel {
    fn new(model: Entity<LibraryModel>, cx: &mut Context<Self>) -> PublishingPanel {
        let templates = Models::get(cx).templates;
        let subscriptions = vec![
            cx.observe(&model, |this, _, cx| {
                this.ensure_history(cx);
                cx.notify();
            }),
            cx.observe(&templates, |_, _, cx| cx.notify()),
        ];
        model.update(cx, |model, cx| model.refresh_managed(cx));
        let mut panel = PublishingPanel {
            model,
            filter: Filter::All,
            selected: None,
            _subscriptions: subscriptions,
        };
        panel.ensure_history(cx);
        panel
    }

    fn visible(&self, cx: &App) -> Vec<PublishingRow> {
        self.model
            .read(cx)
            .rows(cx)
            .into_iter()
            .filter(|row| self.filter.admits(&row.state))
            .collect()
    }

    fn current(&self, rows: &[PublishingRow]) -> Option<usize> {
        self.selected
            .as_ref()
            .and_then(|selected| rows.iter().position(|row| selected.matches(row)))
            .or((!rows.is_empty()).then_some(0))
    }

    fn ensure_history(&mut self, cx: &mut Context<Self>) {
        let rows = self.visible(cx);
        let remote = self
            .current(&rows)
            .and_then(|index| rows[index].remote.as_ref())
            .map(|remote| remote.id.clone());
        if let Some(remote) = remote {
            self.model
                .update(cx, |model, cx| model.load_versions(&remote, cx));
        }
    }

    fn select(&mut self, selection: Option<Selection>, cx: &mut Context<Self>) {
        self.selected = selection;
        self.ensure_history(cx);
        cx.notify();
    }

    fn set_filter(&mut self, filter: Filter, cx: &mut Context<Self>) {
        self.filter = filter;
        self.ensure_history(cx);
        cx.notify();
    }

    fn publish_all(&self, templates: Vec<TemplateId>, window: &mut Window, cx: &mut App) {
        let handle = self.model.clone();
        let count = templates.len();
        let title = if count == 1 {
            "Publish 1 template?".to_owned()
        } else {
            format!("Publish {count} templates?")
        };
        open_choice(
            window,
            cx,
            title,
            "Everyone who uses Missal will see these versions. Only saved changes are published.",
            vec![
                Choice::new("publish-cancel", "Cancel", Tone::Neutral, |_, _| {}),
                Choice::new(
                    "publish-confirm",
                    "Publish",
                    Tone::Primary,
                    move |window, cx| {
                        let templates = templates.clone();
                        handle.update(cx, |model, cx| model.publish_many(templates, window, cx));
                    },
                ),
            ],
        );
    }

    fn primary_action(&self, row: &PublishingRow, busy: bool) -> Option<AnyElement> {
        let handle = self.model.clone();
        let local = row.local.as_ref().map(|local| local.id);
        let remote = row.remote.as_ref().map(|remote| remote.id.clone());
        let name = row.name.clone();
        let element = match (&row.state, local, remote) {
            (PublishState::NotPublished, Some(template), _) => {
                primary_sm("publishing-publish", "Publish")
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |model, cx| model.publish(template, window, cx));
                    })
                    .into_any_element()
            }
            (PublishState::Changed, Some(template), _) => {
                primary_sm("publishing-changes", "Publish changes")
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        handle.update(cx, |model, cx| model.publish(template, window, cx));
                    })
                    .into_any_element()
            }
            (PublishState::NameInLibrary, Some(template), Some(remote)) => {
                primary_sm("publishing-replace", "Publish as new version")
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        open_choice(
                            window,
                            cx,
                            format!("Publish over «{}»?", isolate_urdu(&name)),
                            "Your copy becomes the next version of the library template with this name. The current version stays in its history.",
                            vec![
                                Choice::new("replace-cancel", "Cancel", Tone::Neutral, |_, _| {}),
                                Choice::new("replace-confirm", "Publish", Tone::Primary, move |window, cx| {
                                    let remote = remote.clone();
                                    handle.update(cx, |model, cx| {
                                        model.publish_to(template, Some(remote), window, cx);
                                    });
                                }),
                            ],
                        );
                    })
                    .into_any_element()
            }
            (PublishState::Behind { edited }, _, Some(remote)) => {
                let edited = *edited;
                primary_sm("publishing-update", "Update this computer")
                    .disabled(busy)
                    .on_click(move |_, window, cx| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        if !edited {
                            handle.update(cx, |model, cx| model.install_entry(&remote, window, cx));
                            return;
                        }
                        open_choice(
                            window,
                            cx,
                            format!("Replace your changes to «{}»?", isolate_urdu(&name)),
                            "Your edits to this template will be replaced by the library version. Documents already added to FIRs are not changed.",
                            vec![
                                Choice::new("keep", "Keep my version", Tone::Neutral, |_, _| {}),
                                Choice::new("replace", "Replace", Tone::Primary, move |window, cx| {
                                    handle.update(cx, |model, cx| {
                                        model.install_entry(&remote, window, cx);
                                    });
                                }),
                            ],
                        );
                    })
                    .into_any_element()
            }
            (PublishState::NotInstalled, _, Some(remote)) => {
                primary_sm("publishing-add", "Add to this computer")
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

    fn secondary_actions(&self, row: &PublishingRow, busy: bool) -> Vec<AnyElement> {
        let mut actions = Vec::new();
        if let Some(template) = row.local.as_ref().map(|local| local.id) {
            actions.push(
                button("publishing-open", "Open template")
                    .outline()
                    .sm()
                    .icon(IconName::ExternalLink)
                    .on_click(move |_, window, cx| {
                        window.close_dialog(cx);
                        navigate(Route::Template(template), window, cx);
                    })
                    .into_any_element(),
            );
        }
        if let Some(remote) = row.remote.as_ref().map(|remote| remote.id.clone()) {
            let handle = self.model.clone();
            let name = row.name.clone();
            actions.push(
                crate::ui::popup_menu::menu("publishing-more")
                    .trigger(
                        icon_button("publishing-more", IconName::Ellipsis, "More actions")
                            .disabled(busy),
                    )
                    .width(220.)
                    .content(move |content, _, _| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        let name = name.clone();
                        content.item(
                            crate::ui::popup_menu::MenuEntry::new(
                                "publishing-remove",
                                "Remove from library",
                                move |window, cx| {
                                    let handle = handle.clone();
                                    let remote = remote.clone();
                                    confirm_delete(
                                        window,
                                        cx,
                                        format!("Remove «{}» from the library?", isolate_urdu(&name)),
                                        "Its whole version history is deleted and cannot be restored. Copies people already added stay on their computers.",
                                        "Remove",
                                        move |window, cx| {
                                            handle.update(cx, |model, cx| {
                                                model.unpublish(&remote, window, cx)
                                            });
                                        },
                                    );
                                },
                            )
                            .icon(IconName::Trash)
                            .danger(),
                        )
                    })
                    .into_any_element(),
            );
        }
        actions
    }

    fn list_row(
        &self,
        index: usize,
        row: &PublishingRow,
        active: bool,
        palette: &Palette,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selection = Selection::of(row);
        let meta = match &row.remote {
            Some(remote) => format!("v{} · {}", remote.version, fields_text(remote.field_count)),
            None => row
                .local
                .as_ref()
                .map(|local| fields_text(local.field_count))
                .unwrap_or_default(),
        };
        sidebar_menu_button(("publishing-row", index), active)
            .h(px(ROW_HEIGHT))
            .flex_none()
            .aria_label(row.name.clone())
            .on_click(cx.listener(move |this, _, _, cx| this.select(selection.clone(), cx)))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .gap(px(2.))
                    .child(
                        div()
                            .w_full()
                            .text_size(px(15.))
                            .font_weight(gpui_kit::FontWeight::NORMAL)
                            .text_color(palette.ink)
                            .child(
                                label(row.name.clone())
                                    .rtl()
                                    .leading(LineHeight::Px(26.))
                                    .truncate(),
                            ),
                    )
                    .child(
                        h_flex()
                            .gap(px(8.))
                            .items_center()
                            .child(state_badge(&row.state))
                            .child(div().flex_1())
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .line_height(px(16.))
                                    .font_weight(gpui_kit::FontWeight::NORMAL)
                                    .text_color(palette.ink_faint)
                                    .font_features(crate::ui::tabular())
                                    .child(meta),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn fact(name: &'static str, value: String, palette: &Palette) -> AnyElement {
        h_flex()
            .min_h(px(24.))
            .gap(px(8.))
            .items_center()
            .text_size(px(13.))
            .child(
                div()
                    .w(px(FACT_LABEL))
                    .flex_none()
                    .text_color(palette.ink_muted)
                    .child(name),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(palette.ink)
                    .font_features(crate::ui::tabular())
                    .child(label(value).ltr().truncate()),
            )
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
            .gap(px(2.))
            .children(
                facts
                    .into_iter()
                    .map(|(name, value)| Self::fact(name, value, palette)),
            )
            .into_any_element()
    }

    fn version_row(
        &self,
        remote: &str,
        name: &str,
        version: &TemplateVersion,
        current: i64,
        busy: bool,
        palette: &Palette,
    ) -> AnyElement {
        let mut details = vec![
            date_text(version.published_at),
            short_person(
                version.publisher_name.as_deref(),
                version.publisher_email.as_deref(),
            ),
            fields_text(version.field_count),
            size_text(version.size),
        ];
        if let Some(source) = version.restored_from {
            details.push(format!("restored from v{source}"));
        }
        let number = version.version;
        let handle = self.model.clone();
        let remote = remote.to_owned();
        let name = name.to_owned();
        h_flex()
            .id(("publishing-version", number as usize))
            .min_h(px(40.))
            .px(px(8.))
            .py(px(4.))
            .gap(px(10.))
            .items_center()
            .rounded(px(RADIUS))
            .hover(|style| style.bg(palette.hover))
            .child(
                div()
                    .w(px(36.))
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
                    .on_click(move |_, window, cx| {
                        let handle = handle.clone();
                        let remote = remote.clone();
                        open_choice(
                            window,
                            cx,
                            format!("Restore version {number} of «{}»?", isolate_urdu(&name)),
                            format!(
                                "It is published again as version {}. People with this template are offered the update, and no version is deleted.",
                                current + 1
                            ),
                            vec![
                                Choice::new("restore-cancel", "Cancel", Tone::Neutral, |_, _| {}),
                                Choice::new("restore-confirm", "Restore", Tone::Primary, move |window, cx| {
                                    handle.update(cx, |model, cx| {
                                        model.restore_version(&remote, number, window, cx);
                                    });
                                }),
                            ],
                        );
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
        let id = remote.id.clone();
        let body = match self.model.read(cx).history(&remote.id) {
            None | Some(VersionHistory::Loading) => v_flex()
                .gap(px(6.))
                .children((0..3).map(|_| skeleton_bar(WIDTH - LIST_WIDTH - 100., 32.)))
                .into_any_element(),
            Some(VersionHistory::Failed(message)) => {
                let handle = self.model.clone();
                h_flex()
                    .gap(px(8.))
                    .items_center()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.))
                            .text_color(palette.ink_muted)
                            .child(label(message.clone()).ltr()),
                    )
                    .child(ghost_sm("publishing-history-retry", "Try again").on_click(
                        move |_, _, cx| {
                            handle.update(cx, |model, cx| model.retry_versions(&id, cx));
                        },
                    ))
                    .into_any_element()
            }
            Some(VersionHistory::Ready(versions)) => v_flex()
                .gap(px(2.))
                .children(versions.iter().map(|version| {
                    self.version_row(
                        &remote.id,
                        &row.name,
                        version,
                        remote.version,
                        busy,
                        palette,
                    )
                }))
                .into_any_element(),
        };
        Some(
            v_flex()
                .gap(px(4.))
                .child(section_label("Version history"))
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
        let busy = self.model.read(cx).row_is_busy(row);
        let primary = self.primary_action(row, busy);
        let secondary = self.secondary_actions(row, busy);
        let history = self.history(row, busy, palette, cx);
        v_flex()
            .id("publishing-details")
            .flex_1()
            .min_w_0()
            .h_full()
            .overflow_y_scroll()
            .pl(px(20.))
            .gap(px(14.))
            .child(
                v_flex()
                    .gap(px(6.))
                    .child(
                        div()
                            .w_full()
                            .text_size(px(20.))
                            .text_color(palette.ink)
                            .child(
                                label(row.name.clone())
                                    .rtl()
                                    .leading(LineHeight::Px(34.))
                                    .truncate(),
                            ),
                    )
                    .child(h_flex().justify_end().child(state_badge(&row.state))),
            )
            .child(
                h_flex()
                    .gap(px(6.))
                    .items_center()
                    .children(primary)
                    .children(secondary),
            )
            .child(self.facts(row, palette))
            .children(history)
            .into_any_element()
    }

    fn body(&self, palette: &Palette, cx: &mut Context<Self>) -> AnyElement {
        let model = self.model.read(cx);
        match &model.managed_load {
            Load::Loading => h_flex()
                .gap(px(20.))
                .size_full()
                .items_start()
                .child(
                    v_flex()
                        .w(px(LIST_WIDTH))
                        .gap(px(8.))
                        .children((0..7).map(|_| skeleton_bar(LIST_WIDTH, 52.))),
                )
                .child(
                    v_flex()
                        .flex_1()
                        .gap(px(10.))
                        .children((0..5).map(|_| skeleton_bar(WIDTH - LIST_WIDTH - 100., 28.))),
                )
                .into_any_element(),
            Load::Failed(message) => {
                let handle = self.model.clone();
                div()
                    .pt(px(80.))
                    .child(empty_state_body(
                        IconName::TriangleAlert,
                        "Could not load publishing details",
                        Some(SharedString::from(message.clone())),
                        Some(
                            button("publishing-retry", "Try again")
                                .outline()
                                .sm()
                                .on_click(move |_, _, cx| {
                                    handle.update(cx, |model, cx| model.refresh_managed(cx));
                                })
                                .into_any_element(),
                        ),
                    ))
                    .into_any_element()
            }
            Load::Ready => {
                let rows = self.visible(cx);
                if rows.is_empty() {
                    return div()
                        .pt(px(100.))
                        .child(empty_state_body(
                            IconName::Library,
                            "Nothing here",
                            Some("No templates match this filter.".into()),
                            None,
                        ))
                        .into_any_element();
                }
                let chosen = self.current(&rows).unwrap_or(0);
                let items: Vec<AnyElement> = rows
                    .iter()
                    .enumerate()
                    .map(|(index, row)| self.list_row(index, row, index == chosen, palette, cx))
                    .collect();
                let details = self.details(&rows[chosen], palette, cx);
                h_flex()
                    .size_full()
                    .items_start()
                    .child(
                        v_flex()
                            .id("publishing-list")
                            .w(px(LIST_WIDTH))
                            .flex_none()
                            .h_full()
                            .overflow_y_scroll()
                            .pr(px(10.))
                            .gap(px(2.))
                            .border_r_1()
                            .border_color(palette.line)
                            .children(items),
                    )
                    .child(details)
                    .into_any_element()
            }
        }
    }
}

impl Render for PublishingPanel {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = *Palette::get(cx);
        let height = body_height(window);
        let model = self.model.read(cx);
        let fetching = model.is_fetching();
        let rows = model.rows(cx);
        let pending: Vec<TemplateId> = rows
            .iter()
            .filter(|row| !model.row_is_busy(row))
            .filter_map(publishable)
            .collect();
        let selected_filter = FILTERS
            .iter()
            .position(|(filter, _)| *filter == self.filter)
            .unwrap_or(0);
        let items = FILTERS
            .iter()
            .enumerate()
            .map(|(index, (filter, title))| {
                let filter = *filter;
                let count = rows.iter().filter(|row| filter.admits(&row.state)).count();
                SegmentItem::new(
                    ("publishing-filter", index),
                    format!("{title} {count}"),
                    cx.listener(move |this, _, _, cx| this.set_filter(filter, cx)),
                )
            })
            .collect();
        let handle = self.model.clone();
        let publish_label = if pending.is_empty() {
            "Publish all".to_owned()
        } else {
            format!("Publish all ({})", pending.len())
        };
        let can_publish_all = !pending.is_empty();
        v_flex()
            .gap(px(12.))
            .child(
                h_flex()
                    .items_center()
                    .gap(px(8.))
                    .child(dialog_title("Publishing"))
                    .child(div().flex_1())
                    .child(
                        icon_button("publishing-refresh", IconName::RefreshCw, "Refresh")
                            .disabled(fetching)
                            .on_click(move |_, _, cx| {
                                handle.update(cx, |model, cx| model.refresh(cx));
                            }),
                    )
                    .child(
                        primary_sm("publishing-all", publish_label)
                            .icon(IconName::CloudUpload)
                            .disabled(!can_publish_all)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.publish_all(pending.clone(), window, cx);
                            })),
                    ),
            )
            .child(h_flex().child(segmented(items, selected_filter)))
            .child(div().h(px(height)).child(self.body(&palette, cx)))
            .child(
                dialog_footer().mt(px(12.)).child(div().flex_1()).child(
                    button("publishing-close", "Close")
                        .primary()
                        .on_click(|_, window, cx| window.close_dialog(cx)),
                ),
            )
    }
}
