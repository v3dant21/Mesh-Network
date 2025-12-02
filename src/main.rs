// RP2040 Board Configuration CLI (ESP-IDF style) - Rust
// File: rp2040_cli/src/main.rs
// --------------------------------------------------------------------------------

use std::io::{Read, Write};
use std::path::PathBuf;
use std::time::Duration;

use clap::{Parser, Subcommand};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "rbcli", about = "RP2040 Board CLI (ESP-IDF-like)")]
struct Cli {
    /// Serial port device (overrides config)
    #[arg(short, long)]
    port: Option<String>,

    /// Baud rate (overrides config)
    #[arg(short, long)]
    baud: Option<u32>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Open a serial monitor
    Monitor {},
    /// Query board status
    Status {},
    /// Set or get configuration (persistent local config)
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Trigger board to read firmware from SD and flash it (board-side implementation required)
    FlashFromSd { file: Option<String> },
    /// Send a LoRa message through the board
    LoraSend { payload: String },
    /// Read a sensor value by name
    SensorRead { name: String },
    /// Reset the board
    Reset {},
    /// Send a raw command to the board and print response
    Raw { cmd: String },
    /// Generate default config file
    GenConfig {},
}

// FIX: Added Clone here so Clap can process it
#[derive(Subcommand, Clone, Debug)]
enum ConfigAction {
    /// Set a configuration key
    Set { key: String, value: String },
    /// Get a configuration key
    Get { key: String },
    /// Show all configuration
    Show {},
}

#[derive(Serialize, Deserialize, Debug)]
struct RbConfig {
    serial_port: Option<String>,
    baud: Option<u32>,
    default_spi: Option<String>,
    lora_frequency: Option<u32>,
}

impl Default for RbConfig {
    fn default() -> Self {
        Self {
            serial_port: None,
            baud: Some(115200),
            default_spi: Some("spi0".into()),
            lora_frequency: Some(915_000_000u32),
        }
    }
}

fn config_path() -> Option<PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "yourorg", "rbcli") {
        let cfg_dir = proj_dirs.config_dir();
        std::fs::create_dir_all(cfg_dir).ok()?;
        let cfg_file = cfg_dir.join("config.toml");
        Some(cfg_file)
    } else {
        None
    }
}

fn read_config() -> RbConfig {
    if let Some(path) = config_path() {
        if path.exists() {
            if let Ok(s) = std::fs::read_to_string(&path) {
                if let Ok(cfg) = toml::from_str::<RbConfig>(&s) {
                    return cfg;
                }
            }
        }
    }
    RbConfig::default()
}

fn write_config(cfg: &RbConfig) {
    if let Some(path) = config_path() {
        if let Ok(s) = toml::to_string_pretty(cfg) {
            let _ = std::fs::write(path, s);
        }
    }
}

fn open_serial(port_name: &str, baud: u32) -> Result<Box<dyn serialport::SerialPort>, String> {
    serialport::new(port_name, baud)
        .timeout(Duration::from_millis(2000))
        .open()
        .map_err(|e| format!("Failed to open serial: {}", e))
}

fn send_cmd_and_wait(port: &mut dyn serialport::SerialPort, cmd: &str) -> Result<String, String> {
    let mut tx = cmd.as_bytes().to_vec();
    tx.push(b'\n');
    port.write_all(&tx).map_err(|e| format!("Write failed: {}", e))?;
    std::thread::sleep(Duration::from_millis(150));
    let mut buf = vec![0u8; 4096];
    let n = port.read(&mut buf).unwrap_or(0);
    let resp = String::from_utf8_lossy(&buf[..n]).to_string();
    Ok(resp)
}

fn cmd_monitor(port_name: &str, baud: u32) -> Result<(), String> {
    let mut port = open_serial(port_name, baud)?;
    println!("--- Connected to {} @ {} ---", port_name, baud);
    println!("Press Ctrl-C to exit. Type local commands starting with ':' (like :reset)");

    // Simple monitor: spawn a thread to read from port and print
    let mut rport = port.try_clone().map_err(|e| format!("Clone failed: {}", e))?;
    std::thread::spawn(move || loop {
        let mut buf = [0u8; 256];
        match rport.read(&mut buf) {
            Ok(n) if n > 0 => {
                let s = String::from_utf8_lossy(&buf[..n]);
                print!("{}", s);
            }
            _ => std::thread::sleep(Duration::from_millis(50)),
        }
    });

    // Read user input and send to board
    // FIX: Removed unused 'stdout'
    use std::io::stdin; 
    loop {
        let mut input = String::new();
        stdin().read_line(&mut input).map_err(|e| format!("stdin failed: {}", e))?;
        if input.trim().is_empty() { continue; }
        if input.starts_with(":") {
            match input.trim() {
                ":reset" => {
                    // FIX: Dereference the box with *port
                    let _ = send_cmd_and_wait(&mut *port, "CMD RESET");
                }
                ":status" => { 
                    // FIX: Dereference the box with *port
                    let _ = send_cmd_and_wait(&mut *port, "CMD STATUS"); 
                }
                _ => println!("Unknown local command"),
            }
        } else {
            port.write_all(input.as_bytes()).map_err(|e| format!("Write failed: {}", e))?;
        }
    }
}

fn main() {
    let cli = Cli::parse();
    let mut cfg = read_config();

    // override if provided
    if let Some(p) = cli.port.clone() { cfg.serial_port = Some(p); }
    if let Some(b) = cli.baud { cfg.baud = Some(b); }
    let port_name = cfg.serial_port.clone().unwrap_or_else(|| {
        // default guess
        if cfg!(target_os = "windows") { "COM3".into() } else { "/dev/ttyACM0".into() }
    });
    let baud = cfg.baud.unwrap_or(115200);

    match cli.command {
        Commands::Monitor {} => {
            if let Err(e) = cmd_monitor(&port_name, baud) { eprintln!("{}", e); }
        }
        Commands::Status {} => {
            match open_serial(&port_name, baud) {
                Ok(mut p) => match send_cmd_and_wait(&mut *p, "CMD STATUS") {
                    Ok(r) => println!("Board: {}", r),
                    Err(e) => eprintln!("{}", e),
                },
                Err(e) => eprintln!("{}", e),
            }
        }
        Commands::Config { action } => {
            match action {
                ConfigAction::Set { key, value } => {
                    match key.as_str() {
                        "port" => cfg.serial_port = Some(value),
                        "baud" => cfg.baud = value.parse().ok(),
                        _ => println!("Unknown key"),
                    }
                    write_config(&cfg);
                }
                ConfigAction::Get { key } => {
                    match key.as_str() {
                        "port" => println!("port = {:?}", cfg.serial_port),
                        "baud" => println!("baud = {:?}", cfg.baud),
                        _ => println!("Unknown key"),
                    }
                }
                ConfigAction::Show {} => println!("Config: {:?}", cfg),
            }
        }
        Commands::FlashFromSd { file } => {
            // Request board to flash firmware from SD. Optionally pass file name.
            match open_serial(&port_name, baud) {
                Ok(mut p) => {
                    let cmd = if let Some(f) = file { format!("CMD FLASH SD {}", f) } else { "CMD FLASH SD".into() };
                    match send_cmd_and_wait(&mut *p, &cmd) { Ok(r) => println!("{}", r), Err(e) => eprintln!("{}", e) }
                }
                Err(e) => eprintln!("{}", e),
            }
        }
        Commands::LoraSend { payload } => {
            match open_serial(&port_name, baud) { Ok(mut p) => { let cmd = format!("CMD LORA SEND {}", payload); let _ = send_cmd_and_wait(&mut *p, &cmd); }, Err(e) => eprintln!("{}", e) }
        }
        Commands::SensorRead { name } => {
            match open_serial(&port_name, baud) { Ok(mut p) => { let cmd = format!("CMD SENSOR READ {}", name); match send_cmd_and_wait(&mut *p, &cmd) { Ok(r) => println!("{}", r), Err(e) => eprintln!("{}", e) } }, Err(e) => eprintln!("{}", e) }
        }
        Commands::Reset {} => { match open_serial(&port_name, baud) { Ok(mut p) => { let _ = send_cmd_and_wait(&mut *p, "CMD RESET"); }, Err(e) => eprintln!("{}", e) } }
        Commands::Raw { cmd } => { match open_serial(&port_name, baud) { Ok(mut p) => { match send_cmd_and_wait(&mut *p, &cmd) { Ok(r) => println!("{}", r), Err(e) => eprintln!("{}", e) } }, Err(e) => eprintln!("{}", e) } }
        Commands::GenConfig {} => { let default = RbConfig::default(); write_config(&default); println!("Generated config at {:?}", config_path()); }
    }
}