use rocksdb::{Options, DB};
use serde::Serialize;

use crate::structs::{InsightError, ProtocolHelper};

//Input value into the database with given key
//Key and Value should be serialized
pub fn put(key: String, value: String) -> Result<(), InsightError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(3);
    let path = "database";
    let db = DB::open(&opts, path).expect("DB Open failed");
    match db.put(key, value) {
        Ok(()) => Ok(()),
        Err(e) => Err(InsightError::DBError(e)),
    }
}

//delete value from the database with given key
pub fn delete(key: String) -> Result<(), InsightError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(3);
    let path = "database";
    let db = DB::open(&opts, path).expect("DB Open failed");
    match db.delete(key) {
        Ok(()) => Ok(()),
        Err(e) => Err(InsightError::DBError(e)),
    }
}

//get value from the database with given key
pub fn get(key: String) -> Result<Option<Vec<u8>>, InsightError> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(3);
    let path = "database";
    let db = DB::open(&opts, path).expect("DB Open failed");
    match db.get(key) {
        Ok(o) => Ok(o),
        Err(e) => Err(InsightError::DBError(e)),
    }
}

pub fn serialize<T: Serialize>(value: &T) -> Result<String, InsightError> {
    match serde_json::to_string(value) {
        Ok(s) => Ok(s),
        Err(e) => Err(InsightError::SerdeError(e)),
    }
}

pub fn deserialize<'a, T>(value: &'a Vec<u8>) -> Result<T, InsightError>
where
    T: serde::de::Deserialize<'a>,
{
    match serde_json::from_str(std::str::from_utf8(value).expect("utf8 error")) {
        Ok(t) => Ok(t),
        Err(e) => Err(InsightError::SerdeError(e)),
    }
}
pub fn get_put(key: String, value: String) -> Option<Vec<u8>> {
    let mut opts = Options::default();
    opts.create_if_missing(true);
    opts.increase_parallelism(3);
    let path = "database";
    let db = DB::open(&opts, path).expect("DB Open failed");
    if let Ok(Some(value)) = db.get(key.clone()) {
        Some(value)
    } else {
        db.put(key, value).expect("put failed");
        None
    }
}
pub fn save_helper(h: &ProtocolHelper) {
    put("helper".to_string(), serialize(h).unwrap()).expect("DB ERROR");
}
