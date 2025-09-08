use crate::{
    FsStorage,
    config::{self, GithubReleaseConfig},
    error::ProcessError,
    models::Artifact,
};
use octocrab::{GitHubError, Octocrab, models::repos::Release, repos::ReleasesHandler};
use serde_json::json;
use tokio::io::AsyncReadExt;
use url::Url;

#[tracing::instrument(skip(artifacts, db))]
pub async fn process(artifacts: Vec<Artifact>, channel: String, db: &crate::Db) {
    match crate::db::actions::any_artifacts_exist(db, &artifacts).await {
        Ok(true) => tracing::warn!("Received duplicate artifacts!"),
        Err(e) => {
            tracing::error!(?e, "Error checking for duplicate artifacts in db");
            return;
        },
        _ => (),
    }

    let len = artifacts.len();
    tracing::debug!(?len, "artifacts len");
    tracing::debug!(?artifacts, "Artifacts");

    let channel = match crate::CONFIG.channels.get(&channel) {
        Some(channel) => channel,
        None => unreachable!("channel must exist in config at this point"),
    };

    for artifact in artifacts {
        if let Err(e) = transfer(artifact, channel, db).await {
            tracing::error!(?e, "Failed to transfer artifact");
        }
    }
    if let Err(e) = crate::db::actions::prune(db).await {
        tracing::error!(?e, "Pruning failed");
    }
    match reqwest::Client::builder().build() {
        Ok(client) => {
            for webhook in &channel.webhooks {
                let code = client.get(&webhook.url).send().await.map(|r| r.status());
                match code {
                    Ok(code) if code.is_success() => {},
                    Ok(code) => {
                        tracing::error!(?code, "Webhook Statuscode is not success")
                    },
                    Err(e) => tracing::error!(?e, ?webhook, "Executing Webhook failed"),
                }
            }
        },
        Err(e) => tracing::error!(?e, "Failed to create reqwuest client"),
    }
}

#[tracing::instrument(skip(channel, db))]
async fn transfer(
    mut artifact: Artifact,
    channel: &config::Channel,
    db: &crate::Db,
) -> Result<(), ProcessError> {
    use tokio::{fs::File, io::AsyncWriteExt};

    tracing::info!("Downloading...");

    let mut resp = reqwest::get(artifact.get_artifact_url()).await?;
    let mut file = File::create(&artifact.file_name).await?;
    let mut content = vec![];
    while let Some(chunk) = resp.chunk().await? {
        file.write_all(&chunk).await?;
        content.write_all(&chunk).await?;
    }
    file.sync_data().await?;

    let downloaded_hash = format!("{:x}", md5::compute(content));
    let remote_hash = get_remote_hash(&resp);

    if downloaded_hash != remote_hash {
        tracing::error!(
            ?downloaded_hash,
            ?remote_hash,
            "Hash does not match. Exiting...",
        );
        // Clean up
        tokio::fs::remove_file(&artifact.file_name).await?;
    } else {
        tracing::debug!(?downloaded_hash, "Hash matches remote hash",);
        tracing::info!("Storing...");

        FsStorage::store(&artifact).await?;
        let already_exists: serde_json::Value = json!(
            {"code": "already_exists", "field": "name", "resource": "ReleaseAsset"}
        );

        if let Some(github_release_config) = &channel.github_release_config {
            let upload_to_github_result =
                upload_to_github_release(&artifact.file_name, github_release_config)
                    .await;
            match upload_to_github_result {
                Ok(download_url) => artifact.download_uri = download_url.to_string(),
                Err(ProcessError::Octocrab(octocrab::Error::GitHub {
                    source,
                    backtrace: _,
                })) if source.errors == Some(vec![already_exists]) => {
                    // Octocrab doesnt return the downloadurl, so we rely here on the
                    // hardcoded version
                    let download_url = format!(
                        "https://github.com/{}/{}/releases/download/{}/{}",
                        github_release_config.github_repository_owner,
                        github_release_config.github_repository,
                        github_release_config.github_release,
                        artifact.file_name
                    );
                    tracing::info!(
                        ?source,
                        ?download_url,
                        "skip upload, asset already exists, was this webhook rerun?"
                    );
                    artifact.download_uri = download_url
                },
                Err(e) => tracing::error!(?e, "Couldn't upload to github"),
            }
        }

        // Update database with new information
        crate::db::actions::insert_artifact(db, &artifact).await?;
        tracing::info!("persisted artifact to db");

        // Delete obselete artifact
        tokio::fs::remove_file(&artifact.file_name).await?;
    }
    Ok(())
}

fn get_remote_hash(resp: &reqwest::Response) -> String {
    resp.headers()
        .get(reqwest::header::ETAG)
        .map(|x| x.to_str().expect("always valid ascii?"))
        .unwrap_or("REMOTE_ETAG_MISSING")
        .replace('\"', "")
}

async fn upload_to_github_release(
    file_name: &str,
    github_release_config: &GithubReleaseConfig,
) -> Result<Url, ProcessError> {
    let octocrab = Octocrab::builder()
        .personal_token(github_release_config.github_token.clone())
        .build()?;
    let repo = octocrab.repos(
        &github_release_config.github_repository_owner,
        &github_release_config.github_repository,
    );
    let release_handler = repo.releases();

    let release = get_github_release(&release_handler, github_release_config).await?;

    let file_size = tokio::fs::metadata(file_name).await?.len();
    let mut file = tokio::fs::File::open(file_name).await?;
    let mut buffer = Vec::with_capacity(file_size as usize);
    file.read_to_end(&mut buffer).await?;
    let data = bytes::Bytes::from(buffer);

    let asset = release_handler
        .upload_asset(release.id.0, file_name, data)
        .send()
        .await?;

    Ok(asset.browser_download_url)
}

///Gets the github release set in config if the release exists, otherwise creates and
/// returns it.
#[expect(clippy::needless_lifetimes)]
async fn get_github_release<'octo, 'r>(
    release_handler: &ReleasesHandler<'octo, 'r>,
    github_release_config: &'r GithubReleaseConfig,
) -> Result<Release, ProcessError> {
    let repo_get_result = release_handler
        .get_by_tag(&github_release_config.github_release)
        .await;

    match repo_get_result {
        Err(octocrab::Error::GitHub { source, backtrace }) => {
            let GitHubError { ref message, .. } = *source;
            if message == "Not Found" {
                release_handler
                    .create(&github_release_config.github_release)
                    .send()
                    .await
                    .map_err(ProcessError::Octocrab)
            } else {
                Err(ProcessError::Octocrab(octocrab::Error::GitHub {
                    source,
                    backtrace,
                }))
            }
        },
        result => result.map_err(ProcessError::Octocrab),
    }
}
