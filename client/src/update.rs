use std::{os::unix::fs::PermissionsExt, time::Duration};

use crate::{
    WEB_CLIENT,
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
    nix,
    profiles::Profile,
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
    Errored(String),
}

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile),
    Sync(
        Profile,
        Statemachine<ReqwestCachedRemoteZip<reqwest::Client>, TokioLocalStorage>,
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
    let versions_match = Some(remote_version.clone()) == profile.version;

    if !versions_match {
        tracing::info!("Versions do not match. Fetching remote file infos...");
    } else {
        tracing::debug!("Versions do match. Verifying file hashes");
    }

    profile.version = Some(remote_version.clone());

    let cache_file_parent = crate::fs::get_cache_path().join("remotezip");
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

    let Ok(remote) = ReqwestRemoteZip::with_url(profile.download_url()) else {
        return Some((Progress::Offline, State::Finished));
    };
    let remote = ReqwestCachedRemoteZip::with_inner(remote, cache);
    const KEEP_PATHS: &[&str] = &["userdata/", "screenshots/", "maps/", "veloren.zip"];
    let ignore = KEEP_PATHS.iter().map(|p| p.to_string()).collect();
    let local = TokioLocalStorage::new(profile.directory(), ignore);
    let config = remozipsy::Config::default();
    let statemachine = Statemachine::new(remote.clone(), local, config);

    // we are triggering remozipsy ONCE, so we get the result of the evalute phase
    if let Some((pg, statemachine)) = statemachine.progress().await {
        if need_save_cache {
            if let Some(content) = remote.try_cache_content() {
                if let Ok(ron_string) =
                    to_string_pretty(&content, PrettyConfig::default())
                {
                    if let Err(e) = tokio::fs::write(cache_file, ron_string).await {
                        tracing::warn!(?e, "Could not cache the remote zip");
                    };
                }
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
        TokioLocalStorage,
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
            remozipsy::Progress::Successful => {
                if let Err(e) = final_cleanup(profile.clone()).await {
                    (Progress::Errored(e.to_string()), State::Finished)
                } else {
                    (Progress::Successful(profile.clone()), State::Finished)
                }
            },
            remozipsy::Progress::Errored(e) => {
                (Progress::Errored(e.to_string()), State::Finished)
            },
        }),
        None => None,
    }
}

// permissions, update params
async fn final_cleanup(profile: Profile) -> Result<(), String> {
    #[cfg(unix)]
    {
        let profile_directory = profile.directory();

        // Patch executable files if we are on NixOS
        if nix::is_nixos().map_err(|e| e.to_string())? {
            nix::patch(&profile_directory, VOXYGEN_FILE).map_err(|e| e.to_string())?;
            nix::patch(&profile_directory, SERVER_CLI_FILE).map_err(|e| e.to_string())?;
        } else {
            let p = |path| async move {
                let meta = tokio::fs::metadata(&path)
                    .await
                    .map_err(|e| e.to_string())?;
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                tokio::fs::set_permissions(&path, perm)
                    .await
                    .map_err(|e| e.to_string())?;
                Ok::<(), String>(())
            };

            tracing::info!("patching unix files");
            let voxygen_file = profile_directory.join(VOXYGEN_FILE);
            p(voxygen_file).await?;
            let server_cli_file = profile_directory.join(SERVER_CLI_FILE);
            p(server_cli_file).await?;
        }
    }

    Ok(())
}
