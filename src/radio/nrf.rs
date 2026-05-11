use async_trait::async_trait;
use crate::packet::Packet;
use crate::radio::Radio;
use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task;

pub struct NrfRadio {
    port_name: String,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
}

impl NrfRadio {
    pub fn new(port_name: &str) -> Result<Self, String> {
        let port = serialport::new(port_name, 115200) // NRF bridge usually faster
            .timeout(Duration::from_millis(100))
            .open()
            .map_err(|e| format!("Failed to open NRF bridge {}: {}", port_name, e))?;
            
        Ok(Self {
            port_name: port_name.to_string(),
            port: Arc::new(Mutex::new(port)),
        })
    }
}

#[async_trait]
impl Radio for NrfRadio {
    async fn send(&self, packet: &Packet) -> Result<(), String> {
        let bytes = packet.to_bytes().map_err(|e| e.to_string())?;
        let port = self.port.clone();
        
        task::spawn_blocking(move || {
            let mut p = port.lock().map_err(|_| "Mutex poisoned")?;
            p.write_all(&bytes).map_err(|e| e.to_string())?;
            p.flush().map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }).await.map_err(|e| e.to_string())?
    }

    async fn receive(&self) -> Result<Packet, String> {
        let port = self.port.clone();
        task::spawn_blocking(move || {
            let mut buf = [0u8; 1024];
            let mut p = port.lock().map_err(|_| "Mutex poisoned")?;
            let n = p.read(&mut buf).map_err(|e| e.to_string())?;
            if n > 0 {
                Packet::from_bytes(&buf[0..n]).map_err(|e| e.to_string())
            } else {
                Err("No data".to_string())
            }
        }).await.map_err(|e| e.to_string())?
    }

    fn name(&self) -> &str {
        "NRF24L01 (Serial Bridge)"
    }
}
