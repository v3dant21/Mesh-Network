use async_trait::async_trait;
use crate::packet::Packet;

pub mod tcp;
pub mod nrf;
pub mod bluetooth;

#[async_trait]
pub trait Radio: Send + Sync {
    async fn send(&self, packet: &Packet) -> Result<(), String>;
    async fn receive(&self) -> Result<Packet, String>;
    fn name(&self) -> &str;
}

pub struct RadioManager {
    radios: Vec<Box<dyn Radio>>,
}

impl RadioManager {
    pub fn new() -> Self {
        Self { radios: Vec::new() }
    }

    pub fn add_radio(&mut self, radio: Box<dyn Radio>) {
        self.radios.push(radio);
    }

    pub async fn broadcast(&self, packet: &Packet) {
        for radio in &self.radios {
            if let Err(e) = radio.send(packet).await {
                tracing::error!("Failed to send via {}: {}", radio.name(), e);
            }
        }
    }
}
