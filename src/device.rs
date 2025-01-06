use std::{io, net::{TcpListener, TcpStream}};
use crate::config::{ip_to_id, BoardId};

pub(crate) fn listen(listener: &TcpListener) -> io::Result<Vec<(BoardId, TcpStream)>> {
    let mut reaped = Vec::new();
    
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

                reaped.push((ip, stream));
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return Ok(reaped),
            // TODO: Make this return all the values that it obtained correctly?
            Err(e) => {
                eprintln!("Error when accepting new device connection from listener: {e:?}");
                return Err(e);
            }
        };
    };
}

pub(crate) fn pull() {
    todo!()
}