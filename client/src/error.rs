use std::panic;

#[derive(Clone, thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Error while performing filesystem operations.")]
    IoError,
    #[error("Error while performing network operations.")]
    NetworkError,
    #[error("FATAL: Failed to start GUI!")]
    IcedError,
    #[error("Failed to save/load ron data.")]
    RonError,
    #[error("Failed to parse Veloren News.")]
    RssError,
    #[error("Failed to open webbrowser.")]
    OpenerError,
    #[error("Error parsing url.")]
    UrlParseError,
    #[error("Error reading input.")]
    ReadlineError,
    #[error("Error parsing image.")]
    ImageError,
    #[error("Error performing a task.")]
    TaskError,

    #[cfg(windows)]
    #[error("FATAL: Failed to update airshipper!")]
    UpdateError,
    #[cfg(windows)]
    #[error("Failed to parse version.")]
    VersionError,

    #[error("error during update: {0}")]
    Update(String),

    #[error("{0}")]
    Custom(String),
}

impl From<String> for ClientError {
    fn from(err: String) -> Self {
        Self::Custom(err)
    }
}

impl From<rustyline::error::ReadlineError> for ClientError {
    fn from(_: rustyline::error::ReadlineError) -> Self {
        Self::ReadlineError
    }
}

macro_rules! impl_from {
    ($trait:ty, $variant:expr) => {
        impl From<$trait> for ClientError {
            fn from(err: $trait) -> Self {
                tracing::error!("{} => {}", $variant, err);
                $variant
            }
        }
    };
}
impl_from!(std::io::Error, ClientError::IoError);
impl_from!(reqwest::Error, ClientError::NetworkError);
impl_from!(ron::Error, ClientError::RonError);
impl_from!(ron::de::SpannedError, ClientError::RonError);
impl_from!(rss::Error, ClientError::RssError);
impl_from!(opener::OpenError, ClientError::OpenerError);
impl_from!(url::ParseError, ClientError::UrlParseError);
impl_from!(iced::Error, ClientError::IcedError);
impl_from!(image::error::ImageError, ClientError::ImageError);
impl_from!(tokio::task::JoinError, ClientError::TaskError);
#[cfg(windows)]
impl_from!(self_update::errors::Error, ClientError::UpdateError);
#[cfg(windows)]
impl_from!(semver::Error, ClientError::VersionError);

/// Set up panic handler to relay panics to logs file.
pub fn panic_hook() {
    let default_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        let panic_info_payload = panic_info.payload();
        let payload_string = panic_info_payload.downcast_ref::<String>();
        let reason = match payload_string {
            Some(s) => s.to_string(),
            None => {
                let payload_str = panic_info_payload.downcast_ref::<&str>();
                payload_str.unwrap_or(&"Payload is not a string")
            }
            .to_string(),
        };

        tracing::error!("Airshipper panicked: \n\n{}: {}", reason, panic_info,);

        default_hook(panic_info);
    }));
}
