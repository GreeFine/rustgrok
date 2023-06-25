use std::{
    io::{self, ErrorKind},
    sync::Arc,
    thread,
    time::Duration,
};

use clap::{arg, command, Parser};
use futures::future;
use log::{error, info};
use tokio::{
    io::AsyncWriteExt,
    net::{TcpSocket, TcpStream},
    sync::RwLock,
};

use rustgrok::spawn_stream_sync;

#[cfg(not(feature = "localhost"))]
const PROXY_SERVER: &str = "34.159.32.3:3000";
#[cfg(not(feature = "localhost"))]
const PROXY_SERVER_STREAMS: &str = "34.159.32.3:3001";

#[cfg(feature = "localhost")]
const PROXY_SERVER: &str = "127.0.0.1:3000";
#[cfg(feature = "localhost")]
const PROXY_SERVER_STREAMS: &str = "127.0.0.1:3001";

/// FIXME: load this from env
const API_KEY: &str = "SuperSecret";

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    port: u16,
    #[arg(short, long)]
    name: String,
}

async fn connect_socket(addr: impl AsRef<str>) -> Result<TcpStream, io::Error> {
    let addr = addr.as_ref().parse().unwrap();
    let socket = TcpSocket::new_v4()?;
    let stream = socket.connect(addr).await?;
    Ok(stream)
}

fn spawn_stream(port: u16, local_app_addr: String) {
    tokio::spawn(async move {
        let app_stream: TcpStream = connect_socket(&local_app_addr).await.unwrap();
        info!("Connected to app {local_app_addr}");
        let server_stream = connect_socket(PROXY_SERVER_STREAMS).await.unwrap();
        info!("Connected to server for stream port {port}");

        let (app_recv, app_send) = app_stream.into_split();
        let (server_recv, mut server_send) = server_stream.into_split();

        server_send
            .write_all(API_KEY.as_bytes())
            .await
            .expect("sending api key to server");
        server_send.write_all(&port.to_be_bytes()).await.unwrap();

        let app_recv = Arc::new(RwLock::new(app_recv));
        let app_send = Arc::new(RwLock::new(app_send));
        let server_recv = Arc::new(RwLock::new(server_recv));
        let server_send = Arc::new(RwLock::new(server_send));

        let app_flow = spawn_stream_sync(app_recv, server_send, "app -> server".into());
        let server_flow = spawn_stream_sync(server_recv, app_send, "server -> app".into());

        // Wait for only one of the two stream to finish
        let _ = future::select_all(vec![app_flow, server_flow]).await;

        info!("stream done for {port}");
    });
}

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    std::env::set_var("RUST_LOG", "client,rustgrok");
    pretty_env_logger::init();

    let args: Args = Args::parse();

    let local_app_addr = format!("127.0.0.1:{}", args.port);
    let mut server = connect_socket(PROXY_SERVER).await?;
    info!("Connected to server {PROXY_SERVER}");

    server
        .write_all(API_KEY.as_bytes())
        .await
        .expect("sending api key to server");
    server
        .write_all(args.name.as_bytes())
        .await
        .expect("sending host to server");

    let mut buff = [0; 2];
    loop {
        let read = server.try_read(&mut buff);
        match read {
            Ok(n) => {
                if n == 0 {
                    info!("Connection closed");
                    break Ok(());
                }
                let received_port = u16::from_be_bytes(buff);
                info!("received_port: {received_port}");
                spawn_stream(received_port, local_app_addr.clone());
                info!("after spawn ?");
                thread::sleep(Duration::from_secs(1));
            }
            Err(e) => {
                if !matches!(e.kind(), ErrorKind::WouldBlock) {
                    error!("Proxy error: {e}");
                    break Err(e);
                };
            }
        };
    }
}
