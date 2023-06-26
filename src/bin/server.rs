use std::{collections::HashMap, io, net::SocketAddr, sync::Arc};

use futures::future;
use lazy_static::lazy_static;
use log::{debug, error, info};
#[cfg(feature = "ingress")]
use rustgrok::ingress;
use rustgrok::{spawn_stream_sync, StreamRwTuple};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpListener, TcpStream,
    },
    sync::RwLock,
};

lazy_static! {
    /// Routes registered by the clients, the connection associated is used to ask for a `CLIENT_STREAM`
    /// #
    static ref ROUTES: RwLock<HashMap<String, StreamRwTuple>> = RwLock::new(HashMap::new());
    /// List of users request that are waiting for the client to create the stream
    /// #
    static ref USER_REQUEST_WAITING: RwLock<HashMap<u16, StreamRwTuple>> = RwLock::new(HashMap::new());
}

/// FIXME: load this from env
const API_KEY: &str = "SuperSecret";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "server,rustgrok");
    pretty_env_logger::init();

    const BINDING_ADDR_CLIENT: &str = "0.0.0.0:3000";
    let client = TcpListener::bind(BINDING_ADDR_CLIENT).await?;
    info!("Client server started on {BINDING_ADDR_CLIENT}");
    const BINDING_ADDR_CLIENT_USER_STREAM: &str = "0.0.0.0:3001";
    let client_streams = TcpListener::bind(BINDING_ADDR_CLIENT_USER_STREAM).await?;
    info!("User stream from Client server started on {BINDING_ADDR_CLIENT_USER_STREAM}");
    const BINDING_ADDR_FRONT: &str = "0.0.0.0:8080";
    let receiver = TcpListener::bind(BINDING_ADDR_FRONT).await.unwrap();
    info!("Front server started on {BINDING_ADDR_FRONT}");

    let client_handler = tokio::spawn(async move {
        while let Ok((client, _)) = client.accept().await {
            info!("Client connected");
            tokio::spawn(async move {
                handle_client(client).await.unwrap();
            });
        }
    });

    let client_streams = tokio::spawn(async move {
        while let Ok((client_stream, _)) = client_streams.accept().await {
            info!("New Client Stream");
            tokio::spawn(async move {
                handle_client_stream(client_stream).await.unwrap();
            });
        }
    });

    let receiver_handler = tokio::spawn(async move {
        while let Ok((request, socket)) = receiver.accept().await {
            info!("Incoming request user_port: {:?}", socket.port());
            tokio::spawn(async move {
                handle_request(request, socket).await.unwrap();
            });
        }
    });

    for jh in [client_handler, receiver_handler, client_streams] {
        jh.await.unwrap();
    }

    Ok(())
}

async fn get_host<'a>(client_recv: &mut OwnedReadHalf) -> Option<String> {
    let mut buffer = [0; 1024];

    let read = client_recv.peek(&mut buffer).await.unwrap();
    let raw_http_request = String::from_utf8_lossy(&buffer[..read]);
    let host = raw_http_request
        .split('\n')
        .find(|header| header.starts_with("Host: "))
        .map(|header| header["Host: ".len()..].trim());

    host.map(|h| h.to_string())
}

async fn get_client(host: &String) -> Option<StreamRwTuple> {
    debug!("Acquiring lock for : [handle_request] ROUTES.write().await");
    let routes_r = ROUTES.read().await;
    debug!("Done: [handle_request] ROUTES.write().await");

    routes_r.get(host).cloned()
}

async fn request_client_stream(port: u16, client: StreamRwTuple) -> Result<(), io::Error> {
    let mut client_w = client.1.write().await;
    client_w.write_all(&port.to_be_bytes()).await
}

async fn insert_waiting_client(port: u16, owned_streams_rw: (OwnedReadHalf, OwnedWriteHalf)) {
    let mut user_requests_w = USER_REQUEST_WAITING.write().await;

    let request_recv = Arc::new(RwLock::new(owned_streams_rw.0));
    let request_send = Arc::new(RwLock::new(owned_streams_rw.1));

    user_requests_w.insert(port, (request_recv, request_send));
}

async fn handle_request(
    request_conn: TcpStream,
    socket: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    let (mut request_recv, mut request_send) = request_conn.into_split();

    if let Some(host) = get_host(&mut request_recv).await {
        let port = socket.port();
        info!("Request for host: {host:?} from user_port: {}", port);
        let client_conn = get_client(&host).await;
        if client_conn.is_none() {
            info!("did not found associated route to request");
            request_send.shutdown().await.unwrap();
            return Ok(());
        }
        if let Err(err) = request_client_stream(port, client_conn.unwrap()).await {
            error!("requesting new stream: {err}, removing route: {host}");
            let mut routes_w = ROUTES.write().await;
            routes_w.remove_entry(&host);
        } else {
            insert_waiting_client(port, (request_recv, request_send)).await;
            info!("Request handled and now waiting for client");
        }
    } else {
        let _ = request_send.shutdown().await;
        info!("Host not found in client request, disconnected.")
    }

    Ok(())
}

/// Get the port send by the user after connecting a new stream
///
/// This port is used to identify the `USER_REQUEST_WAITING` we want to attach it to
async fn get_target_port<'a>(client_recv: &mut OwnedReadHalf) -> Result<u16, ()> {
    let mut buff = [0_u8; 2];

    let read = client_recv.read(&mut buff).await.unwrap();
    if read != 2 {
        error!("Unable to read the port from new stream from client");
        return Err(());
    }
    Ok(u16::from_be_bytes(buff))
}

async fn check_api_key(client_recv: &mut OwnedReadHalf) -> Result<(), ()> {
    let mut buff = [0_u8; API_KEY.len()];

    let read = client_recv.read(&mut buff).await.unwrap();
    if read != API_KEY.len() {
        error!("Unable to read the API_KEY from new stream from client");
        return Err(());
    } else if buff != API_KEY.as_bytes() {
        error!(
            "invalid API_KEY new stream from client, received: '{}'",
            String::from_utf8_lossy(&buff)
        );
        return Err(());
    }

    Ok(())
}

async fn handle_client_stream(client_stream_conn: TcpStream) -> Result<(), ()> {
    let (mut client_stream_recv, client_stream_send) = client_stream_conn.into_split();

    check_api_key(&mut client_stream_recv).await?;

    let target_port = get_target_port(&mut client_stream_recv).await.unwrap();
    info!("Received a new stream from client for port: {target_port}");

    let (user_request_recv, user_request_send) = USER_REQUEST_WAITING
        .write()
        .await
        .remove_entry(&target_port)
        .unwrap()
        .1;

    let client_stream_recv = Arc::new(RwLock::new(client_stream_recv));
    let client_stream_send = Arc::new(RwLock::new(client_stream_send));
    let user_flow = spawn_stream_sync(
        user_request_recv,
        client_stream_send,
        "user -> client".into(),
    );
    let client_flow = spawn_stream_sync(
        client_stream_recv,
        user_request_send,
        "client -> user".into(),
    );

    // Wait for only one of the two stream to finish
    let _ = future::select_all(vec![user_flow, client_flow]).await;

    info!("Request/stream done here on server for port: {target_port}");

    Ok(())
}

async fn get_requested_host(client_recv: &mut OwnedReadHalf) -> String {
    let mut buffer = [0; 1024];
    let count = client_recv.read(&mut buffer).await.unwrap();

    #[cfg(feature = "localhost")]
    let new_host = format!("{}", String::from_utf8_lossy(&buffer[..count]));
    #[cfg(not(feature = "localhost"))]
    let new_host = format!(
        "{}.rgrok.blackfoot.dev",
        String::from_utf8_lossy(&buffer[..count])
    );
    new_host
}

async fn handle_client(client_conn: TcpStream) -> Result<(), ()> {
    let (mut recv, send) = client_conn.into_split();

    check_api_key(&mut recv).await?;

    let new_host = get_requested_host(&mut recv).await;

    #[cfg(feature = "ingress")]
    ingress::expose_subdomain(&new_host).await.map_err(|_| ())?;
    {
        debug!("Acquiring lock for : ROUTES.read().await");
        let mut routes_w = ROUTES.write().await;
        debug!("Done: ROUTES.read().await");

        routes_w.insert(
            new_host.trim().to_string(),
            (Arc::new(RwLock::new(recv)), Arc::new(RwLock::new(send))),
        );
        info!("Inserted new route: {new_host}");
    }

    // TODO: check if the client disconnected and prune it's routes

    Ok(())
}
