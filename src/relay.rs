use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast;
use std::net::SocketAddr;

pub async fn run() {
    tracing::info!("Starting Headless Framed Relay on port 8080...");
    let listener = TcpListener::bind("0.0.0.0:8080").await.expect("Failed to bind 8080");
    
    // Broadcast channel for (SenderAddr, FullFramedPacket)
    let (tx, _rx) = broadcast::channel::<(SocketAddr, Vec<u8>)>(1024);

    loop {
        let (socket, addr) = listener.accept().await.expect("Accept failed");
        tracing::info!("[RELAY] New client: {}", addr);

        let tx = tx.clone();
        let mut rx = tx.subscribe();

        tokio::spawn(async move {
            let (mut rd, mut wr) = socket.into_split();
            
            // Client -> Relay -> Broadcast
            let tx_clone = tx.clone();
            let mut reader_task = tokio::spawn(async move {
                loop {
                    let mut len_buf = [0u8; 4];
                    if rd.read_exact(&mut len_buf).await.is_err() { break; }
                    let len = u32::from_be_bytes(len_buf) as usize;
                    if len > 1024 * 1024 { break; } // Sanity check 1MB

                    let mut data = vec![0u8; len];
                    if rd.read_exact(&mut data).await.is_err() { break; }
                    
                    tracing::info!("[RELAY] Forwarding {} bytes from {}", len, addr);
                    
                    let mut framed = len_buf.to_vec();
                    framed.extend_from_slice(&data);
                    let _ = tx_clone.send((addr, framed));
                }
            });

            // Broadcast -> Relay -> Client
            loop {
                tokio::select! {
                    res = rx.recv() => {
                        match res {
                            Ok((sender_addr, framed)) => {
                                if sender_addr != addr {
                                    if wr.write_all(&framed).await.is_err() { break; }
                                }
                            }
                            Err(broadcast::error::RecvError::Lagged(_)) => continue,
                            Err(_) => break,
                        }
                    }
                    _ = &mut reader_task => break,
                }
            }
            tracing::info!("[RELAY] Client disconnected: {}", addr);
        });
    }
}