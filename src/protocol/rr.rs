use crate::structs;
use crate::{blockchain::block::Block, db::db};
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
pub struct BlockRequest(pub String);
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockResponse(pub ResponseEnum);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseEnum {
    NoBlock(String),
    Block(Block),
    Response(structs::ValueList, structs::BlockHelper),
}

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
        match db::deserialize::<String>(&vec) {
            Ok(value) => {
                if value == "block".to_string() {
                    Ok(BlockRequest("block".to_string()))
                } else {
                    Ok(BlockRequest(value))
                }
            }
            _ => Ok(BlockRequest("block".to_string())),
        }
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
        BlockRequest(s): BlockRequest,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        if s == "block".to_string() {
            write_length_prefixed(io, "block".to_string()).await?;
        } else {
            write_length_prefixed(io, s).await?;
        }
        io.close().await?;
        Ok(())
    }

    async fn write_response<T>(
        &mut self,
        _: &RequestProtocol,
        io: &mut T,
        BlockResponse(e): BlockResponse,
    ) -> io::Result<()>
    where
        T: AsyncWrite + Unpin + Send,
    {
        write_length_prefixed(
            io,
            db::serialize(&BlockResponse(e))
                .expect("Serialize Error")
                .into_bytes(),
        )
        .await?;
        io.close().await?;

        Ok(())
    }
}
