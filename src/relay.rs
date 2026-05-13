use tokio::sync::mpsc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use crate::packet::Packet;
use crate::radio::Radio;
use tokio::net::TcpListener;
use crate::radio::tcp::TcpRadio;

type RoutingTable = Arc<RwLock<HashMap<u32, Arc<dyn Radio>>>>;

pub async fn run_multi_relay(bt_ports: Vec<Box<dyn Radio>>) {
    let routes: RoutingTable = Arc::new(RwLock::new(HashMap::new()));
    
    let tcp_routes = routes.clone();
    let listener = TcpListener::bind("0.0.0.0:9090").await.expect("Failed to bind 9090");
    println!("[SYSTEM] TCP Relay listening on 0.0.0.0:9090");
    
    tokio::spawn(async move {
        loop {
            if let Ok((socket, _)) = listener.accept().await {
                let radio = Arc::new(TcpRadio::new(socket));
                let rt = tcp_routes.clone();
                tokio::spawn(async move {
                    handle_radio(radio, rt).await;
                });
            }
        }
    });

    for radio in bt_ports {
        let radio: Arc<dyn Radio> = radio.into();
        let rt = routes.clone();
        tokio::spawn(async move {
            handle_radio(radio, rt).await;
        });
    }

    println!("[SYSTEM] Relay is fully active and waiting for nodes...");
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
    }
}

async fn handle_radio(radio: Arc<dyn Radio>, routes: RoutingTable) {
    let mut my_id: Option<u32> = None;
    loop {
        match radio.receive().await {
            Ok(pkt) => {
                // 1. Handle Registration
                if my_id.is_none() {
                    let id = pkt.from;
                    my_id = Some(id);
                    let mut w = routes.write().await;
                    w.insert(id, radio.clone());
                    println!("[RELAY] Registered Node {} on {}", id, radio.name());
                    drop(w); // Explicitly release the lock!
                }

                // 2. Handle Routing
                let r = routes.read().await;
                if pkt.to == 0 {
                    println!("[ROUTING] Broadcast from Node {}", pkt.from);
                    for (id, other_radio) in r.iter() {
                        if *id != pkt.from {
                            let _ = other_radio.send(&pkt).await;
                        }
                    }
                } else {
                    if let Some(target_radio) = r.get(&pkt.to) {
                        println!("[ROUTING] Direct: Node {} -> Node {}", pkt.from, pkt.to);
                        let _ = target_radio.send(&pkt).await;
                    }
                }
                drop(r); // Explicitly release the lock!
            }
            Err(e) if e.contains("timed out") => continue,
            Err(_) => break,
        }
    }
    if let Some(id) = my_id {
        routes.write().await.remove(&id);
        println!("[RELAY] Node {} disconnected", id);
    }
}

pub async fn run_tcp() {
    let listener = TcpListener::bind("0.0.0.0:9090").await.expect("Failed to bind 9090");
    let routes: RoutingTable = Arc::new(RwLock::new(HashMap::new()));
    loop {
        let (socket, _) = listener.accept().await.unwrap();
        let radio = Arc::new(TcpRadio::new(socket));
        let rt = routes.clone();
        tokio::spawn(async move { handle_radio(radio, rt).await; });
    }
}

pub async fn run() { run_tcp().await; }