use crate::{Context, config::HOOK_TYPE, models::PipelineUpdate, webhook};
use axum::{
    Json,
    body::Body,
    extract::{FromRequestParts, State},
    http::{Response, StatusCode, request::Parts},
    response::IntoResponse,
};
use std::sync::Arc;
use tracing::*;

pub struct GitlabAuthenticated(());

#[derive(Debug)]
pub enum AuthError {
    MissingEvent,
    InvalidEvent,
    MissingSecret,
    InvalidSecret,
}

impl<S> FromRequestParts<S> for GitlabAuthenticated
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> std::result::Result<Self, Self::Rejection> {
        const GITLAB_EVENT: &str = "X-Gitlab-Event";
        const GITLAB_SECRET: &str = "X-Gitlab-Token";
        let event = parts
            .headers
            .get(GITLAB_EVENT)
            .ok_or(AuthError::MissingEvent)?;

        if event.to_str().map_err(|_| AuthError::InvalidEvent)? != HOOK_TYPE {
            return Err(AuthError::InvalidEvent);
        }

        let token = parts
            .headers
            .get(GITLAB_SECRET)
            .ok_or(AuthError::MissingSecret)?;

        let token = token.to_str().map_err(|_| AuthError::InvalidSecret)?;

        if crate::CONFIG
            .channels
            .values()
            .any(|c| c.gitlab_secret == token)
        {
            return Ok(GitlabAuthenticated(()));
        }

        Err(AuthError::InvalidSecret)
    }
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response<Body> {
        let (status, error_message) = match self {
            AuthError::MissingEvent => (StatusCode::BAD_REQUEST, "Missing Event"),
            AuthError::InvalidEvent => (StatusCode::BAD_REQUEST, "Invalid Event"),
            AuthError::MissingSecret => (StatusCode::BAD_REQUEST, "Missing Secret"),
            AuthError::InvalidSecret => (StatusCode::UNAUTHORIZED, "Invalid Secret"),
        };
        (status, error_message).into_response()
    }
}

#[tracing::instrument(skip(update, context))]
pub async fn post_pipeline_update(
    _: GitlabAuthenticated,
    State(context): State<Context>,
    Json(mut update): Json<PipelineUpdate>,
) -> impl IntoResponse {
    let pipeline_id = update.object_attributes.id;
    let _span = span!(Level::INFO, "", ?pipeline_id);
    tracing::info!("Got webhook from gitlab.com for a finished pipeline");
    if !update.early_filter() {
        tracing::trace!("early return");
        return StatusCode::OK;
    }
    //Extend payload with variables
    if let Err(e) = update.extends_variables().await {
        tracing::warn!(?e, "couldn't extend variables");
    };
    tracing::info!(?update.object_attributes.variables, "got variables");

    match update.channel() {
        Some(channel) => {
            let artifacts = update.artifacts(&channel);
            if artifacts.is_empty() {
                tracing::debug!(?channel, "Request rejected, no artifacts");
                StatusCode::OK
            } else {
                for a in &artifacts {
                    context
                        .metrics
                        .increment_artifact_upload(&a.os, &a.arch, &channel);
                }
                let c = channel.clone();
                let db = Arc::clone(&context.db);
                tokio::spawn(async move {
                    webhook::process(artifacts, channel, &db)
                        .instrument(tracing::info_span!(""))
                        .await;
                });
                context.metrics.increment_upload(&c);
                StatusCode::ACCEPTED
            }
        },
        None => {
            tracing::trace!("Request rejected, no channel");
            StatusCode::OK
        },
    }
}
