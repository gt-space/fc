mod config;
mod device;
mod servo;
mod state;
mod sequence;

// TODO: Make VehicleState belong to flight instead of common.
use std::{collections::HashMap, net::UdpSocket, os::unix::net::UnixDatagram, process::Child, thread, time::Duration};
use common::{comm::{flight::DataMessage, FlightControlMessage, NodeMapping, VehicleState}, sequence::{MMAP_PATH, SOCKET_PATH}};
use state::Ingestible;
use crate::{config::ip_to_id, device::Devices, servo::ServoError};
use mmap_sync::synchronizer::Synchronizer;

/// The address that boards can connect to
const LISTENER_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_ADDRESS: (&str, u16) = ("192.168.1.10", 5025);
const FC_SOCKET_ADDRESS: (&str, u16) = ("0.0.0.0", 0);
const SAM_PORT: u16 = 8378;
const MMAP_GRACE_PERIOD: Duration = Duration::from_millis(20);
const TIME_TO_LIVE: Duration = Duration::from_millis(50);

fn main() -> ! {
  println!("Flight Computer running on version {}\n", env!("CARGO_PKG_VERSION"));
  let socket: UdpSocket = UdpSocket::bind(FC_SOCKET_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}", LISTENER_ADDRESS.1, LISTENER_ADDRESS.0));
  socket.set_nonblocking(true).expect("Cannot set incoming to non-blocking.");
  let mut sam_mappings: Vec<NodeMapping> = Vec::new();
  let mut vehicle_state = VehicleState::new();
  let mut devices: Devices = Devices::new();
  let mut sequences: HashMap<String, Child> = HashMap::new();
  let command_socket: UnixDatagram = UnixDatagram::bind(SOCKET_PATH).expect(&format!("Could not open sequence command socket on path '{SOCKET_PATH}'."));
  command_socket.set_nonblocking(true).expect("Cannot set sequence command socket to non-blocking.");
  let synchronizer: Synchronizer = Synchronizer::new(MMAP_PATH.as_ref());
  
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
    let servo_message = servo::pull(&mut servo_stream).unwrap_or_else(|e| {
      eprintln!("Issue in pulling data from Servo: {e}");

      match e {
        ServoError::ServoDisconnected => {
          eprint!("Attempting to reconnect to servo... ");

          match servo::establish(SERVO_ADDRESS, 1, Duration::from_millis(100)) {
            Ok(s) => {
              (servo_stream, servo_address) = s;
              eprintln!("Connection successfully re-established.");
            },
            Err(e) => {
              eprintln!("Connection could not be re-established: {e}. Continuing...")
            },
          };
        },
        ServoError::DeserializationFailed(_) => {},
        ServoError::TransportFailed(_) => {},
      };

      None
    });

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

    if let Err(e) = servo::push(&socket, servo_address, &vehicle_state) {
      eprintln!("Issue in sending servo the vehicle telemetry: {e}");
    }
    
    let messages = device::receive(&socket);
    
    // deals with the data processing
    for (_, message) in &messages {
      message.ingest(&mut vehicle_state, &sam_mappings);
    }
    
    // deals with record keeping
    for (address, message) in messages {
      if let DataMessage::Identity(id) = message {
        devices.add_or_overwrite(id, address);

        let Some(device) = devices.find_by_address(&address) else {
          println!("The device at IP {} couldn't be registered.", address.ip());
          continue;
        };

        if let Err(e) = device.handshake(&socket) {
          println!("There was an error in registering the device at IP {address} as {}: {e}", device.get_board_id());
          continue;
        }

        println!("Registered the device at {} as {}", address.ip(), device.get_board_id());
        continue;
      }

      if let Some(device) = devices.find_by_address(&address) {
        device.data_received();
        continue;
      } 

      println!("Received a message from a device that was never connected to the FC before. Attempting to resolve automatically...");
      if let Ok(id) = ip_to_id(address.ip()) {
        if id == "flight-01" {
          println!("Message may have been sent from the FC. This shouldn't happen, ignoring...");
          continue;
        } else if id == "servo-01" {
          println!("Message may have been sent from Servo. This shouldn't happen, ignoring...");
          continue;
        }

        println!("Resolved the message from a board at IP {} to be {id}.", address.ip());
        if devices.has_id(id) {
          println!("However, this board is already registered! Ignoring...");
        } else {
          devices.add_or_overwrite(id.to_string(), address);
        }
      }
    }

    // Update board lifetimes and send heartbeats to connected boards.
    for device in devices.iter_mut() {
      if device.is_expired() {
        device.set_disconnected();
      }

      if !device.is_disconnected() {
        if let Err(e) = device.send_heartbeat(&socket) {
          println!(
            "There was an error in notifying board {} at IP {} that the FC is still connected: {e}", 
            device.get_board_id(),
            device.get_ip()
          );
        }
      }
    }

    // sequences and triggers

    // triggers

    // ...
  }
}
