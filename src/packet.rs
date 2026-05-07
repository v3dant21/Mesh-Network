use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum PayloadType {
    TextChat(Vec<u8>), // Encrypted payload
    Ack,
    Ping,
    Reconnect,
    Handshake(Vec<u8>), // Public key for key exchange
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet {
    pub message_id: u64,
    pub timestamp: i64,
    pub sender_id: String,
    pub payload: PayloadType,
}

impl Packet {
    pub fn new(message_id: u64, sender_id: String, payload: PayloadType) -> Self {
        Self {
            message_id,
            timestamp: chrono::Utc::now().timestamp_millis(),
            sender_id,
            payload,
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization() {
        let payload = PayloadType::TextChat(vec![1, 2, 3, 4]);
        let packet = Packet::new(42, "Alice".to_string(), payload.clone());

        let bytes = packet.to_bytes().unwrap();
        let deserialized = Packet::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.message_id, 42);
        assert_eq!(deserialized.sender_id, "Alice");
        assert_eq!(deserialized.payload, payload);
    }
}
