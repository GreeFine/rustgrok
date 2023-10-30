use std::{net::SocketAddr, sync::Arc};

use futures::future;
use log::{debug, error, info};
use tokio::{net::TcpStream, sync::RwLock};

use crate::{
    auth, config,
    getter::{self},
    method, ClientConnection,
};

#[cfg(feature = "ingress")]
use crate::ingress;

/// Handle a new client connection, look for the host it wants to connect to and store it
///
/// If ingress is enabled, expose the subdomain
pub async fn handle_client(mut client_conn: TcpStream) -> Result<(), ()> {
    auth::check_api_key(&mut client_conn).await?;

    let new_host = getter::get_client_host(&mut client_conn).await;

    #[cfg(feature = "ingress")]
    ingress::expose_subdomain(&new_host).await.map_err(|_| ())?;
    {
        debug!("[SERVER] Acquiring lock for : ROUTES.write().await");
        let mut routes_w = config::ROUTES.write().await;
        debug!("[SERVER] Done: ROUTES.write().await");

        routes_w.insert(
            new_host.trim().to_string(),
            Arc::new(RwLock::new(client_conn)),
        );
        info!("[SERVER] Inserted new route: {new_host}");
    }

    // TODO: check if the client disconnected and prune it's routes
    Ok(())
}

/// Handle the new stream created by the client, this will host the communication between the client and the user
pub async fn handle_client_stream(mut client_stream_conn: TcpStream) -> Result<(), ()> {
    auth::check_api_key(&mut client_stream_conn).await?;

    let (mut client_stream_recv, client_stream_send) = client_stream_conn.into_split();

    let target_port = getter::get_target_port(&mut client_stream_recv)
        .await
        .unwrap();
    info!("[SERVER] Received a new stream from client for port: {target_port}");

    let (user_request_recv, user_request_send) = config::USER_REQUEST_WAITING
        .write()
        .await
        .remove_entry(&target_port)
        .unwrap()
        .1;

    let client_stream_recv = Arc::new(RwLock::new(client_stream_recv));
    let client_stream_send = Arc::new(RwLock::new(client_stream_send));
    let user_flow = method::spawn_stream_sync(
        user_request_recv,
        client_stream_send,
        "[SERVER] user -> client".into(),
    );
    let client_flow = method::spawn_stream_sync(
        client_stream_recv,
        user_request_send,
        "[SERVER] client -> user".into(),
    );

    // Wait for only one of the two stream to finish
    let _ = future::select_all(vec![user_flow, client_flow]).await;

    info!("[SERVER] Request/stream done here on server for port: {target_port}");

    Ok(())
}

/// Get the associated client connection from the route requested by the user
pub async fn get_client(host: &String) -> Option<ClientConnection> {
    debug!("[SERVER] Acquiring lock for : [handle_request] ROUTES.read().await");
    let routes_r = config::ROUTES.read().await;
    debug!("[SERVER] Done: [handle_request] ROUTES.read().await");

    routes_r.get(host).cloned()
}

/// Handle a request from a user, forward it to the client and wait for the client to connect
pub async fn handle_user_request(request_conn: TcpStream, socket: SocketAddr) {
    let (mut request_recv, request_send) = request_conn.into_split();

    let Some(host) = getter::get_user_host(&mut request_recv).await else {
        error!("[SERVER] Host not found in client request, disconnected.");
        return;
    };

    let port = socket.port();
    info!(
        "[SERVER] Request for host: {host:?} from user_port: {}",
        port
    );
    let client_conn = get_client(&host).await;

    if let Some(client_conn) = client_conn {
        if method::server::request_client_stream(port, client_conn)
            .await
            .is_ok()
        {
            method::server::insert_waiting_client(port, (request_recv, request_send)).await;
            info!("[SERVER] Request handled and now waiting for client");
        } else {
            error!("[SERVER] requesting new stream, removing route: {host}");
            let mut routes_w = config::ROUTES.write().await;
            routes_w.remove_entry(&host);
        }
    } else {
        info!("[SERVER] did not found associated route to request");
    }
}
