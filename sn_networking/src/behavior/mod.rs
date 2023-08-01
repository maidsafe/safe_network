use libp2p::core::{Endpoint, Multiaddr};
use libp2p::identity::PeerId;
use libp2p::swarm::{
    derive_prelude::ExternalAddrConfirmed, dummy, ConnectionDenied, ConnectionId,
    ExternalAddrExpired, FromSwarm, NetworkBehaviour, NewExternalAddrCandidate, PollParameters,
    THandler, THandlerInEvent, THandlerOutEvent, ToSwarm,
};
use std::task::{Context, Poll};

#[derive(Default, Debug)]
pub struct ExternalAddrLogBehaviour;

impl ExternalAddrLogBehaviour {}

impl NetworkBehaviour for ExternalAddrLogBehaviour {
    type ConnectionHandler = dummy::ConnectionHandler;
    type ToSwarm = ();

    fn handle_established_inbound_connection(
        &mut self,
        _: ConnectionId,
        _peer: PeerId,
        _: &Multiaddr,
        _: &Multiaddr,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn handle_pending_outbound_connection(
        &mut self,
        _: ConnectionId,
        _peer: Option<PeerId>,
        _: &[Multiaddr],
        _: Endpoint,
    ) -> Result<Vec<Multiaddr>, ConnectionDenied> {
        Ok(vec![])
    }

    fn handle_established_outbound_connection(
        &mut self,
        _: ConnectionId,
        _peer: PeerId,
        _: &Multiaddr,
        _: Endpoint,
    ) -> Result<THandler<Self>, ConnectionDenied> {
        Ok(dummy::ConnectionHandler)
    }

    fn on_swarm_event(&mut self, event: FromSwarm<Self::ConnectionHandler>) {
        match event {
            FromSwarm::ConnectionClosed(_) => {}
            FromSwarm::ConnectionEstablished(_) => {}
            FromSwarm::AddressChange(_) => {}
            FromSwarm::DialFailure(_) => {}
            FromSwarm::ListenFailure(_) => {}
            FromSwarm::NewListener(_) => {}
            FromSwarm::NewListenAddr(_) => {}
            FromSwarm::ExpiredListenAddr(_) => {}
            FromSwarm::ListenerError(_) => {}
            FromSwarm::ListenerClosed(_) => {}
            FromSwarm::NewExternalAddrCandidate(NewExternalAddrCandidate { addr }) => {
                trace!("NewExternalAddrCandidate: {addr}");
            }
            FromSwarm::ExternalAddrExpired(ExternalAddrExpired { addr }) => {
                trace!("ExternalAddrExpired: {addr}");
            }
            FromSwarm::ExternalAddrConfirmed(ExternalAddrConfirmed { addr }) => {
                trace!("ExternalAddrConfirmed: {addr}");
            }
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
