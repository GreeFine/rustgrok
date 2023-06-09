mod utils;

use std::{io::ErrorKind, thread, time::Duration};

use rustgrok::{self, method};

use reqwest;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_start_server_connect_client() {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_pass_request() {
    std::env::set_var("RUST_LOG", "debug");
    pretty_env_logger::init();

    let _server: tokio::task::JoinHandle<()> = tokio::spawn(async move {
        utils::start_server().await.unwrap();
    });

    // TODO: better wait for ready
    thread::sleep(Duration::from_millis(200));

    let server_conn = method::client::connect_with_server("test_host")
        .await
        .unwrap();
    let buff = &mut [0u8; 0];
    let ready = server_conn.try_read(buff);
    assert!(match ready {
        Ok(n) => {
            n != 0
        }
        Err(e) => {
            matches!(e.kind(), ErrorKind::WouldBlock)
        }
    });
    let client = reqwest::Client::new();
    let body = client
        .get("http://localhost:8080")
        .header("Host", "test_host")
        .send()
        .await;

    let port = method::client::wait_for_stream_request(&server_conn).unwrap();
    assert!(port > 0);
}
