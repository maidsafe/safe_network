use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Could not parse the supplied multiaddr or socket address")]
    InvalidPeerAddr(#[from] libp2p::multiaddr::Error),
    #[error("Could not obtain network contacts from {0} after {1} retries")]
    FailedToObtainPeersFromUrl(String, usize),
    #[error("No valid multaddr was present in the contacts file at {0}")]
    NoMultiAddrObtainedFromNetworkContacts(String),
    #[error("Could not obtain peers through any available options")]
    PeersNotObtained,
    #[error(transparent)]
    ReqwestError(#[from] reqwest::Error),
    #[error(transparent)]
    UrlParseError(#[from] url::ParseError),
}
