use std::{convert::TryFrom, future::Future, io::Read, pin::Pin};

#[cfg(unix)]
use crate::{
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
    nix,
};
use crate::{error::ClientError, profiles::Profile};
use bytes::{Buf, BytesMut};
use compare::Compared;
use download::{Download, DownloadError, ProgressData, StepProgress};
use flate2::read::DeflateDecoder;
use futures_util::{
    FutureExt,
    stream::{FuturesUnordered, Stream},
};
use iced::futures;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tokio::io::AsyncWriteExt;
use zip_core::{
    Signature,
    raw::{LocalFileHeader, parse::Parse},
    structs::CompressionMethod,
};

mod compare;
mod download;
mod local_directory;
mod remote;

pub use download::UpdateContent;
pub use remote::RemoteFileInfo;

#[derive(Debug, Clone)]
pub(crate) enum Progress {
    Evaluating,
    /// If the consumer sees ReadyToDownload a download is necessary, but they can
    /// implement logic to avoid any download
    ReadyToDownload,
    #[allow(clippy::enum_variant_names)]
    InProgress(ProgressData),
    Successful(Profile),
    Errored(ClientError),
}

type Afterburner = FuturesUnordered<
    Pin<Box<dyn Future<Output = Result<Download<RemoteFileInfo>, DownloadError>> + Send>>,
>;

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile),
    Downloading(
        Profile,
        Compared,
        Vec<(BytesMut, RemoteFileInfo, LocalFileHeader)>,
        Afterburner,
        ProgressData,
    ),
    Unzipping(
        Profile,
        Compared,
        Vec<(BytesMut, RemoteFileInfo, LocalFileHeader)>,
        ProgressData,
    ),
    Removing(Profile, Compared),
    FinalCleanup(Profile),
    Finished,
}

pub(crate) fn update(params: Profile) -> impl Stream<Item = Progress> {
    tracing::debug!(?params, "start updating");
    futures::stream::unfold(State::ToBeEvaluated(params), |old_state| {
        old_state.progress()
    })
}

impl State {
    pub(crate) async fn progress(self) -> Option<(Progress, Self)> {
        let res = match self {
            State::ToBeEvaluated(params) => evaluate(params).await,
            State::Downloading(p, cp, f, a, pr) => downloading(p, cp, f, a, pr).await,
            State::Unzipping(p, cp, f, pr) => unzipping(p, cp, f, pr).await,
            State::Removing(p, cp) => removing(p, cp).await,
            State::FinalCleanup(p) => final_cleanup(p).await,
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
    let remote_version = remote::version(profile.version_url()).await?;
    let versions_match = Some(remote_version.clone()) == profile.version;

    if !versions_match || profile.rfiles.is_empty() {
        tracing::info!("Versions do not match. Fetching remote file infos...");
        profile.rfiles = remote::rfile_infos(&profile).await?;
        profile.version = Some(remote_version);
    }

    let file_infos = local_directory::file_infos(profile.directory()).await?;

    let mut compared = compare::build_compared(profile.rfiles.clone(), file_infos);
    if compared.needs_download.is_empty() && compared.needs_deletion_total == 0 {
        tracing::info!("Already up to date.");
        return Ok(Some((Progress::Evaluating, State::FinalCleanup(profile))));
    }
    tracing::info!(
        "Need to download {} bytes and delete {} files",
        compared.needs_download_bytes,
        compared.needs_deletion_total
    );
    tracing::debug!("{} bytes clean", compared.clean_data_total);

    use std::convert::TryFrom;
    if compared.needs_download.iter().any(|rfi| {
        !matches!(
            CompressionMethod::try_from(rfi.compression_method),
            Ok(CompressionMethod::Deflated) | Ok(CompressionMethod::Stored)
        )
    }) {
        return Err(ClientError::Custom(
            "Unsupported compression method found".to_string(),
        ));
    }

    match download::next_partial(&profile, &mut compared) {
        Some(first_partially) => {
            let content = match &first_partially {
                Download::Start(_, _, c, _) => c,
                _ => unreachable!(),
            };

            let progress = ProgressData::new(
                StepProgress::new(compared.needs_download_bytes, content.clone()),
                Default::default(),
            );
            let afterburner = FuturesUnordered::new();
            afterburner.push(first_partially.progress().boxed());

            Ok(Some((
                Progress::ReadyToDownload,
                State::Downloading(profile, compared, vec![], afterburner, progress),
            )))
        },
        None => Ok(Some((
            Progress::ReadyToDownload,
            State::Removing(profile, compared),
        ))),
    }
}

// downloads partial info
async fn downloading(
    profile: Profile,
    mut compared: Compared,
    mut finished: Vec<(BytesMut, RemoteFileInfo, LocalFileHeader)>,
    mut afterburner: Afterburner,
    mut progress: ProgressData,
) -> Result<Option<(Progress, State)>, ClientError> {
    use futures::stream::StreamExt;
    const DOWNLOADS_IN_QUEUE_SPEEDUP: usize = 15;
    // we can max finish 1 download each fn call, so its okay to only add 1 download.
    if afterburner.len() < DOWNLOADS_IN_QUEUE_SPEEDUP {
        if let Some(next_partially) = download::next_partial(&profile, &mut compared) {
            afterburner.push(next_partially.progress().boxed());
        }
    }

    let download = afterburner
        .next()
        .await
        .expect("There should be at least 1 entry to be downloaded")?;

    match download {
        Download::Finished(mut s, r) => {
            let local_header = LocalFileHeader::from_buf(&mut s)
                .map_err(|e| ClientError::Custom(e.to_string()))?;
            if !local_header.is_valid_signature() {
                return Err(ClientError::Custom(
                    "Invalid local header signature".to_string(),
                ));
            }
            finished.push((s, r, local_header));

            if !afterburner.is_empty() {
                Ok(Some((
                    Progress::InProgress(progress.clone()),
                    State::Downloading(
                        profile,
                        compared,
                        finished,
                        afterburner,
                        progress,
                    ),
                )))
            } else {
                let total = finished.iter().map(|e| e.1.compressed_size as u64).sum();
                let pr = ProgressData::new(
                    StepProgress::new(total, UpdateContent::Decompress("".to_string())),
                    Default::default(),
                );
                tracing::info!("unzipping files");
                Ok(Some((
                    Progress::InProgress(progress),
                    State::Unzipping(profile, compared, finished, pr),
                )))
            }
        },
        Download::Progress(_r, _s, mut p, _p) => {
            progress.add_from_step(&mut p);

            afterburner.push(Download::Progress(_r, _s, p, _p).progress().boxed());
            Ok(Some((
                Progress::InProgress(progress.clone()),
                State::Downloading(profile, compared, finished, afterburner, progress),
            )))
        },
        Download::Start(_, _, _, _) => unreachable!(),
    }
}

// downloads partial info
async fn unzipping(
    profile: Profile,
    compared: Compared,
    mut finished: Vec<(BytesMut, RemoteFileInfo, LocalFileHeader)>,
    mut progress: ProgressData,
) -> Result<Option<(Progress, State)>, ClientError> {
    match finished.pop() {
        Some((rbytes, remote, _)) => {
            let remote_file_size = remote.compressed_size as usize;
            if remote_file_size > rbytes.remaining() {
                return Err(ClientError::Custom(
                    "Not enough bytes downloaded".to_string(),
                ));
            }

            let path = profile.directory().join(&remote.file_name);
            if !path.starts_with(profile.directory()) {
                panic!(
                    "{}",
                    "Zip Escape Attack, it seems your zip is compromised and tries to \
                     write outside root, call the veloren team, path tried to write to: \
                     {path:?}",
                );
            }

            let parent = path.parent().unwrap();
            tokio::fs::create_dir_all(parent).await?;

            let file = tokio::spawn(tokio::fs::File::create(path));

            let mut file_data =
                match CompressionMethod::try_from(remote.compression_method) {
                    Ok(CompressionMethod::Deflated) => {
                        let compressed = rbytes.take(remote_file_size);
                        let mut deflate_reader = DeflateDecoder::new(compressed.reader());
                        let mut decompressed = Vec::with_capacity(remote_file_size);
                        deflate_reader.read_to_end(&mut decompressed).unwrap();
                        bytes::Bytes::copy_from_slice(&decompressed)
                    },
                    Ok(CompressionMethod::Stored) => rbytes
                        .take(remote_file_size)
                        .copy_to_bytes(remote_file_size),
                    // should not happen at this point
                    _ => {
                        return Err(ClientError::Custom(
                            "Unsupported compression method found".to_string(),
                        ));
                    },
                };

            let mut file = file.await.unwrap()?;
            // TODO: evaluate splitting this up
            file.write_all_buf(&mut file_data).await?;

            progress.add_chunk(remote_file_size as u64);
            progress.cur_step_mut().content = UpdateContent::Decompress(remote.file_name);

            Ok(Some((
                Progress::InProgress(progress.clone()),
                State::Unzipping(profile, compared, finished, progress),
            )))
        },
        None => {
            tracing::info!("deleting files that should be removed");
            Ok(Some((
                Progress::InProgress(progress),
                State::Removing(profile, compared),
            )))
        },
    }
}

// remove old files
async fn removing(
    profile: Profile,
    mut compared: Compared,
) -> Result<Option<(Progress, State)>, ClientError> {
    match compared.needs_deletion.pop() {
        Some(f) => {
            tracing::debug!("deleting {:?}", &f.path);
            tokio::fs::remove_file(&f.path).await?;
            let mut progress = ProgressData::new(
                StepProgress::new(
                    compared.needs_deletion_total,
                    UpdateContent::DownloadFile(f.local_unix_path.clone()),
                ),
                Default::default(),
            );
            progress.cur_step_mut().processed_bytes =
                compared.needs_deletion_total - compared.needs_deletion.len() as u64;
            Ok(Some((
                Progress::InProgress(progress),
                State::Removing(profile, compared),
            )))
        },
        None => Ok(Some((Progress::Evaluating, State::FinalCleanup(profile)))),
    }
}

// permissions, update params
async fn final_cleanup(
    profile: Profile,
) -> Result<Option<(Progress, State)>, ClientError> {
    #[cfg(unix)]
    {
        let profile_directory = profile.directory();

        // Patch executable files if we are on NixOS
        if nix::is_nixos()? {
            nix::patch(&profile_directory)?;
        } else {
            let p = |path| async move {
                let meta = tokio::fs::metadata(&path).await?;
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                tokio::fs::set_permissions(&path, perm).await?;
                Ok::<(), ClientError>(())
            };

            let voxygen_file = profile_directory.join(VOXYGEN_FILE);
            p(voxygen_file).await?;
            let server_cli_file = profile_directory.join(SERVER_CLI_FILE);
            p(server_cli_file).await?;
        }
    }

    Ok(Some((Progress::Successful(profile), State::Finished)))
}
