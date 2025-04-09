use std::{
    convert::TryFrom,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use crate::{error::ClientError, profiles::Profile};
use bytes::BytesMut;
use compare::Compared;
use futures_util::stream::Stream;
use iced::futures;
use sync::{DeleteResult, DownloadResult, ProgressDetails, UnzipResult};
use tokio::task::JoinHandle;
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
    Syncing(ProgressDetails),
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
        ProgressDetails,
        Vec<JoinHandle<DownloadResult>>,
        Arc<Mutex<Vec<JoinHandle<UnzipResult>>>>,
        JoinHandle<DeleteResult>,
        Vec<UnzipResult>,
        Arc<AtomicUsize>,
        Arc<AtomicUsize>,
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
            State::Sync(p, nd, dowp, dowh, uh, delh, ur, rx, tx) => {
                sync(p, nd, dowp, (dowh, uh, delh), ur, (rx, tx)).await
            },
            State::Finished => Ok(None),
        };
        match res {
            Ok(ok) => ok,
            Err(e) => Some((Progress::Errored(e), State::Finished)),
        }
    }
}

/// checks if an update is necessary
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

/// initializes the sync loop, this is separate mostly just because of the
/// delete handle that I didn't want to make an Option<JoinHandle<_>>
async fn initialize_sync(
    profile: Profile,
    compared: Compared,
) -> Result<Option<(Progress, State)>, ClientError> {
    let progress = ProgressDetails::new(compared.needs_download_bytes);
    Ok(Some((
        Progress::Syncing(progress.clone()),
        State::Sync(
            profile,
            compared.needs_download,
            progress,
            Vec::new(),
            Arc::new(Mutex::new(Vec::new())),
            tokio::spawn(sync::remove_files(compared.needs_deletion)),
            Vec::new(),
            Arc::new(AtomicUsize::new(0)),
            Arc::new(AtomicUsize::new(0)),
        ),
    )))
}

/// coordinates the update: download of new chunks, unzipping files and writing them to
/// disk
#[expect(clippy::type_complexity)]
async fn sync(
    mut profile: Profile,
    mut needs_download: Vec<Vec<RemoteFileInfo>>,
    mut progress: ProgressDetails,
    (mut download_handles, mut unzip_handles, delete_handle): (
        Vec<JoinHandle<DownloadResult>>,
        Arc<Mutex<Vec<JoinHandle<UnzipResult>>>>,
        JoinHandle<DeleteResult>,
    ),
    mut unzip_results: Vec<UnzipResult>,
    (dc, zc): (Arc<AtomicUsize>, Arc<AtomicUsize>),
) -> Result<Option<(Progress, State)>, ClientError> {
    const NUM_PARALLEL_DOWNLOADS: usize = 15;
    if needs_download.is_empty() && download_handles.is_empty() {
        // when `get_mut` succeeds all unzips have been added, because there exist no
        // further references to this arc.
        if let Some(unzip_handles) = Arc::get_mut(&mut unzip_handles) {
            let unzip_finished = unzip_handles.get_mut().unwrap().is_empty();
            if unzip_finished {
                tracing::info!("Download complete. Unzip complete. Deleting entries now");
                delete_handle.await??;

                tracing::debug!("Storing patches to profile");
                for result in unzip_results {
                    if let Some((patch_crc32, file_name)) = result? {
                        // since the executables are at the end I assume rev will be
                        // faster
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
        } else {
            tracing::debug!("Download complete. waiting for Unzip to complete.");
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    {}

    // extract downloads finished in meantime, downloads return now value, as they trigger
    // unzips during runtime
    let finished_handle_iter =
        download_handles.extract_if(.., |handle| handle.is_finished());
    // use iter, otherwith it has no effect
    let cnt = finished_handle_iter.count();
    if cnt > 0 {
        tracing::trace!(?cnt, "downloads finished");
    }

    // extract unzips finished in meantime
    let finished_unzip_handles: Option<Vec<_>> = unzip_handles
        .try_lock()
        .map(|mut guard| {
            guard
                .extract_if(.., |handle| handle.is_finished())
                .collect()
        })
        .ok();
    if let Some(finished_unzip_handles) = finished_unzip_handles {
        unzip_results.reserve(finished_unzip_handles.len());
        for finished_handle in finished_unzip_handles.into_iter() {
            unzip_results.push(finished_handle.await?);
        }
    }

    // spawn new downloads if capacity is there
    let dir = profile.directory();
    let unzip_handles2 = unzip_handles.clone();
    let zc2 = zc.clone();
    let spawn_unzip = move |bytes: BytesMut, rfile: RemoteFileInfo| {
        let dir = dir.clone();
        let zc2 = zc2.clone();
        let name = &rfile.file_name;
        tracing::trace!(?name, "triggering unzip");
        let new_task = tokio::spawn(sync::unzip_file(bytes, rfile, dir, zc2));
        let mut unzip_handles2 = unzip_handles2.lock().unwrap(); //SYNC LOCK: carefull not to hold this over .await
        unzip_handles2.push(new_task);
    };

    while !needs_download.is_empty() && download_handles.len() < NUM_PARALLEL_DOWNLOADS {
        tracing::trace!("triggering download");
        download_handles.push(tokio::spawn(sync::download_batch(
            profile.download_url(),
            needs_download.pop().unwrap(),
            spawn_unzip.clone(),
            dc.clone(),
        )));
    }

    let download_count = dc.swap(0, Ordering::SeqCst);
    let unzip_count = zc.swap(0, Ordering::SeqCst);
    progress.add_chunk(download_count as u64); //we used usize, so be available on most platforms

    if download_count > 0 || unzip_count > 0 {
        let d_len = download_handles.len();
        let u_len = unzip_handles
            .try_lock()
            .map(|l| l.len())
            .unwrap_or_default();
        tracing::trace!(
            ?download_count,
            ?unzip_count,
            ?d_len,
            ?u_len,
            "status changed"
        );
    }

    Ok(Some((
        Progress::Syncing(progress.clone()),
        State::Sync(
            profile,
            needs_download,
            progress,
            download_handles,
            unzip_handles,
            delete_handle,
            unzip_results,
            dc,
            zc,
        ),
    )))
}
