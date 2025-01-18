mod config;
mod device;
mod servo;
mod state;

use std::{collections::HashMap, net::{TcpListener, TcpStream}, time::Duration};

use common::comm::{flight::BoardId, NodeMapping, VehicleState};

// TODO: Make VehicleState belong to flight instead of common.

/// The address that boards can connect to
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_ADDRESS: (&str, u16) = ("192.168.1.10", 5025);
const SAM_PORT: u16 = 8378;

fn main() {
    println!("Flight Computer running at version {}\n", env!("CARGO_PKG_VERSION"));
    let listener = TcpListener::bind(LISTENER_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
    listener.set_nonblocking(true).expect("Cannot set listener to non-blocking.");
    let mut devices: HashMap<BoardId, TcpStream> = HashMap::new();
    let sam_mappings: Vec<NodeMapping> = Vec::new();
    let vehicle_state = VehicleState::new();

    let mut servo_stream = servo::establish(SERVO_ADDRESS, 3, Duration::from_secs(2)).expect("Could't set up initial servo connection");
    loop {
        // SERVO SECTION (logic that interacts with servo)
        if let Some(message) = servo::pull(&mut servo_stream) {
            servo::decode(message);
        }

        servo::push(&mut servo_stream, &vehicle_state);

        // boards (logic that interacts with SAM, AHRS, BMS, etc.)

        // sequences and triggers (logic )

        // triggers

        // ...
    }
}
