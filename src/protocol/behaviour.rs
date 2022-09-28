use libp2p::{
    gossipsub::GossipsubEvent,
    identify::{Identify, IdentifyEvent},
    kad::{store::MemoryStore, Kademlia, KademliaEvent},
    mdns::{Mdns, MdnsEvent},
    request_response::{RequestResponse, RequestResponseEvent},
    NetworkBehaviour,
};

use super::rr::{BlockCodec, BlockRequest, BlockResponse};

#[derive(NetworkBehaviour)]
#[behaviour(out_event = "Event")]
pub struct MyBehaviour {
    pub gossipsub: libp2p::gossipsub::Gossipsub,
    pub kademlia: Kademlia<MemoryStore>,
    pub identify: Identify,
    pub mdns: Mdns,
    pub request: RequestResponse<BlockCodec>,
}

pub enum Event {
    Identify(IdentifyEvent),
    GossipSub(GossipsubEvent),
    Kademlia(KademliaEvent),
    RequestResponse(RequestResponseEvent<BlockRequest, BlockResponse>),
    Mdns(MdnsEvent),
}

impl From<IdentifyEvent> for Event {
    fn from(event: IdentifyEvent) -> Self {
        Self::Identify(event)
    }
}
impl From<GossipsubEvent> for Event {
    fn from(event: GossipsubEvent) -> Self {
        Self::GossipSub(event)
    }
}
impl From<KademliaEvent> for Event {
    fn from(event: KademliaEvent) -> Self {
        Self::Kademlia(event)
    }
}
impl From<RequestResponseEvent<BlockRequest, BlockResponse>> for Event {
    fn from(event: RequestResponseEvent<BlockRequest, BlockResponse>) -> Self {
        Self::RequestResponse(event)
    }
}

impl From<MdnsEvent> for Event {
    fn from(event: MdnsEvent) -> Self {
        Self::Mdns(event)
    }
}
