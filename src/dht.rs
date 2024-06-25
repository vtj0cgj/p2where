use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Peer {
    pub id: String,
    pub ip: String,
    pub port: u16,
}

pub struct DHT {
    peers: HashMap<String, Peer>,
}

impl DHT {
    pub fn new() -> Self {
        DHT {
            peers: HashMap::new(),
        }
    }

    pub fn add_peer(&mut self, ip: &str, port: u16) -> Peer {
        let id = generate_peer_id(ip, port);
        let peer = Peer {
            id: id.clone(),
            ip: ip.to_string(),
            port,
        };
        self.peers.insert(id.clone(), peer.clone());
        peer
    }

    pub fn find_peers(&self) -> Vec<Peer> {
        self.peers.values().cloned().collect()
    }
}

fn generate_peer_id(ip: &str, port: u16) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{}:{}", ip, port));
    format!("{:x}", hasher.finalize())
}
