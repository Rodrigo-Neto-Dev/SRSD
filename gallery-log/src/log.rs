use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct LogEntry {
    pub timestamp: u64,
    pub person_type: String, // "E" or "G"
    pub name: String,
    pub action: String, // "A" or "L"
    pub room: Option<u64>,
    pub prev_hash: Vec<u8>,
    pub mac: Vec<u8>,
}