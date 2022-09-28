use crate::blockchain;
use crate::db::db;
use crate::structs;
use async_std::io;
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite, AsyncWriteExt};
use libp2p::{
    core::upgrade::{read_length_prefixed, write_length_prefixed},
    request_response::{ProtocolName, RequestResponseCodec},
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct RequestProtocol();
#[derive(Clone)]
pub struct BlockCodec();
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockRequest();
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse(pub structs::Accounts, pub blockchain::block::Block);

impl ProtocolName for RequestProtocol {
    fn protocol_name(&self) -> &[u8] {
        "/Block-Response/1".as_bytes()
    }
}

#[async_trait]
impl RequestResponseCodec for BlockCodec {
    type Protocol = RequestProtocol;
    type Request = BlockRequest;
    type Response = BlockResponse;

    async fn read_request<T>(
        &mut self,
        _: &RequestProtocol,
        io: &mut T,
    ) -> io::Result<Self::Request>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(BlockRequest())
    }

    async fn read_response<T>(
        &mut self,
        _: &RequestProtocol,
        io: &mut T,
    ) -> io::Result<Self::Response>
    where
        T: AsyncRead + Unpin + Send,
    {
        let vec = read_length_prefixed(io, 1_000_000).await?;

        if vec.is_empty() {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }

        Ok(db::deserialize::<BlockResponse>(&vec).expect("error"))
    }

    async fn write_request<T>(
        &mut self,
        _: &RequestProtocol,
        io: &mut T,
        BlockRequest(): BlockRequest,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(io, "hash".to_string()).await?;
        io.close().await?;

        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &RequestProtocol,
        io: &mut T,
        BlockResponse(accounts, block): BlockResponse,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(
            io,
            db::serialize(&BlockResponse(accounts, block))
                .expect("Serialize Error")
                .into_bytes(),
        )
        .await?;
        io.close().await?;

        Ok(())
    }
}
