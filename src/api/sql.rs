use std::{
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock},
};

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use axum_extra::{
    headers::{authorization::Basic, Authorization},
    TypedHeader,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use mysql_async::{consts::ColumnType, prelude::Queryable, OptsBuilder, Row, Value};
use serde::Deserialize;
use serde_json::json;
use tracing::{debug, error};
use uuid::Uuid;

use super::AppState;

pub struct Transaction {
    // TODO
}

pub struct ConnectionPool {
    /// A map of username + password combo's to database connections
    connections: RwLock<HashMap<(String, String), mysql_async::Pool>>,
    /// Active database transactions
    sessions: RwLock<HashMap<Uuid, Transaction>>,
}

// `@planetscale/database-js`` compatible SQL API
pub fn mount() -> Router<Arc<AppState>> {
    let pool = Arc::new(ConnectionPool {
        connections: RwLock::new(HashMap::with_capacity(250)),
        sessions: Default::default(),
    });

    Router::new()
    .route(
        "/Execute",
post(move |State(state): State<Arc<AppState>>, TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>, Json(data): Json<SqlRequest>|
        {
            let pool = pool.clone();
            async move {
                let key = (auth.username().to_string(), auth.password().to_string());
                let result = pool.connections.read().unwrap_or_else(PoisonError::into_inner).get(&key).cloned();

                let mut conn = if let Some(db) = result {
                    let Ok(conn) = db.get_conn().await.map_err(|err| error!("Error getting DB connection: {err}")) else {
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                    };

                    conn
                } else {
                    let db = mysql_async::Pool::new(
                        OptsBuilder::from_opts(state.db_opts.clone())
                        .user(Some(auth.username()))
                        .pass(Some(auth.password())));

                    match db.get_conn().await {
                        Ok(conn) => {
                            pool.connections.write().unwrap_or_else(PoisonError::into_inner).insert(key, db.clone());
                            conn
                        },
                        Err(mysql_async::Error::Server(err)) if err.code == 1045 => {
                            return (StatusCode::UNAUTHORIZED, "Unauthorised!").into_response();
                        }
                        Err(err) => {
                            error!("Error getting DB connection: {err}");
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                        }
                    }
                };

                debug!("Executing query {:?} on session {:?}", data.query, ()); // TODO: data.session

                // TODO: `data.session` handling
                // TODO: Proper error handling using correct format

                let start = std::time::Instant::now();

                let result = conn
                    .exec_iter(&data.query, ())
                    .await
                    .unwrap();

                let fields = result.columns_ref()
                    .iter()
                    .map(|col| {
                        let column_type = match col.column_type() {
                            ColumnType::MYSQL_TYPE_DECIMAL => todo!(),
                            ColumnType::MYSQL_TYPE_TINY => todo!(),
                            ColumnType::MYSQL_TYPE_SHORT => todo!(),
                            ColumnType::MYSQL_TYPE_LONG => todo!(),
                            ColumnType::MYSQL_TYPE_FLOAT => todo!(),
                            ColumnType::MYSQL_TYPE_DOUBLE => todo!(),
                            ColumnType::MYSQL_TYPE_NULL => todo!(),
                            ColumnType::MYSQL_TYPE_TIMESTAMP => todo!(),
                            ColumnType::MYSQL_TYPE_LONGLONG => "INT64", // TODO: Is this correct?
                            ColumnType::MYSQL_TYPE_INT24 => todo!(),
                            ColumnType::MYSQL_TYPE_DATE => todo!(),
                            ColumnType::MYSQL_TYPE_TIME => todo!(),
                            ColumnType::MYSQL_TYPE_DATETIME => todo!(),
                            ColumnType::MYSQL_TYPE_YEAR => todo!(),
                            ColumnType::MYSQL_TYPE_NEWDATE => todo!(),
                            ColumnType::MYSQL_TYPE_VARCHAR => todo!(),
                            ColumnType::MYSQL_TYPE_BIT => todo!(),
                            ColumnType::MYSQL_TYPE_TIMESTAMP2 => todo!(),
                            ColumnType::MYSQL_TYPE_DATETIME2 => todo!(),
                            ColumnType::MYSQL_TYPE_TIME2 => todo!(),
                            ColumnType::MYSQL_TYPE_TYPED_ARRAY => todo!(),
                            ColumnType::MYSQL_TYPE_UNKNOWN => todo!(),
                            ColumnType::MYSQL_TYPE_JSON => todo!(),
                            ColumnType::MYSQL_TYPE_NEWDECIMAL => todo!(),
                            ColumnType::MYSQL_TYPE_ENUM => todo!(),
                            ColumnType::MYSQL_TYPE_SET => todo!(),
                            ColumnType::MYSQL_TYPE_TINY_BLOB => todo!(),
                            ColumnType::MYSQL_TYPE_MEDIUM_BLOB => todo!(),
                            ColumnType::MYSQL_TYPE_LONG_BLOB => todo!(),
                            ColumnType::MYSQL_TYPE_BLOB => todo!(),
                            ColumnType::MYSQL_TYPE_VAR_STRING => todo!(),
                            ColumnType::MYSQL_TYPE_STRING => todo!(),
                            ColumnType::MYSQL_TYPE_GEOMETRY => todo!(),
                        };

                        json!({
                            "name": col.name_str().to_string(),
                            "type": column_type,
                            "charset": col.character_set(),
                            "flags": col.flags().bits()
                        })
                    })
                    .collect::<Vec<_>>();

                let rows = result.collect_and_drop::<Row>()
                    .await
                    .unwrap()
                    .into_iter()
                    .map(|mut row| {
                        let mut lengths = Vec::new();
                        let mut values = String::new();

                        while let Some(value) = row.take(0) {
                            let result = match value {
                                Value::NULL => todo!(),
                                Value::Bytes(_) => todo!(),
                                Value::Int(i) => i.to_string(),
                                Value::UInt(_) => todo!(),
                                Value::Float(_) => todo!(),
                                Value::Double(_) => todo!(),
                                Value::Date(_, _, _, _, _, _, _) => todo!(),
                                Value::Time(_, _, _, _, _, _) => todo!(),
                            };

                            lengths.push(result.len());
                            values.push_str(&result);
                        }

                    json!({
                        "lengths": lengths,
                        "values": STANDARD.encode(values),
                    })
                })
                .collect::<Vec<_>>();

                Json(json!({
                    // session: QuerySession // TODO: Transactions
                    "result": json!({
                        "rowsAffected": conn.affected_rows().to_string(),
                        "insertId": conn.last_insert_id().map(|v| v.to_string()),
                        "fields": fields,
                        "rows": rows,
                    }),
                    // error?: VitessError // TODO: Proper error handling
                    "timing": start.elapsed().as_secs_f64(),
                })).into_response()
        }})
    )
    .route(
        "/CreateSession",
        post(|State(state): State<Arc<AppState>>, TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>| async move {
            println!("CREATE SESSION");

            // TODO: Authentication

            // TODO: Create SQL session

            Json(json!({
                "branch": "kv6j8r14afd2",
                "user": {
                  "username": "vo03t3jabf2lzkhbziqu",
                  "psid": "aws-ap-southeast-2-1",
                  "role": "admin"
                },
                "session": {
                  "signature": "anbxhzlZNQvlXTooRSsbCsCOi0DD8LWcrhxXqdjzRCk=",
                  "vitessSession": {
                    "autocommit": true,
                    "options": {
                      "includedFields": "ALL",
                      "clientFoundRows": true
                    },
                    "DDLStrategy": "direct",
                    "SessionUUID": "yb2tOZlGa0d5qybWZzNwaQ",
                    "enableSystemSettings": true
                  }
                }
            }))
        }),
    )
}

#[derive(Deserialize)]
pub struct SqlRequest {
    query: String,
    // session: ()// TODO:
}
