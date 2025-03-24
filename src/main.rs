mod config;
mod device;
mod servo;
mod state;
mod sequence;

// TODO: Make VehicleState belong to flight instead of common?
// TODO: ACTUALLY SET MMAP_PATH AND SOCKET_PATH IN COMMON.
// TODO: Make a wrapper for printing out messages due to errors, instead of having annoying if let blocks.
use std::{collections::HashMap, net::{SocketAddr, TcpStream, UdpSocket}, os::unix::net::UnixDatagram, process::{Child, Command}, thread, time::Duration};
use common::{comm::{FlightControlMessage, NodeMapping, Sequence}, sequence::{MMAP_PATH, SOCKET_PATH}};
use state::Ingestible;
use crate::{device::Devices, servo::ServoError};
use mmap_sync::synchronizer::Synchronizer;

const INCOMING_SOCKET_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
const SERVO_SOCKET_ADDRESS: (&str, u16) = ("192.168.1.10", 5025);
const FC_SOCKET_ADDRESS: (&str, u16) = ("0.0.0.0", 0);
const DEVICE_COMMAND_PORT: u16 = 8378;

/// How quickly a sequence must read from the shared VehicleState before the
/// data becomes corrupted.
const MMAP_GRACE_PERIOD: Duration = Duration::from_millis(20);

/// How long from the last received message before a board is considered
/// disconnected.
const TIME_TO_LIVE: Duration = Duration::from_millis(50);

/// How many times a reconnect will be tried with a disconnected servo.
const SERVO_RECONNECT_RETRY_COUNT: u8 = 1;

/// The TCP timeout for re-establishing connection with a disconnected servo.
const SERVO_RECONNECT_TIMEOUT: Duration = Duration::from_millis(100);

type BoardId = String;

fn main() -> ! {
  let dependency_check = Command::new("python3")
    .arg("-c \"import sequences\"")
    .output().unwrap()
    .status
    .code().unwrap();

  match dependency_check {
    0 => println!("python3 and sequences module detected."),
    1 => panic!("sequences module not detected."),
    127 => panic!("python3 not detected."),
    _ => panic!("python3 and sequences module dependency check failed with error code {dependency_check}"),
  };

  let socket: UdpSocket = UdpSocket::bind(FC_SOCKET_ADDRESS).expect(&format!("Couldn't open port {} on IP address {}", INCOMING_SOCKET_ADDRESS.1, INCOMING_SOCKET_ADDRESS.0));
  socket.set_nonblocking(true).expect("Cannot set incoming to non-blocking.");
  let command_socket: UnixDatagram = UnixDatagram::bind(SOCKET_PATH).expect(&format!("Could not open sequence command socket on path '{SOCKET_PATH}'."));
  command_socket.set_nonblocking(true).expect("Cannot set sequence command socket to non-blocking.");

  let mut mappings: Vec<NodeMapping> = Vec::new();
  let mut devices: Devices = Devices::new();
  let mut sequences: HashMap<String, Child> = HashMap::new();
  let mut synchronizer: Synchronizer = Synchronizer::new(MMAP_PATH.as_ref());
  let mut abort_sequence: Option<Sequence> = None;
  
  println!("Flight Computer running on version {}\n", env!("CARGO_PKG_VERSION"));

  let (mut servo_stream, mut servo_address)= loop {
    match servo::establish(SERVO_SOCKET_ADDRESS, 3, Duration::from_secs(2)) {
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
    let servo_message = get_servo_data(&mut servo_stream, &mut servo_address);

    // decoding servo message, if it was received
    if let Some(command) = servo_message {
      println!("Recieved a FlightControlMessage: {command:#?}");

      match command {
        FlightControlMessage::Abort => {
          if let Some(ref sequence) = abort_sequence {
            for (_, sequence) in &mut sequences {
              if let Err(e) = sequence.kill() {
                println!("Couldn't kill a sequence in preperation for abort, continuing normally: {e}");
              }
            }

            sequence::execute(&mappings, sequence, &mut sequences);
          } else {
            println!("Received an abort command, but no abort sequence has been set. Continuing normally...");
          }
        },
        FlightControlMessage::AhrsCommand(_) => todo!(),
        FlightControlMessage::BmsCommand(_) => todo!(),
        FlightControlMessage::Trigger(_) => todo!(),
        FlightControlMessage::Mappings(m) => mappings = m,
        FlightControlMessage::Sequence(s) if s.name == "abort" => abort_sequence = Some(s),
        FlightControlMessage::Sequence(ref s) => sequence::execute(&mappings, s, &mut sequences),
        FlightControlMessage::StopSequence(n) => {
          if let Err(e) = sequence::kill(&mut sequences, &n) {
            eprintln!("There was an issue in stopping sequence '{n}': {e}");
          }
        },
      };
    }

    // send servo the current vehicle telemetry
    if let Err(e) = servo::push(&socket, servo_address, devices.get_state()) {
      eprintln!("Issue in sending servo the vehicle telemetry: {e}");
    }
    
    // receive and process telemetry from boards
    devices.update_state(device::receive(&socket), &mappings, &socket);

    // updates all running sequences with the newest received data
    if let Err(e) = state::sync_sequences(&mut synchronizer, devices.get_state()) {
      println!("There was an error in synchronizing vehicle state: {e}");
    }

    // Update board lifetimes and send heartbeats to connected boards.
    for device in devices.iter_mut() {
      if device.is_disconnected() {
        continue;
      }

      if let Err(e) = device.send_heartbeat(&socket) {
        println!(
          "There was an error in notifying board {} at IP {} that the FC is still connected: {e}", 
          device.get_board_id(),
          device.get_ip()
        );
      }
    }

    

    // sequences and triggers
    // INVESTIGATE THIS FUNCTION
    let sam_commands = sequence::pull_commands(&command_socket, &mappings, vehicle_state);
    devices.send_sam_commands(&socket, sam_commands);

    // triggers

    // ...
  }
}


/// Pulls data from Servo, if available.
/// # Error Handling
/// 
/// ## FC-Servo Connection Dropped
/// If the connection between the FC and Servo was severed, the connection
/// will tried to be re-established. If a new connection is successfully
/// established, servo_stream and servo_address will be set to mirror the
/// change. Otherwise, a notification will be printed to the terminal and None
/// will be returned.
/// 
/// ## Servo Message Deserialization Fails
/// If postcard returns an error during message deserialization, None will be
/// returned.
/// 
/// ## Transport Layer failed
/// If reading from servo_stream is not possible, None will be returned.
fn get_servo_data(servo_stream: &mut TcpStream, servo_address: &mut SocketAddr) -> Option<FlightControlMessage> {
  match servo::pull(servo_stream) {
    Ok(message) => message,
    Err(e) => {
      eprintln!("Issue in pulling data from Servo: {e}");

      match e {
        ServoError::ServoDisconnected => {
          eprint!("Attempting to reconnect to servo... ");

          match servo::establish(SERVO_SOCKET_ADDRESS, SERVO_RECONNECT_RETRY_COUNT, SERVO_RECONNECT_TIMEOUT) {
            Ok(s) => {
              (*servo_stream, *servo_address) = s;
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
    }
  }
}