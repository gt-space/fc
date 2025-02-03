use std::{io, net::{TcpListener, TcpStream, UdpSocket}, time::Duration, collections::HashSet};
use crate::config::{ip_to_id, BoardId};
use common::comm::{flight::{DataMessage}, sam::SamControlMessage};
use std::io::Read;



//No longer needed for Udp?
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
pub(crate) fn pull<'b>(socket: &UdpSocket) -> Vec<DataMessage<'b>> {

    let mut collected_data = Vec::new();
    const BUFFER_SIZE: usize = 1024;
    let mut buffer = [0u8; BUFFER_SIZE];
    socket.set_nonblocking(true).expect("Could not set non-blocking"); 
    socket.set_read_timeout(Some(Duration::from_millis(100))).unwrap();
    
    //this should be part of shared maybe or some sort of global property?
    //let mut boards: HashSet<BoardId> = HashSet::new();

    loop{
        match socket.recv_from(&mut buffer) {
            Ok((n, senderAddr)) => {
                if(n==0) {
                    continue; 
                    //no data
                }
              let id_result = ip_to_id(senderAddr.ip());  
              match id_result {
                Ok(id) => {
                    let incoming_data = postcard::from_bytes(&buffer[..n]);
                    let incoming_data = match incoming_data {
                      Ok(data) => data,
                      Err(error) => {
                        eprintln!("Failed to interpret buffer data: {error}");
                        continue;
                      }
                    };
                    let message = match incoming_data {
                        DataMessage::Identity(board_id) => {
                            collected_data.push(DataMessage::Identity(id.to_string()));
                            //needs to send an identity message back to sender 
                        }
                        DataMessage::Sam(board_id, datapoints) => {
                            collected_data.push(DataMessage::Sam((id.to_string()), (datapoints)));
                        }

                        DataMessage::Bms(board_id, datapoints) => {
                            collected_data.push(DataMessage::Bms((id.to_string()), (datapoints)));
                        }
                        DataMessage::Ahrs(board_id, datapoints) => {
                            collected_data.push(DataMessage::Ahrs((id.to_string()), (datapoints)));
                        }
                        DataMessage::FlightHeartbeat => {
                            //not a message that we will receive
                        }
                    }; 
                }
                Err(err) => {
                    eprintln!("Could not convert ip to id");
                }
              }
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No data 
                //Maybe we Check heartbeats
                break;
            }
            Err(e) => {
                eprintln!("Error with parsing UDP data: {} " , e );
            }
        }
    }
    collected_data
}

// create a function or series of functions that takes a command and sends it to a board