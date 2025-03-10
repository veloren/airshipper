use std::convert::TryFrom;

use crate::{error::ClientError, profiles::Profile};
use compare::Compared;
use futures_util::stream::Stream;
use iced::futures;
use sync::{DeleteResult, DownloadProgress, DownloadResult, UnzipResult};
use tokio::{
    sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    task::JoinHandle,
};
use zip_core::structs::CompressionMethod;

mod compare;
mod local_directory;
mod remote;
mod sync;

pub use remote::RemoteFileInfo;

#[derive(Debug, Clone)]
pub(crate) enum Progress {
    Evaluating,
    /// If the consumer sees ReadyToSync a download is necessary, but they can
    /// implement logic to avoid any download
    ReadyToSync(Profile),
    Syncing(DownloadProgress),
    Successful(Profile),
    Errored(ClientError),
    Offline,
}

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile),
    InitializeSync(Profile, Compared),
    Sync(
        Profile,
        Vec<Vec<RemoteFileInfo>>,
        DownloadProgress,
        Vec<JoinHandle<DownloadResult>>,
        Vec<JoinHandle<UnzipResult>>,
        JoinHandle<DeleteResult>,
        UnboundedReceiver<u64>,
        UnboundedSender<u64>,
    ),
    Finished,
}

pub(crate) fn update(p: Profile) -> impl Stream<Item = Progress> {
    tracing::debug!("start updating");
    futures::stream::unfold(State::ToBeEvaluated(p), |old_state| old_state.progress())
}

impl State {
    pub(crate) async fn progress(self) -> Option<(Progress, Self)> {
        let res = match self {
            State::ToBeEvaluated(p) => evaluate(p).await,
            State::InitializeSync(p, c) => initialize_sync(p, c).await,
            State::Sync(p, nd, dowp, dowh, uh, delh, rx, tx) => {
                sync(p, nd, dowp, (dowh, uh, delh), (rx, tx)).await
            },
            State::Finished => Ok(None),
        };
        match res {
            Ok(ok) => ok,
            Err(e) => Some((Progress::Errored(e), State::Finished)),
        }
    }
}

// checks if an update is necessary
async fn evaluate(
    mut profile: Profile,
) -> Result<Option<(Progress, State)>, ClientError> {
    tracing::info!("Evaluating remote version...");
    let remote_version = match remote::version(profile.version_url()).await {
        Ok(ok) => ok,
        Err(_) => return Ok(Some((Progress::Offline, State::Finished))),
    };
    let versions_match = Some(remote_version.clone()) == profile.version;

    if !versions_match || profile.rfiles.is_empty() {
        tracing::info!("Versions do not match. Fetching remote file infos...");
        profile.rfiles = remote::rfile_infos(&profile).await?;
        profile.version = Some(remote_version);
    }

    let file_infos = local_directory::file_infos(profile.directory()).await?;

    let compared = compare::build_compared(profile.rfiles.clone(), file_infos);
    if compared.needs_download.is_empty() {
        if compared.needs_deletion.is_empty() {
            tracing::info!("Already up to date.");
        } else {
            tracing::info!(
                "Already up to date, but {} extra files found. Deleting...",
                compared.needs_deletion.len()
            );
            sync::remove_files(compared.needs_deletion).await?;
        }
        return Ok(Some((Progress::Successful(profile), State::Finished)));
    }

    tracing::info!(
        "Need to download {} bytes and delete {} files",
        compared.needs_download_bytes,
        compared.needs_deletion.len(),
    );

    if compared.needs_download.iter().any(|batch| {
        batch.iter().any(|rfi| {
            !matches!(
                CompressionMethod::try_from(rfi.compression_method),
                Ok(CompressionMethod::Deflated) | Ok(CompressionMethod::Stored)
            )
        })
    }) {
        return Err(ClientError::Custom(
            "Unsupported compression method found".to_string(),
        ));
    }

    if compared.needs_download.iter().any(|batch| {
        batch.iter().any(|rfi| {
            let path = profile.directory().join(&rfi.file_name);
            !path.starts_with(profile.directory())
        })
    }) {
        panic!(
            "{}",
            "Zip Escape Attack, it seems your zip is compromised and tries to write \
             outside root, call the veloren team, path tried to write to: {path:?}",
        );
    }

    Ok(Some((
        Progress::ReadyToSync(profile.clone()),
        State::InitializeSync(profile, compared),
    )))
}

// initializes the sync loop, this is separate mostly just because of the
// delete handle that I didn't want to make an Option<JoinHandle<_>>
async fn initialize_sync(
    profile: Profile,
    compared: Compared,
) -> Result<Option<(Progress, State)>, ClientError> {
    let progress = DownloadProgress::new(compared.needs_download_bytes);
    let (tx, rx) = unbounded_channel::<u64>();
    Ok(Some((
        Progress::Syncing(progress.clone()),
        State::Sync(
            profile,
            compared.needs_download,
            progress,
            Vec::new(),
            Vec::new(),
            tokio::spawn(sync::remove_files(compared.needs_deletion)),
            rx,
            tx,
        ),
    )))
}

// does the update
async fn sync(
    mut profile: Profile,
    mut needs_download: Vec<Vec<RemoteFileInfo>>,
    mut progress: DownloadProgress,
    (download_handles, mut unzip_handles, delete_handle): (
        Vec<JoinHandle<DownloadResult>>,
        Vec<JoinHandle<UnzipResult>>,
        JoinHandle<DeleteResult>,
    ),
    (mut rx, tx): (UnboundedReceiver<u64>, UnboundedSender<u64>),
) -> Result<Option<(Progress, State)>, ClientError> {
    const NUM_PARALLEL_DOWNLOADS: usize = 15;
    if needs_download.is_empty() && download_handles.is_empty() {
        tracing::info!("Download complete. Finalizing installation...");
        let mut unzip_results = Vec::new();
        for handle in unzip_handles {
            unzip_results.push(handle.await);
        }
        delete_handle.await??;
        for result in unzip_results {
            if let Some((patch_crc32, file_name)) = result?? {
                // since the executables are at the end I assume rev will be faster
                for rfile in profile.rfiles.iter_mut().rev() {
                    if *rfile.file_name == file_name {
                        rfile.patch_crc32 = Some(patch_crc32);
                        break;
                    }
                }
            }
        }
        return Ok(Some((Progress::Successful(profile), State::Finished)));
    }

    let mut download_results = Vec::new();
    let mut new_download_handles = Vec::new();
    for handle in download_handles {
        if handle.is_finished() {
            download_results.push(handle.await);
        } else {
            new_download_handles.push(handle);
        }
    }
    for result in download_results {
        if let Ok(Ok(mut new_handles)) = result {
            unzip_handles.append(&mut new_handles);
        } else {
            for dh in new_download_handles {
                if let Ok(Ok(handles)) = dh.await {
                    for uh in handles {
                        let _ = uh.await;
                    }
                }
            }
            for uh in unzip_handles {
                let _ = uh.await;
            }
            let _ = delete_handle.await;
            result??;
            // unreachable
            return Ok(None);
        }
    }

    while !needs_download.is_empty()
        && new_download_handles.len() < NUM_PARALLEL_DOWNLOADS
    {
        new_download_handles.push(tokio::spawn(sync::download_batch(
            profile.download_url(),
            needs_download.pop().unwrap(),
            profile.directory(),
            tx.clone(),
        )));
    }

    for _ in 0..rx.len() {
        progress.add_chunk(rx.recv().await.unwrap_or_default());
    }

    Ok(Some((
        Progress::Syncing(progress.clone()),
        State::Sync(
            profile,
            needs_download,
            progress,
            new_download_handles,
            unzip_handles,
            delete_handle,
            rx,
            tx,
        ),
    )))
}
