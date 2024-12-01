use ant_networking::multiaddr_is_global;
use libp2p::{autonat, identify};
use tracing::{debug, info, warn};

use crate::{behaviour::PROTOCOL_VERSION, App};

impl App {
    pub(crate) fn on_event_identify(&mut self, event: identify::Event) {
        match event {
            identify::Event::Received {
                peer_id,
                info,
                connection_id,
            } => {
                debug!(
                    %peer_id,
                    protocols=?info.protocols,
                    observed_address=%info.observed_addr,
                    protocol_version=%info.protocol_version,
                    "Received peer info"
                );

                // Disconnect if peer has incompatible protocol version.
                if info.protocol_version != PROTOCOL_VERSION {
                    warn!(conn_id=%connection_id, %peer_id, "Incompatible protocol version. Disconnecting from peer.");
                    let _ = self.swarm.disconnect_peer_id(peer_id);
                    return;
                }

                // Disconnect if peer has no AutoNAT support.
                if !info
                    .protocols
                    .iter()
                    .any(|p| *p == autonat::DEFAULT_PROTOCOL_NAME)
                {
                    warn!(conn_id=%connection_id, %peer_id, "Peer does not support AutoNAT. Disconnecting from peer.");
                    let _ = self.swarm.disconnect_peer_id(peer_id);
                    return;
                }

                info!(conn_id=%connection_id, %peer_id, "Received peer info: confirmed it supports AutoNAT");

                // If we're a client and the peer has (a) global listen address(es),
                // add it as an AutoNAT server.
                if self.client_state.is_some() {
                    for addr in info.listen_addrs.into_iter().filter(multiaddr_is_global) {
                        self.swarm
                            .behaviour_mut()
                            .autonat
                            .add_server(peer_id, Some(addr));
                    }
                }
                self.check_state();
            }
            identify::Event::Sent { .. } => { /* ignore */ }
            identify::Event::Pushed { .. } => { /* ignore */ }
            identify::Event::Error { .. } => { /* ignore */ }
        }
    }
}
