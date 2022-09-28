use libp2p::{identity::secp256k1::PublicKey, PeerId};
use rocksdb::Error as DBError;
use serde::{Deserialize, Serialize};
use serde_json::Error as SerdeError;
use std::collections::HashMap;

#[derive(Debug)]
pub enum InsightError {
    DBError(DBError),
    SerdeError(SerdeError),
}
pub enum GameRequest {
    AddFriend(PeerId),
    RemoveFriend(PeerId),
    SendTransaction(PublicKey, f32),
    CreateBlock(),
}
pub enum BackendRequest {
    AddFriend(PeerId),
    RemoveFriend(PeerId),
    SendTransaction(PublicKey, f32),
    CreateBlock(),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Accounts {
    pub value: String,
}
impl Accounts {
    pub fn new() -> Accounts {
        Accounts {
            value: "3".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ValueList {
    pub list: HashMap<PeerId, AccountInfo>,
}
impl ValueList {
    pub fn account(&self, peer: &PeerId) -> Option<&AccountInfo> {
        self.list.get(peer)
    }
    pub fn add(&mut self, peer: PeerId, v: f32) {
        let acnt = self.list.entry(peer).or_insert(AccountInfo::default());
        let mut x = *acnt;
        x.value_add(v);
        self.list.insert(peer, x);
    }
    pub fn sub(&mut self, peer: PeerId, v: f32) {
        let acnt = self.list.entry(peer).or_insert(AccountInfo::default());
        let mut x = *acnt;
        x.value_sub(v);
        self.list.insert(peer, x);
    }
    pub fn nonce_increment(&mut self, peer: PeerId) {
        let acnt = self.list.entry(peer).or_insert(AccountInfo::default());
        let mut x = *acnt;
        x.nonce_inc();
        self.list.insert(peer, x);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct AccountInfo {
    pub value: f32,
    pub nonce: u32,
}
impl AccountInfo {
    pub fn default() -> AccountInfo {
        AccountInfo {
            value: 0.0,
            nonce: 1,
        }
    }
    pub fn value_add(&mut self, v: f32) {
        self.value += v;
    }
    pub fn value_sub(&mut self, v: f32) {
        self.value -= v;
    }
    pub fn nonce_inc(&mut self) {
        self.nonce += 1;
    }
}
