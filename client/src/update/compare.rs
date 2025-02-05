use std::collections::HashMap;

use super::{local_directory::LocalFileInfo, remote::RemoteFileInfo};

#[derive(Debug)]
pub(super) struct Compared {
    pub needs_download: Vec<RemoteFileInfo>,
    pub needs_deletion: Vec<LocalFileInfo>,
    pub needs_download_bytes: u64,
    pub needs_deletion_total: u64,
    pub clean_data_total: u64,
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

    let mut needs_download = Vec::new();
    let mut needs_deletion = Vec::new();
    let mut clean_data_total = 0;

    for value in compare_map.into_values() {
        match (value.0, value.1) {
            (None, Some(remote)) => {
                needs_download.push(remote);
            },
            (Some(local), None) => {
                needs_deletion.push(local);
            },
            (Some(local), Some(remote)) => {
                if local.crc32 == remote.crc32 {
                    clean_data_total += remote.compressed_size as u64;
                } else {
                    needs_download.push(remote);
                }
            },
            (None, None) => unreachable!(),
        }
    }

    let needs_download_bytes = needs_download
        .iter()
        .map(|remote| remote.compressed_size as u64)
        .sum();
    let needs_deletion_total = needs_deletion.len() as u64;

    //reorder based by range, so that we read from low to high, in the hope that its
    // better for the remote spinning disk. but reversed, because we .pop from this Vec
    needs_download.sort_by_key(|e| e.start_offset);
    needs_download.reverse();

    Compared {
        needs_download,
        needs_deletion,
        needs_download_bytes,
        needs_deletion_total,
        clean_data_total,
    }
}
