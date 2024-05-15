use libp2p::upnp;
use tracing::{debug, info, warn};

use crate::EventLoop;

impl EventLoop {
    pub(crate) fn on_event_upnp(&mut self, event: upnp::Event) {
        match event {
            upnp::Event::NewExternalAddr(addr) => {
                info!(%addr, "UPnP: New external address detected");
            }
            upnp::Event::ExpiredExternalAddr(addr) => {
                debug!(%addr, "UPnP: External address expired");
            }
            upnp::Event::GatewayNotFound => {
                warn!("UPnP: Gateway not found");
            }
            upnp::Event::NonRoutableGateway => {
                warn!("UPnP: Gateway is not routable");
            }
        }
    }
}
