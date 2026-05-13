use async_trait::async_trait;
use crate::packet::Packet;
use crate::radio::Radio;
use serialport::SerialPort;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::task;

pub struct BluetoothRadio {
    port_name: String,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
}

impl BluetoothRadio {
    pub fn new(port_name: &str) -> Result<Self, String> {
        let port = serialport::new(port_name, 9600)
            .timeout(Duration::from_secs(5))
            .open()
            .map_err(|e| format!("Failed to open BT port {}: {}", port_name, e))?;
            
        Ok(Self {
            port_name: port_name.to_string(),
            port: Arc::new(Mutex::new(port)),
        })
    }
}

#[async_trait]
impl Radio for BluetoothRadio {
    async fn send(&self, packet: &Packet) -> Result<(), String> {
        let bytes = packet.to_bytes().map_err(|e| e.to_string())?;
        let port = self.port.clone();
        
        task::spawn_blocking(move || {
            let mut p = port.lock().map_err(|_| "Mutex poisoned")?;
            let len = bytes.len() as u32;
            p.write_all(&len.to_be_bytes()).map_err(|e| e.to_string())?;
            p.write_all(&bytes).map_err(|e| e.to_string())?;
            p.flush().map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }).await.map_err(|e| e.to_string())?
    }

    async fn receive(&self) -> Result<Packet, String> {
        let port = self.port.clone();
        task::spawn_blocking(move || {
            let mut p = port.lock().map_err(|_| "Mutex poisoned")?;
            
            let mut len_buf = [0u8; 4];
            p.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
            let len = u32::from_be_bytes(len_buf) as usize;
            
            if len > 65536 { return Err("Packet too large".to_string()); }
            
            let mut buf = vec![0u8; len];
            p.read_exact(&mut buf).map_err(|e| e.to_string())?;
            Packet::from_bytes(&buf).map_err(|e| e.to_string())
        }).await.map_err(|e| e.to_string())?
    }

    fn name(&self) -> &str {
        "Bluetooth (HC-05)"
    }
}
