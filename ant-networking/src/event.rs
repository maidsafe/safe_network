use libp2p::{
    identify,
    kad,
    request_response,
};

use crate::messages::{Request, Response};

#[derive(Debug)]
pub enum NodeEvent {
    Identify(identify::Event),
    Kademlia(kad::Event),
    MsgReceived(request_response::Event<Request, Response>),
}

impl From<identify::Event> for NodeEvent {
    fn from(event: identify::Event) -> Self {
        NodeEvent::Identify(event)
    }
}

impl From<kad::Event> for NodeEvent {
    fn from(event: kad::Event) -> Self {
        NodeEvent::Kademlia(event)
    }
}

impl From<request_response::Event<Request, Response>> for NodeEvent {
    fn from(event: request_response::Event<Request, Response>) -> Self {
        NodeEvent::MsgReceived(event)
    }
}

#[derive(Debug)]
pub enum MsgResponder {
    Response(Response),
    Error(String),
}
