pub const SUPPORTED_SERVER_API_VERSION: u32 = 1;
pub const CACHE_VERSION: u8 = 1;

// Filesystem

#[cfg(windows)]
pub const VOXYGEN_FILE: &str = "veloren-voxygen.exe";
#[cfg(unix)]
pub const VOXYGEN_FILE: &str = "veloren-voxygen";

#[cfg(windows)]
pub const LOGS_DIR: &str = "userdata\\voxygen\\logs";

#[cfg(unix)]
pub const LOGS_DIR: &str = "userdata/voxygen/logs";

//#[cfg(windows)]
//pub const SERVER_CLI_FILE: &str = "veloren-server-cli.exe";
#[cfg(unix)]
pub const SERVER_CLI_FILE: &str = "veloren-server-cli";

pub const SAVED_STATE_FILE: &str = "airshipper_state.ron";
pub const LOG_FILE: &str = "airshipper.log";

// Networking

// For querying
pub const CHANGELOG_URL: &str =
    "https://gitlab.com/veloren/veloren/raw/{tag}/CHANGELOG.md";
// For user linking
pub const NEWS_URL: &str = "https://veloren.net/rss.xml";

pub const COMMUNITY_SHOWCASE_URL: &str = "https://veloren.net/community-showcase/rss.xml";

pub const GITLAB_MERGED_MR_URL: &str =
    "https://gitlab.com/veloren/veloren/-/merge_requests?scope=all&sort=merged_at_desc&state=merged";

pub const AIRSHIPPER_RELEASE_URL: &str =
    "https://gitlab.com/veloren/airshipper/-/releases";

pub const OFFICIAL_AUTH_SERVER: &str = "https://auth.veloren.net";

pub const OFFICIAL_SERVER_LIST: &str = "https://serverlist.veloren.net";

pub const GITLAB_SERVER_BROWSER_URL: &str =
    "https://gitlab.com/veloren/serverbrowser#inclusion-of-new-servers-to-the-list";
