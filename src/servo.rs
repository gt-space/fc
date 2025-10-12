use std::{fmt, io::{self, Read, Write}, net::{SocketAddr, TcpStream, ToSocketAddrs, UdpSocket}, time::Duration};
use common::comm::{Computer, FlightControlMessage, VehicleState};
use postcard::experimental::max_size::MaxSize;

use crate::SERVO_DATA_PORT;

type Result<T> = std::result::Result<T, ServoError>;

#[derive(Debug)]
pub(crate) enum ServoError {
  ServoDisconnected,
  TransportFailed(io::Error),
  DeserializationFailed(postcard::Error),
  ServoMessageInTransitStill,
}

impl fmt::Display for ServoError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::ServoDisconnected => write!(f, "Servo can't be reached or has disconnected."),
      Self::DeserializationFailed(e) => write!(f, "postcard encountered an error during message deserialization: {e}"),
      Self::TransportFailed(e) => write!(f, "The Servo transport layer raised an error: {e}"),
      Self::ServoMessageInTransitStill => write!(f, "The last message from servo is still in transit."),
    }
  }
}

pub(crate) fn establish(servo_addresses: &[impl ToSocketAddrs], chances: u8, timeout: Duration) -> Result<(TcpStream, SocketAddr)> {
  // buffer containing the serialized identity message to be sent to the control server
  let mut identity = [0; Computer::POSTCARD_MAX_SIZE];

  if let Err(error) = postcard::to_slice(&Computer::Flight, &mut identity) {
    eprintln!("Failed to serialize Computer: {error}");
    return Err(ServoError::DeserializationFailed(error));
  }

  let mut fatal_error = io::ErrorKind::ConnectionRefused.into();
  let resolved_addresses: Vec<SocketAddr> = servo_addresses.iter().filter_map(|a| a.to_socket_addrs().ok()).flatten().collect();
  for i in 1..=chances {
    for addr in &resolved_addresses {
      println!("[{i}]: Attempting connection with servo at {addr:?}...");
  
      match TcpStream::connect_timeout(addr, timeout) {
        Ok(mut s) => {
          s.set_nodelay(true).map_err(|e| ServoError::TransportFailed(e))?;
          s.set_nonblocking(true).map_err(|e| ServoError::TransportFailed(e))?;

          if let Err(e) = s.write_all(&identity) {
            return Err(ServoError::TransportFailed(e));
          } else {
            return Ok((s, *addr));
          }
        },
        Err(e) => fatal_error = e,
      };
    }
  };

  Err(ServoError::TransportFailed(fatal_error))
}

// "pull" new information from servo
pub(crate) fn pull(servo_stream: &mut TcpStream, previous_bytes_read: usize) -> (Result<Option<FlightControlMessage>>, usize) {
  let mut buffer = vec![0; 1_000_000];

  let mut bytes_read = 0;
  let mut still_reading = false;
  match servo_stream.peek(&mut buffer) {
    Ok(s) if s == 0 && previous_bytes_read == 0 => return (Err(ServoError::ServoDisconnected), 0),
    Ok(s) => {
      bytes_read = s; // number of bytes read. there could still be more bytes in transit that were not ready for us to read
      still_reading = s != previous_bytes_read; // if the number of bytes that we read is not the same as the previously read amount, message still coming in. else, we have read completely, can serialize
      s
    },
    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return (Ok(None), 0),
    Err(e) => return (Err(ServoError::TransportFailed(e)), 0)
  };

  // had a previously (potential) unfinished message, have confirmed it is finished
  if still_reading {
    return (Err(ServoError::ServoMessageInTransitStill), previous_bytes_read + bytes_read);
  } else {
    match postcard::from_bytes::<FlightControlMessage>(&buffer) {
      Ok(m) => (Ok(Some(m)), bytes_read),
      Err(e) => (Err(ServoError::DeserializationFailed(e)), bytes_read),
    }
  }
}

// sends new VehicleState to servo. Refactor to use UDP
pub(crate) fn push(socket: &UdpSocket, servo_socket: SocketAddr, state: &VehicleState) -> Result<usize> {
  
  let message = match postcard::to_allocvec(state) {
    Ok(v) => v,
    Err(e) => return Err(ServoError::DeserializationFailed(e)),
  };

  match socket.send_to(&message, (servo_socket.ip(), SERVO_DATA_PORT)) {
    Ok(s) => Ok(s),
    Err(e) => Err(ServoError::TransportFailed(e)),
  }
}