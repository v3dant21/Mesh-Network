use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use std::net::SocketAddr;

pub async fn run() {
    tracing::info!("Starting headless secure relay node on port 8080...");
    let listener = match TcpListener::bind("127.0.0.1:8080").await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind port 8080: {:?}", e);
            return;
        }
    };
    
    let (tx, _rx) = broadcast::channel::<(SocketAddr, Vec<u8>)>(100);

    loop {
        let (mut socket, addr) = match listener.accept().await {
            Ok(res) => res,
            Err(_) => continue,
        };
        tracing::info!("New connection from: {}", addr);

        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let mut buf = [0u8; 8192];
            loop {
                tokio::select! {
                    result = socket.read(&mut buf) => {
                        let n = match result {
                            Ok(0) => break,
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        let packet_bytes = buf[0..n].to_vec();
                        tracing::info!("Relay forwarded packet from {}", addr);
                        // Blindly forward, without decrypting
                        let _ = tx.send((addr, packet_bytes));
                    }
                    result = rx.recv() => {
                        if let Ok((sender_addr, packet_bytes)) = result {
                            if sender_addr != addr {
                                if let Err(_) = socket.write_all(&packet_bytes).await {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        });
    }
}