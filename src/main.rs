mod config;
mod device;
mod servo;
mod state;
mod sequence;

use std::{collections::HashMap, net::{TcpListener, TcpStream, UdpSocket}, os::unix::net::UnixDatagram, process::Child, thread, time::Duration};
use common::{sequence::{MMAP_PATH, SOCKET_PATH}, comm::{flight::BoardId, FlightControlMessage, NodeMapping, VehicleState}};
use mmap_sync::synchronizer::Synchronizer;
use servo::ServoError;

/// The address that boards can connect to
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_ADDRESS: (&str, u16) = ("192.168.1.10", 5025);
const FC_SOCKET_ADDRESS: (&str, u16) = ("0.0.0.0", 0);
const SAM_PORT: u16 = 8378;
const MMAP_GRACE_PERIOD: Duration = Duration::from_millis(20);

fn main() -> ! {
    println!("Flight Computer running at version {}\n", env!("CARGO_PKG_VERSION"));
    let listener = TcpListener::bind(LISTENER_ADDRESS).expect(&format!("Couldn't open port {:04} on IP address {}", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
    listener.set_nonblocking(true).expect("Cannot set listener to non-blocking.");
    let devices: HashMap<BoardId, TcpStream> = HashMap::new();
    let mut sam_mappings: Vec<NodeMapping> = Vec::new();
    let vehicle_state = VehicleState::new();
    let mut sequences: HashMap<String, Child> = HashMap::new();
    let commands: UnixDatagram = UnixDatagram::bind(SOCKET_PATH).expect(&format!("Could not open sequence command socket on path '{SOCKET_PATH}'."));
    commands.set_nonblocking(true).expect("Cannot set sequence command socket to non-blocking.");
    let synchronizer: Synchronizer = Synchronizer::new(MMAP_PATH.as_ref());
    let servo_socket = UdpSocket::bind(FC_SOCKET_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}", FC_SOCKET_ADDRESS.1, FC_SOCKET_ADDRESS.0));
    
    let (mut servo_stream, mut servo_address)= loop {
        match servo::establish(SERVO_ADDRESS, 3, Duration::from_secs(2)) {
            Ok(s) => {
                println!("Connected to servo successfully. Beginning control cycle...\n");
                break s;
            },
            Err(e) => {
                println!("Couldn't connect due to error: {e}\n");
                thread::sleep(Duration::from_secs(2));
            },
        }    
    };
    
        
    loop {
        // servo (logic that interacts with servo)
        let servo_message = servo::pull(&mut servo_stream)
            .unwrap_or_else(|e| {
                eprintln!("Issue in pulling data from Servo: {e}");

                match e {
                    ServoError::ServoDisconnected => {
                        if let Ok(s) = servo::establish(SERVO_ADDRESS, 1, Duration::from_millis(100)) {
                            (servo_stream, servo_address) = s;
                            eprintln!("Connection successfully re-established.");
                        } else {
                            eprintln!("Connection could not be re-established. Continuing...")
                        }
                    },
                    ServoError::DeserializationFailed(_) => todo!(),
                    ServoError::TransportFailed(_) => todo!(),
                };

                None
            }
        );

        // decoding servo message
        if let Some(command) = servo_message {
            println!("Recieved a FlightControlMessage: {command:#?}");

            match command {
                FlightControlMessage::Abort => todo!(),
                FlightControlMessage::AhrsCommand(_) => todo!(),
                FlightControlMessage::BmsCommand(_) => todo!(),
                FlightControlMessage::Trigger(_) => todo!(),
                FlightControlMessage::Mappings(m) => sam_mappings = m,
                FlightControlMessage::Sequence(s) => sequence::execute(&sam_mappings, s, &mut sequences),
                FlightControlMessage::StopSequence(n) => {
                    if let Err(e) = sequence::kill(&mut sequences, &n) {
                        eprintln!("There was an issue in stopping sequence '{n}': {e}");
                    }
                },
            }
        }

        if let Err(e) = servo::push(&servo_socket, servo_address, &vehicle_state) {
            eprintln!("Issue in sending servo the vehicle telemetry: {e}");
        }
        
        // boards (logic that interacts with SAM, AHRS, BMS, etc.)

        // sequences and triggers

        // triggers

        // ...
    }
}
