// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::{self, ProtocolName},
};
use std::io;
use xor_name::XorName;

// Chuck Storage protocol
#[derive(Debug, Clone)]
pub struct ChunkStorageProtocol();
#[derive(Clone)]
pub struct ChunkStorageCodec();
#[derive(Debug, Clone, PartialEq, Eq)]
// request the xorname of the file
pub struct ChunkRequest(pub XorName);
#[derive(Debug, Clone, PartialEq, Eq)]
// respond with the file
pub struct ChunkResponse(pub Vec<u8>);

impl ProtocolName for ChunkStorageProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/file-exchange/1".as_bytes()
    }
}

#[async_trait]
impl request_response::Codec for ChunkStorageCodec {
    type Protocol = ChunkStorageProtocol;
    type Request = ChunkRequest;
    type Response = ChunkResponse;

    async fn read_request<T>(
        &mut self,
        _: &ChunkStorageProtocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let mut xor_name = XorName::default();
        xor_name.0.copy_from_slice(vec.as_slice());

        Ok(ChunkRequest(xor_name))
    }

    async fn read_response<T>(
        &mut self,
        _: &ChunkStorageProtocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 500_000_000).await?; // update transfer maximum

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(ChunkResponse(vec))
    }

    async fn write_request<T>(
        &mut self,
        _: &ChunkStorageProtocol,
        io: &mut T,
        ChunkRequest(data): ChunkRequest,
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
        _: &ChunkStorageProtocol,
        io: &mut T,
        ChunkResponse(data): ChunkResponse,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, data).await?;
        io.close().await?;

        Ok(())
    }
}
