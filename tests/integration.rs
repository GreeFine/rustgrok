mod utils;

use std::{io::ErrorKind, thread, time::Duration};

use futures::future::join_all;
use rustgrok::{self, method};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpSocket},
    task::yield_now,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_start_server_connect_client() {
    std::env::set_var("RUST_LOG", "debug");
    console_subscriber::init();

    let _server = tokio::spawn(async move {
        utils::start_server().await.unwrap();
    });

    // TODO: better wait for ready
    thread::sleep(Duration::from_millis(200));

    let client = method::client::connect_with_server("test").await.unwrap();
    let buff = &mut [0u8; 0];
    let ready = client.try_read(buff);
    assert!(match ready {
        Ok(n) => {
            n != 0
        }
        Err(e) => {
            matches!(e.kind(), ErrorKind::WouldBlock)
        }
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_pass_request() {
    std::env::set_var("RUST_LOG", "debug");
    console_subscriber::init();

    let server: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        utils::start_server().await.unwrap();
    });
    let _app = utils::spawn_mock_app();

    // TODO: better wait for ready
    thread::sleep(Duration::from_millis(200));

    let requests: Vec<_> = (0..6)
        .map(|index| {
            tokio::spawn(async move {
                let mut server_conn =
                    method::client::connect_with_server(&format!("test_host_{index}"))
                        .await
                        .unwrap();

                let client = reqwest::Client::new();
                let request = tokio::spawn(async move {
                    client
                        .get("http://localhost:8080")
                        .header("Host", format!("test_host_{index}"))
                        .send()
                        .await
                });

                let port = method::client::wait_for_stream_request(&mut server_conn)
                    .await
                    .unwrap();

                let stream = method::client::spawn_new_stream(port, "127.0.0.1:4444".to_string());

                let res = request.await.unwrap().unwrap();
                let body = res.text().await.unwrap();
                assert_eq!(
                    body,
                    "<html>\n<body>\n<h1>Hello, World!</h1>\n</body>\n</html>\n"
                );
                stream.abort();
            })
        })
        .collect();

    join_all(requests).await;
    server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn test_request_while_idle() {
    console_subscriber::init();

    let a = tokio::spawn(async move {
        let server = TcpListener::bind("0.0.0.0:8888").await.unwrap();

        while let Ok((mut client, _)) = server.accept().await {
            dbg!("[SERVER] client connected");
            yield_now().await;

            tokio::spawn(async move {
                client.readable().await.unwrap();
                let mut buff = [0; 32];
                let _read = dbg!(client.read(&mut buff).await);
            });
        }
    });

    let a_abort = a.abort_handle();
    let b = tokio::spawn(async move {
        let addr = "127.0.0.1:8888".parse().unwrap();
        let socket = TcpSocket::new_v4().unwrap();
        let mut server_con = socket.connect(addr).await.unwrap();

        server_con.writable().await.unwrap();
        server_con
            .write_all(b"a_test_a_test_a_test_a_test_")
            .await
            .unwrap();

        let mut buff = [0; 2];
        let _ = loop {
            let read = server_con.try_read(&mut buff);
            match read {
                Ok(n) => {
                    if n == 0 {
                        dbg!("[CLIENT] Connection closed");
                        break Err(None);
                    }
                    let received_port = u16::from_be_bytes(buff);
                    dbg!("[CLIENT] received_port: {received_port}");
                    break Ok(received_port);
                }
                Err(e) => {
                    if !matches!(e.kind(), ErrorKind::WouldBlock) {
                        dbg!("Proxy error: {e}");
                        break Err(Some(e));
                    };
                }
            };
            thread::sleep(Duration::from_millis(125));
        };

        a_abort.abort();
    });
    join_all([a, b]).await;
}
