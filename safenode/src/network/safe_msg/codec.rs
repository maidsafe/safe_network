// Copyright 2023 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under The General Public License (GPL), version 3.
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied. Please review the Licences for the specific language governing
// permissions and limitations relating to use of the SAFE Network Software.

use crate::storage::chunks::Chunk;
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::{self, ProtocolName},
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::io;
use tracing::info;
use xor_name::XorName;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafeRequest {
    GetChunk(XorName),
    GetDBC,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SafeResponse {
    Chunk(Chunk),
    DBC,
}

#[derive(Debug, Clone)]
pub(crate) struct SafeMsgProtocol();
#[derive(Clone)]
pub(crate) struct SafeMsgCodec();

impl ProtocolName for SafeMsgProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/safe-msg/1".as_bytes()
    }
}

#[async_trait]
impl request_response::Codec for SafeMsgCodec {
    type Protocol = SafeMsgProtocol;
    type Request = SafeRequest;
    type Response = SafeResponse;

    async fn read_request<T>(&mut self, _: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_and_decode(io).await
    }

    async fn read_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        read_and_decode(io).await
    }

    async fn write_request<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        req: Self::Request,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        encode_and_write(io, req).await
    }

    async fn write_response<T>(
        &mut self,
        _: &Self::Protocol,
        io: &mut T,
        res: Self::Response,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        encode_and_write(io, res).await
    }
}

async fn encode_and_write<IO, T>(io: &mut IO, data: T) -> io::Result<()>
where
    IO: AsyncWrite + Unpin,
    T: Serialize,
{
    let bytes = rmp_serde::to_vec(&data)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e.to_string()))?;
    write_length_prefixed(io, bytes).await?;
    io.close().await?;
    Ok(())
}

async fn read_and_decode<IO, T>(io: &mut IO) -> io::Result<T>
where
    IO: AsyncRead + Unpin,
    T: DeserializeOwned,
{
    let vec = read_length_prefixed(io, 500_000_000).await?; // update transfer maximum
    if vec.is_empty() {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    rmp_serde::from_slice::<T>(vec.as_slice())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}
