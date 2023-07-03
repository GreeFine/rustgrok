use std::io;

use clap::{arg, command, Parser};
use rustgrok::method;

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
    let server = method::client::connect_with_server(&local_app_addr)
        .await
        .unwrap();
    loop {
        let received_port = method::client::wait_for_stream_request(&server).unwrap();
        method::client::spawn_new_stream(received_port, local_app_addr.clone());
    }
}
