#[cfg(feature = "open-metrics")]
use crate::MetricsRegistries;
#[cfg(feature = "websockets")]
use futures::future::Either;
#[cfg(feature = "websockets")]
use libp2p::{core::upgrade, noise, yamux};
use libp2p::{
    core::{muxing::StreamMuxerBox, transport},
    identity::Keypair,
    PeerId, Transport as _,
};

pub(crate) fn build_transport(
    keypair: &Keypair,
    #[cfg(feature = "open-metrics")] registries: &mut MetricsRegistries,
) -> transport::Boxed<(PeerId, StreamMuxerBox)> {
    let trans = generate_quic_transport(keypair);
    #[cfg(feature = "open-metrics")]
    let trans = libp2p::metrics::BandwidthTransport::new(trans, &mut registries.standard_metrics);

    #[cfg(feature = "websockets")]
    // Using a closure here due to the complex return type
    let generate_ws_transport = || {
        let tcp = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default());
        libp2p::websocket::WsConfig::new(tcp)
            .upgrade(upgrade::Version::V1)
            .authenticate(
                noise::Config::new(keypair)
                    .expect("Signing libp2p-noise static DH keypair failed."),
            )
            .multiplex(yamux::Config::default())
    };

    // With the `websockets` feature enabled, we add it as a fallback transport.
    #[cfg(feature = "websockets")]
    let trans = trans
        .or_transport(generate_ws_transport())
        .map(|either_output, _| match either_output {
            Either::Left((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
            Either::Right((peer_id, muxer)) => (peer_id, StreamMuxerBox::new(muxer)),
        });
    #[cfg(not(feature = "websockets"))]
    let trans = trans.map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)));

    trans.boxed()
}

fn generate_quic_transport(
    keypair: &Keypair,
) -> libp2p::quic::GenTransport<libp2p::quic::tokio::Provider> {
    libp2p::quic::tokio::Transport::new(libp2p::quic::Config::new(keypair))
}
