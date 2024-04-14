use std::{
    collections::HashMap,
    sync::{Arc, PoisonError, RwLock},
};

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use axum_extra::{
    headers::{authorization::Basic, Authorization},
    TypedHeader,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use mysql_async::{
    consts::ColumnType, prelude::Queryable, Conn, OptsBuilder, Pool, Row, Transaction, TxOpts,
    Value,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error};
use uuid::Uuid;

use super::AppState;

pub struct ConnectionPool {
    /// A map of username + password combo's to database connections
    connections: RwLock<HashMap<(String, String), mysql_async::Pool>>,
    /// Active database transactions
    sessions: tokio::sync::RwLock<HashMap<Uuid, Transaction<'static>>>,
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
post({
    let pool = pool.clone();
    move |State(state): State<Arc<AppState>>, TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>, Json(mut data): Json<SqlRequest>|
        {
            let pool = pool.clone();
            async move {
                // TODO: Check the credentials against the transaction session to avoid needing to create another connection that will just be dropped
                let (mut conn, db) = match authentication_and_get_db_conn(&pool, &state, auth).await {
                    Ok(conn) => conn,
                    Err(res) => return res,
                };

                let start = std::time::Instant::now();
                let mut session = None;

                if data.query == "BEGIN" {
                    let Ok(tx) = db.start_transaction(TxOpts::default()).await.map_err(|err| error!("Error starting DB transaction: {err}")) else {
                        return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                    };
    
                    let id = Uuid::new_v4();
                    debug!("Creating new DB session {id:?}");

                    {
                        pool.sessions.write().await.insert(id, tx);
                    }

                    session = Some(TransactionSession {
                        id
                    });

                    // `BEGIN` is run by `db.start_transaction` so we don't actually wanna execute it
                    return Json(json!({
                        "session": session,
                        "result": json!({}),
                        "timing": start.elapsed().as_secs_f64(),
                    })).into_response();
                }

                debug!("Executing query {:?} on session {:?}", data.query, data.session.as_ref().map(|s| s.id));

                let (columns, values) = if let Some(session) = data.session {
                    // TODO: Can we only lock the specific session, not all of them while the DB query is running
                    let mut sessions = pool.sessions.write().await;

                    if data.query == "COMMIT" {
                        let Some(tx) = sessions.remove(&session.id) else {
                            return (StatusCode::BAD_REQUEST, "Invalid session").into_response();
                        };

                        let Ok(_) = tx.commit().await.map_err(|err| error!("Error committing transaction: {err}")) else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                        };
                        debug!("COMMIT transaction {:?}", session.id);

                        return Json(json!({
                            "session": session,
                            "result": json!({}),
                            "timing": start.elapsed().as_secs_f64(),
                        })).into_response();
                    } else if data.query == "ROLLBACK" {
                        let Some(tx) = sessions.remove(&session.id) else {
                            return (StatusCode::BAD_REQUEST, "Invalid session").into_response();
                        };

                        let Ok(_) = tx.rollback().await.map_err(|err| error!("Error rolling back transaction: {err}")) else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                        };
                        debug!("ROLLBACK transaction {:?}", session.id);
                        
                        return Json(json!({
                            "session": session,
                            "result": json!({}),
                            "timing": start.elapsed().as_secs_f64(),
                        })).into_response();
                    } else {
                        let Some(tx) = sessions.get_mut(&session.id) else {
                            return (StatusCode::BAD_REQUEST, "Invalid session").into_response();
                        };

                        let Ok(result) = tx
                        .exec_iter(&data.query, ())
                        .await
                        .map_err(|err| error!("Error executing query: {err}")) else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                        };

                        (result.columns(), result.collect_and_drop::<Row>().await)
                    }                    
                } else {
                    let Ok(result) =  conn
                        .exec_iter(&data.query, ())
                        .await
                        .map_err(|err| error!("Error executing query: {err}")) else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                        };

                    (result.columns(), result.collect_and_drop::<Row>().await)
                };

                let Ok(values) = values.map_err(|err| error!("Error getting values: {err}")) else {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                };

                let fields = columns.as_deref()
                    .unwrap_or(&[])
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

                let rows = values
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
                    "session": session,
                    "result": json!({
                        "rowsAffected": conn.affected_rows().to_string(),
                        "insertId": conn.last_insert_id().map(|v| v.to_string()),
                        "fields": fields,
                        "rows": rows,
                    }),
                    // error?: VitessError // TODO: Proper error handling
                    "timing": start.elapsed().as_secs_f64(),
                })).into_response()
        }}})
    )
    .route(
        "/CreateSession",
        post(move |State(state): State<Arc<AppState>>, TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>| {
            let pool = pool.clone();
            async move {
                let (conn, db) = match authentication_and_get_db_conn(&pool, &state, auth).await {
                    Ok(conn) => conn,
                    Err(res) => return res,
                };
                drop(conn); // TODO: As we use connections for auth creating this is kinda required to cache the credentials (and reducing load on the DB). We can probs workaround this to make this endpoint *wayy* faster in the future.

                let Ok(tx) = db.start_transaction(TxOpts::default()).await.map_err(|err| error!("Error starting DB transaction: {err}")) else {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response();
                };

                let id = Uuid::new_v4();
                debug!("Creating new DB session {id:?}");

                {
                    pool.sessions.write().await.insert(id, tx);
                }

                Json(json!({
                    "session": TransactionSession {
                        id,
                    }
                })).into_response()
            }
        }),
    )
}

async fn authentication_and_get_db_conn(
    pool: &ConnectionPool,
    state: &AppState,
    auth: Basic,
) -> Result<(Conn, Pool), Response> {
    let key = (auth.username().to_string(), auth.password().to_string());
    let result = pool
        .connections
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned();

    Ok(if let Some(db) = result {
        let Ok(conn) = db
            .get_conn()
            .await
            .map_err(|err| error!("Error getting DB connection: {err}"))
        else {
            return Err(
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
            );
        };

        (conn, db)
    } else {
        let db = mysql_async::Pool::new(
            OptsBuilder::from_opts(state.db_opts.clone())
                .user(Some(auth.username()))
                .pass(Some(auth.password()))
                .stmt_cache_size(0), // TODO: Should we renable this? It breaks transactions
        );

        match db.get_conn().await {
            Ok(conn) => {
                pool.connections
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(key, db.clone());
                (conn, db)
            }
            Err(mysql_async::Error::Server(err)) if err.code == 1045 => {
                return Err((StatusCode::UNAUTHORIZED, "Unauthorised!").into_response());
            }
            Err(err) => {
                error!("Error getting DB connection: {err}");
                return Err(
                    (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
                );
            }
        }
    })
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TransactionSession {
    id: Uuid,
}

#[derive(Deserialize)]
pub struct SqlRequest {
    query: String,
    session: Option<TransactionSession>,
}
