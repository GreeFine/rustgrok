use std::collections::HashMap;

use regex::{Regex, RegexBuilder};
use tokio::sync::RwLock;

use crate::{ClientConnection, StreamRwTuple};

lazy_static! {
  /// Routes registered by the clients, the connection associated is used to ask for a `CLIENT_STREAM`
  /// #
  pub static ref ROUTES: RwLock<HashMap<String, ClientConnection>> = RwLock::new(HashMap::new());
  /// List of users request that are waiting for the client to create the stream
  /// #
  pub static ref USER_REQUEST_WAITING: RwLock<HashMap<u16, StreamRwTuple>> = RwLock::new(HashMap::new());
  /// FIXME: load this from env
  pub static ref API_KEY: String = std::env::var("API_KEY").unwrap_or("gub_tEPmAGMzb9SQxzTzh9ZU95Wtj6uP".to_string());
  /// Regex used to extract the host from the request,
  pub static ref HOST_EXTRACT: Regex = RegexBuilder::new("^.*\n?host: (?P<hostname>[A-z-_.0-9]{0,32}(:[0-9]{1,6})?)(\r)?\n").case_insensitive(true).build().unwrap();
}

/// The proxy server address
#[cfg(not(feature = "deployed"))]
pub const PROXY_SERVER: &str = "127.0.0.1:3000";
/// The proxy server address for streams
#[cfg(not(feature = "deployed"))]
pub const PROXY_SERVER_STREAMS: &str = "127.0.0.1:3001";

/// The proxy server address
#[cfg(feature = "deployed")]
pub const PROXY_SERVER: &str = "34.159.32.3:3000";
/// The proxy server address for streams
#[cfg(feature = "deployed")]
pub const PROXY_SERVER_STREAMS: &str = "34.159.32.3:3001";

/// Binding address for the server client entrypoint
pub const BINDING_ADDR_CLIENT: &str = "0.0.0.0:3000";
/// Binding address for the server client entrypoint for streams
pub const BINDING_ADDR_CLIENT_USER_STREAM: &str = "0.0.0.0:3001";
/// Binding address for the server front entrypoint
pub const BINDING_ADDR_FRONT: &str = "0.0.0.0:8080";

/// Size of the buffers used to read/write from/to the streams
/// Here 1Mb
pub const BUFFER_STREAM_SIZE: usize = 1024 * 1024;
