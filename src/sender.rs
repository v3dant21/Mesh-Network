use crate::crypto::{CryptoState};
use crate::packet::{Packet, PayloadType};
use crate::reliability::ReliabilityManager;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use std::io::{self, Write};
use x25519_dalek::PublicKey;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

pub async fn run() {
    tracing::info!("Starting Secure Sender node...");
    
    let mut stream = match TcpStream::connect("127.0.0.1:8080").await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("Failed to connect to relay: {:?}", e);
            return;
        }
    };
    
    let crypto_state = CryptoState::new();
    let my_pub_key = crypto_state.public_key.clone();
    
    // Send Handshake
    tracing::info!("Sending handshake...");
    let handshake_packet = Packet::new(0, "Sender".to_string(), PayloadType::Handshake(my_pub_key.as_bytes().to_vec()));
    let _ = stream.write_all(&handshake_packet.to_bytes().unwrap()).await;
    
    // Wait for receiver handshake
    let mut buf = [0u8; 8192];
    tracing::info!("Waiting for receiver handshake...");
    let n = match stream.read(&mut buf).await {
        Ok(n) if n > 0 => n,
        _ => {
            tracing::error!("Failed to receive handshake from receiver.");
            return;
        }
    };
    
    let recv_packet = match Packet::from_bytes(&buf[0..n]) {
        Ok(p) => p,
        Err(_) => {
            tracing::error!("Corrupted handshake packet.");
            return;
        }
    };
    
    let session_crypto = match recv_packet.payload {
        PayloadType::Handshake(bytes) => {
            let mut pub_key_bytes = [0u8; 32];
            if bytes.len() == 32 {
                pub_key_bytes.copy_from_slice(&bytes);
                crypto_state.compute_shared_secret(PublicKey::from(pub_key_bytes))
            } else {
                tracing::error!("Invalid public key size.");
                return;
            }
        }
        _ => {
            tracing::error!("Expected Handshake packet, got {:?}", recv_packet.payload);
            return;
        }
    };
    
    tracing::info!("Secure connection established! You can now type messages.");
    println!("--- Secure E2E Chat (Sender) ---");
    
    let (mut rd, mut wr) = stream.into_split();
    let session_crypto = Arc::new(session_crypto);
    let session_crypto_clone = session_crypto.clone();
    
    let reliability = Arc::new(Mutex::new(ReliabilityManager::new()));
    let rel_clone = reliability.clone();
    
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Packet>(100);
    let tx_for_reader = tx.clone();
    let tx_for_retry = tx.clone();
    
    // Reader task
    tokio::spawn(async move {
        let mut buf = [0u8; 8192];
        loop {
            match rd.read(&mut buf).await {
                Ok(0) => {
                    tracing::warn!("Connection closed by relay.");
                    break;
                }
                Ok(n) => {
                    if let Ok(packet) = Packet::from_bytes(&buf[0..n]) {
                        let mut rm = rel_clone.lock().await;
                        
                        if matches!(packet.payload, PayloadType::Ack) {
                            rm.handle_ack(packet.message_id);
                            continue;
                        }
                        
                        if rm.is_duplicate(packet.message_id) {
                            continue;
                        }
                        
                        match packet.payload {
                            PayloadType::TextChat(encrypted) => {
                                if let Ok(decrypted) = session_crypto_clone.decrypt(&encrypted) {
                                    let text = String::from_utf8_lossy(&decrypted);
                                    let time = chrono::DateTime::from_timestamp_millis(packet.timestamp).unwrap().format("%H:%M:%S");
                                    println!("\r[{}] {}: {}", time, packet.sender_id, text);
                                    print!("Sender > "); io::stdout().flush().unwrap();
                                }
                                // Send Ack
                                let ack = Packet::new(packet.message_id, "Sender".to_string(), PayloadType::Ack);
                                let _ = tx_for_reader.send(ack).await;
                            }
                            _ => {}
                        }
                    }
                }
                Err(_) => break,
            }
        }
    });
    
    // Writer task
    tokio::spawn(async move {
        while let Some(packet) = rx.recv().await {
            if let Ok(bytes) = packet.to_bytes() {
                let _ = wr.write_all(&bytes).await;
            }
        }
    });

    // Retry task
    let rel_clone_retry = reliability.clone();
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_secs(3));
        loop {
            ticker.tick().await;
            let mut rm = rel_clone_retry.lock().await;
            let retries = rm.get_messages_to_retry();
            for r in retries {
                if let Ok(packet) = Packet::from_bytes(&r) {
                    tracing::info!("Retrying message {}", packet.message_id);
                    let _ = tx_for_retry.send(packet).await;
                }
            }
        }
    });
    
    // UI loop
    use tokio::io::AsyncBufReadExt;
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut line = String::new();
    
    let mut msg_id = rand::random::<u64>();
    loop {
        print!("Sender > ");
        io::stdout().flush().unwrap();
        line.clear();
        if reader.read_line(&mut line).await.is_err() { break; }
        
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        
        if let Ok(encrypted) = session_crypto.encrypt(trimmed.as_bytes()) {
            let packet = Packet::new(msg_id, "Sender".to_string(), PayloadType::TextChat(encrypted));
            
            if let Ok(bytes) = packet.to_bytes() {
                reliability.lock().await.track_unacked(msg_id, bytes);
            }
            
            let _ = tx.send(packet).await;
            msg_id += 1;
        }
    }
}