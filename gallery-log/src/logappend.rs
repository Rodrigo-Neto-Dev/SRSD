use crate::crypto::*;
use crate::log::LogEntry;
use sha2::{Sha256, Digest};

pub fn append_entry(key: &[u8], prev_hash: &[u8], entry: &mut LogEntry) {
    let serialized = serde_json::to_vec(entry).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(prev_hash);
    hasher.update(&serialized);
    let new_hash = hasher.finalize();

    entry.prev_hash = prev_hash.to_vec();
    entry.mac = compute_mac(key, &serialized);
}