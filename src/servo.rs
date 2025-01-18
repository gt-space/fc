use std::{io, net::{TcpStream, ToSocketAddrs, SocketAddr}, time::Duration};
use common::comm::{FlightControlMessage, VehicleState};

/// Example function
// TODO: Implement a stop such that after a certain number of tries it gives up 
pub fn establish(servo_address: impl ToSocketAddrs, chances: u8, timeout: Duration) -> io::Result<TcpStream> {
  let resolved_addresses: Vec<SocketAddr> = servo_address.to_socket_addrs()?.collect();

  for _ in 0..chances {
    for addr in &resolved_addresses {
      println!("Attempting connection with servo at {addr:?}...");
  
      match TcpStream::connect_timeout(addr, timeout) {
        Ok(s) => return Ok(s),
        Err(e) => eprintln!("Connection to {addr} couldn't be established due to: {e}.\n"),
      };
    }
  }

  Err(io::Error::from(io::ErrorKind::ConnectionRefused))
}

// "pull" new information from servo
pub(super) fn pull(servo_stream: &mut TcpStream) -> Option<FlightControlMessage> {
  todo!()
}

// analyze the message and determine what to do with it (may need to pass in vehicle state and mappings, this function will likely be quite large)
pub(super) fn decode(message: FlightControlMessage) {
  todo!()
}

// sends new VehicleState to servo
pub(super) fn push(servo_stream: &mut TcpStream, state: &VehicleState) {
  todo!()
}

#[cfg(test)]
mod tests {
  use super::*;
}