#![allow(warnings)]
mod crypto;
mod packet;
mod receiver;
mod relay;
mod reliability;
mod sender;
mod radio;
mod ui;

use std::env;
use std::process::Command;
use std::thread;
use std::time::Duration;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::ERROR)
        .init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} [sender|relay|receiver|all]", args[0]);
        return;
    }

    let mode = args[1].as_str();
    match mode {
        "all" => {
            println!("🚀 Launching MESHCOM Full Network...");
            
            // 1. Launch Relay
            println!("   - Spawning Relay...");
            let _ = Command::new("cmd")
                .args(&["/C", "start", "cmd", "/K", "title MESHCOM RELAY && cargo run -- relay"])
                .spawn();
            
            thread::sleep(Duration::from_secs(2));

            // 2. Launch Receiver
            println!("   - Spawning Receiver...");
            let _ = Command::new("cmd")
                .args(&["/C", "start", "cmd", "/K", "title MESHCOM RECEIVER && cargo run -- receiver"])
                .spawn();

            thread::sleep(Duration::from_secs(1));

            // 3. Launch Sender
            println!("   - Spawning Sender...");
            let _ = Command::new("cmd")
                .args(&["/C", "start", "cmd", "/K", "title MESHCOM SENDER && cargo run -- sender"])
                .spawn();

            println!("\n✅ All nodes spawned in separate windows.");
        }
        "sender" => {
            let target_ip = args.get(2).map(|s| s.as_str());
            sender::run(target_ip).await
        }
        "relay" => relay::run().await,
        "receiver" => {
            let target_ip = args.get(2).map(|s| s.as_str());
            receiver::run(target_ip).await
        }
        _ => eprintln!("Unknown mode. Use sender, relay, receiver, or all."),
    }
}
