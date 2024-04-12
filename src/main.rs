use std::{
    future::IntoFuture,
    net::{Ipv6Addr, SocketAddr},
    path::PathBuf,
    process,
};

use api::State;
use tokio::signal;
use tracing::{error, info};

mod api;
mod config;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let data_dir = PathBuf::from(std::env::var("DATA_DIR").unwrap_or(".".into()));
    let Ok(port) = std::env::var("PORT")
        .map(|v| v.parse::<u16>())
        .unwrap_or(Ok(2489))
        .map_err(|err| error!("Failed to parse 'PORT' environment variable: {err}"))
    else {
        process::exit(1);
    };

    let Ok(listen_addr) = std::env::var("ADDR")
        .map(|addr| {
            addr.parse::<SocketAddr>()
                .map_err(|err| error!("Failed to parse 'ADDR' environment variable: {err}"))
        })
        .unwrap_or_else(|_| Ok((Ipv6Addr::UNSPECIFIED, port).into()))
    else {
        process::exit(1);
    };

    let Ok(config) = config::ConfigManager::new(data_dir.join("config.json"))
        .map_err(|err| error!("Error loading configuration: {err}"))
    else {
        process::exit(1);
    };
    let state = State { data_dir, config };

    let app = api::mount(state);
    let Ok(listener) = tokio::net::TcpListener::bind(listen_addr)
        .await
        .map_err(|err| error!("Failed to bind to {listen_addr}: {err}"))
    else {
        process::exit(1);
    };
    info!("Listening on http://{listen_addr}");

    let Ok(()) = (tokio::select! {
        result = axum::serve(listener, app).into_future() => result.map_err(|err| error!("Failed to serve: {err}")),
        result = signal::ctrl_c() => result.map_err(|err| error!("Failure with shutdown signal: {err}")),
    }) else {
        process::exit(1);
    };
}
