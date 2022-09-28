use super::transactions::Transaction;
use crate::structs::ValueList;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::SystemTime;

#[derive(Serialize, Deserialize, Debug)]
pub struct HashString(String);

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Blockchain {
    pub chain: Vec<Block>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Block {
    /// Reference to the previous block in the chain.
    pub prev_blockhash: String,
    /// The timestamp of the block, as claimed by the miner.
    pub time: SystemTime,
    // The nonce, selected to obtain a low enough blockhash.
    pub tx: Vec<Transaction>,
    pub world: String,
}

impl Block {
    pub fn default() -> Block {
        Block {
            prev_blockhash: "000000000000000".to_string(),
            tx: Vec::<Transaction>::with_capacity(100),
            world: "".to_string(),
            time: SystemTime::now(),
        }
    }
    pub fn new(prev: String, t: Vec<Transaction>, w: String) -> Block {
        Block {
            prev_blockhash: prev,
            time: SystemTime::now(),
            tx: t,
            world: w,
        }
    }
    pub fn validate(&self) -> bool {
        true
    }
    pub fn validate_work(&self) -> bool {
        true
    }
    pub fn validate_tx(&self, v: &ValueList) -> bool {
        for t in &self.tx {
            if !(t.verify_transaction_sig() && t.verify_value(v)) {
                return false;
            }
        }
        true
    }
    pub fn validate_new(&self, value: &ValueList) -> bool {
        if self.validate_work() && self.validate_tx(value) {
            return true;
        }
        false
    }
    pub fn hash(&self) -> String {
        let mut hasher = Sha256::new();
        let serialized = serde_json::to_string(&self).unwrap();
        hasher.update(serialized);
        let result: String = format!("{:X}", hasher.finalize());
        println!("{:?}", result);
        result
    }
}
