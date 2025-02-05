use bytes::{BufMut, BytesMut};
use std::time::Duration;

use reqwest::{RequestBuilder, StatusCode, header::RANGE};
use thiserror::Error;
use tokio::time::Instant;

use super::{compare::Compared, remote::RemoteFileInfo};
use crate::{GITHUB_CLIENT, error::ClientError, profiles::Profile};

#[derive(Debug, Clone)]
pub struct StepProgress {
    pub total_bytes: u64,
    pub processed_bytes: u64,
    // internal buffer to be applied to overall progress
    buf_processed_bytes: u64,
    pub content: UpdateContent,
}

#[derive(Debug, Clone)]
pub struct OverallProgress {
    last_rate_check: Instant,
    downloaded_since_last_check: u64,
    bytes_per_sec: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ProgressData {
    cur_step: StepProgress,
    overall: OverallProgress,
}

#[derive(Debug, Clone)]
pub enum UpdateContent {
    DownloadFile(String),
    Decompress(String),
}

impl StepProgress {
    pub(crate) fn new(total_bytes: u64, content: UpdateContent) -> Self {
        Self {
            total_bytes,
            processed_bytes: 0,
            buf_processed_bytes: 0,
            content,
        }
    }

    pub(crate) fn add_chunk(&mut self, data: u64) {
        self.processed_bytes += data;
        self.buf_processed_bytes += data;
    }

    pub(crate) fn percent_complete(&self) -> u64 {
        (self.processed_bytes as f32 * 100.0 / self.total_bytes as f32) as u64
    }
}

impl Default for OverallProgress {
    fn default() -> Self {
        Self {
            last_rate_check: Instant::now(),
            downloaded_since_last_check: 0,
            bytes_per_sec: 0,
        }
    }
}

impl OverallProgress {
    fn add_from_step(&mut self, step: &mut StepProgress) -> u64 {
        let data = std::mem::take(&mut step.buf_processed_bytes);

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
        data
    }

    pub fn bytes_per_sec(&self) -> u64 {
        self.bytes_per_sec
    }
}

impl ProgressData {
    pub(crate) fn new(step: StepProgress, overall: OverallProgress) -> Self {
        Self {
            cur_step: step,
            overall,
        }
    }

    pub(crate) fn add_chunk(&mut self, data: u64) {
        self.cur_step.add_chunk(data);
        self.overall.add_from_step(&mut self.cur_step);
    }

    pub(crate) fn add_from_step(&mut self, step: &mut StepProgress) {
        // adds to local overall from other step
        let data = self.overall.add_from_step(step);
        self.cur_step.processed_bytes += data;
    }

    pub fn cur_step(&self) -> &StepProgress {
        &self.cur_step
    }

    pub fn overall(&self) -> &OverallProgress {
        &self.overall
    }

    pub(super) fn cur_step_mut(&mut self) -> &mut StepProgress {
        &mut self.cur_step
    }

    pub(crate) fn cur_step_remaining(&self) -> Duration {
        if self.cur_step.processed_bytes > self.cur_step.total_bytes {
            let process = &self;
            tracing::warn!(
                ?process,
                "Processed Bytes is larger than Total Bytes, something seems off"
            );
        }

        Duration::from_secs_f32(
            (self
                .cur_step
                .total_bytes
                .saturating_sub(self.cur_step.processed_bytes)) as f32
                / self.overall.bytes_per_sec.max(1) as f32,
        )
    }
}

pub(super) fn next_partial(
    profile: &Profile,
    compared: &mut Compared,
) -> Option<Download<RemoteFileInfo>> {
    compared.needs_download.pop().map(|remote| {
        let range = format!("bytes={}-{}", remote.start_offset, remote.end_offset);
        let storage = BytesMut::with_capacity(remote.compressed_size as usize);

        let request_builder = GITHUB_CLIENT
            .get(profile.download_url())
            .header(RANGE, range);

        Download::Start(
            request_builder,
            storage,
            UpdateContent::DownloadFile(remote.file_name.clone()),
            remote,
        )
    })
}

#[derive(Debug, Error)]
pub(super) enum DownloadError {
    #[error("Reqwest: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("Non-OK Status: {0}")]
    InvalidStatus(StatusCode),
    #[error("StorageWrite Error: {0}")]
    WriteError(#[from] std::io::Error),
}

#[derive(Debug)]
pub(super) enum Download<T> {
    Start(RequestBuilder, BytesMut, UpdateContent, T),
    Progress(reqwest::Response, BytesMut, StepProgress, T),
    Finished(BytesMut, T),
}

impl<T> Download<T> {
    /// downloads a single "thing" partially, so it can be showed in UI
    pub(super) async fn progress(self) -> Result<Self, DownloadError> {
        match self {
            Download::Start(request, storage, content, c) => {
                let response = request.send().await?;

                if !response.status().is_success() {
                    return Err(DownloadError::InvalidStatus(response.status()));
                }

                let total = response.content_length().unwrap_or_default();
                let progress = StepProgress::new(total, content);
                Ok(Self::Progress(response, storage, progress, c))
            },
            Download::Progress(mut response, mut storage, mut progress, c) => {
                match response.chunk().await? {
                    Some(chunk) => {
                        progress.add_chunk(chunk.len() as u64);
                        storage.put(chunk);
                        Ok(Self::Progress(response, storage, progress, c))
                    },
                    None => Ok(Self::Finished(storage, c)),
                }
            },
            Download::Finished(storage, c) => Ok(Download::Finished(storage, c)),
        }
    }
}

impl UpdateContent {
    pub fn show(&self) -> &str {
        match self {
            UpdateContent::DownloadFile(x) => x,
            UpdateContent::Decompress(x) => x,
        }
    }
}

impl From<DownloadError> for ClientError {
    fn from(value: DownloadError) -> Self {
        match value {
            DownloadError::InvalidStatus(_) => ClientError::NetworkError,
            DownloadError::Reqwest(_) => ClientError::NetworkError,
            DownloadError::WriteError(_) => ClientError::IoError,
        }
    }
}
