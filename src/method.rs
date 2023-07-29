use std::{io::ErrorKind, sync::Arc, thread, time::Duration};

use futures::future;
use log::{error, info};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpSocket, TcpStream,
    },
    sync::RwLock,
    task::JoinHandle,
};

use crate::{
    config::{self, USER_REQUEST_WAITING},
    ClientConnection,
};

/// Connect to socket with it's address as a &str
async fn connect_socket(addr: impl AsRef<str>) -> Result<TcpStream, io::Error> {
    let addr = addr.as_ref().parse().unwrap();
    let socket = TcpSocket::new_v4()?;
    let stream = socket.connect(addr).await?;
    Ok(stream)
}

/// Proxy the two streams in parameters together,
///
/// Will try to read from the [OwnedReadHalf] part, and write it back in the [OwnedWriteHalf],
///
/// While reading if we fail or get 0 bytes we send a [AsyncWriteExt::shutdown] to the [OwnedWriteHalf]
pub fn spawn_stream_sync(
    recv: Arc<RwLock<OwnedReadHalf>>,
    send: Arc<RwLock<OwnedWriteHalf>>,
    name: String,
) -> JoinHandle<io::Result<()>> {
    info!("Proxy start: {}", name);
    tokio::spawn(async move {
        // looks like the max transferred is 32_768, 2 times that to account for weird things /shrug
        let mut buff = vec![0; 32_768 * 2];
        let recv = recv.read().await;
        let mut send = send.write().await;

        loop {
            recv.readable().await.unwrap();
            send.writable().await.unwrap();
            match recv.try_read(&mut buff) {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(0) => {
                    info!("Proxy stop: {name}");
                    // Shutting down the connection
                    let _ = send.shutdown().await;
                    return Ok(()) as io::Result<()>;
                }
                Ok(n) => {
                    info!("Proxy {name}, received data {n} bytes");
                    let mut to_write = n;
                    // Copy the data back to socket
                    while to_write > 0 {
                        let wrote = send.try_write(&buff[..n]);
                        match wrote {
                            Ok(n) => {
                                info!("Proxy {name}, sent data {n} bytes");
                                to_write -= n;
                            }
                            Err(e) => {
                                if !matches!(e.kind(), ErrorKind::WouldBlock) {
                                    error!("Proxy error: {e}");
                                    let _ = send.shutdown().await;
                                    return Err(e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    // Unexpected socket error. There isn't much we can do
                    // here so just stop processing.
                    if !matches!(e.kind(), ErrorKind::WouldBlock) {
                        error!("Proxy error: {e}");
                        let _ = send.shutdown().await;
                        return Err(e);
                    }
                }
            }
            thread::sleep(Duration::from_millis(25))
        }
    })
}

/// This module contains the methods used by the server to handle the client requests
pub mod server {
    use super::*;

    /// Ask the client to open a new Stream, we send the port used by the request from the user as an identifier.
    ///
    /// For more info see the client side handler: [spawn_new_stream]
    pub async fn request_client_stream(port: u16, client: ClientConnection) -> Result<(), ()> {
        let client_w = client.write().await;
        let mut buff = [0u8; 0];
        match client_w.try_read(&mut buff) {
            Ok(n) => {
                if n == 0 {
                    info!("[SERVER] Client disconnected");
                    return Err(());
                }
            }
            Err(error) => {
                if !matches!(error.kind(), ErrorKind::WouldBlock) {
                    error!("[SERVER] error checking client read: {error}");
                    return Err(());
                }
            }
        }

        let result = client_w.try_write(&port.to_be_bytes());
        match result {
            // Return value of `Ok(0)` signifies that the remote has
            // closed
            Ok(0) => {
                info!("[SERVER] Client closed the connection");
                // Shutting down the connection
                return Err(());
            }
            Ok(n) => {
                info!("[SERVER] Request client stream {port}, sended data {n} bytes");
            }
            Err(e) => {
                // Unexpected socket error. There isn't much we can do
                // here so just stop processing.
                if !matches!(e.kind(), ErrorKind::WouldBlock) {
                    error!("[SERVER] Proxy error: {e}");
                    return Err(());
                }
            }
        }
        Ok(())
    }

    /// Insert the client that made the request into the waiting map, it will later be used when the client create a stream for this request [request_client_stream].
    ///
    /// For more info see the client side handler: [spawn_new_stream]
    pub async fn insert_waiting_client(
        port: u16,
        owned_streams_rw: (OwnedReadHalf, OwnedWriteHalf),
    ) {
        let mut user_requests_w = USER_REQUEST_WAITING.write().await;

        let request_recv = Arc::new(RwLock::new(owned_streams_rw.0));
        let request_send = Arc::new(RwLock::new(owned_streams_rw.1));

        user_requests_w.insert(port, (request_recv, request_send));
    }
}

/// This module contains the methods used by the client to handle the server requests
pub mod client {
    use super::*;

    /// Create a new stream that will connect to the server and be used to received a user request
    ///
    /// We use the port to identify the user request that this stream should be linked with
    pub fn spawn_new_stream(port: u16, local_app_addr: String) -> JoinHandle<()> {
        tokio::spawn(async move {
            let app_stream: TcpStream = connect_socket(&local_app_addr).await.unwrap();
            info!("[CLIENT] Connected to app {local_app_addr}");
            let server_stream = connect_socket(config::PROXY_SERVER_STREAMS).await.unwrap();
            info!("[CLIENT] Connected to server for stream port {port}");

            let (app_recv, app_send) = app_stream.into_split();
            let (server_recv, mut server_send) = server_stream.into_split();

            server_send
                .write_all(config::API_KEY.as_bytes())
                .await
                .expect("sending api key to server");
            server_send.write_all(&port.to_be_bytes()).await.unwrap();

            let app_recv = Arc::new(RwLock::new(app_recv));
            let app_send = Arc::new(RwLock::new(app_send));
            let server_recv = Arc::new(RwLock::new(server_recv));
            let server_send = Arc::new(RwLock::new(server_send));

            let mut buff = [0u8; 32];
            let peeked = server_recv.write().await.peek(&mut buff).await.unwrap();
            info!(
                "peeked {peeked} bytes. peeked: {}",
                String::from_utf8_lossy(&buff[..peeked])
            );

            let app_flow =
                spawn_stream_sync(app_recv, server_send, "[CLIENT] app -> server".into());
            let server_flow =
                spawn_stream_sync(server_recv, app_send, "[CLIENT] server -> app".into());

            // Wait for only one of the two stream to finish
            let _ = future::select_all(vec![app_flow, server_flow]).await;

            info!("[CLIENT] stream done for {port}");
        })
    }

    /// Connect to the server and send the host name to the server
    pub async fn connect_with_server(host_name: &str) -> Result<TcpStream, std::io::Error> {
        assert_eq!(
            config::API_KEY.len(),
            32,
            "server is expecting a 32 bytes key"
        );
        info!("[CLIENT] Connecting to server {}", config::PROXY_SERVER);
        let mut server = connect_socket(config::PROXY_SERVER).await?;
        info!("[CLIENT] Connected to server");

        let mut payload = vec![0_u8; config::API_KEY.len() + host_name.len()];
        payload[..32].copy_from_slice(config::API_KEY.as_bytes());
        payload[32..].copy_from_slice(host_name.as_bytes());

        server
            .write_all(&payload)
            .await
            .expect("sending host to server");

        Ok(server)
    }

    /// Wait for the server to request a new stream, the port returned by the server is the identifier of the user request
    pub fn wait_for_stream_request(server: &TcpStream) -> Result<u16, Option<io::Error>> {
        let mut buff = [0; 2];
        loop {
            let read = server.try_read(&mut buff);
            match read {
                Ok(n) => {
                    if n == 0 {
                        info!("[CLIENT] Connection closed");
                        break Err(None);
                    }
                    let received_port = u16::from_be_bytes(buff);
                    info!("[CLIENT] received_port: {received_port}");
                    return Ok(received_port);
                }
                Err(e) => {
                    if !matches!(e.kind(), ErrorKind::WouldBlock) {
                        error!("Proxy error: {e}");
                        break Err(Some(e));
                    };
                }
            };
            thread::sleep(Duration::from_millis(25))
        }
    }
}
