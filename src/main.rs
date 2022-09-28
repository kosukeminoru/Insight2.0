pub mod blockchain;
pub mod db;
pub mod peers;
pub mod protocol;
pub mod structs;
use crossbeam_channel::unbounded;
use futures::executor::block_on;
use libp2p::{identity, PeerId};
use std::{env, thread};
use structs::{BackendRequest, GameRequest};

fn main() {
    let args: Vec<String> = env::args().collect();
    let query = &args[1];
    let private: identity::Keypair;
    let peerid: PeerId;
    //Bootstrap node
    if query == "1" {
        private = identity::Keypair::from_protobuf_encoding(&peers::P1KEY).expect("Decoding Error");
        peerid = PeerId::from(private.public());
    }
    //random node
    else {
        private = identity::Keypair::generate_ed25519();
        peerid = PeerId::from(private.public());
    }
    //Crossbeam channel set up
    let (backend_sender, _game_reciever) = unbounded::<BackendRequest>();
    let (_game_sender, backend_reciever) = unbounded::<GameRequest>();
    let my_future =
        protocol::core::into_protocol(private, peerid, backend_sender, backend_reciever);
    thread::spawn(move || block_on(my_future).expect("Thread Spawn Error"));
    loop {}
}
