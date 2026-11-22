use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use gpui_kit::{App, AppContext, Context, Entity, Global, SharedString, Window};
use missal_auth::{AuthError, SessionKeeper};
use missal_core::{Revision, TemplateId};
use missal_library::{
    Library, LibraryEntry, LibraryError, ManagedTemplate, Published, TemplateVersion, Upload,
};
use missal_store::{LibraryInstall, LibraryLink, TemplateSummary};

use crate::auth::AuthModel;
use crate::db::Db;
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
            Failure::Library(LibraryError::SignedOut) => {
                "Your session has ended. Sign in again.".to_owned()
            }
            Failure::Auth(error) => error.to_string(),
            Failure::Library(error) => error.to_string(),
            Failure::Store(message) => message.clone(),
        }
    }

    fn ends_session(&self) -> bool {
        match self {
            Failure::Auth(error) => error.ends_session(),
            Failure::Library(error) => *error == LibraryError::SignedOut,
            Failure::Store(_) => false,
        }
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PublishState {
    NotPublished,
    NameInLibrary,
    Current,
    Changed,
    Behind { edited: bool },
    NotInstalled,
}

#[derive(Debug, Clone)]
pub struct PublishingRow {
    pub name: String,
    pub local: Option<TemplateSummary>,
    pub link: Option<LibraryLink>,
    pub remote: Option<ManagedTemplate>,
    pub state: PublishState,
}

#[derive(Debug, Clone)]
pub enum VersionHistory {
    Loading,
    Ready(Vec<TemplateVersion>),
    Failed(String),
}

fn publishing_rows(
    templates: &[TemplateSummary],
    links: &[LibraryLink],
    managed: &[ManagedTemplate],
) -> Vec<PublishingRow> {
    let linked: HashMap<&str, (&LibraryLink, &TemplateSummary)> = links
        .iter()
        .filter_map(|link| {
            let local = templates.iter().find(|item| item.id == link.template_id)?;
            Some((link.remote_id.as_str(), (link, local)))
        })
        .collect();
    let mut claimed: HashSet<TemplateId> = linked.values().map(|(_, local)| local.id).collect();
    let mut rows = Vec::with_capacity(templates.len() + managed.len());
    for remote in managed {
        let (link, local, state) = match linked.get(remote.id.as_str()) {
            Some((link, local)) => {
                let edited = local.revision != link.revision;
                let state = if link.version < remote.version {
                    PublishState::Behind { edited }
                } else if edited {
                    PublishState::Changed
                } else {
                    PublishState::Current
                };
                (Some((*link).clone()), Some((*local).clone()), state)
            }
            None => {
                let twin = templates.iter().find(|item| {
                    !claimed.contains(&item.id) && item.name.trim() == remote.name.trim()
                });
                match twin {
                    Some(item) => {
                        claimed.insert(item.id);
                        (None, Some(item.clone()), PublishState::NameInLibrary)
                    }
                    None => (None, None, PublishState::NotInstalled),
                }
            }
        };
        rows.push(PublishingRow {
            name: local
                .as_ref()
                .map_or_else(|| remote.name.clone(), |item| item.name.clone()),
            local,
            link,
            remote: Some(remote.clone()),
            state,
        });
    }
    rows.extend(
        templates
            .iter()
            .filter(|item| !claimed.contains(&item.id))
            .map(|item| PublishingRow {
                name: item.name.clone(),
                local: Some(item.clone()),
                link: None,
                remote: None,
                state: PublishState::NotPublished,
            }),
    );
    rows.sort_by(|left, right| left.name.cmp(&right.name));
    rows
}

async fn prepare_upload(db: &Db, template: TemplateId) -> Result<(Upload, Revision), Failure> {
    let export = db
        .run(move |store| store.export_template(template))
        .await
        .map_err(|error| Failure::Store(error.message()))?;
    let upload = Upload {
        name: export.name.clone(),
        field_count: export.field_count,
        preview: export.preview_text.chars().take(PREVIEW_LIMIT).collect(),
        content: export.content,
    };
    Ok((upload, export.revision))
}

fn send_upload(
    access: &Access,
    remote: Option<&str>,
    upload: &Upload,
) -> Result<Published, Failure> {
    let token = access.token()?;
    match access.client.publish(&token, remote, upload) {
        Err(LibraryError::NotFound) if remote.is_some() => {
            access.client.publish(&token, None, upload)
        }
        other => other,
    }
    .map_err(Failure::Library)
}

async fn save_link(
    db: &Db,
    template: TemplateId,
    published: Published,
    revision: Revision,
) -> Result<LibraryLink, Failure> {
    db.run(move |store| store.link_template(template, &published.id, published.version, revision))
        .await
        .map_err(|error| Failure::Store(error.message()))
}

pub struct LibraryModel {
    client: Option<Arc<Library>>,
    pub entries: Vec<LibraryEntry>,
    pub publisher: bool,
    pub links: Vec<LibraryLink>,
    pub load: Load,
    pub managed: Vec<ManagedTemplate>,
    pub managed_load: Load,
    versions: HashMap<String, VersionHistory>,
    managed_fetching: bool,
    managed_stale: bool,
    loaded: bool,
    fetching: bool,
    seeding: bool,
    busy: HashSet<String>,
}

struct LibraryHandle(Entity<LibraryModel>);

impl Global for LibraryHandle {}

fn template_key(id: TemplateId) -> String {
    format!("template:{id}")
}

impl LibraryModel {
    pub fn install(models: &Models, cx: &mut App) -> Entity<LibraryModel> {
        let client = AuthModel::get(cx).and_then(|auth| {
            auth.read(cx)
                .api_url()
                .map(|url| Arc::new(Library::new(url)))
        });
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
                managed: Vec::new(),
                managed_load: Load::Loading,
                versions: HashMap::new(),
                managed_fetching: false,
                managed_stale: false,
                loaded: false,
                fetching: false,
                seeding: false,
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
        self.fetching || self.seeding
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

    fn is_admin(cx: &App) -> bool {
        AuthModel::get(cx)
            .is_some_and(|auth| auth.read(cx).account().is_some_and(|a| a.role.is_admin()))
    }

    pub fn rows(&self, cx: &App) -> Vec<PublishingRow> {
        if self.managed_load != Load::Ready {
            return Vec::new();
        }
        let templates = Models::get(cx).templates;
        publishing_rows(&templates.read(cx).items, &self.links, &self.managed)
    }

    pub fn row_is_busy(&self, row: &PublishingRow) -> bool {
        row.local
            .as_ref()
            .is_some_and(|local| self.is_publishing(local.id))
            || row
                .remote
                .as_ref()
                .is_some_and(|remote| self.busy.contains(&remote.id))
    }

    pub fn history(&self, remote: &str) -> Option<&VersionHistory> {
        self.versions.get(remote)
    }

    pub fn refresh_managed(&mut self, cx: &mut Context<Self>) {
        if !Self::is_admin(cx) {
            return;
        }
        let Some(access) = self.access(cx) else {
            return;
        };
        if self.managed_fetching {
            self.managed_stale = true;
            return;
        }
        self.managed_fetching = true;
        cx.notify();
        let task = cx.background_spawn(async move {
            let token = access.token()?;
            access
                .client
                .managed_templates(&token)
                .map_err(Failure::Library)
        });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                this.managed_fetching = false;
                match outcome {
                    Ok(managed) => {
                        this.managed = managed;
                        this.managed_load = Load::Ready;
                        this.versions.clear();
                    }
                    Err(failure) => {
                        let message = this.failed(&failure, cx);
                        if this.managed_load != Load::Ready {
                            this.managed_load = Load::Failed(message.to_string());
                        }
                    }
                }
                if std::mem::take(&mut this.managed_stale) {
                    this.refresh_managed(cx);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn load_versions(&mut self, remote: &str, cx: &mut Context<Self>) {
        if self.versions.contains_key(remote) {
            return;
        }
        let Some(access) = self.access(cx) else {
            return;
        };
        self.versions
            .insert(remote.to_owned(), VersionHistory::Loading);
        cx.notify();
        let id = remote.to_owned();
        let task = cx.background_spawn({
            let id = id.clone();
            async move {
                let token = access.token()?;
                access
                    .client
                    .template_versions(&token, &id)
                    .map_err(Failure::Library)
            }
        });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                let history = match outcome {
                    Ok(versions) => VersionHistory::Ready(versions),
                    Err(failure) => VersionHistory::Failed(this.failed(&failure, cx).to_string()),
                };
                this.versions.insert(id, history);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub fn retry_versions(&mut self, remote: &str, cx: &mut Context<Self>) {
        self.versions.remove(remote);
        self.load_versions(remote, cx);
    }

    pub fn restore_version(
        &mut self,
        remote: &str,
        version: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(access) = self.access(cx) else {
            return;
        };
        if !self.busy.insert(remote.to_owned()) {
            return;
        }
        cx.notify();
        let id = remote.to_owned();
        let task = cx.background_spawn({
            let id = id.clone();
            async move {
                let token = access.token()?;
                access
                    .client
                    .restore_version(&token, &id, version)
                    .map_err(Failure::Library)
            }
        });
        cx.spawn_in(window, async move |this, cx| {
            let outcome = task.await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&id);
                match outcome {
                    Ok(published) => {
                        toast_success(
                            window,
                            cx,
                            format!(
                                "Version {version} is live again as version {}",
                                published.version
                            ),
                        );
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

    fn published(&mut self, link: LibraryLink) {
        self.links.retain(|item| {
            item.template_id != link.template_id && item.remote_id != link.remote_id
        });
        self.links.push(link);
    }

    pub fn publish_many(
        &mut self,
        templates: Vec<TemplateId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(access) = self.access(cx) else {
            return;
        };
        let mut jobs: Vec<(TemplateId, Option<String>)> = Vec::with_capacity(templates.len());
        for template in templates {
            if self.busy.insert(template_key(template)) {
                let remote = self
                    .link_for_template(template)
                    .map(|link| link.remote_id.clone());
                jobs.push((template, remote));
            }
        }
        if jobs.is_empty() {
            return;
        }
        cx.notify();
        let db = db(cx);
        cx.spawn_in(window, async move |this, cx| {
            let keys: Vec<String> = jobs.iter().map(|(id, _)| template_key(*id)).collect();
            let (mut done, mut failed) = (0usize, 0usize);
            let mut problem = None;
            for (template, remote) in jobs {
                let outcome: Result<LibraryLink, Failure> = async {
                    let (upload, revision) = prepare_upload(&db, template).await?;
                    let access = access.clone();
                    let published = cx
                        .background_executor()
                        .spawn(async move { send_upload(&access, remote.as_deref(), &upload) })
                        .await?;
                    save_link(&db, template, published, revision).await
                }
                .await;
                let stop = this
                    .update(cx, |this, cx| {
                        this.busy.remove(&template_key(template));
                        let stop = match outcome {
                            Ok(link) => {
                                this.published(link);
                                done += 1;
                                false
                            }
                            Err(failure) => {
                                failed += 1;
                                let ends = failure.ends_session();
                                problem = Some(this.failed(&failure, cx));
                                ends
                            }
                        };
                        cx.notify();
                        stop
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
            this.update_in(cx, |this, window, cx| {
                for key in &keys {
                    this.busy.remove(key);
                }
                match problem {
                    Some(problem) => toast_error(
                        window,
                        cx,
                        format!("Published {done}, {failed} failed: {problem}"),
                    ),
                    None if done == 1 => toast_success(window, cx, "Published 1 template"),
                    None => toast_success(window, cx, format!("Published {done} templates")),
                }
                this.refresh(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.entries.clear();
        self.managed.clear();
        self.managed_load = Load::Loading;
        self.versions.clear();
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
                        this.seed(cx);
                        this.refresh_managed(cx);
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

    fn seed(&mut self, cx: &mut Context<Self>) {
        let Some(access) = self.access(cx) else {
            return;
        };
        if self.seeding {
            return;
        }
        self.seeding = true;
        let entries = self.entries.clone();
        let db = db(cx);
        let state = db.run(|store| {
            Ok((
                store.library_seeded()?,
                store.library_links()?,
                store.templates()?,
            ))
        });
        cx.spawn(async move |this, cx| {
            let outcome: Result<(), Failure> = async {
                let (seeded, links, templates) = state
                    .await
                    .map_err(|error| Failure::Store(error.message()))?;
                if seeded || entries.is_empty() {
                    return Ok(());
                }
                let wanted = entries.into_iter().filter(|entry| {
                    !links.iter().any(|link| link.remote_id == entry.id)
                        && !templates.iter().any(|template| template.name == entry.name)
                });
                for entry in wanted {
                    let download = cx
                        .background_spawn({
                            let access = access.clone();
                            let id = entry.id.clone();
                            async move {
                                let token = access.token()?;
                                access
                                    .client
                                    .download(&token, &id)
                                    .map_err(Failure::Library)
                            }
                        })
                        .await?;
                    let install = db
                        .run(move |store| {
                            store.install_library_template(
                                &entry.id,
                                download.version,
                                &download.content,
                            )
                        })
                        .await
                        .map_err(|error| Failure::Store(error.message()))?;
                    this.update(cx, |this, cx| {
                        this.installed(&install, cx);
                        cx.notify();
                    })
                    .ok();
                }
                db.run(|store| store.mark_library_seeded())
                    .await
                    .map_err(|error| Failure::Store(error.message()))
            }
            .await;
            this.update(cx, |this, cx| {
                this.seeding = false;
                if let Err(failure) = outcome {
                    this.failed(&failure, cx);
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
        let remote = self
            .link_for_template(template)
            .map(|link| link.remote_id.clone());
        self.publish_to(template, remote, window, cx);
    }

    pub fn publish_to(
        &mut self,
        template: TemplateId,
        remote: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(access) = self.access(cx) else {
            return;
        };
        let key = template_key(template);
        if !self.busy.insert(key.clone()) {
            return;
        }
        cx.notify();
        let first = remote.is_none();
        let db = db(cx);
        cx.spawn_in(window, async move |this, cx| {
            let outcome: Result<LibraryLink, Failure> = async {
                let (upload, revision) = prepare_upload(&db, template).await?;
                let published = cx
                    .background_executor()
                    .spawn(async move { send_upload(&access, remote.as_deref(), &upload) })
                    .await?;
                save_link(&db, template, published, revision).await
            }
            .await;
            this.update_in(cx, |this, window, cx| {
                this.busy.remove(&key);
                match outcome {
                    Ok(link) => {
                        this.published(link);
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
                        this.managed.retain(|entry| entry.id != id);
                        this.versions.remove(&id);
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
