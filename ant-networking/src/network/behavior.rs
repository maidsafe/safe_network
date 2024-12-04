use libp2p::{
    allow_block_list,
    identify,
    kad::{self, store::UnifiedRecordStore},
    relay::{self, client},
    request_response::{self, ProtocolSupport},
    NetworkBehaviour, PeerId,
    swarm::derive_prelude::*,
    upnp,
};

use crate::{
    event::NodeEvent,
    messages::{Request, Response},
};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true, out_event = "NodeEvent")]
pub struct NodeBehaviour {
    pub blocklist: libp2p::allow_block_list::Behaviour,
    pub relay_client: libp2p::relay::client::Behaviour,
    pub relay_server: libp2p::relay::Behaviour,
    #[cfg(feature = "upnp")]
    pub upnp: Toggle<libp2p::upnp::tokio::Behaviour>,
    pub request_response: request_response::cbor::Behaviour<Request, Response>,
    pub kademlia: kad::Behaviour<UnifiedRecordStore>,
    pub identify: identify::Behaviour,
    #[cfg(feature = "local")]
    pub mdns: mdns::tokio::Behaviour,
}

impl NodeBehaviour {
    pub fn new(
        peer_id: PeerId,
        blocklist: libp2p::allow_block_list::Behaviour,
        relay_client: libp2p::relay::client::Behaviour,
        relay_server: libp2p::relay::Behaviour,
        #[cfg(feature = "upnp")]
        upnp: Toggle<libp2p::upnp::tokio::Behaviour>,
        request_response: request_response::cbor::Behaviour<Request, Response>,
        kademlia: kad::Behaviour<UnifiedRecordStore>,
        identify: identify::Behaviour,
        #[cfg(feature = "local")]
        mdns: mdns::tokio::Behaviour,
    ) -> Self {
        Self {
            blocklist,
            relay_client,
            relay_server,
            #[cfg(feature = "upnp")]
            upnp,
            request_response,
            kademlia,
            identify,
            #[cfg(feature = "local")]
            mdns,
        }
    }
}

impl NetworkBehaviourEventProcess<identify::Event> for NodeBehaviour {
    fn inject_event(&mut self, event: identify::Event) {
        // Convert identify events to NodeEvent
        // This will be called when an identify event occurs
    }
}

impl NetworkBehaviourEventProcess<kad::Event> for NodeBehaviour {
    fn inject_event(&mut self, event: kad::Event) {
        // Convert kademlia events to NodeEvent
        // This will be called when a kademlia event occurs
    }
}

impl NetworkBehaviourEventProcess<request_response::Event<Request, Response>> for NodeBehaviour {
    fn inject_event(&mut self, event: request_response::Event<Request, Response>) {
        // Convert request_response events to NodeEvent
        // This will be called when a request_response event occurs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::{
        identity::Keypair,
        kad::Config as KadConfig,
        request_response::Config as RequestResponseConfig,
    };

    #[test]
    fn test_node_behaviour_creation() {
        let keypair = Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();

        // Create identify behavior
        let identify = identify::Behaviour::new(identify::Config::new(
            "test/1.0.0".to_string(),
            keypair.public(),
        ));

        // Create kademlia behavior
        let kad_config = KadConfig::default();
        let kad_store = UnifiedRecordStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, kad_store, kad_config);

        // Create request/response behavior
        let req_res_config = RequestResponseConfig::default();
        let protocol = StreamProtocol::new(Request::PROTOCOL_NAME);
        let request_response = request_response::cbor::Behaviour::new(
            [(protocol, ProtocolSupport::Full)],
            req_res_config,
        );

        // Create node behavior
        let behaviour = NodeBehaviour::new(
            peer_id,
            libp2p::allow_block_list::Behaviour::default(),
            libp2p::relay::client::Behaviour::new(),
            libp2p::relay::Behaviour::new(),
            #[cfg(feature = "upnp")]
            Toggle::On(libp2p::upnp::tokio::Behaviour::new()),
            request_response,
            kademlia,
            identify,
            #[cfg(feature = "local")]
            mdns::tokio::Behaviour::new(),
        );

        assert!(behaviour.identify.local_peer_id() == &peer_id);
    }
}
