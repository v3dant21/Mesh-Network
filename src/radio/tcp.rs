use crate::packet::Packet;
use crate::radio::Radio;
use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use std::sync::Arc;

pub struct TcpRadio {
    reader: Arc<Mutex<ReadHalf<'static>>>,
    writer: Arc<Mutex<WriteHalf<'static>>>,
}

// We use a bit of 'unsafe' or Box::leak here because TcpStream::split 
// creates halves that are bound to the lifetime of the stream. 
// For a simple mesh app, we can leak the stream to make it 'static.
impl TcpRadio {
    pub fn new(stream: TcpStream) -> Self {
        let leaked_stream = Box::leak(Box::new(stream));
        let (rh, wh) = leaked_stream.split();
        Self {
            reader: Arc::new(Mutex::new(rh)),
            writer: Arc::new(Mutex::new(wh)),
        }
    }
}

#[async_trait]
impl Radio for TcpRadio {
    fn name(&self) -> &str {
        "TCP Simulator"
    }

    async fn send(&self, packet: &Packet) -> Result<(), String> {
        let bytes = packet.to_bytes().map_err(|e| e.to_string())?;
        let len = bytes.len() as u32;
        let mut w = self.writer.lock().await;
        w.write_all(&len.to_be_bytes()).await.map_err(|e| e.to_string())?;
        w.write_all(&bytes).await.map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn receive(&self) -> Result<Packet, String> {
        let mut r = self.reader.lock().await;
        
        let mut len_buf = [0u8; 4];
        r.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
        let len = u32::from_be_bytes(len_buf) as usize;

        let mut buf = vec![0u8; len];
        r.read_exact(&mut buf).await.map_err(|e| e.to_string())?;

        Packet::from_bytes(&buf).map_err(|e| e.to_string())
    }
}
