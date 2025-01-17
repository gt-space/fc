use std::net::{TcpStream, ToSocketAddrs};
use common::{comm::VehicleState, sequence::Duration};
use crate::SERVO_ADDRESS;

/// Establishes a TCP stream with Servo.
// TODO: Implement a stop such that after a certain number of tries it gives up and aborts
pub(super) fn establish() -> TcpStream {
  println!("Attempting connection with servo at {}:{}...", SERVO_ADDRESS.0, SERVO_ADDRESS.1);
  loop {
    match TcpStream::connect(SERVO_ADDRESS) {
        Ok(s) => return s,
        Err(e) => eprintln!("Connection couldn't be established due to: {e}. Retrying..."),
    };
  };
}

/// Gets new config/mapping information from Servo
pub(super) fn pull(stream: &mut TcpStream) {
  todo!()
}

/// Sends new VehicleState to servo
pub(super) fn push(stream: &mut TcpStream, state: VehicleState) {
  todo!()
}