use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, mpsc};
use x25519_dalek::PublicKey;

use crate::crypto::{CryptoState, SessionCrypto};
use crate::packet::{Packet, PayloadType};
use crate::radio::RadioType;
use crate::reliability::ReliabilityManager;

enum Command {
    Msg { target: u32, text: String },
    Broadcast(String),
    Peers,
    Unknown,
    Empty,
}

impl Command {
    fn parse(input: &str) -> Self {
        let t = input.trim();
        if t.is_empty() {
            return Self::Empty;
        }

        if t == "/peers" {
            Self::Peers
        } else if let Some(text) = t.strip_prefix("/broadcast ") {
            Self::Broadcast(text.to_string())
        } else if let Some(rest) = t.strip_prefix("/msg ") {
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            if parts.len() == 2 {
                if let Ok(target) = parts[0].parse::<u32>() {
                    return Self::Msg {
                        target,
                        text: parts[1].to_string(),
                    };
                }
            }
            Self::Unknown
        } else {
            Self::Unknown
        }
    }
}

pub struct Node {
    my_id: u32,
    radio: Arc<RadioType>,
    crypto_state: Arc<CryptoState>,
    sessions: Arc<Mutex<HashMap<u32, SessionCrypto>>>,
    reliability: Arc<Mutex<ReliabilityManager>>,
    write_tx: mpsc::Sender<Packet>,
    write_rx: Option<mpsc::Receiver<Packet>>,
}

impl Node {
    pub fn new(radio: RadioType, id_opt: Option<&str>) -> Self {
        let my_id: u32 = id_opt
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| rand::random::<u32>() % 10000);

        let (write_tx, write_rx) = mpsc::channel::<Packet>(100);

        Self {
            my_id,
            radio: Arc::new(radio),
            crypto_state: Arc::new(CryptoState::new()),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            reliability: Arc::new(Mutex::new(ReliabilityManager::new())),
            write_tx,
            write_rx: Some(write_rx),
        }
    }

    pub async fn run(&mut self) {
        println!("--- MESHCOM SECURE NODE ---");
        println!("Radio: {}", self.radio.name());
        println!("My Node ID: {}", self.my_id);

        println!("[CRYPTO] Broadcasting public key...");
        let _ = self.write_tx.send(Packet::new(
            0,
            self.my_id,
            0,
            PayloadType::Handshake(self.crypto_state.public_key.as_bytes().to_vec()),
        )).await;

        self.spawn_radio_sender();
        self.spawn_radio_receiver();
        self.spawn_retry_manager();
        self.run_ui_loop().await;
    }

    fn spawn_radio_sender(&mut self) {
        let mut rx = self.write_rx.take().expect("write_rx already taken");
        let radio = self.radio.clone();
        
        tokio::spawn(async move {
            while let Some(pkt) = rx.recv().await {
                if let Err(e) = radio.send(&pkt).await {
                    eprintln!("Radio send error: {}", e);
                    break;
                }
            }
        });
    }

    fn spawn_radio_receiver(&self) {
        let rr = self.radio.clone();
        let sr = self.sessions.clone();
        let rel_r = self.reliability.clone();
        let cs_r = self.crypto_state.clone();
        let tx_r = self.write_tx.clone();
        let my_id = self.my_id;

        tokio::spawn(async move {
            loop {
                match rr.receive().await {
                    Ok(pkt) => {
                        let mut r_lock = rel_r.lock().await;
                        if r_lock.is_duplicate(pkt.id) {
                            continue;
                        }
                        drop(r_lock);

                        match pkt.payload {
                            PayloadType::Handshake(pk_bytes) if pk_bytes.len() == 32 => {
                                let peer_pk = PublicKey::from(<[u8; 32]>::try_from(pk_bytes).unwrap());
                                sr.lock().await.insert(pkt.from, cs_r.compute_shared_secret(peer_pk));
                                println!("[SESSION] Encrypted link with Node {}", pkt.from);
                                if pkt.to == 0 {
                                    let _ = tx_r.send(Packet::new(
                                        0,
                                        my_id,
                                        pkt.from,
                                        PayloadType::Handshake(cs_r.public_key.as_bytes().to_vec()),
                                    )).await;
                                }
                            }
                            PayloadType::Ack(id) => rel_r.lock().await.handle_ack(id),
                            PayloadType::TextChat(enc) => {
                                if let Some(cipher) = sr.lock().await.get(&pkt.from) {
                                    if let Ok(dec) = cipher.decrypt(&enc) {
                                        println!("\r[Node {}]: {}", pkt.from, String::from_utf8_lossy(&dec));
                                        print!("> ");
                                        io::stdout().flush().unwrap();
                                        let _ = tx_r.send(Packet::new(pkt.id, my_id, pkt.from, PayloadType::Ack(pkt.id))).await;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
                }
            }
        });
    }

    fn spawn_retry_manager(&self) {
        let rel_rt = self.reliability.clone();
        let tx_rt = self.write_tx.clone();
        
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(3));
            loop {
                ticker.tick().await;
                for b in rel_rt.lock().await.get_retries() {
                    if let Ok(p) = Packet::from_bytes(&b) {
                        let _ = tx_rt.send(p).await;
                    }
                }
            }
        });
    }

    async fn run_ui_loop(&self) {
        let mut line = String::new();
        let mut reader = BufReader::new(tokio::io::stdin());
        let mut msg_id = rand::random::<u32>();
        
        println!("Commands: /msg <id> <text> | /peers | /broadcast <text>");
        loop {
            print!("> ");
            io::stdout().flush().unwrap();
            line.clear();
            
            if reader.read_line(&mut line).await.is_err() {
                break;
            }

            match Command::parse(&line) {
                Command::Empty => continue,
                Command::Peers => {
                    let p: Vec<_> = self.sessions.lock().await.keys().map(|k| k.to_string()).collect();
                    println!("Peers: [{}]", p.join(", "));
                }
                Command::Broadcast(text) => {
                    let mut lock = self.sessions.lock().await;
                    for (pid, c) in lock.iter() {
                        if let Ok(enc) = c.encrypt(text.as_bytes()) {
                            let pkt = Packet::new(msg_id, self.my_id, *pid, PayloadType::TextChat(enc));
                            if let Ok(b) = pkt.to_bytes() {
                                self.reliability.lock().await.track(msg_id, b);
                            }
                            let _ = self.write_tx.send(pkt).await;
                            msg_id += 1;
                        }
                    }
                }
                Command::Msg { target, text } => {
                    if let Some(c) = self.sessions.lock().await.get(&target) {
                        if let Ok(enc) = c.encrypt(text.as_bytes()) {
                            let pkt = Packet::new(msg_id, self.my_id, target, PayloadType::TextChat(enc));
                            if let Ok(b) = pkt.to_bytes() {
                                self.reliability.lock().await.track(msg_id, b);
                            }
                            let _ = self.write_tx.send(pkt).await;
                            msg_id += 1;
                        }
                    } else {
                        println!("Node not found.");
                    }
                }
                Command::Unknown => {
                    println!("Unknown command. Usage: /msg <id> <text> | /peers | /broadcast <text>");
                }
            }
        }
    }
}
