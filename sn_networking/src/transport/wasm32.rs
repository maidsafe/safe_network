// wasm32 environments typically only support WebSockets (and WebRTC or WebTransport), so no plain UDP or TCP.

use libp2p::{
    core::{muxing::StreamMuxerBox, transport, upgrade},
    identity::Keypair,
    noise, websocket_websys, yamux, PeerId, Transport as _,
};

pub(crate) fn build_transport(keypair: &Keypair) -> transport::Boxed<(PeerId, StreamMuxerBox)> {
    // We build a single transport here, WebSockets.
    websocket_websys::Transport::default()
        .upgrade(upgrade::Version::V1)
        .authenticate(
            noise::Config::new(keypair).expect("Signing libp2p-noise static DH keypair failed."),
        )
        .multiplex(yamux::Config::default())
        .boxed()
}
