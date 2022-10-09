use super::super::structs;
use super::behaviour::Event;
use crate::{
    blockchain::{block::Block, transactions::Transaction},
    db::db,
    peers,
    protocol::{
        behaviour::MyBehaviour,
        gossipsub, kademlia,
        rr::{self, BlockRequest, BlockResponse},
        transport,
    },
    structs::{NodeStatus, ProtocolHelper},
};
use async_std::io::{self};
use bytemuck::__core::iter;
use crossbeam_channel::{Receiver, Sender};
use futures::AsyncBufReadExt;
use futures::StreamExt;
use libp2p::{
    futures::select,
    gossipsub::{Gossipsub, GossipsubEvent, IdentTopic as Topic},
    identify::{Identify, IdentifyConfig, IdentifyEvent, IdentifyInfo},
    identity,
    kad::Kademlia,
    kad::{record::store::MemoryStore, GetClosestPeersOk, KademliaEvent, QueryResult},
    mdns::{Mdns, MdnsConfig, MdnsEvent},
    request_response::{
        ProtocolSupport, RequestResponse, RequestResponseEvent, RequestResponseMessage,
    },
    swarm::SwarmEvent,
    PeerId, Swarm,
};
use std::{env, error::Error};
use structs::{BackendRequest, GameRequest};

pub async fn into_protocol(
    local_key: identity::Keypair,
    local_peer_id: PeerId,
    _sender: Sender<BackendRequest>,
    _reciever: Receiver<GameRequest>,
) -> Result<(), Box<dyn Error>> {
    // FriendsList

    let mut helper: ProtocolHelper;
    match db::get_put(
        "helper".to_string(),
        db::serialize(&ProtocolHelper::default()).expect("serdeError"),
    ) {
        Some(vector) => helper = db::deserialize(&vector).unwrap(),
        None => helper = ProtocolHelper::default(),
    }
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
        swarm.behaviour_mut().gossipsub.subscribe(&b).unwrap();
    } else {
        swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;
        swarm.listen_on("/ip6/::0/tcp/0".parse()?)?;
        swarm.behaviour_mut().gossipsub.subscribe(&b).unwrap();
        swarm.behaviour_mut().gossipsub.subscribe(&t).unwrap();
    }
    //swarm.listen_on("/ip4/192.168.1.197/tcp/54005".parse()?)?;
    swarm = kademlia::boot(swarm);
    let mut stdin = io::BufReader::new(io::stdin()).lines().fuse();
    for friend in helper.friends_list.friends() {
        swarm.behaviour_mut().kademlia.get_closest_peers(*friend);
    }
    loop {
        select! {
            line = stdin.select_next_some() => {
                println!("{:?}", line);
                match line {
                    Result::Ok(a) => {
                        //publish default block if press 1
                        if a == "1".to_string() {
                            swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(
                                    b.clone(),
                                    db::serialize(&Block::default()).expect("serde errir"),
                                )
                                .expect("pub");
                        } else if a == "2".to_string() {
                            let private = identity::Keypair::from_protobuf_encoding(&peers::P1KEY).expect("Decoding Error");
                            let  peerid = PeerId::from(private.public());
                            swarm.behaviour_mut().request.send_request(&peerid, BlockRequest());
                            let keyz = &identity::Keypair::generate_secp256k1();
                            swarm
                                .behaviour_mut()
                                .gossipsub
                                .publish(
                                    t.clone(),
                                    db::serialize(&Transaction::new(
                                        local_key.clone(),
                                        &keyz.public(),
                                        1,
                                        1,
                                    ))
                                    .expect("serde errir"),
                                )
                                .expect("pub");
                        }
                    }
                    _ => println!("empy"),
                }
            },
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
                    swarm.behaviour_mut().request.send_request(&peer_id, BlockRequest());
                },

                SwarmEvent::Behaviour(Event::Kademlia(KademliaEvent::OutboundQueryCompleted{id: _, result, stats: _})) => {
                    match result {
                        QueryResult::GetClosestPeers(Ok(GetClosestPeersOk{key: _, peers: _})) => {
                        },
                        _ => (),
                    }
                },
                SwarmEvent::Behaviour(Event::Kademlia(KademliaEvent::RoutablePeer{peer, address})) => {
                     swarm.behaviour_mut().kademlia.add_address(&peer, address);
                },
                SwarmEvent::Behaviour(Event::GossipSub(GossipsubEvent::Message {
                    propagation_source: peer_id,
                    message_id: _id,
                    message,
                })) => {
                    match helper.node_status{
                         NodeStatus::Pending => {
                            match message.topic.as_str() {
                                "block" =>{
                                    match db::deserialize::<Block>(&message.data){
                                       Ok(block) => {
                                            helper.pending_blocks.push(block);
                                       },
                                        Err(_) => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                    }
                                },

                            }
                             "tx" => {
                                    match db::deserialize::<Transaction>(&message.data){
                                       Ok(tx) => {
                                        match tx.verify_transaction_sig(){
                                            true => helper.mem_pool.add_tx(tx),
                                            false => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                        }
                                       },
                                       Err(_) => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                    }
                                },
                         }
                        _ => {
                            match message.topic.as_str() {
                                "block" =>{
                                    match db::deserialize::<Block>(&message.data){
                                       Ok(block) => {
                                        helper.pending_blocks.push(block);
                                        while !helper.pending_blocks.is_empty(){
                                            let b = helper.pending_blocks.pop_front();
                                            match b.validate(){
                                                (true, c) => {
                                                    helper.mem_pool.valid_block(block.clone());
                                                    helper.block_helper.add_to_chain(block.hash());
                                                    helper.block_helper.work_increment(c);
                                                    helper.accounts.valid_block(block);
                                                },
                                                (false, _) => (),
                                            }
                                        }
                                    },
                                       Err(_) => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                    }
                                },
                                "tx" => {
                                    match db::deserialize::<Transaction>(&message.data){
                                       Ok(tx) => {
                                        match tx.verify_transaction_sig(){
                                            true => helper.mem_pool.add_tx(tx),
                                            false => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                        }
                                       },
                                       Err(_) => swarm.behaviour_mut().gossipsub.blacklist_peer(&peer_id)
                                    }
                                },
                                _ => println!("err topic"),
                            }
                        }
                       
                }
                SwarmEvent::Behaviour(Event::RequestResponse(RequestResponseEvent::Message { peer: _, message })) => match message {
                //Request
                // We can go through this. Pretty much the goal is to remember the most frequent block.
                //It might be good the create your own algorithm for this. Keep in mind it is a bit funky.
                RequestResponseMessage::Request {
                    request_id: _,
                    request: _,
                    channel,
                } => {
                    swarm.behaviour_mut().request
                        .send_response(
                            channel,
                            BlockResponse(helper.accounts.clone(), helper.block_helper.clone()),
                        )
                        .expect("response error");
                }
                //response
                RequestResponseMessage::Response {
                    request_id: _,
                    response,
                } => {
                    let BlockResponse(accounts, block_help) = response;
                    match helper.node_status {
                        NodeStatus::Confirmed => {
                            if block_help != helper.block_helper{
                                if block_help.work > helper.block_helper.work {

                                }
                            }
                        },
                        NodeStatus::Pending => {

                        },
                    }
                }
            },
                _ => (),
            }
        }
    }
}
