use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::read::DeflateDecoder;
use reqwest::{StatusCode, header::RANGE};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::{convert::TryFrom, io::Read, path::PathBuf, time::Duration};
use thiserror::Error;
use tokio::{
    io::AsyncWriteExt, sync::mpsc::UnboundedSender, task::JoinHandle, time::Instant,
};
use zip_core::{
    Signature,
    raw::{LocalFileHeader, parse::Parse},
    structs::CompressionMethod,
};

use super::{local_directory::LocalFileInfo, remote::RemoteFileInfo};
use crate::{GITHUB_CLIENT, error::ClientError};
#[cfg(unix)]
use crate::{
    consts::{SERVER_CLI_FILE, VOXYGEN_FILE},
    nix,
};

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total_bytes: u64,
    pub processed_bytes: u64,
    last_rate_check: Instant,
    downloaded_since_last_check: u64,
    pub bytes_per_sec: u64,
}

impl DownloadProgress {
    pub fn new(total_bytes: u64) -> Self {
        Self {
            total_bytes,
            processed_bytes: 0,
            last_rate_check: Instant::now(),
            downloaded_since_last_check: 0,
            bytes_per_sec: 0,
        }
    }

    pub fn add_chunk(&mut self, data: u64) {
        self.processed_bytes += data;
        self.downloaded_since_last_check += data;

        let current_time = Instant::now();
        let since_last_check = current_time - self.last_rate_check;
        let since_last_check_f32 = since_last_check.as_secs_f32();
        if since_last_check >= Duration::from_millis(500)
            || (since_last_check_f32 > 0.0 && self.bytes_per_sec == 0)
        {
            let bytes_per_sec =
                (self.downloaded_since_last_check as f32 / since_last_check_f32) as u64;
            self.downloaded_since_last_check = 0;
            self.last_rate_check = current_time;
            if self.bytes_per_sec == 0 {
                self.bytes_per_sec = bytes_per_sec;
            } else {
                self.bytes_per_sec = (self.bytes_per_sec * 3 + bytes_per_sec) / 4;
            }
        }
    }

    pub fn percent_complete(&self) -> u64 {
        self.processed_bytes * 100 / self.total_bytes
    }

    pub fn time_remaining(&self) -> Duration {
        if self.processed_bytes > self.total_bytes {
            let process = &self;
            tracing::warn!(
                ?process,
                "Processed Bytes is larger than Total Bytes, something seems off"
            );
        }

        Duration::from_secs_f32(
            (self.total_bytes.saturating_sub(self.processed_bytes)) as f32
                / self.bytes_per_sec.max(1) as f32,
        )
    }
}

#[derive(Debug, Error)]
pub(super) enum SyncError {
    #[error("Reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Non-OK Status: {0}")]
    InvalidStatus(StatusCode),
    #[error("I/O Error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Download batch cannot be empty!")]
    EmptyDownload,
    #[error("Invalid local header signature")]
    InvalidLocalHeaderSignature,
    #[error("zip-core parse error: {0}")]
    ParseError(#[from] zip_core::raw::parse::DynamicSizeError),
    #[error("The remote file hash doesn't match its calculated one")]
    InvalidHash,
    #[error("Tokio mpsc error: {0}")]
    SendFailure(#[from] tokio::sync::mpsc::error::SendError<u64>),
    #[error("Unsupported compression method found")]
    UnsupportedCompressionMethod,
    #[error("Tokio join error: {0}")]
    JoinFailure(#[from] tokio::task::JoinError),
    #[error("The calculated byte range to download a batch is inaccurate")]
    WrongDownloadRange,
    #[error("The bytes length passed to unzip doesn't match the file size")]
    WrongBytesLength,
    #[error("{0}")]
    ClientError(#[from] ClientError),
}

impl From<SyncError> for ClientError {
    fn from(value: SyncError) -> Self {
        match value {
            SyncError::InvalidStatus(_) => {
                let err = ClientError::NetworkError;
                tracing::error!("{} => {}", value, err);
                err
            },
            SyncError::Reqwest(e) => e.into(),
            SyncError::FileError(e) => e.into(),
            e => ClientError::Custom(e.to_string()),
        }
    }
}

pub(super) type DeleteResult = Result<(), SyncError>;
pub(super) type UnzipResult = Result<Option<(u32, String)>, SyncError>;
pub(super) type DownloadResult = Result<Vec<JoinHandle<UnzipResult>>, SyncError>;

pub(super) async fn download_batch(
    url: String,
    mut batch: Vec<RemoteFileInfo>,
    dir: PathBuf,
    tx: UnboundedSender<u64>,
) -> DownloadResult {
    if batch.is_empty() {
        // do not.
        return Err(SyncError::EmptyDownload);
    }

    let mut iter = batch.iter().peekable();
    let end_offset = iter.peek().unwrap().end_offset;
    let start_offset = iter.last().unwrap().start_offset;

    let range = format!("bytes={}-{}", start_offset, end_offset);
    let request = GITHUB_CLIENT.get(url).header(RANGE, range);
    let mut response = request.send().await?;

    if !response.status().is_success() {
        return Err(SyncError::InvalidStatus(response.status()));
    }

    let mut storage = BytesMut::with_capacity((end_offset - start_offset) as usize);
    let mut unzip_handles = Vec::new();
    let mut consumed: usize = 0;
    let mut next_rfile = batch.pop().unwrap();

    // Since we don't want to leave JoinHandles dangling without being awaited
    // (if the program terminates before a task completes, said task would be
    // cancelled mid-processing which would not be ideal),
    // instead of using the normal ? operator for errors we have to await all
    // the handles first. Ergo, this mess.
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                if let Err(e) = tx.send(chunk.len() as u64) {
                    for handle in unzip_handles.into_iter() {
                        let _ = handle.await;
                    }
                    return Err(e.into());
                }
                storage.put(chunk);
                loop {
                    if (next_rfile.start_offset - start_offset) as usize == consumed {
                        let full_size: usize =
                            (next_rfile.end_offset - next_rfile.start_offset) as usize;
                        if full_size <= storage.len() {
                            let mut bytes = storage.split_to(full_size);
                            consumed += full_size;
                            match LocalFileHeader::from_buf(&mut bytes) {
                                Err(e) => {
                                    for handle in unzip_handles.into_iter() {
                                        let _ = handle.await;
                                    }
                                    return Err(e.into());
                                },
                                Ok(header) => {
                                    if header.is_valid_signature() {
                                        // Why are there 16 extra bytes at the end here
                                        // that we don't need? beats me
                                        unzip_handles.push(tokio::spawn(unzip_file(
                                            bytes.split_to(
                                                next_rfile.compressed_size as usize,
                                            ),
                                            next_rfile,
                                            dir.clone(),
                                        )));
                                        if let Some(next) = batch.pop() {
                                            next_rfile = next;
                                        } else {
                                            return Ok(unzip_handles);
                                        }
                                    } else {
                                        for handle in unzip_handles.into_iter() {
                                            let _ = handle.await;
                                        }
                                        return Err(
                                            SyncError::InvalidLocalHeaderSignature,
                                        );
                                    }
                                },
                            }
                        } else {
                            break;
                        }
                    } else {
                        // it's safe to assume that
                        // (next_rfile.start_offset - start_offset) > consumed
                        // which means there's some junk to clear
                        let junk_len: usize = next_rfile.start_offset as usize
                            - start_offset as usize
                            - consumed;
                        if junk_len <= storage.len() {
                            let _ = storage.split_to(junk_len);
                            consumed += junk_len;
                        } else {
                            break;
                        }
                    }
                }
            },
            Ok(None) => {
                if batch.is_empty() {
                    return Ok(unzip_handles);
                } else {
                    // something's off
                    for handle in unzip_handles.into_iter() {
                        let _ = handle.await;
                    }
                    return Err(SyncError::WrongDownloadRange);
                }
            },
            Err(e) => {
                for handle in unzip_handles.into_iter() {
                    let _ = handle.await;
                }
                return Err(e.into());
            },
        }
    }
}

async fn unzip_file(
    mut compressed: BytesMut,
    rfile: RemoteFileInfo,
    dir: PathBuf,
) -> UnzipResult {
    if compressed.len() != rfile.compressed_size as usize {
        // something's off
        return Err(SyncError::WrongBytesLength);
    }

    let path = dir.join(&rfile.file_name);

    let parent = path.parent().unwrap();
    tokio::fs::create_dir_all(parent).await?;

    let file = tokio::spawn(tokio::fs::File::create(path.clone()));

    let mut file_data = match CompressionMethod::try_from(rfile.compression_method) {
        Ok(CompressionMethod::Deflated) => {
            let mut deflate_reader = DeflateDecoder::new(compressed.reader());
            let mut decompressed = Vec::with_capacity(rfile.compressed_size as usize);
            deflate_reader.read_to_end(&mut decompressed)?;
            bytes::Bytes::copy_from_slice(&decompressed)
        },
        Ok(CompressionMethod::Stored) => {
            compressed.copy_to_bytes(rfile.compressed_size as usize)
        },
        // should not happen at this point
        _ => {
            return Err(SyncError::UnsupportedCompressionMethod);
        },
    };

    if crc32fast::hash(&file_data) != rfile.crc32 {
        return Err(SyncError::InvalidHash);
    }

    let mut file = file.await??;
    file.write_all_buf(&mut file_data).await?;

    #[cfg(unix)]
    {
        if let SERVER_CLI_FILE | VOXYGEN_FILE = rfile.file_name.as_str() {
            if nix::is_nixos()? {
                nix::patch(&dir, rfile.file_name.as_str())?;
                let file_bytes = Bytes::copy_from_slice(&tokio::fs::read(&path).await?);
                let patch_crc32 = crc32fast::hash(&file_bytes);
                return Ok(Some((patch_crc32, rfile.file_name)));
            } else {
                let meta = tokio::fs::metadata(&path).await?;
                let mut perm = meta.permissions();
                perm.set_mode(0o755);
                tokio::fs::set_permissions(&path, perm).await?;
            }
        }
    }

    Ok(None)
}

pub(super) async fn remove_files(files: Vec<LocalFileInfo>) -> DeleteResult {
    for file in files.into_iter() {
        tokio::fs::remove_file(file.path).await?;
    }
    Ok(())
}
