use std::io::{self, ErrorKind};

use tokio::{
    io::AsyncWriteExt,
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    task::JoinHandle,
};

pub fn spawn_stream_sync(
    recv: OwnedReadHalf,
    mut send: OwnedWriteHalf,
) -> JoinHandle<io::Result<()>> {
    tokio::spawn(async move {
        let mut buf = vec![0; 1024];
        loop {
            match recv.try_read(&mut buf) {
                // Return value of `Ok(0)` signifies that the remote has
                // closed
                Ok(0) => return Ok(()) as io::Result<()>,
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
