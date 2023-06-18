use std::sync::Arc;

use clap::{arg, command, Parser};
use log::info;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpSocket, TcpStream},
    sync::RwLock,
    try_join,
};

use rustgrok::spawn_stream_sync;

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

async fn connect_socket(addr: impl AsRef<str>) -> Result<TcpStream, Box<dyn std::error::Error>> {
    let addr = addr.as_ref().parse().unwrap();
    let socket = TcpSocket::new_v4()?;
    let stream = socket.connect(addr).await?;
    Ok(stream)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    let args: Args = Args::parse();

    #[cfg(not(feature = "localhost"))]
    const PROXY_SERVER: &str = "34.159.32.3:3000";
    #[cfg(feature = "localhost")]
    const PROXY_SERVER: &str = "127.0.0.1:3000";
    let binding_addr_client = format!("127.0.0.1:{}", args.port);
    loop {
        // TODO: Would be better to keep this connection open.
        // In order to do that the server need to communicate when the client terminated the request.
        let mut proxy_localsrv_stream = connect_socket(PROXY_SERVER).await?;
        info!("Connected to server {PROXY_SERVER}");

        proxy_localsrv_stream
            .write_all(args.name.as_bytes())
            .await
            .expect("sending host to server");
        let (localsrv_recv, localsrv_send) = proxy_localsrv_stream.into_split();

        let localsrv_recv = Arc::new(RwLock::new(localsrv_recv));
        let localsrv_send = Arc::new(RwLock::new(localsrv_send));
        let proxy_app_stream = connect_socket(&binding_addr_client).await?;
        info!("Connected to app {binding_addr_client}");
        let (app_recv, app_send) = proxy_app_stream.into_split();

        let handle_one = spawn_stream_sync(
            localsrv_recv.clone(),
            Arc::new(RwLock::new(app_send)),
            "localsrv -> app".into(),
        );
        let handle_two = spawn_stream_sync(
            Arc::new(RwLock::new(app_recv)),
            localsrv_send.clone(),
            "app -> localsrv".into(),
        );

        try_join!(handle_two)?.0.unwrap();
        handle_one.abort();
        dbg!("request done here on client");
    }
}
