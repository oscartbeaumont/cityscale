use std::path::PathBuf;

use axum::{routing::get, Router};

use crate::config::ConfigManager;

pub struct State {
    pub data_dir: PathBuf,
    pub config: ConfigManager,
}

pub fn mount(state: State) -> axum::Router {
    Router::new()
        .route("/", get(|| async { "Cityscale!" }))
        .route(
            "/api/version",
            get(|| async { concat!(env!("CARGO_PKG_VERSION"), " - ", env!("GIT_HASH")) }),
        )
}
