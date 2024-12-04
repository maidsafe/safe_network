use serde::{Deserialize, Serialize};
use libp2p::PeerId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub peer_id: PeerId,
    pub request_type: RequestType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RequestType {
    GetRecord { key: Vec<u8> },
    PutRecord { key: Vec<u8>, value: Vec<u8> },
    GetClosestPeers { key: Vec<u8> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub peer_id: PeerId,
    pub response_type: ResponseType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseType {
    Record { key: Vec<u8>, value: Vec<u8> },
    NoRecord { key: Vec<u8> },
    ClosestPeers { key: Vec<u8>, peers: Vec<PeerId> },
    Error { message: String },
}

impl Request {
    pub const PROTOCOL_NAME: &'static str = "/ant-networking/request/1.0.0";
    
    pub fn new(peer_id: PeerId, request_type: RequestType) -> Self {
        Self {
            peer_id,
            request_type,
        }
    }
}

impl Response {
    pub fn new(peer_id: PeerId, response_type: ResponseType) -> Self {
        Self {
            peer_id,
            response_type,
        }
    }
}
