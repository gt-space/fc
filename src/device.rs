use core::fmt;
use std::{io, net::{IpAddr, SocketAddr, UdpSocket}, time::Instant};
use common::comm::flight::DataMessage;

use crate::TIME_TO_LIVE;

#[derive(PartialEq, Clone)]
pub(crate) enum State {
    Connected,
    Disconnected,
}

#[derive(Clone)]
pub(crate) struct Device {
    id: String,
    address: SocketAddr,
    last_recieved: Instant,
    state: State,
}

impl Device {
    fn new(id: String, address: SocketAddr) -> Self {
        Device { id, address, last_recieved: Instant::now(), state: State::Connected }
    }

    /// Should be ran whenever data is received from a board to update
    pub(crate) fn data_received(&mut self) {
        self.last_recieved = Instant::now();

        if self.state == State::Disconnected {
            println!("{} at {} reconnected!", self.address.ip(), self.id);
            self.state = State::Connected;
        }
    }

    // performs a flight handshake with the board.
    pub(crate) fn handshake(&self, socket: &UdpSocket) -> Result<()> {
        let mut buf: [u8; 1024] = [0; 1024];
        let serialized = postcard::to_slice(&DataMessage::Identity("flight-01".to_string()), &mut buf)
            .map_err(|e| Error::SerializationFailed(e))?;
        socket.send_to(serialized, self.address).map_err(|e| Error::HandshakeTransportFailed(e))?;
        Ok(())
    }

    pub(crate) fn send_heartbeat(&self, socket: &UdpSocket) -> Result<()> {
        let mut buf: [u8; 1024] = [0; 1024];
        let serialized = postcard::to_slice(&DataMessage::FlightHeartbeat, &mut buf)
            .map_err(|e| Error::SerializationFailed(e))?;
        socket.send_to(serialized, self.address).map_err(|e| Error::HeartbeatTransportFailed(e))?;
        Ok(())
    }

    pub(crate) fn is_expired(&self) -> bool {
        Instant::now().duration_since(self.last_recieved) > TIME_TO_LIVE
    }

    pub(crate) fn set_disconnected(&mut self) {
        self.state = State::Disconnected;
    }

    pub(crate) fn is_disconnected(&self) -> bool {
        self.state == State::Disconnected
    }

    pub(crate) fn get_board_id(&self) -> &String {
        &self.id
    }

    pub(crate) fn get_ip(&self) -> IpAddr {
        self.address.ip()
    }
}

pub(crate) struct Devices {
    devices: Vec<Device>
}

impl Devices {
    /// Creates an empty set to hold Devices
    pub(crate) fn new() -> Self {
        Devices { devices: Vec::new() }
    }

    /// Inserts a device into the set, overwriting an existing device.
    /// Overwriting a device replaces all of its associated data, as if it were
    /// connecting for the first time. Returns the 
    pub(crate) fn add_or_overwrite(&mut self, id: String, address: SocketAddr) {
        let device = Device::new(id, address);

        if let Some(copy) = self.devices.iter_mut().find(|d| d.id == device.id) {
            *copy = device;
        } else {
            self.devices.push(device);
        }
    }

    pub(crate) fn find_by_address(&mut self, address: &SocketAddr) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.address == *address)
    }

    pub(crate) fn has_id(&self, id: &str) -> bool {
        self.devices.iter().find(|d| d.id == id).is_some()
    }

    pub(crate) fn iter(&self) -> ::core::slice::Iter<'_, Device> {
        self.devices.iter()
    }
    
    pub(crate) fn iter_mut(&mut self) -> ::core::slice::IterMut<'_, Device> {
        self.devices.iter_mut()
    }
}

/// Gets the most recent UDP Commands
pub(crate) fn receive(socket: &UdpSocket) -> Vec<(SocketAddr, DataMessage)> {
    let mut messages = Vec::new();
    let mut buf: [u8; 1024] = [0; 1024];
    
    loop {
        let (size, address) = match socket.recv_from(&mut buf) {
            Ok(metadata) => metadata,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => {
                eprintln!("Can't get receive incoming ethernet packets: {e:#?}");
                break;
            }
        };

        let serialized_message = match postcard::from_bytes::<DataMessage>(&buf) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Received a message from a board, but couldn't decode it, packet was of size {}: {e}", size);
                continue;
            }
        };

        messages.push((address, serialized_message));
    };

    messages
}

type Result<T> = ::std::result::Result<T, Error>;
pub(crate) enum Error {
    SerializationFailed(postcard::Error),
    HandshakeTransportFailed(io::Error),
    HeartbeatTransportFailed(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializationFailed(e) => write!(f, "Couldn't serialize an outgoing message: {e}"),
            Self::HandshakeTransportFailed(e) => write!(f, "Couldn't send the flight handshake: {e}"),
            Self::HeartbeatTransportFailed(e) => write!(f, "Couldn't notify a board that flight is still online: {e}"),
        }
    }
}