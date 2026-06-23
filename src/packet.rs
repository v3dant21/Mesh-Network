use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PayloadType {
    TextChat(Vec<u8>),
    Handshake(Vec<u8>),
    Ack(u32),
    Ping,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet {
    pub id: u32,
    pub from: u32,
    pub to: u32,
    pub payload: PayloadType,
    pub time: u64,
}

impl Packet {
    pub fn new(id: u32, from: u32, to: u32, payload: PayloadType) -> Self {
        Self {
            id, from, to, payload,
            time: chrono::Utc::now().timestamp_millis() as u64,
        }
    }
    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> { bincode::serialize(self) }
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> { bincode::deserialize(bytes) }
}
