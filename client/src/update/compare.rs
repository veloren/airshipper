use std::collections::HashMap;

use super::{local_directory::LocalFileInfo, remote::RemoteFileInfo};

#[derive(Debug)]
pub(super) struct Compared {
    pub needs_download: Vec<Vec<RemoteFileInfo>>,
    pub needs_deletion: Vec<LocalFileInfo>,
    pub needs_download_bytes: u64,
}

pub(super) fn build_compared(
    remote: Vec<RemoteFileInfo>,
    local: Vec<LocalFileInfo>,
) -> Compared {
    let mut compare_map: HashMap<
        String,
        (Option<LocalFileInfo>, Option<RemoteFileInfo>),
    > = HashMap::new();

    for l in local {
        let _ = compare_map
            .entry(l.local_unix_path.clone())
            .or_insert((Some(l), None));
    }
    for r in remote {
        let e = compare_map
            .entry(r.file_name.clone())
            .or_insert((None, None));
        e.1 = Some(r);
    }

    let mut needs_download_flat = Vec::new();
    let mut needs_deletion = Vec::new();
    let mut clean_bytes_total = 0;

    for value in compare_map.into_values() {
        match (value.0, value.1) {
            (None, Some(remote)) => {
                needs_download_flat.push(remote);
            },
            (Some(local), None) => {
                needs_deletion.push(local);
            },
            (Some(local), Some(remote)) => {
                if let Some(patch_crc32) = remote.patch_crc32 {
                    if local.crc32 == patch_crc32 {
                        clean_bytes_total += remote.compressed_size as u64;
                    } else {
                        needs_download_flat.push(remote);
                    }
                } else if local.crc32 == remote.crc32 {
                    clean_bytes_total += remote.compressed_size as u64;
                } else {
                    needs_download_flat.push(remote);
                }
            },
            (None, None) => unreachable!(),
        }
    }

    tracing::debug!(?clean_bytes_total);

    needs_download_flat.sort_by_key(|e| e.index);
    needs_download_flat.reverse();

    let mut needs_download = Vec::new();
    let mut download_batch = Vec::new();
    let mut download_peek = needs_download_flat.iter().skip(1);

    for rfi in needs_download_flat.iter() {
        download_batch.push(rfi.clone());
        if let Some(next) = download_peek.next() {
            if next.index != rfi.index - 1 {
                needs_download.push(download_batch.clone());
                download_batch = Vec::new();
            }
        } else {
            needs_download.push(download_batch.clone());
        }
    }

    let needs_download_bytes = needs_download
        .iter()
        // since batches are sorted we can take the first and last elements
        // instead of doing max and min
        .map(|batch| {
            let mut iter = batch.iter().peekable();
            iter.peek().unwrap().end_offset as u64 - iter.last().unwrap().start_offset as u64
        })
        .sum();

    Compared {
        needs_download,
        needs_deletion,
        needs_download_bytes,
    }
}
