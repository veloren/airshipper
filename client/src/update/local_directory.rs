use std::path::{PathBuf, StripPrefixError};
use thiserror::Error;

use crate::ClientError;

/// Paths which should be ignored by the compare mechanism
const IGNORE_PATHS: &[&str] = &["userdata/", "screenshots/", "maps/"];

#[derive(Error, Debug)]
pub(super) enum LocalDirectoryError {
    #[error("Input/Output Error: ")]
    InputOutput(#[from] std::io::Error),
    #[error("Invalid UTF8-Filename. this code requires filenames to match UTF8")]
    InvalidUtf8Filename,
    #[error("FileName not within Root Directory, is this some escape attack?")]
    StripPrefixError(#[from] StripPrefixError),
}

impl From<LocalDirectoryError> for ClientError {
    fn from(value: LocalDirectoryError) -> Self {
        ClientError::Custom(value.to_string())
    }
}

#[derive(Clone, Debug)]
pub(super) struct LocalFileInfo {
    pub path: PathBuf,
    // with stripped prefix
    pub local_unix_path: String,
    pub crc32: u32,
}

pub(super) async fn file_infos(
    root: PathBuf,
) -> Result<Vec<LocalFileInfo>, LocalDirectoryError> {
    let mut nextdirs = Vec::new();
    let mut file_infos = Vec::new();

    let mut root_dir = tokio::fs::read_dir(&root).await?;
    while let Some(entry) = root_dir.next_entry().await? {
        let path = entry.path();
        let relative_path = path.strip_prefix(&root)?;

        if IGNORE_PATHS
            .iter()
            .any(|ignore| relative_path.starts_with(ignore))
        {
            continue;
        }

        if path.is_dir() {
            nextdirs.push(path);
        } else {
            parse_file_info(&root, path, &mut file_infos).await?;
        }
    }

    while let Some(next) = nextdirs.pop() {
        let mut dir = tokio::fs::read_dir(&next).await?;
        while let Some(entry) = dir.next_entry().await? {
            let path = entry.path();
            if path.is_dir() {
                nextdirs.push(path);
            } else {
                parse_file_info(&root, path, &mut file_infos).await?;
            }
        }
    }

    Ok(file_infos)
}

async fn parse_file_info(
    root: &PathBuf,
    path: PathBuf,
    file_infos: &mut Vec<LocalFileInfo>,
) -> Result<(), LocalDirectoryError> {
    let file_bytes = tokio::fs::read(&path).await?;
    let crc32 = crc32fast::hash(&file_bytes);
    let local_unix_path = path
        .strip_prefix(root)?
        .to_str()
        .ok_or(LocalDirectoryError::InvalidUtf8Filename)?;

    #[cfg(windows)]
    let local_unix_path = local_unix_path.replace(r#"\"#, "/");

    let local_unix_path = local_unix_path.to_string();

    file_infos.push(LocalFileInfo {
        path,
        crc32,
        local_unix_path,
    });
    Ok(())
}
