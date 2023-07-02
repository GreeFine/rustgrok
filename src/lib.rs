//! Internal functions that are used by the client and server of rustgrok

#![warn(unused_extern_crates, missing_docs)]

#[macro_use]
extern crate lazy_static;

/// Minimal authentication method: an API_KEY exchange.
pub mod auth;
/// Configuration variables.
pub mod config;
/// Methods that extract information on the TCP connection
pub mod getter;
/// Handling of users,clients connections
pub mod handler;
/// Mostly stream handling functions
pub mod method;

/// Link with traefik to configure new ingress requirement from request host by the client
#[cfg(feature = "ingress")]
pub mod ingress;

use std::sync::Arc;

use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::RwLock,
};

/// [OwnedReadHalf] and [OwnedWriteHalf] inside [Arc]<[RwLock]> to pass around threads.
pub type StreamRwTuple = (Arc<RwLock<OwnedReadHalf>>, Arc<RwLock<OwnedWriteHalf>>);
/// [TcpStream] wrapped around [Arc]<[RwLock]>
pub type ClientConnection = Arc<RwLock<TcpStream>>;
