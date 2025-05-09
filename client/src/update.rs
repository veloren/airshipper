use std::time::Duration;

use crate::{WEB_CLIENT, profiles::Profile};
use futures_util::{Stream, stream};
use remozipsy::{
    FileSystem, ProgressDetails, RemoteZip, Statemachine, reqwest::ReqwestRemoteZip,
    tokio::TokioLocalStorage,
};

#[derive(Debug)]
pub(crate) enum Progress {
    Offline,
    /// If the consumer sees ReadyToSync a download is necessary, but they can
    /// implement logic to avoid any download
    ReadyToSync(Profile),
    // Status from remozipsy
    DownloadExtracting {
        download: ProgressDetails,
        unzip: ProgressDetails,
    },
    Deleting(ProgressDetails),
    Successful(Profile),
    Errored(
        remozipsy::Error<
            <ReqwestRemoteZip<reqwest::Client> as RemoteZip>::Error,
            <TokioLocalStorage as FileSystem>::Error,
        >,
    ),
}

#[derive(Debug)]
#[allow(private_interfaces)]
pub(super) enum State {
    ToBeEvaluated(Profile),
    Sync(
        Profile,
        Option<ReqwestRemoteZip<reqwest::Client>>,
        Statemachine<ReqwestRemoteZip<reqwest::Client>, TokioLocalStorage>,
    ),
    /// in case its finished early while evaluating
    Finished,
}

pub(crate) fn update(p: Profile) -> impl Stream<Item = Progress> {
    tracing::debug!("start updating");
    stream::unfold(State::ToBeEvaluated(p), |old_state| old_state.progress())
}

async fn version(url: String) -> Result<String, reqwest::Error> {
    Ok(WEB_CLIENT.get(url).send().await?.text().await?)
}

impl State {
    pub(crate) async fn progress(self) -> Option<(Progress, Self)> {
        tokio::time::sleep(Duration::from_millis(5)).await;
        match self {
            State::ToBeEvaluated(profile) => evaluate(profile).await,
            State::Sync(profile, remote, statemachine) => {
                sync(profile, remote, statemachine).await
            },
            State::Finished => None,
        }
    }
}

// checks if an update is necessary
async fn evaluate(mut profile: Profile) -> Option<(Progress, State)> {
    tracing::info!("Evaluating remote version...");
    let remote_version = match version(profile.version_url()).await {
        Ok(ok) => ok,
        Err(_) => return Some((Progress::Offline, State::Finished)),
    };
    let versions_match = Some(remote_version.clone()) == profile.version;

    if !versions_match {
        tracing::info!("Versions do not match. Fetching remote file infos...");
        profile.version = Some(remote_version);

        let Ok(remote) = ReqwestRemoteZip::with_url(profile.download_url()) else {
            return Some((Progress::Offline, State::Finished));
        };
        let local = TokioLocalStorage::new(profile.directory(), vec![]);
        let config = remozipsy::Config::default();
        let statemachine = Statemachine::new(remote.clone(), local, config);

        Some((
            Progress::ReadyToSync(profile.clone()),
            State::Sync(profile, Some(remote), statemachine),
        ))
    } else {
        Some((Progress::Successful(profile), State::Finished))
    }
}

// checks if an update is necessary
async fn sync(
    profile: Profile,
    mut remote: Option<ReqwestRemoteZip<reqwest::Client>>,
    statemachine: Statemachine<ReqwestRemoteZip<reqwest::Client>, TokioLocalStorage>,
) -> Option<(Progress, State)> {
    statemachine.progress().await.map(|(p, s)| match p {
        remozipsy::Progress::DownloadExtracting { download, unzip } => {
            if let Some(_remote) = remote.take() {
                //profile.rfiles = remote
            };
            (
                Progress::DownloadExtracting { download, unzip },
                State::Sync(profile, None, s),
            )
        },
        remozipsy::Progress::Deleting(deleting) => {
            (Progress::Deleting(deleting), State::Sync(profile, None, s))
        },
        remozipsy::Progress::Successful => {
            (Progress::Successful(profile.clone()), State::Finished)
        },
        remozipsy::Progress::Errored(e) => (Progress::Errored(e), State::Finished),
    })
}
