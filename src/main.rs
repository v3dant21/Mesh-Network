mod crypto;
mod node;
mod packet;
mod radio;
mod relay;
mod reliability;

use std::env;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tokio::net::TcpStream;

use node::Node;
use radio::RadioType;
use relay::Relay;

fn spawn_terminal(title: &str, cmd: &str) {
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

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} [node|relay|all]", args.get(0).unwrap_or(&"".into()));
        return;
    }

    match args[1].as_str() {
        "all" => {
            spawn_terminal("MESHCOM_RELAY", "cargo run -- relay");
            thread::sleep(Duration::from_secs(2));
            spawn_terminal("MESHCOM_NODE_101", "cargo run -- node 127.0.0.1 101");
            thread::sleep(Duration::from_secs(1));
            spawn_terminal("MESHCOM_NODE_102", "cargo run -- node 127.0.0.1 102");
            thread::sleep(Duration::from_secs(1));
            spawn_terminal("MESHCOM_NODE_103", "cargo run -- node 127.0.0.1 103");
        }
        "node" => {
            let target = args.get(2).map(|s| s.as_str()).unwrap_or("127.0.0.1");
            let radio = if target.starts_with("/dev/") || target.starts_with("COM") {
                RadioType::new_serial(target, 9600, false).unwrap()
            } else {
                let addr = if target.contains(':') { target.into() } else { format!("{}:9090", target) };
                RadioType::new_tcp(TcpStream::connect(addr).await.unwrap())
            };
            let mut node = Node::new(radio, args.get(3).map(|s| s.as_str()));
            node.run().await;
        }
        "relay" => {
            let bt_ports: Vec<String> = args.into_iter().skip(2).collect();
            let relay = Relay::new();
            relay.run(bt_ports).await;
        }
        _ => eprintln!("Unknown mode."),
    }
}
