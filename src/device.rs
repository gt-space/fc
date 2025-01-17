use std::{io::{self, Read}, net::{TcpListener, TcpStream}};
use postcard;
use serde;
use crate::config::{ip_to_id, BoardId};

pub(crate) fn listen(listener: &TcpListener) -> Vec<(BoardId, TcpStream)> {
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
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => return reaped,
            Err(e) => {
                eprintln!("Error when accepting new device connection from listener: {e:?}");
                return reaped;
            }
        };
    };
}

// protocol is to send the number of bytes to read in big endian
pub(crate) fn pull<'a, T, U>(devices: T) -> ()
where 
    T: Iterator<Item = &'a mut TcpStream>,
{
    todo!()
}
