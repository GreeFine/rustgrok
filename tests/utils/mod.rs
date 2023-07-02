use std::{io::ErrorKind, thread, time::Duration};

use log::{error, info};
use rustgrok::{config, method};
use rustgrok::{
    config::{BINDING_ADDR_CLIENT, BINDING_ADDR_CLIENT_USER_STREAM, BINDING_ADDR_FRONT},
    handler,
};
use tokio::io::AsyncWriteExt;
use tokio::net::TcpListener;

pub async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    std::env::set_var("RUST_LOG", "server,rustgrok");
    pretty_env_logger::init();

    let client = TcpListener::bind(BINDING_ADDR_CLIENT).await?;
    info!("Client server started on {BINDING_ADDR_CLIENT}");
    let client_streams = TcpListener::bind(BINDING_ADDR_CLIENT_USER_STREAM).await?;
    info!("User stream from Client server started on {BINDING_ADDR_CLIENT_USER_STREAM}");
    let receiver = TcpListener::bind(BINDING_ADDR_FRONT).await.unwrap();
    info!("Front server started on {BINDING_ADDR_FRONT}");

    let client_handler = tokio::spawn(async move {
        while let Ok((client, _)) = client.accept().await {
            info!("Client connected");
            tokio::spawn(async move {
                handler::handle_client(client).await.unwrap();
            });
        }
    });

    let client_streams = tokio::spawn(async move {
        while let Ok((client_stream, _)) = client_streams.accept().await {
            info!("New Client Stream");
            tokio::spawn(async move {
                handler::handle_client_stream(client_stream).await.unwrap();
            });
        }
    });

    let receiver_handler = tokio::spawn(async move {
        while let Ok((request, socket)) = receiver.accept().await {
            info!("Incoming request user_port: {:?}", socket.port());
            tokio::spawn(async move {
                handler::handle_request(request, socket).await.unwrap();
            });
        }
    });

    for jh in [client_handler, receiver_handler, client_streams] {
        jh.await.unwrap();
    }

    Ok(())
}

pub async fn start_client(app_port: usize, host_name: &str) -> Result<(), std::io::Error> {
    std::env::set_var("RUST_LOG", "client,rustgrok");
    pretty_env_logger::init();

    let local_app_addr = format!("127.0.0.1:{}", app_port);
    let mut server = method::connect_socket(config::PROXY_SERVER).await?;
    info!("Connected to server {}", config::PROXY_SERVER);

    server
        .write_all(config::API_KEY.as_bytes())
        .await
        .expect("sending api key to server");
    server
        .write_all(host_name.as_bytes())
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
