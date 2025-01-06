mod config;
mod device;

use std::{collections::HashMap, net::TcpListener};
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);

fn main() {
    let listener = TcpListener::bind(LISTENER_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}.", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
    listener.set_nonblocking(true).expect("Cannot set listener to non-blocking.");
    let mut devices = HashMap::new();

    loop {
        if let Ok(connections) = device::listen(&listener) {
            devices.extend(connections);
        }

    }
}
