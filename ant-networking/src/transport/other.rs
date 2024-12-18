#[cfg(feature = "open-metrics")]
use crate::MetricsRegistries;
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

    let trans = trans.map(|(peer_id, muxer), _| (peer_id, StreamMuxerBox::new(muxer)));

    trans.boxed()
}

fn generate_quic_transport(
    keypair: &Keypair,
) -> libp2p::quic::GenTransport<libp2p::quic::tokio::Provider> {
    libp2p::quic::tokio::Transport::new(libp2p::quic::Config::new(keypair))
}
