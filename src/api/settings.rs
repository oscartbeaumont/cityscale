use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use tower_cookies::{Cookies, Key};
use tracing::error;

use super::{AppState, USERNAME_HEADER};

pub fn mount() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/admin",
            get(|State(state): State<Arc<AppState>>, cookies: Cookies| async move {
                let my_username = cookies
                    .private(&Key::from(state.config.get().secret.as_bytes()))
                    .get(USERNAME_HEADER)
                    .expect("checked in auth middleware")
                    .value()
                    .to_string();

                let config = state.config.get();
                Json(config.admins.iter().map(|(username, _)| json!({
                    "username": username.clone(),
                    "is_self": *username == my_username
                })).collect::<Vec<_>>())
            }),
        )
        .route(
            "/admin",
            post(|State(state): State<Arc<AppState>>, Json(data): Json<CreateUserRequest>| async move {               
                let mut config = state.config.edit();
                config.admins.insert(data.username, data.password);

                if config.commit().map_err(|err| error!("Error saving config: {err:?}")).is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit changes!").into_response();
                }

                StatusCode::CREATED.into_response()
            }),
        )
        .route(
            "/admin/:username",
            delete(|State(state): State<Arc<AppState>>, cookies: Cookies, Path(username): Path<String>| async move {
                let my_username = cookies
                    .private(&Key::from(state.config.get().secret.as_bytes()))
                    .get(USERNAME_HEADER)
                    .expect("checked in auth middleware")
                    .to_string();
                if my_username == username {
                    return (StatusCode::FORBIDDEN, "You cannot delete yourself!").into_response();
                }
                
                let mut config = state.config.edit();
                config.admins.remove(&username);

                if config.commit().map_err(|err| error!("Error saving config: {err:?}")).is_err() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to commit changes!").into_response();
                }

                StatusCode::NO_CONTENT.into_response()
            }),
        )
}

#[derive(Deserialize)]
struct CreateUserRequest {
    username: String,
    password: String,
}
