use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error(transparent)]
    MultiAddrError(#[from] libp2p::multiaddr::Error),
    #[error(transparent)]
    ParseError(#[from] libp2p_identity::ParseError),
    #[error(transparent)]
    TonicStatusError(#[from] tonic::Status),
    #[error(transparent)]
    TonicTransportError(#[from] tonic::transport::Error),
    #[error("Could not connect to the RPC endpoint {0:?}")]
    RpcEndpointConnectionFailure(tonic::transport::Error),
}
