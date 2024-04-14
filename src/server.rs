//! We expose a single port for both the admin dashboard and MySQL.
//!
//! This was primarily due to Railway only allowing a single port to be exposed.
//! However it also has the benefit that Drizzle Push (using mysql2) and Drizzle (using database-js) can both take the same connection URL.
//!
//! This file contains a lot of copied logic from `axum::serve`.

use std::{io, net::SocketAddr, time::Duration};

use tokio::net::{TcpListener, TcpStream};
use tracing::error;

fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

async fn tcp_accept(listener: &TcpListener) -> Option<(TcpStream, SocketAddr)> {
    match listener.accept().await {
        Ok(conn) => Some(conn),
        Err(e) => {
            if is_connection_error(&e) {
                return None;
            }

            // [From `hyper::Server` in 0.14](https://github.com/hyperium/hyper/blob/v0.14.27/src/server/tcp.rs#L186)
            //
            // > A possible scenario is that the process has hit the max open files
            // > allowed, and so trying to accept a new connection will fail with
            // > `EMFILE`. In some cases, it's preferable to just wait for some time, if
            // > the application will likely close some files (or connections), and try
            // > to accept the connection again. If this option is `true`, the error
            // > will be logged at the `error` level, since it is still a big deal,
            // > and then the listener will sleep for 1 second.
            //
            // hyper allowed customizing this but axum does not.
            error!("accept error: {e}");
            tokio::time::sleep(Duration::from_secs(1)).await;
            None
        }
    }
}

pub async fn serve(tcp_listener: TcpListener, internal_addr: SocketAddr, mysqld_addr: SocketAddr) {
    loop {
        let (mut tcp_stream, _remote_addr) = match tcp_accept(&tcp_listener).await {
            Some(conn) => conn,
            None => continue,
        };

        tokio::spawn(async move {
            let mut _buf = [0u8; 1];
            tokio::select! {
                _ = tcp_stream.peek(&mut _buf) => {
                    // Proxy to Axum // TODO: Replace this with just a direct Hyper integration
                    let Ok(mut out) = TcpStream::connect(internal_addr).await.map_err(|err| error!("Failed to connect to {internal_addr}: {err}")) else {
                        return;
                    };
                    tokio::io::copy_bidirectional(&mut tcp_stream, &mut out).await.map_err(|err| error!("Failed to copy bidirectional: {err}")).ok();
                }
                // MySQL's protocol starts with a message from the server, not client.
                // https://dev.mysql.com/doc/dev/mysql-server/latest/page_protocol_connection_phase.html
                _ = tokio::time::sleep(std::time::Duration::from_millis(500)) => {
                    // Proxy to `mysqld` // TODO: Replace this with Unix domain socket?
                    let Ok(mut out) = TcpStream::connect(mysqld_addr).await.map_err(|err| error!("Failed to connect to {mysqld_addr}: {err}")) else {
                        return;
                    };
                    tokio::io::copy_bidirectional(&mut tcp_stream, &mut out).await.map_err(|err| error!("Failed to copy bidirectional: {err}")).ok();

                }
            }
        });
    }
}
