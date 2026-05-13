#![allow(warnings)]
mod crypto;
mod packet;
mod node;
mod relay;
mod reliability;
mod radio;
mod ui;

use crate::radio::tcp::TcpRadio;
use crate::radio::bluetooth::BluetoothRadio;
use crate::radio::Radio;
use tokio::net::TcpStream;

use std::env;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn spawn_terminal(title: &str, cmd: &str) {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("cmd")
            .args(&["/C", "start", "cmd", "/K", &format!("title {} && {}", title, cmd)])
            .spawn();
    }
    
    #[cfg(target_os = "macos")]
    {
        if let Ok(dir) = env::current_dir() {
            let script = format!("tell application \"Terminal\" to do script \"cd {} && {}; exec bash\"", dir.display(), cmd);
            let _ = Command::new("osascript")
                .args(&["-e", &script])
                .spawn();
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        let bash_cmd = format!("{}; exec bash", cmd);
        let terminals = [
            ("ghostty", vec!["-e", "bash", "-c"]),
            ("kitty", vec!["--title", title, "bash", "-c"]),
            ("alacritty", vec!["-t", title, "-e", "bash", "-c"]),
            ("wezterm", vec!["start", "--", "bash", "-c"]),
            ("gnome-terminal", vec!["--title", title, "--", "bash", "-c"]),
            ("konsole", vec!["-e", "bash", "-c"]),
            ("xfce4-terminal", vec!["-T", title, "-x", "bash", "-c"]),
            ("x-terminal-emulator", vec!["-e", "bash", "-c"]),
            ("xterm", vec!["-T", title, "-e", "bash", "-c"]),
        ];

        let mut success = false;
        for (term, args) in terminals.iter() {
            if let Ok(_) = Command::new(term).args(args).arg(&bash_cmd).spawn() {
                success = true;
                break;
            }
        }
        
        if !success {
            eprintln!("Failed to spawn any terminal. Please run nodes manually.");
        }
    }
}

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
            
            println!("   - Spawning Relay...");
            spawn_terminal("MESHCOM_RELAY", "cargo run -- relay");
            
            thread::sleep(Duration::from_secs(2));

            println!("   - Spawning Node 101...");
            spawn_terminal("MESHCOM_NODE_101", "cargo run -- node 127.0.0.1 101");

            thread::sleep(Duration::from_secs(1));

            println!("   - Spawning Node 102...");
            spawn_terminal("MESHCOM_NODE_102", "cargo run -- node 127.0.0.1 102");

            thread::sleep(Duration::from_secs(1));

            println!("   - Spawning Node 103...");
            spawn_terminal("MESHCOM_NODE_103", "cargo run -- node 127.0.0.1 103");

            println!("\n✅ All nodes spawned in separate windows.");
        }
        "node" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1");
            let node_id = args.get(3).map(|s| s.as_str());
            
            let radio: Box<dyn Radio> = if target.starts_with("/dev/") || target.starts_with("COM") {
                Box::new(BluetoothRadio::new(target).expect("Failed to open Bluetooth port"))
            } else {
                let addr = if target.contains(':') { target.to_string() } else { format!("{}:9090", target) };
                let stream = TcpStream::connect(addr).await.expect("Failed to connect to relay");
                Box::new(TcpRadio::new(stream))
            };

            node::run(radio, node_id).await
        }
        "relay" => {
            let mut radios: Vec<Box<dyn Radio>> = Vec::new();
            for port in args.iter().skip(2) {
                match BluetoothRadio::new(port) {
                    Ok(r) => radios.push(Box::new(r)),
                    Err(e) => eprintln!("⚠️  Warning: Could not open Bluetooth port {}: {}", port, e),
                }
            }
            relay::run_multi_relay(radios).await;
        },
        _ => eprintln!("Unknown mode. Use node, relay, or all."),
    }
}
