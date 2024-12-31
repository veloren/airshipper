use std::{convert::TryFrom, future::Future, io::Read, pin::Pin};

use crate::{
    GITHUB_CLIENT, WEB_CLIENT, consts::DOWNLOAD_FILE, error::ClientError,
    profiles::Profile,
};
#[cfg(unix)]
use crate::{
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
    nix,
};
use bytes::{Buf, BytesMut};
use compare::{Compared, prepare_local_with_remote};
use download::{Download, DownloadError, ProgressData, StepProgress};
use flate2::read::DeflateDecoder;
use futures_util::{
    FutureExt,
    stream::{FuturesUnordered, Stream},
};
use iced::futures;
use local_directory::{FileInformation, LocalDirectory};
use remote_zip::{RemoteZipError, gen_classic};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use tokio::io::AsyncWriteExt;
use zip_core::{
    Signature,
    raw::{CentralDirectoryHeader, LocalFileHeader, parse::Parse},
    structs::CompressionMethod,
};

mod compare;
mod download;
mod local_directory;
mod remote_zip;

pub use download::{Storage, UpdateContent};

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
    Pin<
        Box<
            dyn Future<Output = Result<Download<CentralDirectoryHeader>, DownloadError>>
                + Send,
        >,
    >,
>;

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile, bool),
    DownloadEndOfCentralDirectory(Profile, bool),
    DownloadCentralDirectory(Profile, bool, Download<()>),
    CrcLocalDirectory(Profile, LocalDirectory, Vec<CentralDirectoryHeader>),
    DownloadingClassic(Profile, Download<()>, ProgressData),
    UnzipClassic(Profile),
    FilterMissingFiles(Profile, Vec<CentralDirectoryHeader>, Vec<FileInformation>),
    DownloadingPartially(
        Profile,
        Compared,
        Vec<(BytesMut, CentralDirectoryHeader, LocalFileHeader)>,
        Afterburner,
        ProgressData,
    ),
    UnzipPartial(
        Profile,
        Compared,
        Vec<(BytesMut, CentralDirectoryHeader, LocalFileHeader)>,
        ProgressData,
    ),
    RemovingPartially(Profile, Compared),
    FinalCleanup(Profile),
    Finished,
}

pub(crate) fn update(
    params: Profile,
    force_complete_redownload: bool,
) -> impl Stream<Item = Progress> {
    tracing::debug!(?params, "start updating");
    futures::stream::unfold(
        State::ToBeEvaluated(params, force_complete_redownload),
        |old_state| old_state.progress(),
    )
}

impl State {
    pub(crate) async fn progress(self) -> Option<(Progress, Self)> {
        let res = match self {
            State::ToBeEvaluated(params, f) => evaluate_remote_version(params, f).await,
            State::DownloadEndOfCentralDirectory(p, m) => {
                evaluate_remote_eocd(p, m).await
            },
            State::DownloadCentralDirectory(p, m, d) => evaluate_remote_cd(p, m, d).await,
            State::CrcLocalDirectory(p, ld, cds) => evaluate_local_dir(p, ld, cds).await,
            State::DownloadingClassic(p, d, pr) => downloading_classic(p, d, pr).await,
            State::UnzipClassic(p) => unzip_classic(p).await,
            State::FilterMissingFiles(p, cds, fi) => filter_missings(p, cds, fi).await,
            State::DownloadingPartially(p, cp, f, a, pr) => {
                downloading_partial(p, cp, f, a, pr).await
            },
            State::UnzipPartial(p, cp, f, pr) => unzip_partial(p, cp, f, pr).await,
            State::RemovingPartially(p, cp) => removing_partial(p, cp).await,
            State::FinalCleanup(p) => final_cleanup(p).await,
            State::Finished => Ok(None),
        };
        match res {
            Ok(ok) => ok,
            Err(e) => Some((Progress::Errored(e), State::Finished)),
        }
    }
}

// asks airshipper server for version and compares with profile
async fn evaluate_remote_version(
    mut profile: Profile,
    force_complete_redownload: bool,
) -> Result<Option<(Progress, State)>, ClientError> {
    tracing::debug!("evaluate_remote_version");
    if force_complete_redownload {
        tracing::info!("force redownload, no matter what");
        return Ok(Some((Progress::ReadyToDownload, gen_classic(profile))));
    }

    let remote = WEB_CLIENT
        .get(profile.version_url())
        .send()
        .await?
        .text()
        .await?;

    let remote_matches_profile =
        Some(remote.clone()) == profile.version && profile.installed();

    profile.version = Some(remote);

    if !remote_matches_profile && profile.disable_partial_download {
        Ok(Some((
            Progress::ReadyToDownload,
            gen_classic(profile.clone()),
        )))
    } else {
        Ok(Some((
            Progress::Evaluating,
            State::DownloadEndOfCentralDirectory(profile, remote_matches_profile),
        )))
    }
}

fn fallback(
    profile: &Profile,
    remote_matches_profile: bool,
) -> Result<Option<(Progress, State)>, ClientError> {
    if remote_matches_profile {
        // If content_length is unavailable and profile matches, we assume
        // everything is fine
        Ok(Some((
            Progress::Evaluating,
            State::FinalCleanup(profile.clone()),
        )))
    } else {
        // if profiles dont match, we enforce classic download
        Ok(Some((
            Progress::ReadyToDownload,
            gen_classic(profile.clone()),
        )))
    }
}

// does 1 HEAD and 1 gets to get zip internal information
async fn evaluate_remote_eocd(
    profile: Profile,
    remote_matches_profile: bool,
) -> Result<Option<(Progress, State)>, ClientError> {
    tracing::debug!("evaluate_remote_eocd");
    let zip_url = profile.download_url();
    let content_length_resp = GITHUB_CLIENT.head(&zip_url).send().await?;

    let content_length = match content_length_resp.content_length() {
        Some(len) => len,
        None => return fallback(&profile, remote_matches_profile),
    };

    // download EOCD
    let eocd = match remote_zip::download_eocd(content_length, &zip_url).await {
        Ok(eocd) => eocd,
        Err(RemoteZipError::Reqwest(e)) => {
            return Err(e)?;
        },
        Err(_) => return fallback(&profile, remote_matches_profile),
    };

    tracing::trace!("starting CD download");
    let download = remote_zip::download_cds(&eocd, &zip_url);

    Ok(Some((
        Progress::Evaluating,
        State::DownloadCentralDirectory(profile, remote_matches_profile, download),
    )))
}

// fetching the CD might take ~1,5s, so do it partially
async fn evaluate_remote_cd(
    profile: Profile,
    remote_matches_profile: bool,
    download: Download<()>,
) -> Result<Option<(Progress, State)>, ClientError> {
    let download = download.progress().await?;
    match download {
        Download::Finished(storage, ()) => {
            //download CDs
            let bytes = match storage {
                Storage::Memory(bytes) => bytes.freeze(),
                _ => unreachable!(),
            };
            let cds = match remote_zip::extract_cds(bytes) {
                Some(cds) => cds,
                None => {
                    return fallback(&profile, remote_matches_profile);
                },
            };

            tracing::trace!("starting verifying disk");
            let local_directory = LocalDirectory::Start(profile.directory());

            Ok(Some((
                Progress::Evaluating,
                State::CrcLocalDirectory(profile, local_directory, cds),
            )))
        },
        Download::Progress(_, _, _, _) => Ok(Some((
            Progress::Evaluating,
            State::DownloadCentralDirectory(profile, remote_matches_profile, download),
        ))),
        Download::Start(_, _, _, _) => unreachable!(),
    }
}

// crc all local files to check for diffs
async fn evaluate_local_dir(
    profile: Profile,
    local_dir: LocalDirectory,
    cds: Vec<CentralDirectoryHeader>,
) -> Result<Option<(Progress, State)>, ClientError> {
    let local_dir = local_dir
        .progress()
        .await
        .map_err(|e| ClientError::Custom(e.to_string()))?;
    match local_dir {
        LocalDirectory::Finished(file_infos) => Ok(Some((
            Progress::Evaluating,
            State::FilterMissingFiles(profile, cds, file_infos),
        ))),
        LocalDirectory::Progress(_, _, _, _, ref progress) => Ok(Some((
            Progress::InProgress(progress.clone()),
            State::CrcLocalDirectory(profile, local_dir, cds),
        ))),
        LocalDirectory::Start(_) => unreachable!(),
    }
}

// continues_downloading
async fn downloading_classic(
    profile: Profile,
    download: Download<()>,
    mut progress: ProgressData,
) -> Result<Option<(Progress, State)>, ClientError> {
    let download = download.progress().await?;
    match download {
        Download::Finished(_, ()) => Ok(Some((
            Progress::InProgress(ProgressData::new(
                StepProgress::new(0, UpdateContent::DownloadFullZip),
                Default::default(),
            )),
            State::UnzipClassic(profile),
        ))),
        Download::Progress(_r, _s, mut pr, _t) => {
            progress.cur_step_mut().total_bytes = pr.total_bytes;
            progress.add_from_step(&mut pr);
            Ok(Some((
                Progress::InProgress(progress.clone()),
                State::DownloadingClassic(
                    profile,
                    Download::Progress(_r, _s, pr, _t),
                    progress,
                ),
            )))
        },
        Download::Start(_, _, _, _) => unreachable!(),
    }
}

// continues_downloading
async fn unzip_classic(
    profile: Profile,
) -> Result<Option<(Progress, State)>, ClientError> {
    let dir = profile.directory();
    tracing::info!("Unzipping to {:?}", &dir);
    let mut zip_file = std::fs::File::open(dir.join(DOWNLOAD_FILE))?;

    let mut archive = zip::ZipArchive::new(&mut zip_file)?;

    // Delete all assets to ensure that no obsolete assets will remain.
    let assets = profile.directory().join("assets");
    if assets.exists() {
        std::fs::remove_dir_all(assets)?;
    }

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = profile
            .directory()
            .join(file.enclosed_name().ok_or(ClientError::ArchiveError)?);

        if file.is_dir() {
            std::fs::create_dir_all(path)?;
        } else {
            let mut target = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)?;

            std::io::copy(&mut file, &mut target)?;
        }
    }

    // Delete downloaded zip
    tracing::trace!("Extracted files, deleting zip archive.");
    tokio::fs::remove_file(profile.directory().join(DOWNLOAD_FILE)).await?;

    Ok(Some((Progress::Evaluating, State::FinalCleanup(profile))))
}

// compares if files are different and prepares next steps
async fn filter_missings(
    profile: Profile,
    cds: Vec<CentralDirectoryHeader>,
    file_infos: Vec<FileInformation>,
) -> Result<Option<(Progress, State)>, ClientError> {
    tracing::debug!("filter_missings");
    let mut compared = prepare_local_with_remote(cds, file_infos);

    tracing::debug!("need to download {} files", compared.needs_redownload.len());
    tracing::debug!("need to delete {} files", compared.needs_deletion_total);
    tracing::trace!("{} bytes clean", compared.clean_data_total);

    use std::convert::TryFrom;
    if compared.needs_redownload.iter().any(|cd| {
        !matches!(
            CompressionMethod::try_from(cd.fixed.compression_method),
            Ok(CompressionMethod::Deflated) | Ok(CompressionMethod::Stored)
        )
    }) {
        tracing::warn!(
            "unsupported compression method found, falling back to classic mode"
        );
        return fallback(&profile, false);
    }

    const MAX_FILES_FOR_PARTIAL_DOWNLOAD: usize = 200;
    if compared.needs_redownload.len() > MAX_FILES_FOR_PARTIAL_DOWNLOAD {
        tracing::info!(
            "too many files changed to make partial download efficient, falling back to \
             classic mode"
        );
        return fallback(&profile, false);
    }

    if compared.needs_redownload.is_empty() && compared.needs_deletion_total == 0 {
        // already up to date
        return Ok(Some((Progress::Evaluating, State::FinalCleanup(profile))));
    }

    match remote_zip::next_partial(&profile, &mut compared) {
        Some(first_partially) => {
            let content = match &first_partially {
                Download::Start(_, _, c, _) => c,
                _ => unreachable!(),
            };

            let progress = ProgressData::new(
                StepProgress::new(compared.needs_redownload_bytes, content.clone()),
                Default::default(),
            );
            let afterburner = FuturesUnordered::new();
            afterburner.push(first_partially.progress().boxed());

            Ok(Some((
                Progress::ReadyToDownload,
                State::DownloadingPartially(
                    profile,
                    compared,
                    vec![],
                    afterburner,
                    progress,
                ),
            )))
        },
        None => Ok(Some((
            Progress::ReadyToDownload,
            State::RemovingPartially(profile, compared),
        ))),
    }
}

// downloads partial info
async fn downloading_partial(
    profile: Profile,
    mut compared: Compared,
    mut finished: Vec<(BytesMut, CentralDirectoryHeader, LocalFileHeader)>,
    mut afterburner: Afterburner,
    mut progress: ProgressData,
) -> Result<Option<(Progress, State)>, ClientError> {
    use futures::stream::StreamExt;
    const DOWNLOADS_IN_QUEUE_SPEEDUP: usize = 15;
    // we can max finish 1 download each fn call, so its okay to only add 1 download.
    if afterburner.len() < DOWNLOADS_IN_QUEUE_SPEEDUP {
        if let Some(next_partially) = remote_zip::next_partial(&profile, &mut compared) {
            afterburner.push(next_partially.progress().boxed());
        }
    }

    let download = afterburner
        .next()
        .await
        .expect("There should be at least 1 entry to be downloaded")?;

    match download {
        Download::Finished(s, r) => {
            let mut bytes = match s {
                Storage::Memory(b) => b,
                _ => unreachable!(),
            };

            let local_header = match LocalFileHeader::from_buf(&mut bytes) {
                Ok(lh) => lh,
                Err(_) => return fallback(&profile, false),
            };
            if !local_header.is_valid_signature() {
                return fallback(&profile, false);
            }
            finished.push((bytes, r, local_header));

            if !afterburner.is_empty() {
                Ok(Some((
                    Progress::InProgress(progress.clone()),
                    State::DownloadingPartially(
                        profile,
                        compared,
                        finished,
                        afterburner,
                        progress,
                    ),
                )))
            } else {
                let total = finished
                    .iter()
                    .map(|e| e.1.fixed.compressed_size as u64)
                    .sum();
                let pr = ProgressData::new(
                    StepProgress::new(total, UpdateContent::Decompress("".to_string())),
                    Default::default(),
                );
                tracing::info!("unzipping files");
                Ok(Some((
                    Progress::InProgress(progress),
                    State::UnzipPartial(profile, compared, finished, pr),
                )))
            }
        },
        Download::Progress(_r, _s, mut p, _p) => {
            progress.add_from_step(&mut p);

            afterburner.push(Download::Progress(_r, _s, p, _p).progress().boxed());
            Ok(Some((
                Progress::InProgress(progress.clone()),
                State::DownloadingPartially(
                    profile,
                    compared,
                    finished,
                    afterburner,
                    progress,
                ),
            )))
        },
        Download::Start(_, _, _, _) => unreachable!(),
    }
}

// downloads partial info
async fn unzip_partial(
    profile: Profile,
    compared: Compared,
    mut finished: Vec<(BytesMut, CentralDirectoryHeader, LocalFileHeader)>,
    mut progress: ProgressData,
) -> Result<Option<(Progress, State)>, ClientError> {
    match finished.pop() {
        Some((rbytes, remote, _)) => {
            let remote_file_size = remote.fixed.compressed_size as usize;
            if remote_file_size > rbytes.remaining() {
                tracing::warn!(
                    "Actually xMAC guessed wrong with the 1400 extra bytes, ping him \
                     please"
                );
                return fallback(&profile, false);
            }

            let filename = String::from_utf8_lossy(&remote.file_name);

            let path = profile.directory().join(filename.to_string());
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
                match CompressionMethod::try_from(remote.fixed.compression_method) {
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
                    _ => return fallback(&profile, false),
                };

            let mut file = file.await.unwrap()?;
            // TODO: evaluate splitting this up
            file.write_all_buf(&mut file_data).await?;

            progress.add_chunk(remote_file_size as u64);
            progress.cur_step_mut().content =
                UpdateContent::Decompress(filename.to_string());

            Ok(Some((
                Progress::InProgress(progress.clone()),
                State::UnzipPartial(profile, compared, finished, progress),
            )))
        },
        None => {
            tracing::info!("deleting files that should be removed");
            Ok(Some((
                Progress::InProgress(progress),
                State::RemovingPartially(profile, compared),
            )))
        },
    }
}

// remove old files
async fn removing_partial(
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
                State::RemovingPartially(profile, compared),
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
