use std::{collections::HashMap, sync::Arc};

use log::info;
use rustgrok::{spawn_stream_sync, StreamRwTuple};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{tcp::OwnedReadHalf, TcpListener, TcpStream},
    sync::RwLock,
    try_join,
};

use lazy_static::lazy_static;

lazy_static! {
    static ref ROUTES: RwLock<HashMap<String, StreamRwTuple>> = RwLock::new(HashMap::new());
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    const BINDING_ADDR_CLIENT: &str = "0.0.0.0:3000";
    let proxy_server_client = TcpListener::bind(BINDING_ADDR_CLIENT).await?;
    info!("Client server started on {BINDING_ADDR_CLIENT}");
    const BINDING_ADDR_FRONT: &str = "0.0.0.0:8080";
    let proxy_server_front = TcpListener::bind(BINDING_ADDR_FRONT).await.unwrap();
    info!("Front server started on {BINDING_ADDR_FRONT}");

    let client_handler = tokio::spawn(async move {
        while let Ok((client, _)) = proxy_server_client.accept().await {
            info!("Client connected");
            tokio::spawn(async move {
                handle_client(client).await.unwrap();
            });
        }
    });

    let front_handler = tokio::spawn(async move {
        while let Ok((client, _)) = proxy_server_front.accept().await {
            info!("Incoming request");
            tokio::spawn(async move {
                handle_request(client).await.unwrap();
            });
        }
    });

    for jh in [client_handler, front_handler] {
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

async fn handle_request(mut client_conn: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let (mut client_recv, mut client_send) = client_conn.into_split();

    let server_conn = {
        let route_r = ROUTES.read().await;
        let host = get_host(&mut client_recv).await;
        info!("Request for host: {host:?}");
        host.and_then(|destination| route_r.get(&destination).cloned())
    };

    if server_conn.is_none() {
        info!("did not found associated route to request");
        client_send.shutdown().await.unwrap();
        return Ok(());
    }
    let stream_server_conn = server_conn.unwrap();
    let client_recv = Arc::new(RwLock::new(client_recv));
    let client_send = Arc::new(RwLock::new(client_send));
    let handle_one = spawn_stream_sync(stream_server_conn.0, client_send);
    let handle_two = spawn_stream_sync(client_recv, stream_server_conn.1);

    try_join!(handle_two)?;
    handle_one.abort();

    dbg!("request done here on server");
    Ok(())
}

async fn handle_client(mut client_conn: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 1024];

    let count = client_conn.read(&mut buffer).await.unwrap();
    let new_host = String::from_utf8_lossy(&buffer[..count]);

    {
        let mut route_w = ROUTES.write().await;
        let (recv, send) = client_conn.into_split();
        route_w.insert(
            new_host.trim().to_string(),
            (Arc::new(RwLock::new(recv)), Arc::new(RwLock::new(send))),
        );
        info!("Inserted new route: {new_host}");
    }

    Ok(())
}
