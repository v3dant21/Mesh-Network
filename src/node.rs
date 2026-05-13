use crate::crypto::{CryptoState, SessionCrypto};
use crate::packet::{Packet, PayloadType};
use crate::reliability::ReliabilityManager;
use crate::radio::Radio;
use crate::ui;
use colored::Colorize;
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::sync::{Mutex, mpsc};
use std::sync::Arc;
use std::collections::HashMap;
use x25519_dalek::PublicKey;

pub async fn run(radio: Box<dyn Radio>, node_id_opt: Option<&str>) {
    ui::clear_screen();
    ui::print_logo();
    ui::print_header("MESHCOM SECURE NODE", &format!("Radio: {}", radio.name()));

    let my_id: u32 = node_id_opt
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| rand::random::<u32>() % 10000);

    ui::print_status("IDENTITY", &format!("My Node ID: {}", my_id));

    let radio = Arc::new(radio);
    let (write_tx, mut write_rx) = mpsc::channel::<Packet>(100);
    
    let crypto = CryptoState::new();
    let my_pk = crypto.public_key.clone();
    let crypto_state = Arc::new(crypto);
    
    let sessions: Arc<Mutex<HashMap<u32, SessionCrypto>>> = Arc::new(Mutex::new(HashMap::new()));
    let reliability = Arc::new(Mutex::new(ReliabilityManager::new()));

    // Handshake broadcast
    ui::print_status("CRYPTO", "Broadcasting public key...");
    let hs = Packet::new(0, my_id, 0, PayloadType::Handshake(my_pk.as_bytes().to_vec()));
    let _ = write_tx.send(hs).await;

    // Background Writer
    let radio_writer = radio.clone();
    tokio::spawn(async move {
        while let Some(pkt) = write_rx.recv().await {
            if let Err(e) = radio_writer.send(&pkt).await {
                tracing::error!("Radio send error: {}", e);
                break;
            }
        }
    });

    // Background Reader
    let sessions_reader = sessions.clone();
    let rel_reader = reliability.clone();
    let write_tx_reader = write_tx.clone();
    let cs_reader = crypto_state.clone();
    let radio_reader = radio.clone();
    tokio::spawn(async move {
        loop {
            match radio_reader.receive().await {
                Ok(pkt) => {
                    ui::print_status("DEBUG", &format!("Received packet from Node {}", pkt.from));
                    match pkt.payload {
                        PayloadType::Handshake(pk_bytes) => {
                            if pk_bytes.len() == 32 {
                                let peer_pk = PublicKey::from(<[u8; 32]>::try_from(pk_bytes).unwrap());
                                let shared = cs_reader.compute_shared_secret(peer_pk);
                                sessions_reader.lock().await.insert(pkt.from, shared);
                                ui::print_status("SESSION", &format!("End-to-End Encryption Established with Node {}.", pkt.from));
                                
                                if pkt.to == 0 {
                                    let hs_reply = Packet::new(0, my_id, pkt.from, PayloadType::Handshake(cs_reader.public_key.as_bytes().to_vec()));
                                    let _ = write_tx_reader.send(hs_reply).await;
                                }
                            }
                        }
                        PayloadType::Ack(id) => {
                            rel_reader.lock().await.handle_ack(id);
                        }
                        PayloadType::TextChat(encrypted) => {
                            let sessions_lock = sessions_reader.lock().await;
                            if let Some(cipher) = sessions_lock.get(&pkt.from) {
                                if let Ok(dec) = cipher.decrypt(&encrypted) {
                                    let text = String::from_utf8_lossy(&dec);
                                    ui::print_received(pkt.from, &text);
                                    ui::set_input_prompt(&format!("Node {}", my_id));
                                    
                                    let ack = Packet::new(pkt.id, my_id, pkt.from, PayloadType::Ack(pkt.id));
                                    let _ = write_tx_reader.send(ack).await;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                Err(e) if e.contains("timed out") => continue,
                Err(e) => {
                    if e.contains("Broken pipe") || e.contains("Handshake") || e.contains("EOF") {
                        ui::print_status("ERROR", &format!("Bluetooth Link Lost: {}. Retrying in 5s...", e));
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    } else {
                        tracing::error!("Radio receive error: {}", e);
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
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
    println!("Commands: /msg <id> <text> | /peers | /broadcast <text>");
    loop {
        ui::set_input_prompt(&format!("Node {}", my_id));
        line.clear();
        if stdin_reader.read_line(&mut line).await.is_err() { break; }
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        if trimmed.starts_with("/peers") {
            let sessions_lock = sessions.lock().await;
            let peers: Vec<String> = sessions_lock.keys().map(|k| k.to_string()).collect();
            println!("  Connected Peers: [{}]", peers.join(", ").green());
            continue;
        }

        if trimmed.starts_with("/handshake") {
            ui::print_status("CRYPTO", "Manual handshake broadcast...");
            let hs = Packet::new(0, my_id, 0, PayloadType::Handshake(my_pk.as_bytes().to_vec()));
            let _ = write_tx.send(hs).await;
            continue;
        }

        if trimmed.starts_with("/broadcast ") {
            let text = &trimmed[11..];
            let sessions_lock = sessions.lock().await;
            for (peer_id, cipher) in sessions_lock.iter() {
                if let Ok(enc) = cipher.encrypt(text.as_bytes()) {
                    let pkt = Packet::new(msg_id, my_id, *peer_id, PayloadType::TextChat(enc));
                    if let Ok(bytes) = pkt.to_bytes() {
                        reliability.lock().await.track(msg_id, bytes);
                    }
                    let _ = write_tx.send(pkt).await;
                    msg_id += 1;
                }
            }
            continue;
        }

        if trimmed.starts_with("/msg ") {
            let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();
            if parts.len() == 3 {
                if let Ok(target_id) = parts[1].parse::<u32>() {
                    let text = parts[2];
                    let sessions_lock = sessions.lock().await;
                    if let Some(cipher) = sessions_lock.get(&target_id) {
                        if let Ok(enc) = cipher.encrypt(text.as_bytes()) {
                            let pkt = Packet::new(msg_id, my_id, target_id, PayloadType::TextChat(enc));
                            if let Ok(bytes) = pkt.to_bytes() {
                                reliability.lock().await.track(msg_id, bytes);
                            }
                            let _ = write_tx.send(pkt).await;
                            msg_id += 1;
                        }
                    } else {
                        println!("  {} Node {} not found in session list.", "ERROR:".red(), target_id);
                    }
                } else {
                    println!("  {} Invalid node ID.", "ERROR:".red());
                }
            } else {
                println!("  {} Usage: /msg <id> <text>", "ERROR:".red());
            }
            continue;
        }

        println!("  {} Unknown command. Use /msg, /peers, or /broadcast", "INFO:".yellow());
    }
}
