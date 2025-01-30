mod config;
mod device;
mod servo;
mod state;
mod sequence;

use std::{os::unix::net::UnixDatagram, collections::HashMap, net::{TcpListener, TcpStream}, time::Duration, process::Output};
use common::comm::{flight::BoardId, NodeMapping, VehicleState};
use mmap_sync::synchronizer::Synchronizer;

/// The address that boards can connect to
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_ADDRESS: (&str, u16) = ("192.168.1.10", 5025);
const SAM_PORT: u16 = 8378;
const DATAGRAM_PATH: &str = "";
const MMAP_PATH: &str = "";
const MMAP_GRACE_PERIOD: Duration = Duration::from_millis(100);

fn main() -> ! {
    println!("Flight Computer running at version {}\n", env!("CARGO_PKG_VERSION"));
    let listener = TcpListener::bind(LISTENER_ADDRESS)
        .expect(&format!("Couldn't open port {} on IP address {}", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
    listener.set_nonblocking(true).expect("Cannot set listener to non-blocking.");
    let devices: HashMap<BoardId, TcpStream> = HashMap::new();
    let sam_mappings: Vec<NodeMapping> = Vec::new();
    let vehicle_state = VehicleState::new();
    let sequences: HashMap<String, Output> = HashMap::new();
    let commands: UnixDatagram = UnixDatagram::bind(DATAGRAM_PATH)
        .expect(&format!("Could not open sequence command socket on path '{DATAGRAM_PATH}'."));
    commands.set_nonblocking(true).expect("Cannot set sequence command socket to non-blocking.");
    let synchronizer: Synchronizer = Synchronizer::new(MMAP_PATH.as_ref());

    let mut servo_stream = servo::establish(SERVO_ADDRESS, 3, Duration::from_secs(2)).expect("Could't set up initial servo connection");
    loop {
        // servo (logic that interacts with servo)
        if let Some(message) = servo::pull(&mut servo_stream) {
            servo::decode(message);
        }

        servo::push(&mut servo_stream, &vehicle_state);

        // boards (logic that interacts with SAM, AHRS, BMS, etc.)

        // sequences and triggers

        // triggers

        // ...
    }
}
