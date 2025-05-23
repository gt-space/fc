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
}

impl fmt::Display for ServoError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::ServoDisconnected => write!(f, "Servo can't be reached or has disconnected."),
      Self::DeserializationFailed(e) => write!(f, "postcard encountered an error during message deserialization: {e}"),
      Self::TransportFailed(e) => write!(f, "The Servo transport layer raised an error: {e}"),
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
pub(crate) fn pull(servo_stream: &mut TcpStream) -> Result<Option<FlightControlMessage>> {
  let mut buffer = vec![0; 3_000];

  match servo_stream.read(&mut buffer) {
    Ok(s) if s == 0 => return Err(ServoError::ServoDisconnected),
    Ok(s) => s,
    Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(None),
    Err(e) => return Err(ServoError::TransportFailed(e))
  };

  match postcard::from_bytes::<FlightControlMessage>(&buffer) {
    Ok(m) => Ok(Some(m)),
    Err(e) => Err(ServoError::DeserializationFailed(e)),
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