use libp2p::upnp;
use tracing::{debug, info};
use tracing_log::log::error;

use crate::App;

impl App {
    pub(crate) fn on_event_upnp(&mut self, event: upnp::Event) {
        match event {
            upnp::Event::NewExternalAddr(addr) => {
                info!(%addr, "UPnP: New external address detected");
            }
            upnp::Event::ExpiredExternalAddr(addr) => {
                debug!(%addr, "UPnP: External address expired");
            }
            upnp::Event::GatewayNotFound => {
                error!("UPnP: No gateway not found");
            }
            upnp::Event::NonRoutableGateway => {
                error!("UPnP: Gateway is not routable");
            }
        }
    }
}
