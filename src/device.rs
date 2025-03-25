use core::fmt;
use std::{io, net::{IpAddr, SocketAddr, UdpSocket}, time::Instant};
use common::comm::{ahrs, bms, flight::{DataMessage, SequenceDomainCommand}, sam::SamControlMessage, CompositeValveState, NodeMapping, ValveState, VehicleState};

use crate::{Ingestible, DEVICE_COMMAND_PORT, TIME_TO_LIVE};

pub(crate) type Mappings = Vec<NodeMapping>;

#[derive(Clone)]
pub(crate) struct Device {
    id: String,
    address: SocketAddr,
    last_recieved: Instant,
}

impl Device {
    fn new(id: String, address: SocketAddr) -> Self {
        Device { id, address, last_recieved: Instant::now() }
    }

    /// Should be ran whenever data is received from a board to update.
    pub(crate) fn reset_timer(&mut self) {
        if self.is_disconnected() {
            println!("{} at {} reconnected!", self.address.ip(), self.id);
        }

        self.last_recieved = Instant::now();
    }

    pub(crate) fn send_heartbeat(&self, socket: &UdpSocket) -> Result<()> {
        let mut buf: [u8; 1024] = [0; 1024];
        let serialized = postcard::to_slice(&DataMessage::FlightHeartbeat, &mut buf)
            .map_err(|e| Error::SerializationFailed(e))?;
        socket.send_to(serialized, self.address).map_err(|e| Error::TransportFailed(e))?;
        Ok(())
    }

    pub(crate) fn is_disconnected(&self) -> bool {
        Instant::now().duration_since(self.last_recieved) > TIME_TO_LIVE
    }

    /// Sends data to the device via a given socket.
    pub(crate) fn send(&self, socket: &UdpSocket, buf: &[u8]) -> Result<()> {
        socket.send_to(buf, (self.address.ip(), DEVICE_COMMAND_PORT)).map_err(|e| Error::TransportFailed(e))?;
        Ok(())
    }

    pub(crate) fn get_board_id(&self) -> &String {
        &self.id
    }

    pub(crate) fn get_ip(&self) -> IpAddr {
        self.address.ip()
    }
}

pub(crate) struct Devices {
    devices: Vec<Device>,
    state: VehicleState,
}

impl Devices {
    /// Creates an empty set to hold Devices
    pub(crate) fn new() -> Self {
        Devices { devices: Vec::new(), state: VehicleState::new() }
    }

    /// Inserts a device into the set, overwriting an existing device.
    /// Overwriting a device replaces all of its associated data, as if it were
    /// connecting for the first time. Returns a reference to the newly inserted
    /// device and the overwritten device, if it existed.
    pub(crate) fn register_device(&mut self, id: &String, address: SocketAddr) -> Option<Device> {
        let device = Device::new(id.clone(), address);

        if let Some(copy) = self.devices.iter_mut().find(|d| d.id == device.id) {
            let old = copy.clone();
            *copy = device;
            return Some(old);
        } else {
            self.devices.push(device);
            return None;
        }
    }

    pub(crate) fn update_state(&mut self, telemetry: Vec<(SocketAddr, DataMessage)>, mappings: &Mappings, socket: &UdpSocket) {
        for (address, message) in telemetry {
            println!("Received telemetry: {:?}", message);

            match message {
                DataMessage::FlightHeartbeat => continue,
                DataMessage::Ahrs(ref id, _) |
                DataMessage::Bms(ref id, _) |
                DataMessage::Sam(ref id, _) => {
                    let Some(device) = self.devices.iter_mut().find(|d| d.id == *id) else {
                        println!("Received data from a device that hasn't been registered. Ignoring...");
                        continue;
                    };

                    device.reset_timer();
                },
                DataMessage::Identity(ref id) => {
                    if let Err(e) = handshake(&address, socket) {
                        println!("Connection with {id} couldn't be established: {e}");
                    } else {
                        println!("Connection established with {id}.");
                        if let Some(old_device) = self.register_device(id, address) {
                            println!("Overwrote data of previously registered {id} at {}", old_device.address.ip());
                        }
                    }

                    continue;
                }
            }

            message.ingest(&mut self.state, mappings);
        }
    }

    /// Sends a message on a socket to a board with id `destination`
    fn serialize_and_send<T: serde::ser::Serialize>(&self, socket: &UdpSocket, destination: &str, message: &T) -> std::result::Result<(), String> {
        let mut buf: [u8; 1024] = [0; 1024];

        let Some(device) = self.devices.iter().find(|d| d.id == *destination) else {
            return Err("Tried to sent a message to a board that hasn't been connected yet.".to_string());
        };

        if let Err(e) = postcard::to_slice::<T>(message, &mut buf) {
            return Err(format!("Couldn't serialize message: {e}"));
        };

        if let Err(e) = device.send(socket, &buf) {
            return Err(format!("Couldn't send message to {destination}: {e}"));
        };

        return Ok(())
    }

    ///
    pub(crate) fn send_sam_commands(&mut self, socket: &UdpSocket, mappings: &Mappings, commands: Vec<SequenceDomainCommand>) -> bool {
        let mut should_abort = false;
        
        for command in commands {
            match command {
                SequenceDomainCommand::ActuateValve { valve, state } => {
                    let Some(mapping) = mappings.iter().find(|m| m.text_id == valve) else {
                        eprintln!("Failed to actuate valve: mapping '{valve}' is not defined.");
                        continue;
                    };
    
                    let closed = state == ValveState::Closed;
                    let normally_closed = mapping.normally_closed.unwrap_or(true);
                    let powered = closed != normally_closed;

                    if let Some(existing) = self.state.valve_states.get_mut(&valve) {
                        existing.commanded = state;
                    } else {
                        self.state.valve_states.insert(
                            valve,
                            CompositeValveState {
                                commanded: state,
                                actual: ValveState::Undetermined
                            }
                        );
                    }

                    let command = SamControlMessage::ActuateValve { channel: mapping.channel, powered };

                    if let Err(msg) = self.serialize_and_send(socket, &mapping.board_id, &command) {
                        println!("{}", msg);
                    }
                }
                SequenceDomainCommand::Abort => should_abort = true,
            }
        }

        should_abort
    }

    pub(crate) fn send_bms_command(&self, socket: &UdpSocket, command: bms::Command) {
        let Some(bms) = self.devices.iter().find(|d| d.id.starts_with("bms")) else {
            println!("Couldn't send a BMS command as BMS isn't connected.");
            return;
        };

        if let Err(msg) = self.serialize_and_send(socket, &bms.id, &command) {
            println!("{}", msg);
        }
    }

    pub(crate) fn send_ahrs_command(&self, socket: &UdpSocket, command: ahrs::Command) {
        let Some(ahrs) = self.devices.iter().find(|d| d.id.starts_with("ahrs")) else {
            println!("Couldn't send an AHRS command as AHRS isn't connected.");
            return;
        };

        if let Err(msg) = self.serialize_and_send(socket, &ahrs.id, &command) {
            println!("{}", msg);
        }
    }

    pub(crate) fn get_state(&self) -> &VehicleState {
        return &self.state;
    }
    
    pub(crate) fn iter_mut(&mut self) -> ::core::slice::IterMut<'_, Device> {
        self.devices.iter_mut()
    }
}

/// performs a flight handshake with the board.
pub(crate) fn handshake(address: &SocketAddr, socket: &UdpSocket) -> Result<()> {
    let mut buf: [u8; 1024] = [0; 1024];
    let serialized = postcard::to_slice(&DataMessage::Identity("flight-01".to_string()), &mut buf)
        .map_err(|e| Error::SerializationFailed(e))?;
    socket.send_to(serialized, address).map_err(|e| Error::TransportFailed(e))?;
    Ok(())
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

        let serialized_message = match postcard::from_bytes::<DataMessage>(&buf[..size]) {
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
    TransportFailed(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SerializationFailed(e) => write!(f, "Couldn't serialize an outgoing message: {e}"),
            Self::TransportFailed(e) => write!(f, "Couldn't send data to a device: {e}"),
        }
    }
}