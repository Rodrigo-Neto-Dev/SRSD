use crate::crypto::*;
use crate::log::LogEntry;

pub fn verify_entry(key: &[u8], entry: &LogEntry) -> bool {
    let serialized = serde_json::to_vec(entry).unwrap();
    verify_mac(key, &serialized, &entry.mac)
}