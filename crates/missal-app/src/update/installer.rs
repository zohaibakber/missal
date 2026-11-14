use std::path::Path;
use std::sync::Mutex;
use std::sync::mpsc::Sender;

use velopack::sources::{HttpSource, UpdateSource};
use velopack::{
    Error, HttpHeader, HttpOptions, UpdateCheck, UpdateManager, UpdateOptions, VelopackApp, VelopackAsset,
    VelopackAssetFeed,
};

use super::release::{self, REPOSITORY};

const API_TIMEOUT_MILLISECONDS: u64 = 20_000;

pub fn run_hooks() {
    VelopackApp::build().run();
}

struct GithubReleases {
    repository: &'static str,
    chosen: Mutex<Option<String>>,
}

impl GithubReleases {
    fn new(repository: &'static str) -> GithubReleases {
        GithubReleases {
            repository,
            chosen: Mutex::new(None),
        }
    }

    fn options() -> HttpOptions {
        HttpOptions {
            Headers: vec![
                HttpHeader {
                    Name: "Accept".to_owned(),
                    Value: "application/vnd.github+json".to_owned(),
                },
                HttpHeader {
                    Name: "User-Agent".to_owned(),
                    Value: format!("Missal/{}", env!("CARGO_PKG_VERSION")),
                },
            ],
            TimeoutMilliseconds: API_TIMEOUT_MILLISECONDS,
        }
    }

    fn chosen_base(&self) -> Option<String> {
        self.chosen.lock().unwrap_or_else(|error| error.into_inner()).clone()
    }
}

impl UpdateSource for GithubReleases {
    fn get_release_feed(
        &self,
        channel: &str,
        app: &velopack::bundle::Manifest,
        staged_user_id: &str,
    ) -> Result<VelopackAssetFeed, Error> {
        let json = velopack::download::download_url_as_string(
            &release::releases_url(self.repository),
            Some(&Self::options()),
        )?;
        let releases = release::parse_releases(&json).map_err(Error::Other)?;
        let Some(newest) = release::newest_update(&releases, &app.version, channel) else {
            *self.chosen.lock().unwrap_or_else(|error| error.into_inner()) = None;
            return Ok(VelopackAssetFeed { Assets: Vec::new() });
        };
        let base = release::download_base(self.repository, &newest.tag);
        *self.chosen.lock().unwrap_or_else(|error| error.into_inner()) = Some(base.clone());
        HttpSource::new(base).get_release_feed(channel, app, staged_user_id)
    }

    fn download_release_entry(
        &self,
        asset: &VelopackAsset,
        local_file: &Path,
        progress_sender: Option<Sender<i16>>,
    ) -> Result<(), Error> {
        let base = self
            .chosen_base()
            .ok_or_else(|| Error::Other("no release has been selected".to_owned()))?;
        HttpSource::new(base).download_release_entry(asset, local_file, progress_sender)
    }
}

pub struct Ready {
    manager: UpdateManager,
    asset: VelopackAsset,
}

impl Ready {
    pub fn version(&self) -> &str {
        &self.asset.Version
    }

    pub fn apply_on_exit(&self) -> Result<(), String> {
        self.manager
            .wait_exit_then_apply_updates(&self.asset, false, true, Vec::<String>::new())
            .map_err(|error| error.to_string())
    }
}

pub fn fetch() -> Result<Option<Ready>, String> {
    let options = UpdateOptions {
        MaximumDeltasBeforeFallback: 1,
        ..UpdateOptions::default()
    };
    let manager = match UpdateManager::new(GithubReleases::new(REPOSITORY), Some(options), None) {
        Ok(manager) => manager,
        Err(_) => return Ok(None),
    };
    if let Some(asset) = manager.get_update_pending_restart() {
        return Ok(Some(Ready { manager, asset }));
    }
    match manager.check_for_updates().map_err(|error| error.to_string())? {
        UpdateCheck::UpdateAvailable(info) => {
            manager
                .download_updates(&info, None)
                .map_err(|error| error.to_string())?;
            let asset = info.TargetFullRelease.clone();
            Ok(Some(Ready { manager, asset }))
        }
        UpdateCheck::NoUpdateAvailable | UpdateCheck::RemoteIsEmpty => Ok(None),
    }
}
