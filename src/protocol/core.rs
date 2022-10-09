use super::super::structs;
use super::behaviour::Event;
use crate::{
    blockchain::{block::Block, transactions::Transaction},
    db::db::{self, deserialize},
    peers,
    protocol::{
        behaviour::MyBehaviour,
        gossipsub, kademlia,
        rr::{self, BlockRequest, BlockResponse, ResponseEnum},
        transport,
    },
    structs::{NodeStatus, PotentialChain, ProtocolHelper},
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
    kad::{
        record::store::MemoryStore, record::Key, GetClosestPeersOk, GetRecordError, GetRecordOk,
        KademliaEvent, QueryResult, Quorum,
    },
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
    sender: Sender<BackendRequest>,
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
                                    db::serialize(&Block::empty()).expect("serde errir"),
                                )
                                .expect("pub");
                        } else if a == "2".to_string() {
                            let private = identity::Keypair::from_protobuf_encoding(&peers::P1KEY).expect("Decoding Error");
                            let  peerid = PeerId::from(private.public());
                            swarm.behaviour_mut().request.send_request(&peerid, BlockRequest("block".to_string()));
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
                    swarm.behaviour_mut().request.send_request(&peer_id, BlockRequest("block".to_string()));
                },

                SwarmEvent::Behaviour(Event::Kademlia(KademliaEvent::OutboundQueryCompleted{id: _, result, stats: _})) => {
                    match result {
                        QueryResult::GetClosestPeers(Ok(GetClosestPeersOk{key: _, peers: _})) => {
                        },
                        QueryResult::GetRecord(Ok(GetRecordOk{records, cache_candidates: _})) => {
                            let mut block: Block= Block::empty();
                            let mut check: bool = false;
                            for record in records {
                                match db::deserialize::<Block>(&record.record.value){
                                    Ok(r) => {
                                        if Key::new(&r.hash()) == record.record.key{
                                            block = r;
                                            check = true;
                                            break;
                                        }
                                    },
                                    Err(_) => {
                                        continue;
                                    }
                                }
                            }
                            if check {
                                let mut remove: Vec<u8> = Vec::new();
                                let mut ind = 0;
                                for chain in &mut helper.pontential_chains{
                                    let index: usize = chain.current_i;
                                    if chain.block_help.chain.get(index).unwrap() == &block.hash() {
                                        if block.validate(&chain.account).0 {
                                            chain.current_i = chain.current_i + 1;
                                            chain.account.valid_block(&block);
                                            chain.block_help.work_increment(1);
                                            if chain.current_i == chain.block_help.chain.len() -1 {
                                                if chain.block_help.work > helper.block_helper.work {
                                                    helper.accounts = chain.account.clone();
                                                    helper.block_helper = chain.block_help.clone();
                                                    helper.node_status = NodeStatus::Confirmed;
                                                }
                                                else{
                                                    remove.push(ind);
                                                }
                                            }
                                            else {
                                                swarm.behaviour_mut().kademlia.get_record(Key::new(chain.block_help.chain.get(chain.current_i).unwrap()), libp2p::kad::Quorum::One);
                                            }
                                        }
                                        else {
                                            remove.push(ind);
                                        }
                                    }
                                    ind += 1;
                                }
                                let mut subtractor = 0;
                                for mut i in remove{
                                    i = i - subtractor;
                                    helper.pontential_chains.remove(i.into());
                                    subtractor += 1;
                                }
                            }

                        },
                        QueryResult::GetRecord(Err(error)) => {
                            match error {
                                GetRecordError::NotFound{ key, closest_peers: _} => {
                                let bs: String = deserialize::<String>(&key.to_vec()).unwrap();
                                let private = identity::Keypair::from_protobuf_encoding(&peers::P1KEY).expect("Decoding Error");
                                let peerid = PeerId::from(private.public());
                                swarm.behaviour_mut().request.send_request(&peerid, BlockRequest(bs));
                                },
                                GetRecordError::Timeout{key, records, quorum: _} => {
                                    if records.is_empty() {
                                        swarm.behaviour_mut().kademlia.get_record(key, libp2p::kad::Quorum::One);
                                    }
                                    else{
                                        let mut block: Block= Block::empty();
                            let mut check: bool = false;
                            for record in records {
                                match db::deserialize::<Block>(&record.record.value){
                                    Ok(r) => {
                                        if Key::new(&r.hash()) == record.record.key{
                                            block = r;
                                            check = true;
                                            break;
                                        }
                                    },
                                    Err(_) => {
                                        continue;
                                    }
                                }
                            }
                            if check {
                                let mut remove: Vec<u8> = Vec::new();
                                let mut ind = 0;
                                for chain in &mut helper.pontential_chains{
                                    let index: usize = chain.current_i;
                                    if chain.block_help.chain.get(index).unwrap() == &block.hash() {
                                        if block.validate(&chain.account).0 {
                                            chain.current_i = chain.current_i + 1;
                                            chain.account.valid_block(&block);
                                            chain.block_help.work_increment(1);
                                            if chain.current_i == chain.block_help.chain.len() -1 {
                                                if chain.block_help.work > helper.block_helper.work {
                                                    helper.accounts = chain.account.clone();
                                                    helper.block_helper = chain.block_help.clone();
                                                    helper.node_status = NodeStatus::Confirmed;
                                                }
                                                else{
                                                    remove.push(ind);
                                                }
                                            }
                                            else {
                                                swarm.behaviour_mut().kademlia.get_record(Key::new(chain.block_help.chain.get(chain.current_i).unwrap()), libp2p::kad::Quorum::One);
                                            }
                                        }
                                        else {
                                            remove.push(ind);
                                        }
                                    }
                                    ind += 1;
                                }
                                let mut subtractor = 0;
                                for mut i in remove{
                                    i = i - subtractor;
                                    helper.pontential_chains.remove(i.into());
                                    subtractor += 1;
                                }
                            }

                                    }
                                },
                                _ => (),
                            }
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
                                            helper.pending_blocks.push_back(block);
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
                                _ => {}
                            }
                         }
                        _ => {
                            match message.topic.as_str() {
                                "block" =>{
                                    match db::deserialize::<Block>(&message.data){
                                       Ok(block) => {
                                        helper.pending_blocks.push_back(block.clone());
                                        while !helper.pending_blocks.is_empty(){
                                            let b = helper.pending_blocks.pop_front().unwrap();
                                            match b.validate(&helper.accounts){
                                                (true, c) => {
                                                    helper.mem_pool.valid_block(&b);
                                                    helper.block_helper.add_to_chain(b.hash());
                                                    helper.block_helper.work_increment(c);
                                                    helper.accounts.valid_block(&b);
                                                    db::save_helper(&helper);
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
                },
                SwarmEvent::Behaviour(Event::RequestResponse(RequestResponseEvent::Message { peer, message })) => {
                match message {
                //Request
                // We can go through this. Pretty much the goal is to remember the most frequent block.
                //It might be good the create your own algorithm for this. Keep in mind it is a bit funky.
                RequestResponseMessage::Request {
                    request_id: _,
                    request,
                    channel,
                } => {
                    let BlockRequest(s) = request;
                    if s == "block".to_string() {
                    swarm.behaviour_mut().request
                        .send_response(
                            channel,
                            BlockResponse(ResponseEnum::Response(helper.accounts.clone(), helper.block_helper.clone())),
                        )
                        .expect("response error");
                    }
                    else {
                        match &db::get(s.clone()){
                            Ok(Some(block)) => {
                                match deserialize::<Block>(block){
                                    Ok(b) => {
                                        swarm.behaviour_mut().request
                                        .send_response(
                                        channel,
                                        BlockResponse(ResponseEnum::Block(b)))
                                        .expect("response error");
                                    },
                                    _ => {
                                        swarm.behaviour_mut().request
                                        .send_response(
                                        channel,
                                        BlockResponse(ResponseEnum::NoBlock(s.clone())))
                                        .expect("response error");
                                    },
                                }
                            },
                            _ => {
                                swarm.behaviour_mut().request
                                .send_response(
                                channel,
                                BlockResponse(ResponseEnum::NoBlock(s.clone())))
                                .expect("response error");
                            },
                        }
                    }

            },

                //response
                RequestResponseMessage::Response {
                    request_id: _,
                    response,
                } => {
                    let BlockResponse(r) = response;
                    match r {
                    ResponseEnum::Response(accounts, block_help) => {
                    match helper.node_status {
                        NodeStatus::Confirmed => {
                        },
                        NodeStatus::Pending => {
                            for p in helper.friends_list.friends() {
                                if &peer == p {
                                    helper.node_status = NodeStatus::PendingFriend;
                                    sender.send(BackendRequest::Start(accounts.clone())).expect("sender error");
                                }
                            }
                        },
                        NodeStatus::PendingFriend => { },
                    }
                    if block_help != helper.block_helper{
                        if block_help.work > helper.block_helper.work {
                            let s = block_help.chain[0].clone();
                            helper.pontential_chains.push(PotentialChain::new(block_help.chain));
                            swarm.behaviour_mut().kademlia.get_record(Key::new(&s), libp2p::kad::Quorum::One);
                        }
                    }
                }
                ResponseEnum::Block(b) => {
                    let mut remove: Vec<u8> = Vec::new();
                    let mut ind = 0;
                    for chain in &mut helper.pontential_chains{
                        let index: usize = chain.current_i;
                        if chain.block_help.chain.get(index).unwrap() == &b.hash() {
                        if b.validate(&chain.account).0 {
                            chain.current_i = chain.current_i + 1;
                            chain.account.valid_block(&b);
                            chain.block_help.work_increment(1);
                            if chain.current_i == chain.block_help.chain.len() -1 {
                                if chain.block_help.work > helper.block_helper.work {
                                    helper.accounts = chain.account.clone();
                                    helper.block_helper = chain.block_help.clone();
                                    helper.node_status = NodeStatus::Confirmed;
                                }
                                else{
                                    remove.push(ind);
                                }
                            }
                            else {
                                swarm.behaviour_mut().kademlia.get_record(Key::new(chain.block_help.chain.get(chain.current_i).unwrap()), libp2p::kad::Quorum::One);
                            }
                        }
                        else {
                            remove.push(ind);
                        }
                        }
                        ind += 1;
                    }
                    let mut subtractor = 0;
                    for mut i in remove{
                        i = i - subtractor;
                        helper.pontential_chains.remove(i.into());
                        subtractor += 1;
                    }
                }
                ResponseEnum::NoBlock(s) => {
                    let mut ind = 0;
                    for chain in &mut helper.pontential_chains{
                        let index: usize = chain.current_i;
                        if chain.block_help.chain.get(index).unwrap() == &s {
                            helper.pontential_chains.remove(ind);
                            break;
                        }
                        ind = ind + 1;
                    }
                }
                }}
                }
            },
                _ => (),
            }

        }
    }
}
