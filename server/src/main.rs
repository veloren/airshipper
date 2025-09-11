// How to send manual webhooks:
// curl --header "Content-Type: application/json" --request POST --data "@<FILE_WITH_WEBHOOK_DATA>" --header "X-Gitlab-Event: Pipeline Hook" --header "X-Gitlab-Token: <TOKEN>" http://<ADDRESS>

mod config;
mod db;
mod error;
mod logger;
mod metrics;
mod models;
mod routes;
mod webhook;

use axum::{
    Router,
    body::Body,
    extract::{MatchedPath, Request, State},
    http::Response,
    middleware::{self, Next},
    routing::{get, post},
};
use config::{CONFIG_PATH, Config, LOCAL_STORAGE_PATH, loading};
use db::{Db, FsStorage};
use metrics::Metrics;
use std::{net::SocketAddr, path::Path, sync::Arc};

lazy_static::lazy_static! {
    /// Contains all configuration needed.
    static ref CONFIG: Config = Config::compile(loading::Config::load(Path::new(CONFIG_PATH)).unwrap_or_else(|_| panic!("Couldn't open config file {}", CONFIG_PATH))).unwrap();
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let _guard = Arc::new(logger::init(tracing::metadata::LevelFilter::INFO));

    let local_storage_folder = CONFIG.get_local_storage_path();
    if !local_storage_folder.exists() {
        std::fs::create_dir_all(local_storage_folder.clone()).unwrap();
    }

    tracing::info!("Starting veloren airshipper");

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(server());
    tracing::info!("server stopped");

    Ok(())
}

#[derive(Clone)]
struct Context {
    /// Prometheus metrics
    pub metrics: Arc<Metrics>,
    pub db: Arc<Db>,
}

async fn server() {
    tracing::debug!("Starting up server");

    let metrics = Metrics::new().expect("Failed to create prometheus statistics!");
    let metrics = Arc::new(metrics);

    let context = Context {
        metrics: Arc::clone(&metrics),
        db: Arc::new(
            Db::new(
                crate::CONFIG
                    .get_db_file_path()
                    .as_path()
                    .to_str()
                    .expect("non-UTF8 path"),
            )
            .await
            .unwrap(),
        ),
    };

    let lsp = format!("/{}", LOCAL_STORAGE_PATH);
    let local_storage_folder = CONFIG.get_local_storage_path();
    if !local_storage_folder.exists() {
        tokio::fs::create_dir_all(local_storage_folder.clone())
            .await
            .unwrap();
    }

    async fn empty() {}

    // build our application with a route
    let app = Router::new()
        .route("/metrics", get(routes::metrics::metrics))
        .route("/gitlab", post(routes::gitlab::post_pipeline_update))
        .route("/api/version", get(routes::api::api_version))
        .route("/announcement", get(routes::api::announcement))
        .route("/channels/{os}/{arch}", get(routes::api::channels))
        .route("/version/{os}/{arch}/{channel}", get(routes::api::version))
        .route("/latest/{os}/{arch}/{channel}", get(routes::api::download))
        .route("/", get(routes::user::index))
        .route("/health", get(empty))
        .route("/favicon.ico", get(empty))
        .route("/robots.txt", get(routes::user::robots))
        .nest_service(
            &lsp,
            tower_http::services::ServeDir::new(local_storage_folder),
        )
        .route_layer(middleware::from_fn_with_state(metrics, track_metrics))
        .with_state(context);

    // run our app with hyper
    // `axum::Server` is a re-export of `hyper::Server`
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    tracing::debug!(?addr, "listening");
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("can't bind to web-port.");
    tracing::info!(?addr, "listening on");
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );
    server.await.unwrap();
    tracing::debug!("Shutdown server");
}

async fn track_metrics(
    State(metrics): State<Arc<Metrics>>,
    req: Request,
    next: Next,
) -> Response<Body> {
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    metrics.increment_http_routes_in(&path);

    next.run(req).await
}
