//! Deals with all filesystem specific details

use crate::consts;
use ron::ser::PrettyConfig;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

lazy_static::lazy_static! {
    // Base for config, profiles, ...
    pub static ref BASE_PATH: PathBuf = base();
}

/// Returns the base path where all airshipper files like config, profiles belong.
///
/// |Platform | Example                                                       |
/// | ------- | ------------------------------------------------------------- |
/// | Linux   | /home/alice/.local/share/barapp                               |
/// | macOS   | /Users/Alice/Library/Application Support/com.Foo-Corp.Bar-App |
/// | Windows | C:\Users\Alice\AppData\Roaming                                |
fn base() -> PathBuf {
    let path = std::env::var("AIRSHIPPER_ROOT").map_or_else(
        |_| {
            dirs::data_dir()
                .expect("Couldn't locate where to put launcher data!")
                .join("airshipper")
        },
        PathBuf::from,
    );
    std::fs::create_dir_all(&path).expect("failed to create data directory!");
    path
}

pub fn base_path() -> impl std::fmt::Display {
    BASE_PATH.display()
}

pub fn get_cache_path() -> PathBuf {
    let cache_path = dirs::cache_dir()
        .expect("Couldn't find OS cache directory")
        .join(env!("CARGO_PKG_NAME"));
    std::fs::create_dir_all(&cache_path).expect("failed to create cache directory!");
    cache_path
}

pub fn verify_cache() {
    let cache_version_file = get_cache_path().join("cache_version.ron");
    match std::fs::File::open(&cache_version_file) {
        Ok(file) => match ron::de::from_reader(file) {
            Ok(cache_version) => {
                let cache_version: u8 = cache_version;
                if cache_version == consts::CACHE_VERSION {
                    tracing::debug!("Cache version matches");
                    return;
                } else {
                    tracing::debug!("Cache version doesn't match. Clearing cache");
                }
            },
            Err(e) => {
                tracing::debug!(?e, "Failed to decode cache version. Clearing cache")
            },
        },
        Err(e) => {
            tracing::debug!(
                ?e,
                "Failed to read cache version file, probably doesn't exist. Clearing \
                 cache"
            );
        },
    }
    let _ = std::fs::remove_dir_all(get_cache_path());
    // Create cache dir again
    let _ = get_cache_path();
    let cache_version =
        ron::ser::to_string_pretty(&consts::CACHE_VERSION, PrettyConfig::default())
            .expect("Failed to serialize cache version!");
    let mut file = std::fs::File::create(cache_version_file)
        .expect("Failed to create the cache version file!");
    file.write_all(cache_version.as_bytes())
        .expect("Failed to write to cache version file!");
}

/// Returns path to the file which saves the current state
pub fn savedstate_file() -> PathBuf {
    BASE_PATH.join(consts::SAVED_STATE_FILE)
}

/// Returns path to a profile while creating the folder
pub fn profile_path(profile_name: &str) -> PathBuf {
    let path = BASE_PATH.join("profiles").join(profile_name);
    std::fs::create_dir_all(&path).expect("failed to profile directory!"); // TODO
    path
}

/// Returns path to the file where the logs will be stored
pub fn log_file() -> PathBuf {
    BASE_PATH.join(consts::LOG_FILE)
}

/// Returns log-directory and log-file
pub fn log_path_file() -> (&'static Path, &'static str) {
    (&BASE_PATH, consts::LOG_FILE)
}
