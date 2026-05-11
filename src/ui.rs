use colored::*;
use std::io::{self, Write};

pub fn print_logo() {
    println!("{}", r#"
 ███    ███ ███████ ███████ ██   ██  ██████  ██████  ███    ███ 
 ████  ████ ██      ██      ██   ██ ██      ██    ██ ████  ████ 
 ██ ████ ██ █████   ███████ ███████ ██      ██    ██ ██ ████ ██ 
 ██  ██  ██ ██           ██ ██   ██ ██      ██    ██ ██  ██  ██ 
 ██      ██ ███████ ███████ ██   ██  ██████  ██████  ██      ██ 
    "#.white().bold());
    println!("{}", "--------------------------------------------------------------------------------".white());
}

pub fn print_header(title: &str, subtitle: &str) {
    let title_line = format!("║ {:^76} ║", title);
    let sub_line = format!("║ {:^76} ║", subtitle);
    
    println!("{}", "╔══════════════════════════════════════════════════════════════════════════════╗".white());
    println!("{}", title_line.white().bold());
    println!("{}", sub_line.white());
    println!("{}", "╚══════════════════════════════════════════════════════════════════════════════╝".white());
}

pub fn print_status(label: &str, status: &str) {
    println!("  {} {}", label.white().bold(), status.white());
}

pub fn print_received(from: u32, message: &str) {
    let time = chrono::Local::now().format("%H:%M:%S");
    println!("\r[{}] {} Node {}: {}", time.to_string().white(), "●".white(), from, message.white().bold());
}

pub fn set_input_prompt(label: &str) {
    print!("\n{} {} ", "❯".white().bold(), label.white());
    io::stdout().flush().unwrap();
}

pub fn clear_screen() {
    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
}
