// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::core::{ConnectedPoint, Endpoint, Multiaddr};
use libp2p::identity::PeerId;
use libp2p::swarm::{
    behaviour::ConnectionEstablished, dummy, ConnectionClosed, ConnectionDenied, ConnectionId,
    FromSwarm, NetworkBehaviour, PollParameters, THandler, THandlerInEvent, THandlerOutEvent,
    ToSwarm,
};
use std::collections::HashSet;
use std::fmt;
use std::task::{Context, Poll};

#[derive(Default)]
pub struct Behaviour {
    inbound_connections: HashSet<PeerId>,
    outbound_connections: HashSet<PeerId>,
}

impl Behaviour {
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectionAlreadyEstablished {
    Inbound,
    Outbound,
}
impl std::error::Error for ConnectionAlreadyEstablished {}

impl fmt::Display for ConnectionAlreadyEstablished {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "connection is already established in {} direction",
            match self {
                ConnectionAlreadyEstablished::Inbound => "inbound",
                ConnectionAlreadyEstablished::Outbound => "outbound",
            }
        )
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = ();

    fn handle_pending_inbound_connection(
        &mut self,
        _: ConnectionId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        Ok(())
    }

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.inbound_connections.contains(&peer) {
            return Err(ConnectionDenied::new(ConnectionAlreadyEstablished::Inbound));
        }

        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _: ConnectionId,
        _: Option<PeerId>,
        _: &[Multiaddr],
        _: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        Ok(vec![])
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        if self.outbound_connections.contains(&peer) {
            return Err(ConnectionDenied::new(
                ConnectionAlreadyEstablished::Outbound,
            ));
        }

        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionClosed(ConnectionClosed {
                peer_id, endpoint, ..
            }) => match endpoint {
                ConnectedPoint::Dialer { .. } => {
                    self.outbound_connections.remove(&peer_id);
                }
                ConnectedPoint::Listener { .. } => {
                    self.inbound_connections.remove(&peer_id);
                }
            },
            FromSwarm::ConnectionEstablished(ConnectionEstablished {
                peer_id, endpoint, ..
            }) => match endpoint {
                ConnectedPoint::Dialer { .. } => {
                    self.outbound_connections.insert(peer_id);
                }
                ConnectedPoint::Listener { .. } => {
                    self.inbound_connections.insert(peer_id);
                }
            },
            FromSwarm::DialFailure(_) => {}
            FromSwarm::AddressChange(_) => {}
            FromSwarm::ListenFailure(_) => {}
            FromSwarm::NewListener(_) => {}
            FromSwarm::NewListenAddr(_) => {}
            FromSwarm::ExpiredListenAddr(_) => {}
            FromSwarm::ListenerError(_) => {}
            FromSwarm::ListenerClosed(_) => {}
            FromSwarm::NewExternalAddrCandidate(_) => {}
            FromSwarm::ExternalAddrExpired(_) => {}
            FromSwarm::ExternalAddrConfirmed(_) => {}
        }
    }

    fn on_connection_handler_event(
        &mut self,
        _id: PeerId,
        _: ConnectionId,
        _: THandlerOutEvent<Self>,
    ) {
    }

    fn poll(
        &mut self,
        _: &mut Context<'_>,
        _: &mut impl PollParameters,
    ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
        Poll::Pending
    }
}
