use crate::crypto::{CryptoState, SessionCrypto};
use crate::packet::{Packet, PayloadType};
use crate::reliability::ReliabilityManager;
use crate::ui;
use colored::Colorize;
use tokio::net::TcpStream;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, AsyncBufReadExt};
use tokio::sync::{Mutex, mpsc};
use std::sync::Arc;
use x25519_dalek::PublicKey;

pub async fn run(target_ip: Option<&str>) {
    ui::clear_screen();
    ui::print_logo();
    ui::print_header("MESHCOM SECURE TRANSMITTER", "Initializing Encrypted Mesh-Link...");

    let target = target_ip.unwrap_or("127.0.0.1");
    let direct_addr = format!("{}:8081", target);
    let relay_addr = format!("{}:8080", target);

    // 1. Connection logic
    ui::print_status("NETWORK", &format!("Scanning for peers at {}...", target));
    let stream = loop {
        if let Ok(s) = TcpStream::connect(&direct_addr).await {
            ui::print_status("LINK", &format!("Connected DIRECTLY to Receiver at {}.", direct_addr));
            break s;
        }
        if let Ok(s) = TcpStream::connect(&relay_addr).await {
            ui::print_status("LINK", &format!("Connected via RELAY at {}.", relay_addr));
            break s;
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    };

    let (mut rd, mut wr) = stream.into_split();
    let (write_tx, mut write_rx) = mpsc::channel::<Packet>(100);
    
    let crypto = CryptoState::new();
    let my_pk = crypto.public_key.clone();
    let crypto_opt = Arc::new(Mutex::new(Some(crypto)));
    
    let session_crypto = Arc::new(Mutex::new(None::<SessionCrypto>));
    let reliability = Arc::new(Mutex::new(ReliabilityManager::new()));

    // Handshake
    ui::print_status("CRYPTO", "Exchanging public keys...");
    let hs = Packet::new(0, 100, PayloadType::Handshake(my_pk.as_bytes().to_vec()));
    let hs_bytes = hs.to_bytes().unwrap();
    let hs_len = hs_bytes.len() as u32;
    wr.write_all(&hs_len.to_be_bytes()).await.unwrap();
    wr.write_all(&hs_bytes).await.unwrap();

    // Background Writer
    tokio::spawn(async move {
        while let Some(pkt) = write_rx.recv().await {
            if let Ok(bytes) = pkt.to_bytes() {
                let len = bytes.len() as u32;
                if wr.write_all(&len.to_be_bytes()).await.is_err() { break; }
                if wr.write_all(&bytes).await.is_err() { break; }
            }
        }
    });

    // Background Reader
    let sc_reader = session_crypto.clone();
    let rel_reader = reliability.clone();
    let write_tx_reader = write_tx.clone();
    let cs_reader = crypto_opt.clone();
    tokio::spawn(async move {
        loop {
            let mut len_buf = [0u8; 4];
            if rd.read_exact(&mut len_buf).await.is_err() { break; }
            let len = u32::from_be_bytes(len_buf) as usize;
            let mut data = vec![0u8; len];
            if rd.read_exact(&mut data).await.is_err() { break; }
            
            if let Ok(pkt) = Packet::from_bytes(&data) {
                match pkt.payload {
                    PayloadType::Handshake(pk_bytes) => {
                        let mut cs_lock = cs_reader.lock().await;
                        if let Some(cs) = cs_lock.take() {
                            if pk_bytes.len() == 32 {
                                let peer_pk = PublicKey::from(<[u8; 32]>::try_from(pk_bytes).unwrap());
                                let shared = cs.compute_shared_secret(peer_pk);
                                *sc_reader.lock().await = Some(shared);
                                ui::print_status("SESSION", "End-to-End Encryption Enabled.");
                            }
                        }
                    }
                    PayloadType::Ack(id) => {
                        rel_reader.lock().await.handle_ack(id);
                    }
                    PayloadType::TextChat(encrypted) => {
                        let sc_lock = sc_reader.lock().await;
                        if let Some(ref cipher) = *sc_lock {
                            if let Ok(dec) = cipher.decrypt(&encrypted) {
                                let text = String::from_utf8_lossy(&dec);
                                ui::print_received(pkt.from, &text);
                                ui::set_input_prompt("Sender");
                                
                                let ack = Packet::new(pkt.id, 100, PayloadType::Ack(pkt.id));
                                let _ = write_tx_reader.send(ack).await;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });

    // Retry Task
    let rel_retry = reliability.clone();
    let write_tx_retry = write_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            interval.tick().await;
            let mut rm = rel_retry.lock().await;
            for bytes in rm.get_retries() {
                if let Ok(pkt) = Packet::from_bytes(&bytes) {
                    let _ = write_tx_retry.send(pkt).await;
                }
            }
        }
    });

    // UI Loop
    let mut line = String::new();
    let mut stdin_reader = BufReader::new(tokio::io::stdin());
    let mut msg_id = rand::random::<u32>();
    println!("\n{}", "--- SYSTEM ONLINE ---".white().dimmed());
    loop {
        ui::set_input_prompt("Sender");
        line.clear();
        if stdin_reader.read_line(&mut line).await.is_err() { break; }
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let sc_lock = session_crypto.lock().await;
        if let Some(ref cipher) = *sc_lock {
            if let Ok(enc) = cipher.encrypt(trimmed.as_bytes()) {
                let pkt = Packet::new(msg_id, 100, PayloadType::TextChat(enc));
                if let Ok(bytes) = pkt.to_bytes() {
                    reliability.lock().await.track(msg_id, bytes);
                }
                let _ = write_tx.send(pkt).await;
                msg_id += 1;
            }
        } else {
            ui::print_status("WARN", "Waiting for secure session...");
        }
    }
}