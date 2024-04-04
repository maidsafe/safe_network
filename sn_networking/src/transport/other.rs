// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

#![allow(unused_variables)]

#[cfg(feature = "websockets")]
use futures::future::Either;
#[cfg(feature = "websockets")]
use libp2p::{core::upgrade, noise, yamux};
use libp2p::{
    core::{muxing::StreamMuxerBox, transport},
    identity::Keypair,
    PeerId, Transport as _,
};

pub(crate) fn build_transport(keypair: &Keypair) -> transport::Boxed<(PeerId, StreamMuxerBox)> {
    check_feature_flags();
    #[cfg(feature = "tcp")]
    let trans = generate_tcp_transport(keypair);
    #[cfg(feature = "quic")]
    let trans = generate_quic_transport(keypair);

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

#[cfg(feature = "quic")]
fn generate_quic_transport(
    keypair: &Keypair,
) -> libp2p::quic::GenTransport<libp2p::quic::tokio::Provider> {
    libp2p::quic::tokio::Transport::new(libp2p::quic::Config::new(keypair))
}

#[cfg(feature = "tcp")]
fn generate_tcp_transport(keypair: &Keypair) -> transport::Boxed<(PeerId, StreamMuxerBox)> {
    libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default())
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(
            libp2p::noise::Config::new(keypair)
                .expect("Signing libp2p-noise static DH keypair failed."),
        )
        .multiplex(libp2p::yamux::Config::default())
        .boxed()
}

fn check_feature_flags() {
    if cfg!(feature = "tcp") && cfg!(feature = "quic") {
        panic!("Both `tcp` and `quic` feature flags cannot be set simultaneously.");
    }
    if cfg!(feature = "websocket") && !cfg!(feature = "quic") && !cfg!(feature = "tcp") {
        panic!("The `websocket` feature flag must be used with either `quic` or `tcp`.");
    }
}
