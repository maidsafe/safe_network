use libp2p::swarm::NetworkBehaviour;
use libp2p::{autonat, identity};
use std::time::Duration;

use crate::CONFIDENCE_MAX;

mod auto_nat;
mod identify;

pub(crate) const PROTOCOL_VERSION: &str = "/sn_nat_detection/0.1.0";

#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    pub identify: libp2p::identify::Behaviour,
    pub auto_nat: autonat::Behaviour,
}

impl Behaviour {
    pub(crate) fn new(local_public_key: identity::PublicKey, client_mode: bool) -> Self {
        let far_future = Duration::MAX / 10; // `MAX` on itself causes overflows. This is a workaround.
        Self {
            identify: libp2p::identify::Behaviour::new(
                libp2p::identify::Config::new(
                    PROTOCOL_VERSION.to_string(),
                    local_public_key.clone(),
                )
                // Exchange information every 5 minutes.
                .with_interval(Duration::from_secs(5 * 60)),
            ),
            auto_nat: autonat::Behaviour::new(
                local_public_key.to_peer_id(),
                if client_mode {
                    autonat::Config {
                        // Use dialed peers for probing.
                        use_connected: true,
                        // Start probing 3 seconds after swarm init. This gives us time to connect to the dialed server.
                        boot_delay: Duration::from_secs(3),
                        // Reuse probe server immediately even if it's the only one.
                        throttle_server_period: Duration::ZERO,
                        retry_interval: Duration::from_secs(10),
                        // We do not want to refresh.
                        refresh_interval: far_future,
                        confidence_max: CONFIDENCE_MAX,
                        ..Default::default()
                    }
                } else {
                    autonat::Config {
                        // Do not ask for dial-backs, essentially putting us in server mode.
                        use_connected: false,
                        // Never start probing, as we are a server.
                        boot_delay: far_future,
                        ..Default::default()
                    }
                },
            ),
        }
    }
}
