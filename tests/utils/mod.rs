use std::{thread, time::Duration};

use log::info;
use rustgrok::{
    config::{BINDING_ADDR_CLIENT, BINDING_ADDR_CLIENT_USER_STREAM, BINDING_ADDR_FRONT},
    handler,
};
use tokio::net::TcpListener;
use tokio::{io::AsyncWriteExt, task::JoinHandle};

pub async fn start_server() -> Result<(), Box<dyn std::error::Error>> {
    let client = TcpListener::bind(BINDING_ADDR_CLIENT).await?;
    info!("[SERVER] Client server started on {BINDING_ADDR_CLIENT}");
    let client_streams = TcpListener::bind(BINDING_ADDR_CLIENT_USER_STREAM).await?;
    info!("[SERVER] User stream from Client server started on {BINDING_ADDR_CLIENT_USER_STREAM}");
    let receiver = TcpListener::bind(BINDING_ADDR_FRONT).await.unwrap();
    info!("[SERVER] Front server started on {BINDING_ADDR_FRONT}");

    let client_handler = tokio::spawn(async move {
        while let Ok((client, _)) = client.accept().await {
            info!("[SERVER] client connected");
            tokio::spawn(async move {
                handler::handle_client(client).await.unwrap();
            });
        }
    });

    let client_streams = tokio::spawn(async move {
        while let Ok((client_stream, _)) = client_streams.accept().await {
            info!("[SERVER] New client stream");
            tokio::spawn(async move {
                handler::handle_client_stream(client_stream).await.unwrap();
            });
        }
    });

    let receiver_handler = tokio::spawn(async move {
        while let Ok((request, socket)) = receiver.accept().await {
            info!("[SERVER] Incoming request user_port: {:?}", socket.port());
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

const EXAMPLE_PAYLOAD: &str = r#"HTTP/1.1 200 OK
Last-Modified: Wed, 22 Jul 2009 19:15:56 GMT
Content-Length: 53
Content-Type: text/html
Connection: Closed

<html>
<body>
<h1>Hello, World!</h1>
</body>
</html>
"#;

/// Start an app that will listen on 4444
///
/// It will return a basic hello world page [EXAMPLE_PAYLOAD]
pub fn spawn_mock_app() -> JoinHandle<()> {
    tokio::spawn(async move {
        let app = TcpListener::bind("0.0.0.0:4444").await.unwrap();

        while let Ok((mut client, _)) = app.accept().await {
            info!("[APP] client connected to the app");
            thread::sleep(Duration::from_millis(100));
            client.write_all(EXAMPLE_PAYLOAD.as_bytes()).await.unwrap();
            client.flush().await.unwrap();
            client.shutdown().await.unwrap();
            info!("[APP] disconnected client from the app");
        }
    })
}
