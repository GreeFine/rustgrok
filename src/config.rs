use std::collections::HashMap;

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
}

#[cfg(not(feature = "deployed"))]
pub const PROXY_SERVER: &str = "127.0.0.1:3000";
#[cfg(not(feature = "deployed"))]
pub const PROXY_SERVER_STREAMS: &str = "127.0.0.1:3001";

#[cfg(feature = "deployed")]
pub const PROXY_SERVER: &str = "34.159.32.3:3000";
#[cfg(feature = "deployed")]
pub const PROXY_SERVER_STREAMS: &str = "34.159.32.3:3001";

pub const BINDING_ADDR_CLIENT: &str = "0.0.0.0:3000";
pub const BINDING_ADDR_CLIENT_USER_STREAM: &str = "0.0.0.0:3001";
pub const BINDING_ADDR_FRONT: &str = "0.0.0.0:8080";
