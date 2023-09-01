// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::{
    core::{ConnectedPoint, Endpoint, Multiaddr},
    identity::PeerId,
    swarm::{
        behaviour::{ConnectionEstablished, DialFailure, ListenFailure},
        dummy, ConnectionClosed, ConnectionDenied, ConnectionId, FromSwarm, NetworkBehaviour,
        PollParameters, THandler, THandlerInEvent, THandlerOutEvent, ToSwarm,
    },
};
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::task::{Context, Poll};

pub struct Behaviour {
    limits: ConnectionLimits,

    pending_inbound_connections: HashSet<ConnectionId>,
    pending_outbound_connections: HashSet<ConnectionId>,
    established_inbound_connections: HashSet<ConnectionId>,
    established_outbound_connections: HashSet<ConnectionId>,
    established_per_peer: HashMap<PeerId, HashSet<ConnectionId>>,
}

impl Behaviour {
    pub fn new(limits: ConnectionLimits) -> Self {
        Self {
            limits,
            pending_inbound_connections: Default::default(),
            pending_outbound_connections: Default::default(),
            established_inbound_connections: Default::default(),
            established_outbound_connections: Default::default(),
            established_per_peer: Default::default(),
        }
    }

    fn check_limit(
        &mut self,
        limit: Option<u32>,
        current: usize,
        kind: Kind,
    ) -> Result<(), ConnectionDenied> {
        let limit = limit.unwrap_or(u32::MAX);
        let current = current as u32;

        if current >= limit {
            return Err(ConnectionDenied::new(Exceeded { limit, kind }));
        }

        Ok(())
    }
}

/// A connection limit has been exceeded.
#[derive(Debug, Clone, Copy)]
pub struct Exceeded {
    limit: u32,
    kind: Kind,
}

impl Exceeded {
    pub fn limit(&self) -> u32 {
        self.limit
    }
}

impl fmt::Display for Exceeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "connection limit exceeded: at most {} {} are allowed",
            self.limit, self.kind
        )
    }
}

#[derive(Debug, Clone, Copy)]
enum Kind {
    PendingIncoming,
    PendingOutgoing,
    EstablishedIncoming,
    EstablishedOutgoing,
    EstablishedPerPeer,
    IncomingEstablishedPerPeer,
    OutgoingEstablishedPerPeer,
    EstablishedTotal,
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Kind::PendingIncoming => write!(f, "pending incoming connections"),
            Kind::PendingOutgoing => write!(f, "pending outgoing connections"),
            Kind::EstablishedIncoming => write!(f, "established incoming connections"),
            Kind::EstablishedOutgoing => write!(f, "established outgoing connections"),
            Kind::EstablishedPerPeer => write!(f, "established connections per peer"),
            Kind::IncomingEstablishedPerPeer => {
                write!(f, "incomding established connections per peer")
            }
            Kind::OutgoingEstablishedPerPeer => {
                write!(f, "outgoing established connections per peer")
            }
            Kind::EstablishedTotal => write!(f, "established connections"),
        }
    }
}

impl std::error::Error for Exceeded {}

/// The configurable connection limits.
#[derive(Debug, Clone, Default)]
pub struct ConnectionLimits {
    max_pending_incoming: Option<u32>,
    max_pending_outgoing: Option<u32>,
    max_established_incoming: Option<u32>,
    max_established_outgoing: Option<u32>,
    max_established_per_peer: Option<u32>,
    max_established_total: Option<u32>,
}

impl ConnectionLimits {
    /// Configures the maximum number of concurrently incoming connections being established.
    pub fn with_max_pending_incoming(mut self, limit: Option<u32>) -> Self {
        self.max_pending_incoming = limit;
        self
    }

    /// Configures the maximum number of concurrently outgoing connections being established.
    pub fn with_max_pending_outgoing(mut self, limit: Option<u32>) -> Self {
        self.max_pending_outgoing = limit;
        self
    }

    /// Configures the maximum number of concurrent established inbound connections.
    pub fn with_max_established_incoming(mut self, limit: Option<u32>) -> Self {
        self.max_established_incoming = limit;
        self
    }

    /// Configures the maximum number of concurrent established outbound connections.
    pub fn with_max_established_outgoing(mut self, limit: Option<u32>) -> Self {
        self.max_established_outgoing = limit;
        self
    }

    /// Configures the maximum number of concurrent established connections (both
    /// inbound and outbound).
    ///
    /// Note: This should be used in conjunction with
    /// [`ConnectionLimits::with_max_established_incoming`] to prevent possible
    /// eclipse attacks (all connections being inbound).
    pub fn with_max_established(mut self, limit: Option<u32>) -> Self {
        self.max_established_total = limit;
        self
    }

    /// Configures the maximum number of concurrent established connections per peer,
    /// regardless of direction (incoming or outgoing).
    pub fn with_max_established_per_peer(mut self, limit: Option<u32>) -> Self {
        self.max_established_per_peer = limit;
        self
    }
}

impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = ();

    fn handle_pending_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<(), ConnectionDenied> {
        self.check_limit(
            self.limits.max_pending_incoming,
            self.pending_inbound_connections.len(),
            Kind::PendingIncoming,
        )?;

        self.pending_inbound_connections.insert(connection_id);

        Ok(())
    }

    fn handle_established_inbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.pending_inbound_connections.remove(&connection_id);

        self.check_limit(
            self.limits.max_established_incoming,
            self.established_inbound_connections.len(),
            Kind::EstablishedIncoming,
        )?;
        // incoming
        self.check_limit(
            self.limits.max_established_per_peer,
            self.established_per_peer
                .get(&peer)
                .map(|connections| {
                    connections
                        .iter()
                        .filter(|id| self.established_inbound_connections.contains(id))
                        .count()
                })
                .unwrap_or(0),
            Kind::IncomingEstablishedPerPeer,
        )
        .map_err(|err| {
            error!(
                "{peer} has exeeced max inbound connections of GG {:?}",
                self.limits.max_established_per_peer
            );
            err
        })?;

        // outgoing
        self.check_limit(
            self.limits.max_established_per_peer,
            self.established_per_peer
                .get(&peer)
                .map(|connections| {
                    connections
                        .iter()
                        .filter(|id| self.established_outbound_connections.contains(id))
                        .count()
                })
                .unwrap_or(0),
            Kind::OutgoingEstablishedPerPeer,
        )
        .map_err(|err| {
            error!(
                "{peer} has exeeced max outbound connections of {:?}",
                self.limits.max_established_per_peer
            );
            err
        })?;
        self.check_limit(
            self.limits.max_established_total,
            self.established_inbound_connections.len()
                + self.established_outbound_connections.len(),
            Kind::EstablishedTotal,
        )?;

        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        _: Option<PeerId>,
        _: &[Multiaddr],
        _: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        self.check_limit(
            self.limits.max_pending_outgoing,
            self.pending_outbound_connections.len(),
            Kind::PendingOutgoing,
        )?;

        self.pending_outbound_connections.insert(connection_id);

        Ok(vec![])
    }

    fn handle_established_outbound_connection(
        &mut self,
        connection_id: ConnectionId,
        peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        self.pending_outbound_connections.remove(&connection_id);

        self.check_limit(
            self.limits.max_established_outgoing,
            self.established_outbound_connections.len(),
            Kind::EstablishedOutgoing,
        )?;
        // incoming
        self.check_limit(
            self.limits.max_established_per_peer,
            self.established_per_peer
                .get(&peer)
                .map(|connections| {
                    connections
                        .iter()
                        .filter(|id| self.established_inbound_connections.contains(id))
                        .count()
                })
                .unwrap_or(0),
            Kind::IncomingEstablishedPerPeer,
        )
        .map_err(|err| {
            error!(
                "{peer} has exeeced max inbound connections of {:?}",
                self.limits.max_established_per_peer
            );
            err
        })?;
        // outgoing
        self.check_limit(
            self.limits.max_established_per_peer,
            self.established_per_peer
                .get(&peer)
                .map(|connections| {
                    connections
                        .iter()
                        .filter(|id| self.established_outbound_connections.contains(id))
                        .count()
                })
                .unwrap_or(0),
            Kind::OutgoingEstablishedPerPeer,
        )
        .map_err(|err| {
            error!(
                "{peer} has exeeced max outbound connections of GG {:?}",
                self.limits.max_established_per_peer
            );
            err
        })?;
        self.check_limit(
            self.limits.max_established_total,
            self.established_inbound_connections.len()
                + self.established_outbound_connections.len(),
            Kind::EstablishedTotal,
        )?;

        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionClosed(ConnectionClosed {
                peer_id,
                connection_id,
                ..
            }) => {
                self.established_inbound_connections.remove(&connection_id);
                self.established_outbound_connections.remove(&connection_id);
                self.established_per_peer
                    .entry(peer_id)
                    .or_default()
                    .remove(&connection_id);
            }
            FromSwarm::ConnectionEstablished(ConnectionEstablished {
                peer_id,
                endpoint,
                connection_id,
                ..
            }) => {
                match endpoint {
                    ConnectedPoint::Listener { .. } => {
                        self.established_inbound_connections.insert(connection_id);
                    }
                    ConnectedPoint::Dialer { .. } => {
                        self.established_outbound_connections.insert(connection_id);
                    }
                }

                self.established_per_peer
                    .entry(peer_id)
                    .or_default()
                    .insert(connection_id);
            }
            FromSwarm::DialFailure(DialFailure { connection_id, .. }) => {
                self.pending_outbound_connections.remove(&connection_id);
            }
            FromSwarm::AddressChange(_) => {}
            FromSwarm::ListenFailure(ListenFailure { connection_id, .. }) => {
                self.pending_inbound_connections.remove(&connection_id);
            }
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
        _event: THandlerOutEvent<Self>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::swarm::{
        behaviour::toggle::Toggle, dial_opts::DialOpts, DialError, ListenError, Swarm, SwarmEvent,
    };
    use libp2p_swarm_test::SwarmExt;
    use quickcheck::*;
    use rand::Rng;
    use tokio::runtime::Runtime;

    #[test]
    fn max_outgoing() {
        let outgoing_limit = rand::thread_rng().gen_range(1..10);

        let mut network = Swarm::new_ephemeral(|_| {
            Behaviour::new(
                ConnectionLimits::default().with_max_pending_outgoing(Some(outgoing_limit)),
            )
        });
        let addr: Multiaddr = "/memory/1234".parse().unwrap();
        let target = PeerId::random();

        for _ in 0..outgoing_limit {
            network
                .dial(
                    DialOpts::peer_id(target)
                        .addresses(vec![addr.clone()])
                        .build(),
                )
                .expect("Unexpected connection limit.");
        }

        match network
            .dial(DialOpts::peer_id(target).addresses(vec![addr]).build())
            .expect_err("Unexpected dialing success.")
        {
            DialError::Denied { cause } => {
                let exceeded = cause
                    .downcast::<Exceeded>()
                    .expect("connection denied because of limit");

                assert_eq!(exceeded.limit(), outgoing_limit);
            }
            e => panic!("Unexpected error: {e:?}"),
        }

        let info = network.network_info();
        assert_eq!(info.num_peers(), 0);
        assert_eq!(
            info.connection_counters().num_pending_outgoing(),
            outgoing_limit
        );
    }

    #[test]
    fn max_established_incoming() {
        fn prop(Limit(limit): Limit) {
            let rt = Runtime::new().unwrap();
            let mut swarm1 = Swarm::new_ephemeral(|_| {
                Behaviour::new(
                    ConnectionLimits::default().with_max_established_incoming(Some(limit)),
                )
            });
            let mut swarm2 = Swarm::new_ephemeral(|_| {
                Behaviour::new(
                    ConnectionLimits::default().with_max_established_incoming(Some(limit)),
                )
            });

            rt.block_on(async {
                let (listen_addr, _) = swarm1.listen().await;

                for _ in 0..limit {
                    swarm2.connect(&mut swarm1).await;
                }

                swarm2.dial(listen_addr).unwrap();

                tokio::task::spawn(swarm2.loop_on_next());

                let cause = swarm1
                    .wait(|event| match event {
                        SwarmEvent::IncomingConnectionError {
                            error: ListenError::Denied { cause },
                            ..
                        } => Some(cause),
                        _ => None,
                    })
                    .await;

                assert_eq!(cause.downcast::<Exceeded>().unwrap().limit, limit);
            });
        }

        #[derive(Debug, Clone)]
        struct Limit(u32);

        impl Arbitrary for Limit {
            fn arbitrary(g: &mut Gen) -> Self {
                let arb = u32::arbitrary(g) % 9;
                Self(arb + 1) // equivalent to gen_range(1..10)
            }
        }

        quickcheck(prop as fn(_));
    }

    /// Another sibling [`NetworkBehaviour`] implementation might deny established connections in
    /// [`handle_established_outbound_connection`] or [`handle_established_inbound_connection`].
    /// [`Behaviour`] must not increase the established counters in
    /// [`handle_established_outbound_connection`] or [`handle_established_inbound_connection`], but
    /// in [`SwarmEvent::ConnectionEstablished`] as the connection might still be denied by a
    /// sibling [`NetworkBehaviour`] in the former case. Only in the latter case
    /// ([`SwarmEvent::ConnectionEstablished`]) can the connection be seen as established.
    #[test]
    fn support_other_behaviour_denying_connection() {
        let rt = Runtime::new().unwrap();
        let mut swarm1 = Swarm::new_ephemeral(|_| {
            Behaviour::new_with_connection_denier(ConnectionLimits::default())
        });
        let mut swarm2 = Swarm::new_ephemeral(|_| Behaviour::new(ConnectionLimits::default()));

        rt.block_on( async {
            // Have swarm2 dial swarm1.
            let (listen_addr, _) = swarm1.listen().await;
            swarm2.dial(listen_addr).unwrap();
            tokio::task::spawn(swarm2.loop_on_next());

            // Wait for the ConnectionDenier of swarm1 to deny the established connection.
            let cause = swarm1
                .wait(|event| match event {
                    SwarmEvent::IncomingConnectionError {
                        error: ListenError::Denied { cause },
                        ..
                    } => Some(cause),
                    _ => None,
                })
                .await;

            cause.downcast::<std::io::Error>().unwrap();

            assert_eq!(
                0,
                swarm1
                    .behaviour_mut()
                    .limits
                    .established_inbound_connections
                    .len(),
                "swarm1 connection limit behaviour to not count denied established connection as established connection"
            )
        });
    }

    #[derive(libp2p::swarm::NetworkBehaviour)]
    #[behaviour(prelude = "libp2p::swarm::derive_prelude")]
    struct Behaviour {
        limits: super::Behaviour,
        keep_alive: libp2p::swarm::keep_alive::Behaviour,
        connection_denier: Toggle<ConnectionDenier>,
    }

    impl Behaviour {
        fn new(limits: ConnectionLimits) -> Self {
            Self {
                limits: super::Behaviour::new(limits),
                keep_alive: libp2p::swarm::keep_alive::Behaviour,
                connection_denier: None.into(),
            }
        }
        fn new_with_connection_denier(limits: ConnectionLimits) -> Self {
            Self {
                limits: super::Behaviour::new(limits),
                keep_alive: libp2p::swarm::keep_alive::Behaviour,
                connection_denier: Some(ConnectionDenier {}).into(),
            }
        }
    }

    struct ConnectionDenier {}

    impl NetworkBehaviour for ConnectionDenier {
        type ConnectionHandler = dummy::ConnectionHandler;
        type ToSwarm = ();

        fn handle_established_inbound_connection(
            &mut self,
            _connection_id: ConnectionId,
            _peer: PeerId,
            _local_addr: &Multiaddr,
            _remote_addr: &Multiaddr,
        ) -> Result<THandler<Self>, ConnectionDenied> {
            Err(ConnectionDenied::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ConnectionDenier",
            )))
        }

        fn handle_established_outbound_connection(
            &mut self,
            _connection_id: ConnectionId,
            _peer: PeerId,
            _addr: &Multiaddr,
            _role_override: Endpoint,
        ) -> Result<THandler<Self>, ConnectionDenied> {
            Err(ConnectionDenied::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ConnectionDenier",
            )))
        }

        fn on_swarm_event(&mut self, _event: FromSwarm<Self::ConnectionHandler>) {}

        fn on_connection_handler_event(
            &mut self,
            _peer_id: PeerId,
            _connection_id: ConnectionId,
            _event: THandlerOutEvent<Self>,
        ) {
        }

        fn poll(
            &mut self,
            _cx: &mut Context<'_>,
            _params: &mut impl PollParameters,
        ) -> Poll<ToSwarm<Self::ToSwarm, THandlerInEvent<Self>>> {
            Poll::Pending
        }
    }
}
