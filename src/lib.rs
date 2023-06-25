pub mod ingress;

use std::{
    io::{self, ErrorKind},
    sync::Arc,
};

use log::{error, info};
use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::RwLock,
    task::JoinHandle,
};

pub type StreamRwTuple = (Arc<RwLock<OwnedReadHalf>>, Arc<RwLock<OwnedWriteHalf>>);

pub fn spawn_stream_sync(
    recv: Arc<RwLock<OwnedReadHalf>>,
    send: Arc<RwLock<OwnedWriteHalf>>,
    name: String,
) -> JoinHandle<io::Result<()>> {
    info!("Proxy start: {}", name);
    tokio::spawn(async move {
        // looks like the max transferred is 32_768, 2 times that to account for weird things /shrug
        let mut buff = vec![0; 32_768 * 2];
        let recv = recv.read().await;
        let mut send = send.write().await;
        loop {
            match recv.try_read(&mut buff) {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(0) => {
                    info!("Proxy stop: {name}");
                    // Shutting down the connection
                    let _ = send.shutdown().await;
                    return Ok(()) as io::Result<()>;
                }
                Ok(n) => {
                    info!("Proxy {name}, received data {n} bytes");
                    // Copy the data back to socket
                    send.write_all(&buff[..n]).await?;
                }
                Err(e) => {
                    // Unexpected socket error. There isn't much we can do
                    // here so just stop processing.
                    if !matches!(e.kind(), ErrorKind::WouldBlock) {
                        error!("Proxy error: {e}");
                        return Err(e);
                    }
                }
            }
        }
    })
}
