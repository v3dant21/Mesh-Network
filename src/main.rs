mod crypto;
mod packet;
mod reciver;
mod relay;
mod reliability;
mod sender;

use std::env;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} [sender|relay|receiver]", args[0]);
        return;
    }

    let mode = args[1].as_str();
    match mode {
        "sender" => sender::run().await,
        "relay" => relay::run().await,
        "receiver" => reciver::run().await,
        _ => eprintln!("Unknown mode. Use sender, relay, or receiver."),
    }
}
