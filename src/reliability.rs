use std::collections::{HashSet, HashMap};
use tokio::time::{Duration, Instant};

pub struct ReliabilityManager {
    seen_messages: HashSet<u64>,
    unacked_messages: HashMap<u64, (Vec<u8>, Instant)>,
    retry_timeout: Duration,
}

impl ReliabilityManager {
    pub fn new() -> Self {
        Self {
            seen_messages: HashSet::new(),
            unacked_messages: HashMap::new(),
            retry_timeout: Duration::from_secs(3),
        }
    }

    pub fn is_duplicate(&mut self, message_id: u64) -> bool {
        if self.seen_messages.contains(&message_id) {
            true
        } else {
            self.seen_messages.insert(message_id);
            false
        }
    }

    pub fn track_unacked(&mut self, message_id: u64, packet_bytes: Vec<u8>) {
        self.unacked_messages.insert(message_id, (packet_bytes, Instant::now()));
    }

    pub fn handle_ack(&mut self, message_id: u64) {
        self.unacked_messages.remove(&message_id);
    }

    pub fn get_messages_to_retry(&mut self) -> Vec<Vec<u8>> {
        let mut to_retry = Vec::new();
        let now = Instant::now();
        for (_, (bytes, last_sent)) in self.unacked_messages.iter_mut() {
            if now.duration_since(*last_sent) >= self.retry_timeout {
                to_retry.push(bytes.clone());
                *last_sent = now;
            }
        }
        to_retry
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deduplication() {
        let mut rm = ReliabilityManager::new();
        assert_eq!(rm.is_duplicate(1), false);
        assert_eq!(rm.is_duplicate(1), true);
        assert_eq!(rm.is_duplicate(2), false);
    }

    #[tokio::test]
    async fn test_retry_mechanism() {
        let mut rm = ReliabilityManager::new();
        rm.retry_timeout = Duration::from_millis(10);
        
        rm.track_unacked(1, vec![1, 2, 3]);
        
        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(15)).await;
        
        let retries = rm.get_messages_to_retry();
        assert_eq!(retries.len(), 1);
        assert_eq!(retries[0], vec![1, 2, 3]);
        
        // Ack it
        rm.handle_ack(1);
        tokio::time::sleep(Duration::from_millis(15)).await;
        let retries_after_ack = rm.get_messages_to_retry();
        assert_eq!(retries_after_ack.len(), 0);
    }
}
