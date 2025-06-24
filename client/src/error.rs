use std::panic;

#[derive(Clone, thiserror::Error, Debug)]
pub enum ClientError {
    #[error("Error while performing filesystem operations: {0}")]
    Io(String),
    #[error("Error while performing network operations: {0}")]
    Network(String),
    #[error("FATAL: Failed to start GUI! Error: {0}")]
    Iced(String),
    #[error("Failed to save/load ron data: {0}")]
    Ron(String),
    #[error("Failed to parse Veloren News: {0}")]
    Rss(String),
    #[error("Failed to open webbrowser: {0}")]
    Opener(String),
    #[error("Error parsing url: {0}")]
    UrlParse(String),
    #[error("Error reading input: {0}")]
    Readline(String),
    #[error("Error parsing image: {0}")]
    Image(String),
    #[error("Error performing a task: {0}")]
    Task(String),
    #[error("Error while updating the game: {0}")]
    GameUpdate(String),

    #[cfg(windows)]
    #[error("FATAL: Failed to update airshipper! Error: {0}")]
    SelfUpdate(String),
    #[cfg(windows)]
    #[error("Failed to parse version: {0}")]
    Version(String),

    #[error("Error: {0}")]
    Custom(String),
}

macro_rules! impl_from {
    ($foreign:ty, $local:expr) => {
        impl From<$foreign> for ClientError {
            fn from(err: $foreign) -> Self {
                $local(err.to_string())
            }
        }
    };
}
impl_from!(std::io::Error, ClientError::Io);
impl_from!(reqwest::Error, ClientError::Network);
impl_from!(iced::Error, ClientError::Iced);
impl_from!(ron::Error, ClientError::Ron);
impl_from!(ron::de::SpannedError, ClientError::Ron);
impl_from!(rss::Error, ClientError::Rss);
impl_from!(opener::OpenError, ClientError::Opener);
impl_from!(url::ParseError, ClientError::UrlParse);
impl_from!(rustyline::error::ReadlineError, ClientError::Readline);
impl_from!(image::error::ImageError, ClientError::Image);
impl_from!(tokio::task::JoinError, ClientError::Task);
impl_from!(remozipsy::Error<
    <remozipsy::reqwest::ReqwestRemoteZip<reqwest::Client> as remozipsy::RemoteZip>::Error,
    <remozipsy::tokio::TokioLocalStorage as remozipsy::FileSystem>::Error,
>, ClientError::GameUpdate);
#[cfg(windows)]
impl_from!(self_update::errors::Error, ClientError::UpdateError);
#[cfg(windows)]
impl_from!(semver::Error, ClientError::VersionError);
impl_from!(String, ClientError::Custom);

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
