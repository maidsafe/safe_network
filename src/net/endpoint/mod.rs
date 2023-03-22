// Copyright 2020 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under the MIT license <LICENSE-MIT
// http://opensource.org/licenses/MIT> or the Modified BSD license <LICENSE-BSD
// https://opensource.org/licenses/BSD-3-Clause>, at your option. This file may not be copied,
// modified, or distributed except according to those terms. Please review the Licences for the
// specific language governing permissions and limitations relating to use of the SAFE Network
// Software.

mod builder;

use builder::EndpointBuilder;
use builder::SERVER_NAME;

use super::{connection::Connection, error::ConnectionError};
use std::net::SocketAddr;

/// Endpoint instance which can be used to communicate with peers.
#[derive(Clone)]
pub struct Endpoint {
    pub(crate) inner: quinn::Endpoint,
    #[allow(dead_code)]
    pub(crate) local_addr: SocketAddr,
}

impl Endpoint {
    pub async fn accept(&self) -> Option<quinn::Connecting> {
        self.inner.accept().await
    }

    /// Establish new connection to peer.
    pub async fn connect(&self, addr: &SocketAddr) -> Result<Connection, ConnectionError> {
        let connecting = match self.inner.connect(*addr, SERVER_NAME) {
            Ok(conn) => Ok(conn),
            Err(error) => Err(ConnectionError::from(error)),
        }?;

        let new_conn = match connecting.await {
            Ok(new_conn) => Ok(Connection::new(new_conn)),
            Err(error) => Err(ConnectionError::from(error)),
        }?;

        Ok(new_conn)
    }

    // /// Close all the connections of this endpoint immediately and stop accepting new connections.
    // pub fn close(&self) {
    //     self.inner.close(0_u32.into(), b"Endpoint closed")
    // }

    /// Builder to create an `Endpoint`.
    pub fn builder() -> EndpointBuilder {
        EndpointBuilder::default()
    }
}
