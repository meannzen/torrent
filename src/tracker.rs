use serde::{Deserialize, Serialize};

use crate::Peers;

#[derive(Debug, Clone, Serialize)]
pub struct TrackerRequest {
    pub peer_id: String,
    pub port: u16,
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    pub compact: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
pub struct TrackerResponse {
    pub interval: usize,
    pub peers: Peers,
}
