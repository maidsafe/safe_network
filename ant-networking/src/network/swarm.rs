use crate::{
    event::NodeEvent,
    messages::{Request, Response},
    network::{behavior::NodeBehaviour, error::{NetworkError, Result}},
};

use futures::{StreamExt, future::Either};
use libp2p::{
    swarm::{SwarmEvent, dial_opts::DialOpts, SwarmBuilder, NetworkBehaviour},
    Multiaddr, PeerId, Swarm,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

pub struct SwarmDriver {
    swarm: Swarm<NodeBehaviour>,
    command_receiver: mpsc::UnboundedReceiver<LocalSwarmCmd>,
}

impl SwarmDriver {
    pub fn new(
        swarm: Swarm<NodeBehaviour>,
        command_receiver: mpsc::UnboundedReceiver<LocalSwarmCmd>,
    ) -> Self {
        Self {
            swarm,
            command_receiver,
        }
    }

    pub async fn run(mut self) {
        loop {
            let evt = {
                let swarm_next = self.swarm.next();
                let command_next = self.command_receiver.recv();

                match futures::future::select(Box::pin(swarm_next), Box::pin(command_next)).await {
                    Either::Left((swarm_event, _)) => {
                        if let Some(event) = swarm_event {
                            Some(NetworkEvent::Swarm(event))
                        } else {
                            None
                        }
                    }
                    Either::Right((command, _)) => {
                        if let Some(cmd) = command {
                            Some(NetworkEvent::Command(cmd))
                        } else {
                            None
                        }
                    }
                }
            };

            match evt {
                Some(NetworkEvent::Swarm(event)) => {
                    if let Err(e) = self.handle_swarm_event(event).await {
                        error!("Error handling swarm event: {:?}", e);
                    }
                }
                Some(NetworkEvent::Command(cmd)) => {
                    if let Err(e) = self.handle_command(cmd).await {
                        error!("Error handling command: {:?}", e);
                    }
                }
                None => {
                    debug!("No more events to process");
                    break;
                }
            }
        }
    }

    async fn handle_swarm_event(&mut self, event: SwarmEvent<NodeEvent>) -> Result<()> {
        match event {
            SwarmEvent::Behaviour(NodeEvent::Identify(event)) => {
                debug!("Identify event: {:?}", event);
            }
            SwarmEvent::Behaviour(NodeEvent::Kademlia(event)) => {
                debug!("Kademlia event: {:?}", event);
            }
            SwarmEvent::Behaviour(NodeEvent::MsgReceived(event)) => {
                debug!("Request/Response event: {:?}", event);
            }
            SwarmEvent::NewListenAddr { address, .. } => {
                info!("Listening on {:?}", address);
            }
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                info!("Connected to {:?}", peer_id);
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                warn!("Disconnected from {:?}", peer_id);
            }
            _ => {
                debug!("Other swarm event: {:?}", event);
            }
        }
        Ok(())
    }

    async fn handle_command(&mut self, cmd: LocalSwarmCmd) -> Result<()> {
        match cmd {
            LocalSwarmCmd::StartListening(addr) => {
                if let Err(e) = self.swarm.listen_on(addr) {
                    error!("Failed to start listening: {:?}", e);
                    return Err(NetworkError::Other(format!("Failed to listen: {}", e)));
                }
            }
            LocalSwarmCmd::Dial(peer_id, addr) => {
                let opts = DialOpts::peer_id(peer_id)
                    .addresses(vec![addr])
                    .build();
                if let Err(e) = self.swarm.dial(opts) {
                    error!("Failed to dial peer: {:?}", e);
                    return Err(NetworkError::DialError(peer_id, e.to_string()));
                }
            }
            LocalSwarmCmd::SendRequest(peer_id, request) => {
                self.swarm.behaviour_mut().request_response.send_request(&peer_id, request);
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum LocalSwarmCmd {
    StartListening(Multiaddr),
    Dial(PeerId, Multiaddr),
    SendRequest(PeerId, Request),
}

#[derive(Debug)]
enum NetworkEvent {
    Swarm(SwarmEvent<NodeEvent>),
    Command(LocalSwarmCmd),
}

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::{
        identity::Keypair,
        kad::{self, Config as KadConfig, store::MemoryStore},
        request_response::{self, cbor::Behaviour as CborBehaviour, Config as RequestResponseConfig, ProtocolSupport},
        StreamProtocol,
        core::transport::MemoryTransport,
    };
    use crate::messages::Request;

    #[tokio::test]
    async fn test_swarm_driver() {
        let keypair = Keypair::generate_ed25519();
        let peer_id = keypair.public().to_peer_id();

        // Create behavior components
        let kad_config = KadConfig::default();
        let kad_store = MemoryStore::new(peer_id);
        let kad = kad::Behaviour::new(peer_id, kad_store, kad_config);

        let identify = identify::Behaviour::new(identify::Config::new(
            "test/1.0.0".to_string(),
            keypair.public(),
        ));

        let req_res_config = RequestResponseConfig::default();
        let protocol = StreamProtocol::new(Request::PROTOCOL_NAME);
        let request_response = CborBehaviour::new(
            [(protocol, ProtocolSupport::Full)],
            req_res_config,
        );

        let behaviour = NodeBehaviour::new(
            peer_id,
            identify,
            kad,
            request_response,
        );

        // Create transport and swarm
        let transport = MemoryTransport::default();
        let swarm = SwarmBuilder::with_tokio_executor(transport, behaviour, peer_id).build();

        // Create command channel
        let (cmd_sender, cmd_receiver) = mpsc::unbounded_channel();

        // Create and run SwarmDriver
        let driver = SwarmDriver::new(swarm, cmd_receiver);
        
        // Run the driver for a short time
        tokio::spawn(async move {
            driver.run().await;
        });

        // Send a test command
        let test_addr = "/memory/1234".parse().unwrap();
        cmd_sender.send(LocalSwarmCmd::StartListening(test_addr)).unwrap();

        // Wait a bit to let the command be processed
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
