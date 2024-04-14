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
    consts::{ColumnFlags, ColumnType},
    prelude::Queryable,
    Column, Conn, OptsBuilder, Pool, Row, Transaction, TxOpts, Value,
};
use secstr::SecStr;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, error};
use uuid::Uuid;

use super::AppState;

pub struct ConnectionPool {
    /// A map of username + database combo's to database connection & password
    connections: RwLock<HashMap<(String, String), (SecStr, mysql_async::Pool)>>,
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
    move |State(state): State<Arc<AppState>>, TypedHeader(Authorization(auth)): TypedHeader<Authorization<Basic>>, Json(data): Json<SqlRequest>|
        {
            let pool = pool.clone();
            async move {
                // TODO: Check the credentials against the transaction session to avoid needing to create another connection that will just be dropped
                let (mut conn, db) = authentication_and_get_db_conn(&pool, &state, auth).await?;

                let start = std::time::Instant::now();
                let mut session = None;

                if data.query == "BEGIN" {
                    let tx = db.start_transaction(TxOpts::default()).await.map_err(|err| {
                        error!("Error starting DB transaction: {err}");
                        error(format!("error starting DB transaction: {err:?}"))
                    })?;

                    let id = Uuid::new_v4();
                    debug!("Creating new DB session {id:?}");

                    {
                        pool.sessions.write().await.insert(id, tx);
                    }

                    session = Some(TransactionSession {
                        id
                    });

                    // `BEGIN` is run by `db.start_transaction` so we don't actually wanna execute it
                    return Ok(Json(json!({
                        "session": session,
                        "result": json!({}),
                        "timing": start.elapsed().as_secs_f64(),
                    })).into_response());
                }

                let (columns, values) = if let Some(session) = data.session {
                    // TODO: Can we only lock the specific session, not all of them while the DB query is running
                    let mut sessions = pool.sessions.write().await;

                    if data.query == "COMMIT" {
                        let tx = sessions.remove(&session.id).ok_or_else(|| {
                            debug!("Attempted to commit non-existent transaction {:?}", session.id);
                            error(format!("error committing non-existent transaction {:?}", session.id))
                        })?;

                        tx.commit().await.map_err(|err| {
                            error!("Error committing transaction: {err}");
                            error(format!("error committing transaction {:?}: {err:?}", session.id))
                        })?;
                        debug!("COMMIT transaction {:?}", session.id);

                        return Ok(Json(json!({
                            "session": session,
                            "result": json!({}),
                            "timing": start.elapsed().as_secs_f64(),
                        })).into_response());
                    } else if data.query == "ROLLBACK" {
                        let tx = sessions.remove(&session.id).ok_or_else(|| {
                            debug!("Attempted to rollback non-existent transaction {:?}", session.id);
                            error(format!("error rolling back non-existent transaction {:?}", session.id))
                        })?;

                        tx.rollback().await.map_err(|err| {
                            error!("Error rolling back transaction: {err}");
                            error(format!("error rolling back transaction {:?}: {err:?}", session.id))
                        })?;
                        debug!("ROLLBACK transaction {:?}", session.id);

                        return Ok(Json(json!({
                            "session": session,
                            "result": json!({}),
                            "timing": start.elapsed().as_secs_f64(),
                        })).into_response());
                    } else {
                        debug!("Executing query {:?} on session {:?}", data.query, session.id);
                        let tx = sessions.get_mut(&session.id).ok_or_else(|| {
                            debug!("Attempted to getting non-existent transaction {:?}", session.id);
                            error(format!("error getting non-existent transaction {:?}", session.id))
                        })?;

                        let result = tx
                            .exec_iter(&data.query, ())
                            .await
                            .map_err(|err| {
                                error!("Error executing query against transaction {:?}: {err}", session.id);
                                error(format!("error executing query: {err:?}"))
                            })?;

                        (result.columns(), result.collect_and_drop::<Row>().await)
                    }
                } else {
                    debug!("Executing query {:?}", data.query);
                    let result =  conn
                        .exec_iter(&data.query, ())
                        .await
                        .map_err(|err| {
                            error!("Error executing query {:?}: {err}", session.as_ref().map(|s| s.id));
                            error(format!("error executing query: {err:?}"))
                        })?;

                    (result.columns(), result.collect_and_drop::<Row>().await)
                };

                let values = values.map_err(|err| {
                    error!("Error getting values: {err}");
                    error(format!("error decoding values: {err:?}"))
                })?;

                let fields = columns.as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|col|
                        json!({
                            "name": col.name_str().to_string(),
                            "type": column_type_to_str(&col),
                            "charset": col.character_set(),
                            "flags": col.flags().bits()
                        })
                    )
                    .collect::<Vec<_>>();

                let rows = values
                    .into_iter()
                    .map(|mut row| {
                        let mut lengths = Vec::new();
                        let mut values = Vec::new();

                        for i in 0..row.len() {
                            let Some(value) = row.take(i) else {
                                continue;
                            };

                            let result = match value {
                                Value::NULL => {
                                    lengths.push(-1i64);
                                    continue;
                                },
                                Value::Bytes(v) => {
                                    lengths.push(v.len().try_into().expect("unable to cast usize to i64. How big are your damn pointers?"));
                                    values.extend(v);
                                    continue;
                                },
                                Value::Int(i) => i.to_string(),
                                Value::UInt(i) => i.to_string(),
                                Value::Float(i) => i.to_string(),
                                Value::Double(i) => i.to_string(),
                                // TODO: Planetscale seems to wipe out the fractional seconds, idk why but we are gonna copy for now.
                                Value::Date(year, month, day, hour, minute, second, _) => {
                                    if row.columns_ref()[i].column_type() == ColumnType::MYSQL_TYPE_DATE {
                                        format!("{:04}-{:02}-{:02}", year, month, day)
                                    } else {
                                        format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, minute, second)
                                    }
                                },
                                // TODO: Planetscale seems to wipe out the fractional seconds, idk why but we are gonna copy for now.
                                Value::Time(neg, d, h, i, s, _) => {
                                    if neg {
                                        format!("-{:02}:{:02}:{:02}", d * 24 + u32::from(h), i, s)
                                    } else {
                                        format!("{:02}:{:02}:{:02}", d * 24 + u32::from(h), i, s)
                                    }
                                }
                            };

                            lengths.push(result.len().try_into().expect("unable to cast usize to i64. How big are your damn pointers?"));
                            values.extend(result.as_bytes());
                        }

                    json!({
                        "lengths": lengths,
                        "values": STANDARD.encode(values),
                    })
                })
                .collect::<Vec<_>>();

                Ok::<Response, Response>(Json(json!({
                    "session": session,
                    "result": json!({
                        "rowsAffected": conn.affected_rows().to_string(),
                        "insertId": conn.last_insert_id().map(|v| v.to_string()),
                        "fields": fields,
                        "rows": rows,
                    }),
                    "timing": start.elapsed().as_secs_f64(),
                })).into_response())
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
    let Some((username, database)) = auth.username().split_once("%3B") else {
        return Err((
            StatusCode::BAD_REQUEST,
            "Invalid username. Must be in form 'username;db'",
        )
            .into_response());
    };
    let password = SecStr::from(auth.password());

    let key = (username.to_string(), database.to_string());
    let result = pool
        .connections
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .get(&key)
        .cloned();

    Ok(if let Some((actual_password, db)) = result {
        // TODO: If the user were to change password this would fail.
        // TODO: Can we list to the biglog and invalidate the cache when users are modified???
        if actual_password != password {
            return Err(StatusCode::UNAUTHORIZED.into_response());
        }

        let conn = db.get_conn().await.map_err(|err| {
            error!("Error getting DB connection: {err}");
            error(format!("error retrieving database connection"))
        })?;

        (conn, db)
    } else {
        let db = mysql_async::Pool::new(
            OptsBuilder::from_opts(state.db_opts.clone())
                .user(Some(username))
                .pass(Some(auth.password()))
                .db_name(Some(database)) // TODO: This will cause issues - https://github.com/oscartbeaumont/cityscale/issues/23
                .stmt_cache_size(0), // TODO: Should we renable this? It breaks transactions
        );

        match db.get_conn().await {
            Ok(conn) => {
                pool.connections
                    .write()
                    .unwrap_or_else(PoisonError::into_inner)
                    .insert(key, (password, db.clone()));
                (conn, db)
            }
            Err(mysql_async::Error::Server(err)) if err.code == 1045 => {
                return Err(StatusCode::UNAUTHORIZED.into_response());
            }
            Err(mysql_async::Error::Server(err)) if err.code == 1049 => {
                return Err(error(format!("unknown database {database:?}. Ensure your connection URI contains a valid database name.")));
            }
            Err(err) => {
                error!("Error getting DB connection to new pool: {err}");
                return Err(error(format!(
                    "error retrieving database connection: {err}"
                )));
            }
        }
    })
}

fn error(msg: String) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": {
                "message": msg,
            }
        })),
    )
        .into_response()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct TransactionSession {
    id: Uuid,
}

#[derive(Deserialize)]
struct SqlRequest {
    query: String,
    session: Option<TransactionSession>,
}

// Convert MySQL column types to Vitess column types
//
// Ref:
// - https://github.com/vitessio/vitess/blob/9e40015748ede158357bd7291f583db138abc3df/go/sqltypes/type.go#L142
// - https://vitess.io/files/version-pdfs/Vitess-Docs-6.0-04-29-2020.pdf
fn column_type_to_str(col: &Column) -> &'static str {
    let is_signed = !col.flags().contains(ColumnFlags::UNSIGNED_FLAG);
    let is_binary = col.flags().contains(ColumnFlags::BINARY_FLAG);

    if col.flags().contains(ColumnFlags::ENUM_FLAG) {
        return "ENUM";
    } else if col.flags().contains(ColumnFlags::SET_FLAG) {
        return "SET";
    }

    match col.column_type() {
        ColumnType::MYSQL_TYPE_DECIMAL => "DECIMAL",
        ColumnType::MYSQL_TYPE_TINY => t(is_signed, "INT8", "UINT8"),
        ColumnType::MYSQL_TYPE_SHORT => t(is_signed, "INT16", "UINT16"),
        ColumnType::MYSQL_TYPE_LONG => t(is_signed, "INT32", "UINT32"),
        ColumnType::MYSQL_TYPE_FLOAT => "FLOAT32",
        ColumnType::MYSQL_TYPE_DOUBLE => "FLOAT64",
        ColumnType::MYSQL_TYPE_NULL => "NULL",
        ColumnType::MYSQL_TYPE_TIMESTAMP => "TIMESTAMP",
        ColumnType::MYSQL_TYPE_LONGLONG => t(is_signed, "INT64", "UINT64"),
        ColumnType::MYSQL_TYPE_INT24 => t(is_signed, "INT24", "UINT24"),
        ColumnType::MYSQL_TYPE_DATE => "DATE",
        ColumnType::MYSQL_TYPE_TIME => "TIME",
        ColumnType::MYSQL_TYPE_DATETIME => "DATETIME",
        ColumnType::MYSQL_TYPE_YEAR => "YEAR",
        ColumnType::MYSQL_TYPE_NEWDATE => unreachable!("Internal to MySQL."),
        ColumnType::MYSQL_TYPE_VARCHAR => "VARCHAR",
        ColumnType::MYSQL_TYPE_BIT => "BIT",
        ColumnType::MYSQL_TYPE_TIMESTAMP2 => todo!(),
        ColumnType::MYSQL_TYPE_DATETIME2 => todo!(),
        ColumnType::MYSQL_TYPE_TIME2 => todo!(),
        ColumnType::MYSQL_TYPE_TYPED_ARRAY => unreachable!("Used for replication only."),
        ColumnType::MYSQL_TYPE_UNKNOWN => unreachable!(),
        ColumnType::MYSQL_TYPE_JSON => "JSON",
        ColumnType::MYSQL_TYPE_NEWDECIMAL => todo!(),
        ColumnType::MYSQL_TYPE_ENUM => "ENUM",
        ColumnType::MYSQL_TYPE_SET => "SET",
        ColumnType::MYSQL_TYPE_TINY_BLOB
        | ColumnType::MYSQL_TYPE_MEDIUM_BLOB
        | ColumnType::MYSQL_TYPE_LONG_BLOB
        | ColumnType::MYSQL_TYPE_BLOB => t(is_binary, "BLOB", "TEXT"),
        ColumnType::MYSQL_TYPE_VAR_STRING => t(is_binary, "VARBINARY", "VARCHAR"),
        ColumnType::MYSQL_TYPE_STRING => t(is_binary, "BINARY", "CHAR"),
        ColumnType::MYSQL_TYPE_GEOMETRY => "GEOMETRY",
    }
}

fn t<T>(a_or_b: bool, a: T, b: T) -> T {
    if a_or_b {
        a
    } else {
        b
    }
}
