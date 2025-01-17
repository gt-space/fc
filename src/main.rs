mod config;
mod device;
mod servo;
mod state;

use std::{collections::HashMap, net::{TcpListener, TcpStream}};

use config::BoardId;
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_ADDRESS: (&str, u16) = ("192.168.1.10", 4573);

fn main() {
    println!("Flight Computer running at version {}", env!("CARGO_PKG_VERSION"));
    let listener = TcpListener::bind(LISTENER_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}.", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
    listener.set_nonblocking(true).expect("Cannot set listener to non-blocking.");
    let mut devices: HashMap<BoardId, TcpStream> = HashMap::new();

    let servo_stream = servo::establish();

}
