use async_trait::async_trait;
use futures::{prelude::*, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::{Codec, ProtocolName},
};
use std::io;

// SAFE Messaging Protocol

#[derive(Debug, Clone)]
pub(crate) struct SafeProtocol();
#[derive(Clone)]
pub(crate) struct SafeCodec();
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeRequest(pub(crate) Vec<u8>);
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeResponse(pub(crate) Vec<u8>);

impl ProtocolName for SafeProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/safe/1".as_bytes()
    }
}

#[async_trait]
impl Codec for SafeCodec {
    type Protocol = SafeProtocol;
    type Request = SafeRequest;
    type Response = SafeResponse;

    async fn read_request<T>(&mut self, _: &SafeProtocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(SafeRequest(vec))
    }

    async fn read_response<T>(&mut self, _: &SafeProtocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1024).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(SafeResponse(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &SafeProtocol,
        io: &mut T,
        SafeRequest(data): SafeRequest,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &SafeProtocol,
        io: &mut T,
        SafeResponse(data): SafeResponse,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}
