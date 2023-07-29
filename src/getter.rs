use log::error;
use tokio::{
    io::AsyncReadExt,
    net::{tcp::OwnedReadHalf, TcpStream},
};

use crate::config::{self};

/// Get the host send by the client
///
/// If the feature `deployed` is enabled, the host will be suffixed with `.rgrok.blackfoot.dev`
pub async fn get_client_host(client: &mut TcpStream) -> String {
    let mut buffer = [0; 1024];
    let count = client.read(&mut buffer).await.unwrap();

    // FIXME: Check if host is valid
    #[cfg(not(feature = "deployed"))]
    let new_host = format!("{}", String::from_utf8_lossy(&buffer[..count]));
    #[cfg(feature = "deployed")]
    let new_host = format!(
        "{}.rgrok.blackfoot.dev",
        String::from_utf8_lossy(&buffer[..count])
    );
    new_host
}

/// Get the host send by the user, this is used to identify the route we want to attach it to
pub async fn get_user_host<'a>(client_recv: &mut OwnedReadHalf) -> Option<String> {
    let mut buffer = [0; 1024];

    let read = client_recv.peek(&mut buffer).await.unwrap();
    let raw_http_request = String::from_utf8_lossy(&buffer[..read]);

    let captures = config::HOST_EXTRACT.captures(&raw_http_request);

    captures.and_then(|captures| captures.name("hostname").map(|m| m.as_str().to_string()))
}

/// Get the port send by the user after connecting a new stream
///
/// This port is used to identify the `USER_REQUEST_WAITING` we want to attach it to
pub async fn get_target_port<'a>(client_recv: &mut OwnedReadHalf) -> Result<u16, ()> {
    let mut buff = [0_u8; 2];

    let read = client_recv.read(&mut buff).await.unwrap();
    if read != 2 {
        error!("Unable to read the port from new stream from client");
        return Err(());
    }
    Ok(u16::from_be_bytes(buff))
}

#[test]
fn test_get_host() {
    const RAW_HEADER: &str = "GET / HTTP/1.1\r\nHost: jambon.rgrok.blackfoot.dev\r\nUser-Agent: curl/7.68.0\r\nAccept: */*\r\nX-";
    let captures = config::HOST_EXTRACT.captures(RAW_HEADER);
    assert_eq!(
        captures.unwrap().name("hostname").unwrap().as_str(),
        "jambon.rgrok.blackfoot.dev"
    );
}
