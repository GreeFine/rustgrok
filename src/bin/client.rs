use clap::{arg, command, Parser};
use log::info;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpSocket, TcpStream},
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

    let args = Args::parse();

    const PROXY_SERVER: &str = "127.0.0.1:3000";
    let mut proxy_server_stream = connect_socket(PROXY_SERVER).await?;
    info!("Connected to server {PROXY_SERVER}");

    proxy_server_stream
        .write_all(args.name.as_bytes())
        .await
        .expect("sending host to server");
    let (server_recv, server_send) = proxy_server_stream.into_split();

    let binding_addr_client = format!("127.0.0.1:{}", args.port);
    let proxy_app_stream = connect_socket(&binding_addr_client).await?;
    info!("Connected to app {binding_addr_client}");
    let (app_recv, app_send) = proxy_app_stream.into_split();

    let _handle_one = spawn_stream_sync(server_recv, app_send);
    let handle_two = spawn_stream_sync(app_recv, server_send);

    try_join!(handle_two)?.0.unwrap();

    Ok(())
}
