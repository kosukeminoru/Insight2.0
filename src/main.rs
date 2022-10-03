pub mod blockchain;
pub mod db;
pub mod peers;
pub mod protocol;
pub mod structs;
use crossbeam_channel::{unbounded, Sender};
use ffi::GameSend;
use futures::executor::block_on;
use libp2p::{identity, PeerId};
use std::{env, thread};
use structs::{BackendRequest, GameRequest};

#[cxx::bridge(namespace = "org::blobstore")]
mod ffi {
    #[derive(Debug)]
    enum GameSend {
        Block,
        Transaction,
    }
    enum BackendSend {
        Move,
        MOOve,
    }

    // Shared structs with fields visible to both languages.
    struct BlobMetadata {
        size: usize,
        tags: Vec<String>,
    }

    // Rust types and signatures exposed to C++.
    extern "Rust" {

        type MultiBuf;
        fn send(buf: &mut MultiBuf, gs: GameSend);
        fn next_chunk(buf: &mut MultiBuf) -> &[u8];
    }

    // C++ types and signatures exposed to Rust.
    unsafe extern "C++" {
        include!("insight2/include/blobstore.h");

        type BlobstoreClient;

        fn new_blobstore_client() -> UniquePtr<BlobstoreClient>;
        fn put(&self, parts: &mut MultiBuf) -> u64;
        fn tag(&self, blobid: u64, tag: &str);
        fn metadata(&self, blobid: u64) -> BlobMetadata;
    }
}
pub struct MultiBuf {
    pub senda: Sender<GameSend>,
    chunks: Vec<Vec<u8>>,
    pos: usize,
}

pub fn send(buf: &mut MultiBuf, gs: GameSend) {
    buf.senda.send(gs);
}
pub fn next_chunk(buf: &mut MultiBuf) -> &[u8] {
    let next = buf.chunks.get(buf.pos);
    buf.pos += 1;
    next.map_or(&[], Vec::as_slice)
}

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
        private = identity::Keypair::generate_secp256k1();
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
