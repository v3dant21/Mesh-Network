use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::radio::RadioType;

pub struct Relay {
    routes: Arc<RwLock<HashMap<u32, Arc<RadioType>>>>,
}

impl Relay {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn run(&self, bt_ports: Vec<String>) {
        let listener = TcpListener::bind("0.0.0.0:9090").await.unwrap();
        println!("[SYSTEM] Relay listening on 0.0.0.0:9090");

        let rt = self.routes.clone();
        tokio::spawn(async move {
            loop {
                if let Ok((socket, _)) = listener.accept().await {
                    Self::spawn_handler(Arc::new(RadioType::new_tcp(socket)), rt.clone());
                }
            }
        });

        for port in bt_ports {
            if let Ok(radio) = RadioType::new_serial(&port, 9600, false) {
                Self::spawn_handler(Arc::new(radio), self.routes.clone());
            } else {
                eprintln!("[SYSTEM] Failed to bind Bluetooth port: {}", port);
            }
        }

        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    }

    fn spawn_handler(radio: Arc<RadioType>, routes: Arc<RwLock<HashMap<u32, Arc<RadioType>>>>) {
        tokio::spawn(async move {
            let mut my_id = None;
            loop {
                match radio.receive().await {
                    Ok(pkt) => {
                        if my_id.is_none() {
                            my_id = Some(pkt.from);
                            routes.write().await.insert(pkt.from, radio.clone());
                            println!("Registered Node {}", pkt.from);
                        }
                        
                        let r = routes.read().await;
                        if pkt.to == 0 {
                            for (id, rad) in r.iter() {
                                if *id != pkt.from {
                                    let _ = rad.send(&pkt).await;
                                }
                            }
                        } else if let Some(rad) = r.get(&pkt.to) {
                            let _ = rad.send(&pkt).await;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Some(id) = my_id {
                routes.write().await.remove(&id);
                println!("Node {} disconnected", id);
            }
        });
    }
}
