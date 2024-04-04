// Copyright 2024 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use libp2p::{
    core::{muxing::StreamMuxerBox, transport, upgrade},
    identity::Keypair,
    noise, websocket_websys, yamux, PeerId, Transport as _,
};

// wasm32 environments typically only support WebSockets (and WebRTC or WebTransport), so no plain UDP or TCP.
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
