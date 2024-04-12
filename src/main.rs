use std::{
    env,
    fs::{self, Permissions},
    future::IntoFuture,
    net::{Ipv6Addr, SocketAddr},
    path::PathBuf,
    process,
    sync::Arc,
};

use mysql_async::OptsBuilder;
use tokio::{process::Command, signal};
use tracing::{error, info};

use crate::api::AppState;

mod api;
mod config;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let data_dir = PathBuf::from(env::var("DATA_DIR").unwrap_or(".".into()));
    fs::create_dir_all(&data_dir).ok();
    let Ok(port) = env::var("PORT")
        .map(|v| v.parse::<u16>())
        .unwrap_or(Ok(2489))
        .map_err(|err| error!("Failed to parse 'PORT' environment variable: {err}"))
    else {
        process::exit(1);
    };

    let Ok(listen_addr) = env::var("ADDR")
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

    info!("Starting MySQL server...");
    {
        let mysql_dir = data_dir.join("mysql");

        if !mysql_dir.exists() {
            info!("Initializing MySQL data directory...");
            fs::create_dir_all(&mysql_dir).ok();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&mysql_dir, Permissions::from_mode(0o750))
                    .map_err(|err| {
                        error!("Failed to set permissions of MySQL data directory: {err}")
                    })
                    .ok();

                Command::new("chown")
                    .arg("mysql:mysql")
                    .arg(&mysql_dir)
                    .output()
                    .await
                    .map_err(|err| error!("Failed to chown MySQL data directory: {err}"))
                    .ok();
            }

            // We rely on the Docker entrypoint for setup cause MySQL makes it a pain in the ass.
        }

        let Ok(mut cmd) = Command::new("/usr/local/bin/docker-entrypoint.sh")
            // These args are forwarded to `mysqld``
            .arg("--datadir")
            .arg(mysql_dir)
            // Configure the root password in the Docker entrypoints setup script
            .env(
                "MYSQL_ROOT_PASSWORD",
                config.get().mysql_root_password.clone(),
            )
            .spawn()
            .map_err(|err| error!("Failed to start MySQL: {err}"))
        else {
            process::exit(1);
        };

        tokio::spawn(async move {
            let Ok(status) = cmd
                .wait()
                .await
                .map_err(|err| error!("MySQL exited: {err}"))
            else {
                process::exit(1);
            };
            info!("MySQL exited with status: {status}");
        });
    }

    let db = mysql_async::Pool::new(
        OptsBuilder::default()
            .ip_or_hostname("127.0.0.1")
            .tcp_port(3306)
            .user(Some("root"))
            .pass(Some(config.get().mysql_root_password.clone())),
    );
    let state = Arc::new(AppState {
        db,
        data_dir,
        config,
    });

    let app = api::mount(state);
    let Ok(listener) = tokio::net::TcpListener::bind(listen_addr)
        .await
        .map_err(|err| error!("Failed to bind to {listen_addr}: {err}"))
    else {
        process::exit(1);
    };
    info!("Cityscale listening on http://{listen_addr}");

    let Ok(()) = (tokio::select! {
        result = axum::serve(listener, app).into_future() => result.map_err(|err| error!("Failed to serve: {err}")),
        result = signal::ctrl_c() => result.map_err(|err| error!("Failure with shutdown signal: {err}")),
    }) else {
        process::exit(1);
    };
}
