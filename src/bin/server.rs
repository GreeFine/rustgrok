use log::info;
#[cfg(feature = "ingress")]
use rustgrok::ingress;
use rustgrok::{
    config::{BINDING_ADDR_CLIENT, BINDING_ADDR_CLIENT_USER_STREAM, BINDING_ADDR_FRONT},
    handler,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
                handler::handle_user_request(request, socket).await.unwrap();
            });
        }
    });

    for jh in [client_handler, receiver_handler, client_streams] {
        jh.await.unwrap();
    }

    Ok(())
}
