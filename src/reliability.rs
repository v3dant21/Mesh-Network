use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

pub struct ReliabilityManager {
    seen_ids: HashSet<u32>,
    unacked: HashMap<u32, (Vec<u8>, Instant)>,
}

impl ReliabilityManager {
    pub fn new() -> Self { Self { seen_ids: HashSet::new(), unacked: HashMap::new() } }
    pub fn is_duplicate(&mut self, id: u32) -> bool { if id == 0 { false } else { !self.seen_ids.insert(id) } }
    pub fn track(&mut self, id: u32, data: Vec<u8>) { self.unacked.insert(id, (data, Instant::now())); }
    pub fn handle_ack(&mut self, id: u32) { self.unacked.remove(&id); }
    pub fn get_retries(&mut self) -> Vec<Vec<u8>> {
        let now = Instant::now();
        let mut retries = Vec::new();
        for (data, last_sent) in self.unacked.values_mut() {
            if now.duration_since(*last_sent) > Duration::from_secs(3) {
                retries.push(data.clone());
                *last_sent = now;
            }
        }
        retries
    }
}
