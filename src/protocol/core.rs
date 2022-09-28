use super::super::structs;
use super::behaviour::Event;
use crate::{
    blockchain::{block::Block, transactions::Transaction},
    db::db,
    protocol::{behaviour::MyBehaviour, gossipsub, kademlia, rr, transport},
};
use bytemuck::__core::iter;
use crossbeam_channel::{Receiver, Sender};
use futures::{executor::block_on, StreamExt};
use libp2p::{
    futures::select,
    gossipsub::{Gossipsub, GossipsubEvent, IdentTopic as Topic},
    identify::{Identify, IdentifyConfig, IdentifyEvent, IdentifyInfo},
    identity,
    kad::record::store::MemoryStore,
    kad::Kademlia,
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    request_response::{ProtocolSupport, RequestResponse},
    swarm::SwarmEvent,
    Multiaddr, PeerId, Swarm,
};
use std::{env, error::Error};
use structs::{BackendRequest, GameRequest};

pub async fn into_protocol(
    local_key: identity::Keypair,
    local_peer_id: PeerId,
    sender: Sender<BackendRequest>,
    reciever: Receiver<GameRequest>,
) -> Result<(), Box<dyn Error>> {
    println!("{:?}", local_peer_id);
    //Initializing behaviors
    let mut swarm = {
        let transport = transport::build_transport(local_key.clone()).await?;
        let gossipsub: Gossipsub = gossipsub::create_gossip(local_key.clone());
        let kademlia: Kademlia<MemoryStore> = kademlia::create_kademlia(local_key.clone());
        let mdns = Mdns::new(MdnsConfig::default()).await?;
        let identify = Identify::new(IdentifyConfig::new(
            "1.0".to_string(),
            local_key.clone().public(),
        ));

        let request = RequestResponse::new(
            rr::BlockCodec(),
            iter::once((rr::RequestProtocol(), ProtocolSupport::Full)),
            Default::default(),
        );
        let behaviour = MyBehaviour {
            gossipsub,
            kademlia,
            identify,
            mdns,
            request,
        };
        Swarm::new(transport, behaviour, local_peer_id)
    };
    let args: Vec<String> = env::args().collect();
    let query = &args[1];
    let b = Topic::new("block");
    let t = Topic::new("tx");
    if query == "1" {
        swarm.listen_on("/ip4/10.150.99.25/tcp/65427".parse()?)?;
        swarm.listen_on("/ip6/::0/tcp/0".parse()?)?;
        swarm.behaviour_mut().gossipsub.subscribe(&t).unwrap();
        swarm
            .behaviour_mut()
            .gossipsub
            .publish(
                b.clone(),
                db::serialize(&Block::default()).expect("serde errir"),
            )
            .expect("pub");
    } else {
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        swarm.listen_on("/ip6/::0/tcp/0".parse()?)?;
        swarm.behaviour_mut().gossipsub.subscribe(&b).unwrap();
        if let identity::Keypair::Secp256k1(key) = local_key {
            swarm
                .behaviour_mut()
                .gossipsub
                .publish(
                    t.clone(),
                    db::serialize(&Transaction::new(key.clone(), key.public(), 1.0, 1))
                        .expect("serde errir"),
                )
                .expect("pub");
        }
    }
    //swarm.listen_on("/ip4/192.168.1.197/tcp/54005".parse()?)?;
    swarm = kademlia::boot(swarm);

    loop {
        select! {
            event = swarm.select_next_some() => match event {
                SwarmEvent::NewListenAddr { address, .. } => {
                    println!("Listening in {:?}", address);
                },

                SwarmEvent::Behaviour(Event::Mdns(MdnsEvent::Discovered(list))) => {
                    for (peer_id, multiaddr) in list {
                        println!("identify event: {:?}", peer_id);
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, multiaddr);
                    }

                }
                SwarmEvent::Behaviour(Event::Identify(IdentifyEvent::Received {peer_id, info: IdentifyInfo {listen_addrs,..}})) => {
                    println!("{:?}", peer_id);
                    for addr in listen_addrs {
                        swarm.behaviour_mut().kademlia.add_address(&peer_id, addr);
                    }
                },
                SwarmEvent::Behaviour(Event::GossipSub(GossipsubEvent::Message {
                    propagation_source: _peer_id,
                    message_id: _id,
                    message,
                })) => {
                    match message.topic.as_str() {
                    "block" => println!("{:?}", db::deserialize::<Block>(&message.data)),
                    "tx" => println!("{:?}", db::deserialize::<Transaction>(&message.data)),
                    _ => println!("err topic"),
                    }

                }
                _ => (),
            }
        }
    }
}
