use reqwest::header::RANGE;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use zip_core::{
    Signature,
    raw::{
        CentralDirectoryHeader, EndOfCentralDirectory, EndOfCentralDirectoryFixed,
        parse::{Parse, find_next_signature},
    },
};

use crate::{ClientError, GITHUB_CLIENT, WEB_CLIENT, profiles::Profile};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RemoteFileInfo {
    pub crc32: u32,
    pub compressed_size: u32,
    pub compression_method: u16,
    pub file_name: String,
    pub start_offset: u32,
    pub end_offset: u32,
}

#[derive(Debug, Error)]
pub(super) enum RemoteError {
    #[error("Reqwest Error: ")]
    Reqwest(#[from] reqwest::Error),
    #[error("Remote Zip invalid, no EOCD found")]
    NoEocdFound,
    #[error("Content Length of Remote Zip unavailable")]
    ContentLengthUnavailable,
    #[error("Remote Zip invalid, invalid CentralDirectoryHeader signature")]
    InvalidSignature,
    #[error("Remote Zip invalid, no CentralDirectoryHeaders found")]
    NoCentralDirectoryHeaderFound,
    #[error("Remote Zip invalid, CentralDirectoryHeader has invalid file name")]
    InvalidFileName,
}

const APPROX_MTU: u64 = 1400;

pub(super) async fn version(url: String) -> Result<String, RemoteError> {
    Ok(WEB_CLIENT.get(url).send().await?.text().await?)
}

pub(super) async fn rfile_infos(
    profile: &Profile,
) -> Result<Vec<RemoteFileInfo>, RemoteError> {
    let url = profile.download_url();
    let eocd = download_eocd(&url).await?;
    let mut cds = download_cds(&eocd, &url).await?;

    cds.sort_by_key(|e| e.fixed.relative_offset_of_local_header);
    let mut next_offsets = cds
        .iter()
        .skip(1)
        .map(|cd| cd.fixed.relative_offset_of_local_header);
    let rfiles = cds.iter().map(|cd| Ok(RemoteFileInfo {
        crc32: cd.fixed.crc_32,
        compressed_size: cd.fixed.compressed_size,
        compression_method: cd.fixed.compression_method,
        file_name: String::from_utf8(cd.file_name.clone()).map_err(|_| RemoteError::InvalidFileName)?,
        start_offset: cd.fixed.relative_offset_of_local_header,
        end_offset: next_offsets.next().unwrap_or(eocd.fixed.offset_of_start_of_central_directory_with_respect_to_the_starting_disk_number),
    })).collect::<Result<Vec<_>, RemoteError>>()?;
    Ok(rfiles
        .into_iter()
        .filter(|rfi| rfi.compressed_size != 0)
        .collect())
}

async fn download_eocd(url: &str) -> Result<EndOfCentralDirectory, RemoteError> {
    let content_length = GITHUB_CLIENT
        .head(url)
        .send()
        .await?
        .content_length()
        .ok_or(RemoteError::ContentLengthUnavailable)?;

    let approx_eocd_start = content_length.saturating_sub(APPROX_MTU);
    let range = format!("bytes={}-{}", approx_eocd_start, content_length);
    let eocd_res = GITHUB_CLIENT.get(url).header(RANGE, range).send().await?;
    let eocd_bytes = eocd_res.bytes().await?;

    let pos = find_next_signature(
        &eocd_bytes,
        EndOfCentralDirectoryFixed::END_OF_CENTRAL_DIR_SIGNATURE.to_le_bytes(),
    )
    .ok_or(RemoteError::NoEocdFound)?;
    let mut buf = &eocd_bytes[pos..];
    EndOfCentralDirectory::from_buf(&mut buf).map_err(|_| RemoteError::NoEocdFound)
}

async fn download_cds(
    eocd: &EndOfCentralDirectory,
    url: &str,
) -> Result<Vec<CentralDirectoryHeader>, RemoteError> {
    let cd_start = eocd
        .fixed
        .offset_of_start_of_central_directory_with_respect_to_the_starting_disk_number;
    let cd_end = cd_start.saturating_add(eocd.fixed.size_of_the_central_directory);
    let range = format!("bytes={}-{}", cd_start, cd_end);

    let cds_res = GITHUB_CLIENT.get(url).header(RANGE, range).send().await?;
    let cds_bytes = cds_res.bytes().await?;

    let mut buf = &cds_bytes[..];
    let mut cds = Vec::new();
    while let Ok(cd) = CentralDirectoryHeader::from_buf(&mut buf) {
        if !cd.is_valid_signature() {
            return Err(RemoteError::InvalidSignature);
        }
        cds.push(cd);
    }

    if cds.is_empty() {
        return Err(RemoteError::NoCentralDirectoryHeaderFound);
    }

    Ok(cds)
}

impl From<RemoteError> for ClientError {
    fn from(value: RemoteError) -> Self {
        match value {
            RemoteError::Reqwest(_) => ClientError::NetworkError,
            e => ClientError::Custom(e.to_string()),
        }
    }
}
