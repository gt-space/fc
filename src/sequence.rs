use common::comm::{sam::SamControlMessage, CompositeValveState, NodeMapping, SensorType, Sequence, ValveState, VehicleState, flight::SequenceDomainCommand};
use std::{collections::HashMap, io, os::unix::net::UnixDatagram, process::{Child, Command}};

// TODO: Refactor this code to use custom error types

fn run(mappings: &Vec<NodeMapping>, sequence: &Sequence) -> io::Result<Child> {
    let mut script = String::from("from sequences import *;");
    for mapping in mappings {
        let definition = match mapping.sensor_type {
            SensorType::Valve => format!("{0} = Valve('{0}');", mapping.text_id),
            _ => format!("{0} = Sensor('{0}');", mapping.text_id),
        };

        script.push_str(&definition);
    }
    
    script.push_str(&sequence.script);
    Command::new("python3")
        .args(["-c", &script])
        .spawn()
}

pub(crate) fn execute(mappings: &Vec<NodeMapping>, sequence: Sequence, sequences: &mut HashMap<String, Child>) {
    if let Some(running) = sequences.get_mut(&sequence.name) {
        match running.try_wait() {
            Ok(Some(_)) => {},
            Ok(None) => {
                println!("The '{}' sequence is already running. Stop it before re-attempting execution.", sequence.name);
                return;
            },
            Err(e) => {
                eprintln!("Another '{}' sequence was previously ran, but it's status couldn't be determined: {e}", sequence.name);
                return;
            },
        }
    }
    
    let process = match run(mappings, &sequence) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Error in running python3: {e}");
            return;
        }
    };

    sequences.insert(sequence.name, process);
}

pub(crate) fn kill(sequences: &mut HashMap<String, Child>, name: &String) -> io::Result<()> {
    let sequence = match sequences.get_mut(name) {
        Some(c) => {
            if let Ok(Some(_)) = c.try_wait() {
                println!("A sequence named '{name}' isn't running.");
                return Ok(());
            }

            c
        }
        None => {
            println!("A sequence named '{name}' isn't running.");
            return Ok(());
        }
    };

    sequence.kill()
}

pub(crate) fn handle_commands(socket: &UnixDatagram, mappings: &Vec<NodeMapping>, vehicle_state: &mut VehicleState) -> Vec<SamControlMessage> {
    let mut buf: [u8; 1024] = [0; 1024];
    let mut commands: Vec<SamControlMessage> = Vec::new();

    loop {
        let size = match socket.recv(&mut buf) {
            Ok(s) => s,
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) => {
                eprintln!("Error in receiving from sequence command socket: {e}");
                break;
            }
        };

        let command = match postcard::from_bytes::<SequenceDomainCommand>(&buf[..size]) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Error in deserializing SequenceDomainCommand from sequence: {e}");
                continue;
            }
        };

        match command {
            SequenceDomainCommand::ActuateValve { valve, state } => {
                let Some(mapping) = mappings.iter().find(|m| m.text_id == valve) else {
                    eprintln!("Failed to actuate valve: mapping '{valve}' is not defined.");
                    continue;
                };

                let closed = state == ValveState::Closed;
                let normally_closed = mapping.normally_closed.unwrap_or(true);
                let powered = closed != normally_closed;

                commands.push(
                    SamControlMessage::ActuateValve {
                        channel: mapping.channel,
                        powered,
                    }
                );

                if let Some(existing) = vehicle_state.valve_states.get_mut(&valve) {
                    existing.commanded = state;
                } else {
                    vehicle_state.valve_states.insert(
                        valve,
                        CompositeValveState {
                            commanded: state,
                            actual: ValveState::Undetermined
                        }
                    );
                }
            }
            SequenceDomainCommand::Abort => todo!()
        }
    }

    commands
}