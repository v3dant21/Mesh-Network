use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use serialport::SerialPort;
use crate::packet::Packet;

pub enum RadioType {
    Tcp {
        reader: Arc<Mutex<ReadHalf<'static>>>,
        writer: Arc<Mutex<WriteHalf<'static>>>,
    },
    Bluetooth {
        port: Arc<std::sync::Mutex<Box<dyn SerialPort>>>,
        is_nrf: bool,
    },
}

impl RadioType {
    pub fn new_tcp(stream: TcpStream) -> Self {
        let leaked = Box::leak(Box::new(stream));
        let (r, w) = leaked.split();
        Self::Tcp { reader: Arc::new(Mutex::new(r)), writer: Arc::new(Mutex::new(w)) }
    }

    pub fn new_serial(name: &str, baud: u32, is_nrf: bool) -> Result<Self, String> {
        let port = serialport::new(name, baud).timeout(Duration::from_millis(500)).open().map_err(|e| e.to_string())?;
        Ok(Self::Bluetooth { port: Arc::new(std::sync::Mutex::new(port)), is_nrf })
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Tcp { .. } => "TCP Simulator",
            Self::Bluetooth { is_nrf: true, .. } => "NRF24L01",
            Self::Bluetooth { .. } => "Bluetooth (HC-05)",
        }
    }

    pub async fn send(&self, packet: &Packet) -> Result<(), String> {
        let bytes = packet.to_bytes().map_err(|e| e.to_string())?;
        match self {
            Self::Tcp { writer, .. } => {
                let mut w = writer.lock().await;
                w.write_all(&(bytes.len() as u32).to_be_bytes()).await.map_err(|e| e.to_string())?;
                w.write_all(&bytes).await.map_err(|e| e.to_string())?;
            }
            Self::Bluetooth { port, is_nrf } => {
                let port = port.clone();
                let is_nrf_val = *is_nrf;
                tokio::task::spawn_blocking(move || {
                    let mut p = port.lock().unwrap();
                    if !is_nrf_val {
                        p.write_all(&(bytes.len() as u32).to_be_bytes()).map_err(|e| e.to_string())?;
                    }
                    p.write_all(&bytes).map_err(|e| e.to_string())?;
                    p.flush().map_err(|e| e.to_string())?;
                    Ok::<(), String>(())
                }).await.unwrap()?;
            }
        }
        Ok(())
    }

    pub async fn receive(&self) -> Result<Packet, String> {
        match self {
            Self::Tcp { reader, .. } => {
                let mut r = reader.lock().await;
                let mut len_buf = [0u8; 4];
                r.read_exact(&mut len_buf).await.map_err(|e| e.to_string())?;
                let len = u32::from_be_bytes(len_buf) as usize;
                let mut buf = vec![0u8; len];
                r.read_exact(&mut buf).await.map_err(|e| e.to_string())?;
                Packet::from_bytes(&buf).map_err(|e| e.to_string())
            }
            Self::Bluetooth { port, is_nrf } => {
                let port = port.clone();
                let is_nrf_val = *is_nrf;
                tokio::task::spawn_blocking(move || {
                    let mut p = port.lock().unwrap();
                    if is_nrf_val {
                        let mut buf = [0u8; 1024];
                        let n = p.read(&mut buf).map_err(|e| e.to_string())?;
                        if n > 0 { Packet::from_bytes(&buf[0..n]).map_err(|e| e.to_string()) }
                        else { Err("No data".into()) }
                    } else {
                        let mut len_buf = [0u8; 4];
                        p.read_exact(&mut len_buf).map_err(|e| e.to_string())?;
                        let len = u32::from_be_bytes(len_buf) as usize;
                        if len > 65536 { return Err("Too large".into()); }
                        let mut buf = vec![0u8; len];
                        p.read_exact(&mut buf).map_err(|e| e.to_string())?;
                        Packet::from_bytes(&buf).map_err(|e| e.to_string())
                    }
                }).await.unwrap()
            }
        }
    }
}
