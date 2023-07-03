use log::error;
use tokio::{io::AsyncReadExt, net::TcpStream};

use crate::config::API_KEY;

/// Check if the API key received in the stream, is the one we expect
pub async fn check_api_key(client: &mut TcpStream) -> Result<(), ()> {
    assert_eq!(
        API_KEY.len(),
        32,
        "api key is expected to be 32 bytes or less"
    );
    let mut buff = [0_u8; 32];

    let read = client.read(&mut buff).await.unwrap();
    if read != API_KEY.len() {
        error!("Unable to read the API_KEY from new stream from client");
        return Err(());
    } else if buff != API_KEY.as_bytes() {
        error!(
            "invalid API_KEY new stream from client, received: '{}'",
            String::from_utf8_lossy(&buff)
        );
        return Err(());
    }

    Ok(())
}
