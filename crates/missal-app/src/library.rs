use std::collections::HashSet;
use std::sync::Arc;

use gpui_kit::{App, AppContext, Context, Entity, Global, SharedString, Window};
use missal_auth::{AuthError, SessionKeeper};
use missal_core::TemplateId;
use missal_library::{Library, LibraryConfig, LibraryEntry, LibraryError, Published, Upload};
use missal_store::{LibraryInstall, LibraryLink};

use crate::auth::AuthModel;
use crate::state::templates::TemplatesEvent;
use crate::state::{Load, Models, db, toast_error, toast_success};

const PREVIEW_LIMIT: usize = 400;

enum Failure {
    Auth(AuthError),
    Library(LibraryError),
    Store(String),
}

impl Failure {
    fn message(&self) -> String {
        match self {
            Failure::Auth(AuthError::Rejected(_)) | Failure::Library(LibraryError::SignedOut) => {
                "Your session has ended. Sign in again.".to_owned()
            }
            Failure::Auth(AuthError::Offline(_)) => {
                "Could not reach the sign-in service. Check the internet connection and try again."
                    .to_owned()
            }
            Failure::Auth(other) => format!("Could not confirm your session: {other}."),
            Failure::Library(error) => error.to_string(),
            Failure::Store(message) => message.clone(),
        }
    }

    fn ends_session(&self) -> bool {
        matches!(
            self,
            Failure::Auth(AuthError::Rejected(_)) | Failure::Library(LibraryError::SignedOut)
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryStatus {
    Available,
    Installed,
    UpdateAvailable { edited: bool },
    Busy,
}

#[derive(Clone)]
struct Access {
    client: Arc<Library>,
    keeper: Arc<SessionKeeper>,
}

impl Access {
    fn token(&self) -> Result<String, Failure> {
        self.keeper.access_token().map_err(Failure::Auth)
    }
}

pub struct LibraryModel {
    client: Option<Arc<Library>>,
    pub entries: Vec<LibraryEntry>,
    pub publisher: bool,
    pub links: Vec<LibraryLink>,
    pub load: Load,
    loaded: bool,
    fetching: bool,
    busy: HashSet<String>,
}

struct LibraryHandle(Entity<LibraryModel>);

impl Global for LibraryHandle {}

fn template_key(id: TemplateId) -> String {
    format!("template:{id}")
}

impl LibraryModel {
    pub fn install(models: &Models, cx: &mut App) -> Entity<LibraryModel> {
        let client = LibraryConfig::resolve().map(|config| Arc::new(Library::new(&config)));
        let entity = cx.new(|cx| {
            cx.subscribe(
                &models.templates,
                |this: &mut LibraryModel, _, _: &TemplatesEvent, cx| this.reload_links(cx),
            )
            .detach();
            if let Some(auth) = AuthModel::get(cx) {
                cx.observe(&auth, |this: &mut LibraryModel, auth, cx| {
                    let signed_in = auth.read(cx).account().is_some();
                    if signed_in && !this.loaded && !this.fetching {
                        this.refresh(cx);
                    }
                    if !signed_in && this.loaded {
                        this.forget(cx);
                    }
                })
                .detach();
            }
            LibraryModel {
                client,
                entries: Vec::new(),
                publisher: false,
                links: Vec::new(),
                load: Load::Loading,
                loaded: false,
                fetching: false,
                busy: HashSet::new(),
            }
        });
        cx.set_global(LibraryHandle(entity.clone()));
        let signed_in = AuthModel::get(cx).is_some_and(|auth| auth.read(cx).account().is_some());
        entity.update(cx, |model, cx| {
            model.reload_links(cx);
            if signed_in {
                model.refresh(cx);
            }
        });
        entity
    }

    pub fn get(cx: &App) -> Option<Entity<LibraryModel>> {
        cx.try_global::<LibraryHandle>()
            .map(|handle| handle.0.clone())
    }

    fn access(&self, cx: &App) -> Option<Access> {
        let keeper = AuthModel::get(cx)?.read(cx).keeper()?;
        Some(Access {
            client: self.client.clone()?,
            keeper,
        })
    }

    pub fn available(&self, cx: &App) -> bool {
        self.access(cx).is_some()
    }

    pub fn can_publish(&self, cx: &App) -> bool {
        self.publisher && self.available(cx)
    }

    pub fn is_fetching(&self) -> bool {
        self.fetching
    }

    pub fn link_for_template(&self, id: TemplateId) -> Option<&LibraryLink> {
        self.links.iter().find(|link| link.template_id == id)
    }

    pub fn link_for_entry(&self, entry: &str) -> Option<&LibraryLink> {
        self.links.iter().find(|link| link.remote_id == entry)
    }

    pub fn is_publishing(&self, id: TemplateId) -> bool {
        self.busy.contains(&template_key(id))
    }

    pub fn status(&self, entry: &LibraryEntry, cx: &App) -> EntryStatus {
        if self.busy.contains(&entry.id) {
            return EntryStatus::Busy;
        }
        let Some(link) = self.link_for_entry(&entry.id) else {
            return EntryStatus::Available;
        };
        let edited = Models::get(cx)
            .templates
            .read(cx)
            .get(link.template_id)
            .is_some_and(|summary| summary.revision != link.revision);
        if link.version < entry.version {
            EntryStatus::UpdateAvailable { edited }
        } else {
            EntryStatus::Installed
        }
    }

    pub fn has_unpublished_changes(&self, id: TemplateId, cx: &App) -> bool {
        self.link_for_template(id).is_some_and(|link| {
            Models::get(cx)
                .templates
                .read(cx)
                .get(id)
                .is_some_and(|summary| summary.revision != link.revision)
        })
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        self.publisher = false;
        self.loaded = false;
        self.load = Load::Loading;
        cx.notify();
    }

    fn reload_links(&mut self, cx: &mut Context<Self>) {
        let reply = db(cx).run(|store| store.library_links());
        cx.spawn(async move |this, cx| {
            if let Ok(links) = reply.await {
                this.update(cx, |this, cx| {
                    this.links = links;
                    cx.notify();
                })
                .ok();
            }
        })
        .detach();
    }

    fn failed(&mut self, failure: &Failure, cx: &mut Context<Self>) -> SharedString {
        if failure.ends_session()
            && let Some(auth) = AuthModel::get(cx)
        {
            auth.update(cx, |auth, cx| auth.end_session(cx));
        }
        failure.message().into()
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(access) = self.access(cx) else {
            return;
        };
        if self.fetching {
            return;
        }
        self.fetching = true;
        if !self.loaded {
            self.load = Load::Loading;
        }
        cx.notify();
        let catalog = cx.background_spawn(async move {
            let token = access.token()?;
            access.client.catalog(&token).map_err(Failure::Library)
        });
        cx.spawn(async move |this, cx| {
            let catalog = catalog.await;
            this.update(cx, |this, cx| {
                this.fetching = false;
                match catalog {
                    Ok(catalog) => {
                        this.entries = catalog.templates;
                        this.publisher = catalog.publisher;
                        this.loaded = true;
                        this.load = Load::Ready;
                    }
                    Err(failure) => {
                        let message = this.failed(&failure, cx);
                        if !this.loaded {
                            this.load = Load::Failed(message.to_string());
                        }
                    }
                }
                this.reload_links(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn installed(&mut self, install: &LibraryInstall, cx: &mut Context<Self>) {
        self.links
            .retain(|link| link.template_id != install.link.template_id);
        self.links.push(install.link.clone());
        let models = Models::get(cx);
        let summary = install.summary.clone();
        models
            .templates
            .update(cx, |templates, cx| templates.upsert(summary, cx));
        models.catalog.update(cx, |catalog, cx| catalog.reload(cx));
    }

    pub fn install_entry(&mut self, entry: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(access) = self.access(cx) else {
            return;
        };
        if !self.busy.insert(entry.to_owned()) {
            return;
        }
        cx.notify();
        let id = entry.to_owned();
        let download = cx.background_spawn({
            let id = id.clone();
            async move {
                let token = access.token()?;
                access
                    .client
                    .download(&token, &id)
                    .map_err(Failure::Library)
            }
        });
        let db = db(cx);
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match download.await {
                Ok(download) => {
                    let remote = id.clone();
                    db.run(move |store| {
                        store.install_library_template(&remote, download.version, &download.content)
                    })
                    .await
                    .map_err(|error| Failure::Store(error.message()))
                }
                Err(failure) => Err(failure),
            };
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(install) => {
                        this.installed(&install, cx);
                        let message = if install.replaced {
                            "Template updated from the library"
                        } else {
                            "Template added from the library"
                        };
                        toast_success(window, cx, message);
                    }
                    Err(failure) => {
                        let message = this.failed(&failure, cx);
                        toast_error(window, cx, message);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn publish(&mut self, template: TemplateId, window: &mut Window, cx: &mut Context<Self>) {
        let Some(access) = self.access(cx) else {
            return;
        };
        let key = template_key(template);
        if !self.busy.insert(key.clone()) {
            return;
        }
        cx.notify();
        let remote = self
            .link_for_template(template)
            .map(|link| link.remote_id.clone());
        let first = remote.is_none();
        let db = db(cx);
        let export = db.run(move |store| store.export_template(template));
        cx.spawn_in(window, async move |this, cx| {
            let outcome: Result<LibraryLink, Failure> = async {
                let export = export
                    .await
                    .map_err(|error| Failure::Store(error.message()))?;
                let upload = Upload {
                    name: export.name.clone(),
                    field_count: export.field_count,
                    preview: export.preview_text.chars().take(PREVIEW_LIMIT).collect(),
                    content: export.content,
                };
                let published: Published = cx
                    .background_executor()
                    .spawn(async move {
                        let token = access.token()?;
                        match access.client.publish(&token, remote.as_deref(), &upload) {
                            Err(LibraryError::NotFound) if remote.is_some() => {
                                access.client.publish(&token, None, &upload)
                            }
                            other => other,
                        }
                        .map_err(Failure::Library)
                    })
                    .await?;
                let revision = export.revision;
                db.run(move |store| {
                    store.link_template(template, &published.id, published.version, revision)
                })
                .await
                .map_err(|error| Failure::Store(error.message()))
            }
            .await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&key);
                match outcome {
                    Ok(link) => {
                        this.links.retain(|item| {
                            item.template_id != link.template_id && item.remote_id != link.remote_id
                        });
                        this.links.push(link);
                        let message = if first {
                            "Published to the library"
                        } else {
                            "Library copy updated"
                        };
                        toast_success(window, cx, message);
                        this.refresh(cx);
                    }
                    Err(failure) => {
                        let message = this.failed(&failure, cx);
                        toast_error(window, cx, message);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn unpublish(&mut self, entry: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(access) = self.access(cx) else {
            return;
        };
        if !self.busy.insert(entry.to_owned()) {
            return;
        }
        cx.notify();
        let id = entry.to_owned();
        let removal = cx.background_spawn({
            let id = id.clone();
            async move {
                let token = access.token()?;
                match access.client.unpublish(&token, &id) {
                    Ok(()) | Err(LibraryError::NotFound) => Ok(()),
                    Err(error) => Err(Failure::Library(error)),
                }
            }
        });
        let db = db(cx);
        cx.spawn_in(window, async move |this, cx| {
            let outcome = match removal.await {
                Ok(()) => {
                    let remote = id.clone();
                    db.run(move |store| store.unlink_library(&remote))
                        .await
                        .map_err(|error| Failure::Store(error.message()))
                }
                Err(failure) => Err(failure),
            };
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(()) => {
                        this.entries.retain(|entry| entry.id != id);
                        this.links.retain(|link| link.remote_id != id);
                        toast_success(window, cx, "Removed from the library");
                    }
                    Err(failure) => {
                        let message = this.failed(&failure, cx);
                        toast_error(window, cx, message);
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}
