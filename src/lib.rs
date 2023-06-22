pub mod ingress;

use std::{
    io::{self, ErrorKind},
    sync::Arc,
};

use log::info;
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
        let mut buf = vec![0; 1_000_000];
        let recv = recv.read().await;
        let mut send = send.write().await;
        loop {
            match recv.try_read(&mut buf) {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(0) => {
                    info!("Proxy stop: {}", name);
                    // TODO: we probably want to send a disconnect over channels
                    let _ = send.shutdown().await;
                    return Ok(()) as io::Result<()>;
                }
                Ok(n) => {
                    // Copy the data back to socket
                    send.write_all(&buf[..n]).await?;
                }
                Err(e) => {
                    // Unexpected socket error. There isn't much we can do
                    // here so just stop processing.
                    if !matches!(e.kind(), ErrorKind::WouldBlock) {
                        return Err(e);
                    }
                }
            }
        }
    })
}
