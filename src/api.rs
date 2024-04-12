use std::{path::PathBuf, sync::Arc};

use axum::{
    extract::State,
    routing::{delete, get, post},
    Router,
};
use mysql_async::prelude::Queryable;

use crate::config::ConfigManager;

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub config: ConfigManager,
    pub db: mysql_async::Pool,
}

pub fn mount(state: Arc<AppState>) -> axum::Router {
    Router::new()
        .route("/", get(|| async { "Cityscale!" }))
        .route(
            "/sql",
            get(|State(state): State<Arc<AppState>>| async move {
                let i: Vec<u64> = state
                    .db
                    .get_conn()
                    .await
                    .unwrap()
                    .query("SELECT 1;")
                    .await
                    .unwrap();

                // TODO: Proper Drizzle spec stuff
                "ok!"
            }),
        )
        // TODO: Authentication
        .route(
            "/api/version",
            get(|| async { concat!(env!("CARGO_PKG_VERSION"), " - ", env!("GIT_HASH")) }),
        )
        .route("/api/admin", get(|| async { todo!() }))
        .route("/api/admin", post(|| async { todo!() }))
        .route("/api/admin", delete(|| async { todo!() }))
        .route("/api/database", get(|| async { todo!() }))
        .route("/api/database", post(|| async { todo!() }))
        .route("/api/database", delete(|| async { todo!() }))
        .with_state(state)
}
