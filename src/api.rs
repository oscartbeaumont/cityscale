use std::{path::PathBuf, sync::Arc};

use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::{
    extract::{Path, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use include_dir::{include_dir, Dir};
use mysql_async::prelude::*;
use rand::distributions::{Alphanumeric, DistString};
use serde::Deserialize;
use serde_json::json;
use tower_cookies::{Cookie, CookieManagerLayer, Cookies, Key};
use tower_serve_static::ServeDir;
use tracing::{error, warn};

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
                // TODO: Error handling
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
                .route("/admin", get(|| async { todo!() }))
                .route("/admin", post(|| async { todo!() }))
                .route("/admin", delete(|| async { todo!() }))
                .route(
                    "/database",
                    get(|State(state): State<Arc<AppState>>| async move {
                        let Ok(mut conn) = state
                            .db
                            .get_conn()
                            .await
                            .map_err(|err| error!("Error getting DB connection: {err}"))
                        else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                                .into_response();
                        };

                        let Ok(dbs): Result<Vec<String>, _> = conn
                            .query("SHOW DATABASES;")
                            .await
                            .map_err(|err| error!("Error getting DB connection: {err}"))
                        else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error")
                                .into_response();
                        };

                        let dbs = dbs
                            .into_iter()
                            .filter(|name| {
                                !(name == "information_schema"
                                    || name == "mysql"
                                    || name == "performance_schema"
                                    || name == "sys")
                            })
                            .map(|name| {
                                json!({
                                    "name": name,
                                })
                            })
                            .collect::<Vec<_>>();

                        (StatusCode::OK, Json(dbs)).into_response()
                    }),
                )
                .route("/database", post(|State(state): State<Arc<AppState>>, Json(data): Json<CreateDatabaseRequest>| async move {
                    let Ok(mut conn) = state
                        .db
                        .get_conn()
                        .await
                        .map_err(|err| error!("Error getting DB connection: {err}"))
                    else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    // TODO: This is a crude way to prevent SQL injection, can we do something better here?
                    // TODO: SQL parameters are not supported in CREATE DATABASE
                    if !data.name.chars().all(|c| c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid database name").into_response();
                    }

                    let Ok(_) = format!("CREATE DATABASE `{}`", data.name)
                        .ignore(&mut conn)
                        .await
                        .map_err(|err| error!("Error creating DB '{}': {err}", data.name)) else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    (StatusCode::OK, "ok").into_response()
                }))
                .route("/database/:db", delete(|State(state): State<Arc<AppState>>, Path(db_name): Path<String>| async move {
                    let Ok(mut conn) = state
                        .db
                        .get_conn()
                        .await
                        .map_err(|err| error!("Error getting DB connection: {err}"))
                    else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    // TODO: Forbid droppping system tables

                    // TODO: This is a crude way to prevent SQL injection, can we do something better here?
                    // TODO: SQL parameters are not supported in CREATE DATABASE
                    if !db_name.chars().all(|c| c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid database name").into_response();
                    }

                    let Ok(_) = format!("DROP DATABASE `{}`", db_name)
                        .ignore(&mut conn)
                        .await
                        .map_err(|err| error!("Error dropping DB 'test': {err}")) else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    (StatusCode::OK, "ok").into_response()
                }))
                .route(
                    "/database/:db",
                    get(
                        |State(state): State<Arc<AppState>>, Path(db_name): Path<String>| async move {
                            let Ok(mut conn) = state
                                .db
                                .get_conn()
                                .await
                                .map_err(|err| error!("Error getting DB connection: {err}"))
                            else {
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Internal Server Error",
                                )
                                    .into_response();
                            };

                            let db_name = &db_name;
                            // TODO: This is a crude way to prevent SQL injection, can we do something better here?
                            // TODO: SQL parameters are not supported in CREATE DATABASE
                            if !db_name.chars().all(|c| c.is_alphanumeric()) {
                                return (StatusCode::BAD_REQUEST, "Invalid database name").into_response();
                            }

                            let Ok(_) = format!("USE `{}`", db_name)
                                .ignore(&mut conn)
                                .await
                                .map_err(|err| error!("Error selecting DB '{}': {err}", db_name)) else {
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Internal Server Error",
                                )
                                    .into_response();
                            };

                            let Ok(table_names) = "SELECT table_name FROM information_schema.tables WHERE table_type='BASE TABLE' AND table_schema = :db_name;"
                                .with(params! {
                                    "db_name" => db_name
                                })
                                .map(&mut conn, |table_name: String| table_name)
                                .await
                                .map_err(|err| error!("Error getting tables in DB '{}': {err}", db_name)) else {
                                return (
                                    StatusCode::INTERNAL_SERVER_ERROR,
                                    "Internal Server Error",
                                )
                                    .into_response();
                                };


                            let mut tables = Vec::new();
                            for table_name in table_names {
                                // TODO: Proper SQL escaping
                                if !table_name.chars().all(|c| c.is_alphanumeric()) {
                                    warn!("Found non-numeric table name '{}', skipping", table_name);
                                    continue;
                                }
                                
                                let Ok(schema) = format!("SHOW CREATE TABLE {db_name};")
                                    .map(&mut conn, |(_, table_name): (String, String)| table_name)
                                    .await
                                    .map_err(|err| error!("Error getting table schema in DB '{}': {err}", db_name)) else {
                                        return (
                                            StatusCode::INTERNAL_SERVER_ERROR,
                                            "Internal Server Error",
                                        ).into_response();
                                };

                                tables.push(json!({
                                    "name": table_name,
                                    "schema": schema.into_iter().nth(0).unwrap_or_default()
                                }));
                            }

                            let Ok(users) = r#"SELECT USER FROM INFORMATION_SCHEMA.USER_ATTRIBUTES WHERE ATTRIBUTE->>"$.cityscale_db"=:db_name;"#
                                .with(params! {
                                    "db_name" => db_name
                                })
                                .map(&mut conn, |username: String| username)
                                .await
                                .map_err(|err| error!("Error getting users in DB '{}': {err}", db_name)) else {
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        "Internal Server Error",
                                    ).into_response();
                            };

                            let db = json!({
                                "name": db_name,
                                "tables": tables,
                                "users": users
                                    .into_iter()
                                    .map(|username| json!({ "username": username }))
                                    .collect::<Vec<_>>(),
                            });

                            (StatusCode::OK, Json(db)).into_response()
                        },
                    ),
                )
                .route("/database/:db/user", post(|State(state): State<Arc<AppState>>, Path(db_name): Path<String>, Json(data): Json<CreateDatabaseUser>| async move {
                    let Ok(mut conn) = state
                        .db
                        .get_conn()
                        .await
                        .map_err(|err| error!("Error getting DB connection: {err}"))
                    else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    // TODO: Throw a nice error if the username is already in use

                    let password = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);

                    // TODO: Proper SQL escaping
                    if data.username.chars().any(|c| !c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid username").into_response();
                    }
                    if password.chars().any(|c| !c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid password").into_response();
                    }
                    if db_name.chars().any(|c| !c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid database name").into_response();
                    }

                    let Ok(_) = format!(r#"CREATE USER '{}'@'%' IDENTIFIED BY '{password}' ATTRIBUTE '{{"cityscale_db": "{db_name}"}}';"#, data.username)
                        .ignore(&mut conn)
                        .await
                        .map_err(|err| error!("Error creating user '{}': {err}", data.username)) else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    // TODO: Configure user permissions based on selected role + restrict with only access to the current database

                    (StatusCode::OK, Json(json!({
                        "username": data.username,
                        "password": password
                    }))).into_response()
                }))
                .route("/database/:db/user/:username", delete(|State(state): State<Arc<AppState>>, Path((db_name, username)): Path<(String, String)>| async move { 
                    let Ok(mut conn) = state
                        .db
                        .get_conn()
                        .await
                        .map_err(|err| error!("Error getting DB connection: {err}"))
                    else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    let Ok(user) = r#"SELECT USER FROM INFORMATION_SCHEMA.USER_ATTRIBUTES WHERE ATTRIBUTE->>"$.cityscale_db"=:db_name AND USER = :username;"#
                        .with(params! {
                            "db_name" => &db_name,
                            "username" => &username
                        })
                        .map(&mut conn, |username: String| username)
                        .await
                        .map_err(|err| error!("Error getting user '{}': {err}", username)) else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    if user.is_empty() {
                        return (StatusCode::NOT_FOUND, "User not found").into_response();
                    }

                    if !username.chars().any(|c| c.is_alphanumeric()) {
                        return (StatusCode::BAD_REQUEST, "Invalid username").into_response();
                    }

                    let Ok(_) = format!("DROP USER '{username}';")
                        .ignore(&mut conn)
                        .await
                        .map_err(|err| error!("Error dropping user '{}': {err}", username)) else {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response();
                    };

                    (StatusCode::OK, "ok").into_response()
                }))
                .route_layer(middleware::from_fn_with_state(state.clone(), auth)),
        )
        .fallback_service(ServeDir::new(&ASSETS_DIR))
        .layer(CookieManagerLayer::new())
        .with_state(state)
}

#[derive(Deserialize)]
pub struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
pub struct CreateDatabaseRequest {
    name: String,
}

#[derive(Deserialize)]
pub struct CreateDatabaseUser {
    username: String,
    // role: () // TODO
}
