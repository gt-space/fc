use std::{io, net::{TcpListener, TcpStream}};
use crate::config::{ip_to_id, BoardId};
use common::comm::{flight::DataMessage, sam::SamControlMessage};

pub(crate) fn listen(listener: &TcpListener) -> Vec<(BoardId, TcpStream)> {
    let mut connections = Vec::new();
    
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                let ip = match ip_to_id(address.ip()) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{}", e);
                        continue;
                    }
                };

                connections.push((ip, stream));
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return connections,
            Err(e) => {
                eprintln!("Error when accepting new device connection from listener: {e:#?}");
                return connections;
            }
        };
    };
}

// lifetime specifiers here are tricky. we want the data within the data message to be dropped
// once it's processed by state::ingest
pub(crate) fn pull<'a, 'b>(devices: impl Iterator<Item = &'a mut TcpStream>) -> Vec<DataMessage<'b>> {
    todo!()
}

// create a function or series of functions that takes a command and sends it to a board