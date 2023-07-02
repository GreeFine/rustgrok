use std::{
    io::{self, ErrorKind},
    thread,
    time::Duration,
};

use clap::{arg, command, Parser};
use log::{error, info};
use rustgrok::{config, method};
use tokio::io::AsyncWriteExt;

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

#[tokio::main]
async fn main() -> Result<(), io::Error> {
    std::env::set_var("RUST_LOG", "client,rustgrok");
    pretty_env_logger::init();

    let args: Args = Args::parse();

    let local_app_addr = format!("127.0.0.1:{}", args.port);
    let mut server = method::connect_socket(config::PROXY_SERVER).await?;
    info!("Connected to server {}", config::PROXY_SERVER);

    server
        .write_all(config::API_KEY.as_bytes())
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
                method::client::spawn_new_stream(received_port, local_app_addr.clone());
                thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                if !matches!(e.kind(), ErrorKind::WouldBlock) {
                    error!("Proxy error: {e}");
                    break Err(e);
                };
            }
        };
        thread::sleep(Duration::from_millis(25))
    }
}
