use bytes::{Bytes, BytesMut};
use reqwest::header::RANGE;
use thiserror::Error;
use zip_core::{
    Signature,
    raw::{
        CentralDirectoryHeader, EndOfCentralDirectory, EndOfCentralDirectoryFixed,
        parse::{Parse, find_next_signature},
    },
};

use crate::{GITHUB_CLIENT, profiles::Profile};

use super::{
    State,
    compare::Compared,
    download::{Download, ProgressData, StepProgress, Storage, UpdateContent},
};

#[derive(Debug, Error)]
pub(super) enum RemoteZipError {
    #[error("Reqwest Error: ")]
    Reqwest(#[from] reqwest::Error),
    #[error("Remote Zip invalid, no central Directory Found")]
    NoCentralDirectoryFound,
}

const APPROX_MTU: u64 = 1400;

pub(super) async fn download_eocd(
    content_length: u64,
    url: &str,
) -> Result<EndOfCentralDirectory, RemoteZipError> {
    let approx_eocd_start = content_length.saturating_sub(APPROX_MTU);
    let range = format!("bytes={}-{}", approx_eocd_start, content_length);
    let eocd_res = GITHUB_CLIENT.get(url).header(RANGE, range).send().await?;
    let eocd_bytes = eocd_res.bytes().await?;

    let pos = find_next_signature(
        &eocd_bytes,
        EndOfCentralDirectoryFixed::END_OF_CENTRAL_DIR_SIGNATURE.to_le_bytes(),
    );

    let mut end_of_central_dir = None;
    if let Some(pos) = pos {
        let mut buf = &eocd_bytes[pos..];
        if let Ok(eocd) = EndOfCentralDirectory::from_buf(&mut buf) {
            end_of_central_dir = Some(eocd);
        };
    }

    end_of_central_dir.ok_or(RemoteZipError::NoCentralDirectoryFound)
}

pub(super) fn download_cds(eocd: &EndOfCentralDirectory, url: &str) -> Download<()> {
    let cd_start = eocd
        .fixed
        .offset_of_start_of_central_directory_with_respect_to_the_starting_disk_number;
    let cd_end = cd_start.saturating_add(eocd.fixed.size_of_the_central_directory);
    let range = format!("bytes={}-{}", cd_start, cd_end);

    let bytes =
        BytesMut::with_capacity(eocd.fixed.size_of_the_central_directory as usize);

    let request_builder = GITHUB_CLIENT.get(url).header(RANGE, range);
    let storage = Storage::Memory(bytes);

    Download::Start(
        request_builder,
        storage,
        UpdateContent::CentralDirectory,
        (),
    )
}

pub(super) fn extract_cds(mut cd_bytes: Bytes) -> Option<Vec<CentralDirectoryHeader>> {
    let mut cds = Vec::new();
    while let Ok(cd) = CentralDirectoryHeader::from_buf(&mut cd_bytes) {
        if !cd.is_valid_signature() {
            return None;
        }
        cds.push(cd);
    }

    Some(cds)
}

pub(super) fn gen_classic(profile: Profile) -> State {
    let request_builder = GITHUB_CLIENT.get(profile.download_url());
    let storage = Storage::FileInfo(profile.download_path());

    let download =
        Download::Start(request_builder, storage, UpdateContent::DownloadFullZip, ());
    let progress = ProgressData::new(
        StepProgress::new(0, UpdateContent::DownloadFullZip),
        Default::default(),
    );
    State::DownloadingClassic(profile, download, progress)
}

pub(super) fn next_partial(
    profile: &Profile,
    compared: &mut Compared,
) -> Option<Download<CentralDirectoryHeader>> {
    compared.needs_redownload.pop().map(|remote| {
        let remote_file_size = remote.fixed.compressed_size as usize;

        const APPROX_MTU: u64 = 1400;
        let probable_end = remote.fixed.relative_offset_of_local_header as u64
            + remote_file_size as u64
            + APPROX_MTU;
        let range = format!(
            "bytes={}-{}",
            remote.fixed.relative_offset_of_local_header, probable_end
        );
        let bytes = BytesMut::with_capacity(remote.fixed.compressed_size as usize);
        let storage = Storage::Memory(bytes);

        let request_builder = GITHUB_CLIENT
            .get(profile.download_url())
            .header(RANGE, range);

        let remote_file = std::str::from_utf8(&remote.file_name)
            .unwrap_or("<unknown>")
            .to_string();

        Download::Start(
            request_builder,
            storage,
            UpdateContent::DownloadFile(remote_file),
            remote,
        )
    })
}
