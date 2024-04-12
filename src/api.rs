use std::{path::PathBuf, sync::Arc};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use include_dir::{include_dir, Dir};
use mysql_async::prelude::Queryable;
use serde::Deserialize;
use tower_cookies::{Cookie, CookieManagerLayer, Cookies, Key};
use tower_serve_static::ServeDir;
use tracing::error;

use crate::config::ConfigManager;

#[derive(Clone)]
pub struct AppState {
    pub data_dir: PathBuf,
    pub config: ConfigManager,
    pub db: mysql_async::Pool,
}

const USERNAME_HEADER: &str = "username";

async fn auth(
    State(state): State<Arc<AppState>>,
    cookies: Cookies,
    request: Request,
    next: Next,
) -> Response {
    if cookies
        .private(&Key::from(state.config.get().secret.as_bytes()))
        .get(USERNAME_HEADER)
        .is_none()
    {
        return Response::builder()
            .status(401)
            .body("Unauthorized".into_response().into_body())
            .expect("hardcoded response will be valid");
    }

    next.run(request).await
}

static ASSETS_DIR: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/web/dist");

pub fn mount(state: Arc<AppState>) -> axum::Router {
    Router::new()
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
        .route(
            "/api/login",
            post(
                |State(state): State<Arc<AppState>>,
                 cookies: Cookies,
                 Json(data): Json<LoginRequest>| async move {
                    let config = state.config.get();
                    let Some(password) = config.admins.get(&data.username) else {
                        return (StatusCode::UNAUTHORIZED, "Unauthorized");
                    };

                    let Ok(parsed_hash) = PasswordHash::new(&password) else {
                        error!("Failed to parse password hash for user '{}'", data.username);
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
                    };

                    let Ok(_) =
                        Argon2::default().verify_password(data.password.as_bytes(), &parsed_hash)
                    else {
                        return (StatusCode::UNAUTHORIZED, "Unauthorized");
                    };

                    cookies
                        .private(&Key::from(state.config.get().secret.as_bytes()))
                        .add(Cookie::new(USERNAME_HEADER, data.username));

                    (StatusCode::OK, "ok")
                },
            ),
        )
        .nest(
            "/api",
            Router::new()
                .route(
                    "/version",
                    get(|| async { concat!(env!("CARGO_PKG_VERSION"), " - ", env!("GIT_HASH")) }),
                )
                .route(
                    "/me",
                    get(
                        |State(state): State<Arc<AppState>>, cookies: Cookies| async move {
                            cookies
                                .private(&Key::from(state.config.get().secret.as_bytes()))
                                .get(USERNAME_HEADER)
                                .expect("Authentication was checked in the auth middleware")
                                .value()
                                .to_string()
                        },
                    ),
                )
                .route(
                    "/logout",
                    post(
                        |State(state): State<Arc<AppState>>, cookies: Cookies| async move {
                            let key = Key::from(state.config.get().secret.as_bytes());
                            let private_cookies = cookies.private(&key);

                            let auth_cookie = private_cookies
                                .get(USERNAME_HEADER)
                                .expect("Authentication was checked in the auth middleware");

                            private_cookies.remove(auth_cookie);

                            "ok!"
                        },
                    ),
                )
                .route("/api/admin", post(|| async { todo!() }))
                .route("/api/admin", delete(|| async { todo!() }))
                .route("/api/database", get(|| async { todo!() }))
                .route("/api/database", post(|| async { todo!() }))
                .route("/api/database", delete(|| async { todo!() }))
                .route_layer(middleware::from_fn_with_state(state.clone(), auth)),
        )
        .nest_service("/", ServeDir::new(&ASSETS_DIR))
        .layer(CookieManagerLayer::new())
        .with_state(state)
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}
