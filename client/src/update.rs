use std::{
    future::Future,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    time::{Duration, SystemTime},
};

use crate::{
    ClientError, WEB_CLIENT,
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
    nix,
    profiles::{PatchedInfo, Profile},
};
use futures_util::{Stream, stream};
use remozipsy::{
    ProgressDetails, Statemachine,
    reqwest::{ReqwestCachedRemoteZip, ReqwestRemoteZip},
    tokio::TokioLocalStorage,
};
use ron::ser::{PrettyConfig, to_string_pretty};

#[derive(Debug, Clone)]
pub(crate) enum Progress {
    Offline,
    /// If the consumer sees ReadyToSync a download is necessary, but they can
    /// implement logic to avoid any download
    ReadyToSync {
        version: String,
    },
    // Status from remozipsy
    DownloadExtracting {
        download: ProgressDetails,
        unzip: ProgressDetails,
    },
    Deleting(ProgressDetails),
    Successful(Profile),
    Errored(ClientError),
}

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile),
    Sync(
        Profile,
        Statemachine<ReqwestCachedRemoteZip<reqwest::Client>, PatchedLocalStorage>,
    ),
    /// in case its finished early while evaluating
    Finished,
}

pub(crate) fn update(p: Profile) -> impl Stream<Item = Progress> {
    tracing::debug!("start updating");
    stream::unfold(State::ToBeEvaluated(p), |old_state| old_state.progress())
}

async fn version(url: String) -> Result<String, reqwest::Error> {
    WEB_CLIENT.get(url).send().await?.text().await
}

fn cache_base_path() -> PathBuf {
    crate::fs::get_cache_path().join("remotezip")
}

impl State {
    pub(crate) async fn progress(self) -> Option<(Progress, Self)> {
        tokio::time::sleep(Duration::from_millis(5)).await;
        match self {
            State::ToBeEvaluated(profile) => evaluate(profile).await,
            State::Sync(profile, statemachine) => sync(profile, statemachine).await,
            State::Finished => None,
        }
    }
}

// checks if an update is necessary
async fn evaluate(mut profile: Profile) -> Option<(Progress, State)> {
    tracing::info!("Evaluating remote version...");
    let remote_version = match version(profile.version_url()).await {
        Ok(ok) => ok,
        Err(_) => return Some((Progress::Offline, State::Finished)),
    };

    profile.version = Some(remote_version.clone());

    let cache_file_parent = cache_base_path();
    let cache_file = cache_file_parent.join(format!("{remote_version}.ron"));
    let mut cache = None;
    if tokio::fs::create_dir_all(cache_file_parent).await.is_ok() {
        if let Ok(file_content) = tokio::fs::read_to_string(&cache_file).await {
            if let Ok(content) = ron::from_str(&file_content) {
                cache = Some(content);
            }
        }
    };
    let need_save_cache = cache.is_none();

    if need_save_cache {
        tracing::info!(
            "Remote file list not found in cache. Fetching remote file infos..."
        );
    } else {
        tracing::debug!("Remote file list found in cache. Verifying file hashes");
    }

    let Ok(remote) = ReqwestRemoteZip::with_url(profile.download_url()) else {
        return Some((Progress::Offline, State::Finished));
    };
    let remote = ReqwestCachedRemoteZip::with_inner(remote, cache);
    const KEEP_PATHS: &[&str] = &["userdata/", "screenshots/", "maps/", "veloren.zip"];
    let ignore = KEEP_PATHS.iter().map(|p| p.to_string()).collect();
    let local = PatchedLocalStorage {
        inner: TokioLocalStorage::new(profile.directory(), ignore),
        patches: profile.patched_crc32s.clone(),
    };
    let config = remozipsy::Config::default();
    let statemachine = Statemachine::new(remote.clone(), local, config);

    // we are triggering remozipsy ONCE, so we get the result of the evalute phase
    if let Some((pg, statemachine)) = statemachine.progress().await {
        if need_save_cache {
            match remote.try_cache_content() {
                Some(content) => {
                    match to_string_pretty(&content, PrettyConfig::default()) {
                        Ok(ron_string) => {
                            if let Err(e) = tokio::fs::write(cache_file, ron_string).await
                            {
                                tracing::warn!(?e, "Could not cache the remote zip");
                            };
                        },
                        Err(e) => tracing::warn!(
                            ?e,
                            "Could not serialize remote zip file list for caching"
                        ),
                    }
                },
                None => tracing::warn!(
                    "Could not obtain lock on remote zip file list for caching"
                ),
            }
        }

        if !matches!(pg, remozipsy::Progress::Successful) {
            return Some((
                Progress::ReadyToSync {
                    version: remote_version,
                },
                State::Sync(profile, statemachine),
            ));
        }
    };

    Some((Progress::Successful(profile), State::Finished))
}

// checks if an update is necessary
async fn sync(
    profile: Profile,
    statemachine: Statemachine<
        ReqwestCachedRemoteZip<reqwest::Client>,
        PatchedLocalStorage,
    >,
) -> Option<(Progress, State)> {
    match statemachine.progress().await {
        Some((p, s)) => Some(match p {
            remozipsy::Progress::DownloadExtracting { download, unzip } => (
                Progress::DownloadExtracting { download, unzip },
                State::Sync(profile, s),
            ),
            remozipsy::Progress::Deleting(deleting) => {
                (Progress::Deleting(deleting), State::Sync(profile, s))
            },
            remozipsy::Progress::Successful => match final_cleanup(profile).await {
                Ok(p) => (Progress::Successful(p), State::Finished),
                Err(e) => (Progress::Errored(e), State::Finished),
            },
            remozipsy::Progress::Errored(e) => {
                (Progress::Errored(e.into()), State::Finished)
            },
        }),
        None => None,
    }
}

// permissions, update params
async fn final_cleanup(mut profile: Profile) -> Result<Profile, ClientError> {
    // dont error, if cleanup fails
    const DAYS_14: Duration = Duration::from_secs(14 * 86400);
    if let (Ok(dir), Some(max_age)) = (
        std::fs::read_dir(cache_base_path()),
        SystemTime::now().checked_sub(DAYS_14),
    ) {
        for file in dir.flatten() {
            if let Err(e) = || -> Result<(), Box<dyn std::error::Error>> {
                let meta = file.metadata()?;
                if !meta.is_file() {
                    return Ok(());
                }

                let time = meta.modified()?;
                if time < max_age {
                    std::fs::remove_file(file.path())?;
                    tracing::info!("removed old cache file: {:?}", file.file_name());
                }

                Ok(())
            }() {
                tracing::warn!(?e, "Failed to cleanup download cache")
            }
        }
    }

    profile.patched_crc32s.clear();

    #[cfg(unix)]
    {
        let profile_directory = profile.directory();

        // Patch executable files if we are on NixOS
        if nix::is_nixos()? {
            let info = nix::patch(&profile_directory, VOXYGEN_FILE)?;
            profile.patched_crc32s.push(info);
            let info = nix::patch(&profile_directory, SERVER_CLI_FILE)?;
            profile.patched_crc32s.push(info);
        } else {
            let p = |path| async move {
                let meta = tokio::fs::metadata(&path).await?;
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                tokio::fs::set_permissions(&path, perm).await?;
                Ok::<(), ClientError>(())
            };

            tracing::info!("patching unix exec files");
            let voxygen_file = profile_directory.join(VOXYGEN_FILE);
            p(voxygen_file).await?;
            let server_cli_file = profile_directory.join(SERVER_CLI_FILE);
            p(server_cli_file).await?;
        }
    }

    Ok(profile)
}

/// allows patching the actual local files with some data that we have stored, is used in
/// nixos to prevent always-redownload of binary files
#[derive(Debug, Clone)]
pub struct PatchedLocalStorage {
    inner: TokioLocalStorage,
    patches: Vec<PatchedInfo>,
}

impl remozipsy::FileSystem for PatchedLocalStorage {
    type Error = remozipsy::tokio::TokioLocalStorageError;
    type StorePrepare = tokio::fs::File;

    async fn all_files(&mut self) -> Result<Vec<remozipsy::FileInfo>, Self::Error> {
        let mut all_files = self.inner.all_files().await?;

        for patches in &self.patches {
            if let Some(to_be_manipulated) = all_files.iter_mut().find(|e| {
                e.local_unix_path == patches.local_unix_path
                    && e.crc32 == patches.post_crc32
            }) {
                to_be_manipulated.crc32 = patches.pre_crc32;
            }
        }

        Ok(all_files)
    }

    fn delete_file(
        &self,
        info: remozipsy::FileInfo,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        self.inner.delete_file(info)
    }

    fn prepare_store_file(
        &self,
        info: remozipsy::FileInfo,
    ) -> impl Future<Output = Result<Self::StorePrepare, Self::Error>> {
        self.inner.prepare_store_file(info)
    }

    fn store_file(
        &self,
        prepared: Self::StorePrepare,
        data: bytes::Bytes,
    ) -> impl Future<Output = Result<(), Self::Error>> {
        self.inner.store_file(prepared, data)
    }
}
