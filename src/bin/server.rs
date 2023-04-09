use std::{collections::HashMap, io, ops::DerefMut, sync::Arc};

use log::info;
use rustgrok::spawn_stream_sync;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf, ReadHalf},
        TcpListener, TcpStream,
    },
    sync::RwLock,
    try_join,
};

use lazy_static::lazy_static;

type StreamRwTuple = (RwLock<OwnedReadHalf>, RwLock<OwnedWriteHalf>);

lazy_static! {
    static ref ROUTES: RwLock<HashMap<String, Arc<TcpStream>>> = RwLock::new(HashMap::new());
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
    let (server_recv, server_send) = server_conn.cloned().unwrap().into_split();
    // let handle_one = async { tokio::io::copy(server_recv.deref_mut(), &mut client_send).await };
    let handle_two = spawn_stream_sync(server_recv, client_send);
    // let handle_two = async { tokio::io::copy(&mut client_recv, server_send).await };
    let handle_two = spawn_stream_sync(client_recv, server_send);

    try_join!(handle_two)?;

    Ok(())
}

async fn handle_client(mut client_conn: TcpStream) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer = [0; 1024];

    let count = client_conn.read(&mut buffer).await.unwrap();
    let new_host = String::from_utf8_lossy(&buffer[..count]);

    {
        let mut route_w = ROUTES.write().await;
        route_w.insert(new_host.trim().to_string(), Arc::new(client_conn));
        info!("Inserted new route: {new_host}");
    }

    Ok(())
}
