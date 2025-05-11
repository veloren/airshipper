use crate::{channels::Channel, consts, fs};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;
use tracing::error;

// TODO: Support multiple profiles and manage them here.

/// Represents a version with channel, name and path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub server: Server,
    pub channel: Channel,
    // FIXME: This field is currently ignored.
    // Persisting the storage path led to problems with the snap package because
    // the directory the snap is allowed to write to changes with each new snap version.
    // Since there is currently no use in persisting this path anyway, we ignore it.
    // It is not removed either to guarantee backwards-compatibility by making sure
    // configuration files containing this field can still be successfully parsed
    #[serde(rename = "directory")]
    _directory: PathBuf,
    pub version: Option<String>,
    pub wgpu_backend: WgpuBackend,
    pub log_level: LogLevel,
    pub env_vars: String,
    // TODO: make a file-picker UI for this
    pub assets_override: Option<String>,

    #[serde(skip)]
    pub supported_wgpu_backends: Vec<WgpuBackend>,
}

const DEFAULT_PROFILE_NAME: &str = "default";
impl Default for Profile {
    fn default() -> Self {
        Profile::new(
            DEFAULT_PROFILE_NAME.to_owned(),
            Server::Production,
            Channel("weekly".to_owned()),
        )
    }
}

#[derive(
    Debug, derive_more::Display, Clone, Copy, Serialize, Deserialize, PartialEq, Eq,
)]
pub enum WgpuBackend {
    Auto,
    OpenGl,
    DX11,
    DX12,
    Metal,
    Vulkan,
}

#[cfg(target_os = "windows")]
static WGPU_BACKENDS: &[WgpuBackend] = &[
    WgpuBackend::Auto,
    WgpuBackend::DX11,
    WgpuBackend::DX12,
    WgpuBackend::Vulkan,
];

#[cfg(target_os = "linux")]
static WGPU_BACKENDS: &[WgpuBackend] = &[WgpuBackend::Auto, WgpuBackend::Vulkan];

#[cfg(target_os = "macos")]
static WGPU_BACKENDS: &[WgpuBackend] = &[WgpuBackend::Auto, WgpuBackend::Metal];

pub async fn query_wgpu_backends(process_path: &Path) -> Vec<WgpuBackend> {
    if let Some(res) = Command::new(process_path)
        .arg("list-wgpu-backends")
        .stdout(Stdio::piped())
        .output()
        .await
        .ok()
        .filter(|res| res.status.success())
    {
        let res = String::from_utf8_lossy(&res.stdout);
        res.lines()
            .filter_map(|backend| {
                Some(match backend {
                    "vulkan" => WgpuBackend::Vulkan,
                    "dx11" => WgpuBackend::DX11,
                    "dx12" => WgpuBackend::DX12,
                    "opengl" => WgpuBackend::OpenGl,
                    "metal" => WgpuBackend::Metal,
                    other => {
                        error!(?other, "Invalid list-wgpu-backends output detected");
                        return None;
                    },
                })
            })
            .chain(std::iter::once(WgpuBackend::Auto))
            .collect()
    } else {
        error!("failed to query WGPU Backends, falling back to defaults");
        WGPU_BACKENDS.to_vec()
    }
}

#[derive(
    Debug, derive_more::Display, Clone, Copy, Serialize, Deserialize, PartialEq, Eq,
)]
pub enum Server {
    Production,
    Staging,
    Test,
}

pub static SERVERS: &[Server] = &[Server::Production, Server::Staging, Server::Test];

#[derive(
    Debug,
    Default,
    derive_more::Display,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
)]
pub enum LogLevel {
    #[default]
    Default,
    Debug,
    Trace,
}

pub static LOG_LEVELS: &[LogLevel] =
    &[LogLevel::Default, LogLevel::Debug, LogLevel::Trace];

impl Server {
    pub fn url(&self) -> &str {
        match self {
            Server::Production => "https://download.veloren.net",
            Server::Staging => "https://download.staging.veloren.net",
            Server::Test => "https://download.test.veloren.net",
        }
    }
}

impl Profile {
    pub fn new(name: String, server: Server, channel: Channel) -> Self {
        Self {
            _directory: fs::profile_path(&name),
            name,
            server,
            channel,
            version: None,
            wgpu_backend: WgpuBackend::Auto,
            log_level: LogLevel::Default,
            env_vars: String::new(),
            assets_override: None,
            supported_wgpu_backends: Vec::new(),
        }
    }

    pub fn directory(&self) -> PathBuf {
        fs::profile_path(&self.name)
    }

    /// Returns path to voxygen binary.
    /// e.g. <base>/profiles/default/veloren-voxygen.exe
    pub fn voxygen_path(&self) -> PathBuf {
        self.directory().join(consts::VOXYGEN_FILE)
    }

    /// Returns path to the voxygen logs directory
    /// e.g. <base>/profiles/default/logs
    pub fn voxygen_logs_path(&self) -> PathBuf {
        self.directory().join(consts::LOGS_DIR)
    }

    /// Returns the download url for this profile
    pub fn download_url(&self) -> String {
        format!(
            "{}/latest/{}/{}/{}",
            self.server.url(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.channel
        )
    }

    pub(crate) fn version_url(&self) -> String {
        format!(
            "{}/version/{}/{}/{}",
            self.server.url(),
            std::env::consts::OS,
            std::env::consts::ARCH,
            self.channel
        )
    }

    pub(crate) fn channel_url(&self) -> String {
        format!(
            "{}/channels/{}/{}",
            self.server.url(),
            std::env::consts::OS,
            std::env::consts::ARCH,
        )
    }

    pub(crate) fn api_version_url(&self) -> String {
        format!("{}/api/version", self.server.url(),)
    }

    pub(crate) fn announcement_url(&self) -> String {
        format!("{}/announcement", self.server.url(),)
    }

    // TODO: add possibility to start the server too
    pub fn start(profile: &Profile, game_server_address: Option<&str>) -> Command {
        let mut envs = HashMap::new();
        let userdata_dir = profile.directory().join("userdata").into_os_string();
        let screenshot_dir = profile.directory().join("screenshots").into_os_string();
        let assets_dir = profile.directory().join("assets").into_os_string();

        if profile.log_level != LogLevel::Default {
            let log_level = match profile.log_level {
                LogLevel::Default => OsString::from("info"),
                LogLevel::Debug => OsString::from("debug"),
                LogLevel::Trace => OsString::from("trace"),
            };
            envs.insert("RUST_LOG", log_level);
        }

        if let Some(path) = &profile.assets_override {
            if Path::new(&path).exists() {
                envs.insert("VELOREN_ASSETS_OVERRIDE", path.into());
            } else {
                tracing::warn!("We can't find this assets override path: {}", path);
            }
        }

        envs.insert("VOXYGEN_SCREENSHOT", screenshot_dir);
        envs.insert("VELOREN_USERDATA", userdata_dir);
        envs.insert("VELOREN_ASSETS", assets_dir);

        if profile.wgpu_backend != WgpuBackend::Auto {
            let wgpu_backend = match profile.wgpu_backend {
                WgpuBackend::OpenGl => "gl",
                WgpuBackend::DX11 => "dx11",
                WgpuBackend::DX12 => "dx12",
                WgpuBackend::Metal => "metal",
                WgpuBackend::Vulkan => "vulkan",
                _ => unreachable!(
                    "Unsupported WgpuBackend value: {}",
                    profile.wgpu_backend
                ),
            };
            envs.insert("WGPU_BACKEND", OsString::from(wgpu_backend));
        }

        let (env_vars, env_var_errors) = parse_env_vars(&profile.env_vars);
        for err in env_var_errors {
            tracing::warn!("Environment variable error: {}", err);
        }
        for (var, value) in env_vars {
            envs.insert(var, OsString::from(value));
        }

        tracing::debug!("Launching {}", profile.voxygen_path().display());
        tracing::debug!("CWD: {:?}", profile.directory());
        tracing::debug!("ENV: {:?}", envs);

        let mut cmd = Command::new(profile.voxygen_path());
        cmd.current_dir(profile.directory());
        cmd.envs(envs);

        // If a server is selected in the server browser pass it through to Voxygen
        if let Some(game_server_address) = game_server_address {
            cmd.args(["--server", game_server_address]);
        }

        cmd
    }

    /// Returns whether the profile is ready to be started
    pub fn installed(&self) -> bool {
        self.voxygen_path().exists() && self.version.is_some()
    }

    pub fn reload_wgpu_backends(&mut self) {
        if self.installed() {
            self.supported_wgpu_backends = iced::futures::executor::block_on(
                query_wgpu_backends(&self.voxygen_path()),
            );
            let supported = |backend| self.supported_wgpu_backends.contains(&backend);
            // Update selected backend if it isn't available.
            if self.wgpu_backend != WgpuBackend::Auto && !supported(self.wgpu_backend) {
                self.wgpu_backend = match self.wgpu_backend {
                    WgpuBackend::DX11 if supported(WgpuBackend::OpenGl) => {
                        WgpuBackend::OpenGl
                    },
                    WgpuBackend::OpenGl if supported(WgpuBackend::DX11) => {
                        WgpuBackend::DX11
                    },
                    _ => WgpuBackend::Auto,
                };
            }
        } else {
            self.supported_wgpu_backends = Vec::new();
        }
    }
}

pub fn parse_env_vars(env_vars: &str) -> (Vec<(&str, &str)>, Vec<String>) {
    let env_vars = env_vars.trim();
    let mut errors = Vec::new();

    let vars = if env_vars.is_empty() {
        Vec::new()
    } else {
        env_vars
            .split(',')
            .filter_map(|var| {
                let var = var.trim();
                if let Some((key, value)) = var.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    if key.chars().count() == 0 {
                        errors.push(format!("Invalid variable '{}'", key))
                    }
                    Some((key, value))
                } else {
                    if var.chars().count() == 0 {
                        errors.push("Unnecessary ',' in variable list".to_string());
                    } else {
                        errors.push(format!(
                            "Variable '{}' has no corresponding value",
                            var
                        ));
                    }
                    None
                }
            })
            .collect()
    };
    (vars, errors)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_config() {
        let (vars, errors) = parse_env_vars("");
        assert_eq!(vars, Vec::new());
        assert!(errors.is_empty());
    }

    #[test]
    fn test_normal_config() {
        let (vars, errors) =
            parse_env_vars("       FOO=foo, BAR= bar,BAZ = baz, BAK = bak  ");
        assert_eq!(vars, vec![
            ("FOO", "foo"),
            ("BAR", "bar"),
            ("BAZ", "baz"),
            ("BAK", "bak")
        ]);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_bad_config() {
        let (vars, errors) =
            parse_env_vars("      FOO=foo,,BAR= bar,    = baz, BAK =   , EMM ");
        assert_eq!(vars, vec![
            ("FOO", "foo"),
            ("BAR", "bar"),
            ("", "baz"),
            ("BAK", "")
        ]);
        assert_eq!(errors, vec![
            "Unnecessary ',' in variable list".to_string(),
            "Invalid variable ''".to_string(),
            "Variable 'EMM' has no corresponding value".to_string()
        ]);
    }
}
