use crate::crypto::{CryptoState, SessionCrypto};
use crate::packet::{Packet, PayloadType};
use crate::reliability::ReliabilityManager;
use crate::ui;
use colored::Colorize;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt, AsyncBufReadExt};
use tokio::sync::{Mutex, mpsc};
use std::sync::Arc;
use std::io;
use x25519_dalek::PublicKey;

pub async fn run(target_ip: Option<&str>) {
    ui::clear_screen();
    ui::print_logo();
    ui::print_header("MESHCOM SECURE RECEIVER", "Awaiting Encrypted Mesh-Link...");

    let reliability = Arc::new(Mutex::new(ReliabilityManager::new()));
    let session_crypto = Arc::new(Mutex::new(None::<SessionCrypto>));
    let last_active_addr = Arc::new(Mutex::new(None::<std::net::SocketAddr>));
    let (write_tx, mut write_rx) = mpsc::channel::<(Option<std::net::SocketAddr>, Packet)>(100);

    // Direct listener on 8081 (bind to all interfaces for external access)
    let listener = TcpListener::bind("0.0.0.0:8081").await.unwrap();
    let direct_conns = Arc::new(Mutex::new(std::collections::HashMap::<std::net::SocketAddr, tokio::net::tcp::OwnedWriteHalf>::new()));

    // Packet Processor Closure
    let process_packet = {
        let reliability = reliability.clone();
        let session_crypto = session_crypto.clone();
        let write_tx = write_tx.clone();
        let crypto_state = Arc::new(Mutex::new(Some(CryptoState::new())));
        let last_active = last_active_addr.clone();

        move |packet: Packet, addr: Option<std::net::SocketAddr>| {
            let rel = reliability.clone();
            let sc = session_crypto.clone();
            let wtx = write_tx.clone();
            let cs_atom = crypto_state.clone();
            let la = last_active.clone();

            tokio::spawn(async move {
                if let Some(a) = addr {
                    *la.lock().await = Some(a);
                }

                let mut rm = rel.lock().await;
                match packet.payload {
                    PayloadType::Ack(id) => {
                        rm.handle_ack(id);
                        return;
                    }
                    _ => {
                        if rm.is_duplicate(packet.id) { return; }
                    }
                }
                drop(rm);

                match packet.payload {
                    PayloadType::Handshake(pk_bytes) => {
                        ui::print_status("HANDSHAKE", &format!("Request from node {}", packet.from));
                        let mut cs_lock = cs_atom.lock().await;
                        let cs = cs_lock.take().unwrap_or_else(CryptoState::new);
                        let my_pk = cs.public_key.clone();
                        
                        if pk_bytes.len() == 32 {
                            let peer_pk = PublicKey::from(<[u8; 32]>::try_from(pk_bytes).unwrap());
                            let shared = cs.compute_shared_secret(peer_pk);
                            *sc.lock().await = Some(shared);
                            
                            let resp = Packet::new(0, 200, PayloadType::Handshake(my_pk.as_bytes().to_vec()));
                            let _ = wtx.send((addr, resp)).await;
                            ui::print_status("SESSION", "Secure Link established.");
                        }
                    }
                    PayloadType::TextChat(encrypted) => {
                        let sc_lock = sc.lock().await;
                        if let Some(ref cipher) = *sc_lock {
                            match cipher.decrypt(&encrypted) {
                                Ok(data) => {
                                    let text = String::from_utf8_lossy(&data);
                                    ui::print_received(packet.from, &text);
                                    ui::set_input_prompt("Receiver");
                                    
                                    let ack = Packet::new(packet.id, 200, PayloadType::Ack(packet.id));
                                    let _ = wtx.send((addr, ack)).await;
                                }
                                Err(_) => ui::print_status("ERROR", "Decryption failed."),
                            }
                        }
                    }
                    _ => {}
                }
            });
        }
    };

    // Relay Connection
    let relay_tx = {
        let (tx, mut rx) = mpsc::channel::<Packet>(100);
        let process = process_packet.clone();
        let target = target_ip.unwrap_or("127.0.0.1").to_string();
        tokio::spawn(async move {
            let relay_addr = format!("{}:8080", target);
            loop {
                if let Ok(stream) = TcpStream::connect(&relay_addr).await {
                    ui::print_status("RELAY", &format!("Connected to mesh at {}.", relay_addr));
                    let (mut rd, mut wr) = stream.into_split();
                    
                    let reader = {
                        let p = process.clone();
                        tokio::spawn(async move {
                            loop {
                                let mut len_buf = [0u8; 4];
                                if rd.read_exact(&mut len_buf).await.is_err() { break; }
                                let len = u32::from_be_bytes(len_buf) as usize;
                                let mut data = vec![0u8; len];
                                if rd.read_exact(&mut data).await.is_err() { break; }
                                if let Ok(pkt) = Packet::from_bytes(&data) { p(pkt, None); }
                            }
                        })
                    };

                    while let Some(pkt) = rx.recv().await {
                        if let Ok(bytes) = pkt.to_bytes() {
                            let len = bytes.len() as u32;
                            if wr.write_all(&len.to_be_bytes()).await.is_err() { break; }
                            if wr.write_all(&bytes).await.is_err() { break; }
                        }
                    }
                    reader.abort();
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        tx
    };

    // Direct Writer Dispatcher
    let d_conns = direct_conns.clone();
    let r_tx = relay_tx.clone();
    tokio::spawn(async move {
        while let Some((addr, pkt)) = write_rx.recv().await {
            if let Ok(bytes) = pkt.to_bytes() {
                let len = bytes.len() as u32;
                let mut sent_direct = false;
                
                if let Some(target) = addr {
                    let mut conns = d_conns.lock().await;
                    if let Some(wr) = conns.get_mut(&target) {
                        if wr.write_all(&len.to_be_bytes()).await.is_ok() && wr.write_all(&bytes).await.is_ok() {
                            sent_direct = true;
                        }
                    }
                }

                if !sent_direct {
                    let _ = r_tx.send(pkt).await;
                }
            }
        }
    });

    // Direct Listener Task
    let d_conns_listener = direct_conns.clone();
    let process_d = process_packet.clone();
    tokio::spawn(async move {
        loop {
            if let Ok((socket, addr)) = listener.accept().await {
                ui::print_status("LINK", &format!("Direct connection from {}", addr));
                let (mut rd, wr) = socket.into_split();
                d_conns_listener.lock().await.insert(addr, wr);
                let p = process_d.clone();
                tokio::spawn(async move {
                    loop {
                        let mut len_buf = [0u8; 4];
                        if rd.read_exact(&mut len_buf).await.is_err() { break; }
                        let len = u32::from_be_bytes(len_buf) as usize;
                        let mut data = vec![0u8; len];
                        if rd.read_exact(&mut data).await.is_err() { break; }
                        if let Ok(pkt) = Packet::from_bytes(&data) { p(pkt, Some(addr)); }
                    }
                });
            }
        }
    });

    // UI Loop
    let mut line = String::new();
    let stdin = tokio::io::stdin();
    let mut reader = tokio::io::BufReader::new(stdin);
    let mut msg_id = rand::random::<u32>();
    println!("\n{}", "--- SYSTEM ONLINE ---".white().dimmed());
    loop {
        ui::set_input_prompt("Receiver");
        line.clear();
        if reader.read_line(&mut line).await.is_err() { break; }
        let trimmed = line.trim();
        if trimmed.is_empty() { continue; }

        let sc_lock = session_crypto.lock().await;
        if let Some(ref cipher) = *sc_lock {
            if let Ok(enc) = cipher.encrypt(trimmed.as_bytes()) {
                let pkt = Packet::new(msg_id, 200, PayloadType::TextChat(enc));
                if let Ok(bytes) = pkt.to_bytes() {
                    reliability.lock().await.track(msg_id, bytes);
                }
                
                // Smart Routing: Use last active address if available
                let target = *last_active_addr.lock().await;
                let _ = write_tx.send((target, pkt)).await;
                msg_id += 1;
            }
        } else {
            ui::print_status("WARN", "No secure session established.");
        }
    }
}