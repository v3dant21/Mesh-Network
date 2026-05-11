use async_trait::async_trait;
use crate::packet::Packet;
use crate::radio::Radio;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct TcpRadio {
    stream: Arc<Mutex<TcpStream>>,
}

impl TcpRadio {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            stream: Arc::new(Mutex::new(stream)),
        }
    }
}

#[async_trait]
impl Radio for TcpRadio {
    async fn send(&self, packet: &Packet) -> Result<(), String> {
        let bytes = packet.to_bytes().map_err(|e| e.to_string())?;
        let len = bytes.len() as u32;
        let mut s = self.stream.lock().await;
        s.write_all(&len.to_be_bytes()).await.map_err(|e| e.to_string())?;
        s.write_all(&bytes).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn receive(&self) -> Result<Packet, String> {
        let mut s = self.stream.lock().await;
        
        // Read length prefix
        let mut len_buf = [0u8; 4];
        s.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(len_buf) as usize;
        
        if len > 65536 {
            return Err("Packet too large".to_string());
        }
        
        let mut buf = vec![0u8; len];
        s.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
        Packet::from_bytes(&buf).map_err(|e| e.to_string())
    }

    fn name(&self) -> &str {
        "TCP Simulator"
    }
}
