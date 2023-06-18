use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use log::{debug, info};
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
    try_join,
};

use lazy_static::lazy_static;

lazy_static! {
    /// Routes registered by the clients, the connection associated is used to ask for a `CLIENT_STREAM`
    /// #
    static ref ROUTES: RwLock<HashMap<String, StreamRwTuple>> = RwLock::new(HashMap::new());
    /// List of users request that are waiting for the client to create the stream
    /// #
    static ref USER_REQUEST_WAITING: RwLock<HashMap<u16, StreamRwTuple>> = RwLock::new(HashMap::new());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "debug");
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

    for jh in [client_handler, receiver_handler] {
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

async fn get_client(host: &Option<String>) -> Option<StreamRwTuple> {
    debug!("Acquiring lock for : [handle_request] ROUTES.write().await");
    let routes_w = ROUTES.read().await;
    debug!("Done: [handle_request] ROUTES.write().await");

    host.as_ref()
        .and_then(|destination| routes_w.get(destination).cloned())
}

async fn request_client_stream(port: u16, client: StreamRwTuple) {
    let mut client_w = client.1.write().await;
    client_w
        .write_all(&port.to_be_bytes())
        .await
        .expect("asking for a new TcpStream");
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

    let host = get_host(&mut request_recv).await;
    let port = socket.port();
    info!("Request for host: {host:?} from user_port: {}", port);
    let client_conn = get_client(&host).await;
    if client_conn.is_none() {
        info!("did not found associated route to request");
        request_send.shutdown().await.unwrap();
        return Ok(());
    }
    request_client_stream(port, client_conn.unwrap()).await;

    insert_waiting_client(port, (request_recv, request_send)).await;

    dbg!("Request handled and now waiting for client");

    Ok(())
}

async fn handle_client_stream(
    mut client_conn: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    // TODO: Get the port this stream is linked to.
    // Link the client / user stream together
    todo!()
    // let stream_client_conn = client_conn.unwrap();
    // let request_recv = Arc::new(RwLock::new(request_recv));
    // let request_send = Arc::new(RwLock::new(request_send));
    // let handle_one = spawn_stream_sync(
    //     stream_client_conn.0,
    //     request_send,
    //     "localsrv -> client".into(),
    // );
    // let handle_two = spawn_stream_sync(
    //     request_recv,
    //     stream_client_conn.1,
    //     "client -> localsrv".into(),
    // );

    // try_join!(handle_two)?.0.unwrap();
    // handle_one.abort();
}

async fn handle_client(mut client_conn: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 1024];

    let count = client_conn.read(&mut buffer).await.unwrap();

    #[cfg(feature = "localhost")]
    let new_host = format!("{}", String::from_utf8_lossy(&buffer[..count]));
    #[cfg(not(feature = "localhost"))]
    let new_host = format!(
        "{}.rgrok.blackfoot.dev",
        String::from_utf8_lossy(&buffer[..count])
    );
    #[cfg(feature = "ingress")]
    ingress::expose_subdomain(&new_host).await?;
    {
        debug!("Acquiring lock for : ROUTES.read().await");
        let mut routes_w = ROUTES.write().await;
        debug!("Done: ROUTES.read().await");

        let (recv, send) = client_conn.into_split();
        routes_w.insert(
            new_host.trim().to_string(),
            (Arc::new(RwLock::new(recv)), Arc::new(RwLock::new(send))),
        );
        info!("Inserted new route: {new_host}");
    }

    Ok(())
}
