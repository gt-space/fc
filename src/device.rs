use std::{collections::HashMap, io, net::{IpAddr, TcpListener, TcpStream, UdpSocket}, time::{Duration, Instant}};
use crate::config::{ip_to_id, BoardId};
use common::comm::flight::DataMessage;
use bimap::BiHashMap;

pub struct BoardState {
    last_message: Instant, 
    is_dead: bool,
    //Other Meta Data we need to keep track of 
  }


//No longer needed for Udp?
pub(crate) fn listen(listener: &TcpListener) -> Vec<(BoardId, TcpStream)> {
    let mut connections = Vec::new();
    
    loop {
        match listener.accept() {
            Ok((stream, address)) => {
                let id = match ip_to_id(address.ip()) {
                    Ok(a) => a,
                    Err(e) => {
                        eprintln!("{}", e);
                        continue;
                    }
                };

                connections.push((id, stream));
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
pub(crate) fn pull<'b>(socket: &UdpSocket, ip_mappings: BiHashMap<BoardId, IpAddr>, board_states: HashMap<BoardId, BoardState>) -> Vec<DataMessage<'b>> {

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
                    //add to time tracker
                    board_states.insert(id, BoardState {
                        last_message: Instant::now(),
                        is_dead: false,
                    });
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
                            if(board_id != id.to_string()) {
                                //handle this edge case
                            }
                            //add to mapping
                            ip_mappings.insert(board_id, senderAddr.ip());

                            //Send Handshake Back
                            const FC_BOARD_ID: &str = "flight-01";
                            let identity = DataMessage::Identity(String::from(FC_BOARD_ID));

                            let handshake = match postcard::to_slice(&identity, &mut buffer) {
                              Ok(identity) => identity,
                              Err(error) => {
                                eprintln!("Failed to deserialize identity message: {error}");
                                continue;
                              }
                            };
                            match socket.send_to(handshake, senderAddr) {
                                Ok(_) => {
                                    println!("Sent identity handshake to {senderAddr}.");
                                }
                                Err(e) => {
                                    eprintln!("Failed to send identity handshake to {senderAddr}: {e}");
                                }
                            }
                            collected_data.push(DataMessage::Identity(id.to_string()));
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

//checks if it has been too lond since we received a message from a given board
pub(crate) fn checkBoards(board_states: &mut HashMap<BoardId, BoardState>) {
    const TIME_LIMIT: Duration = Duration::from_millis(100);
    let now = Instant::now();
    for (board_id, state) in board_states.iter_mut() {
        if (now.duration_since(state.last_message) > TIME_LIMIT) {
            state.is_dead = true;
            handleDeadBoard(board_id);
        }
    }
}

pub fn handleDeadBoard(id: BoardId) {
    //handle dead boards
}

// call this function at a given time interval
pub fn sendHeartBeat(ip_mappings: BiHashMap<BoardId, IpAddr>, board_states: HashMap<BoardId, BoardState>, socket: &UdpSocket ) {
    const HEARTBEAT_BUFFER_SIZE: usize = 1_024; 
    const SWITCHBOARD_ADDRESS: (&str, u16) = ("0.0.0.0", 4573);
    let mut buf = vec![0; HEARTBEAT_BUFFER_SIZE];
    let heartbeat = postcard::to_slice(&DataMessage::FlightHeartbeat, &mut buf);
    let heartbeat = match heartbeat {
      Ok(package) => package,
      Err(error) => {
        eprintln!("Failed to serialize serialize heartbeat: {error}");
        return;
      }
    };
    for (board_id, ip) in ip_mappings.iter_mut() {
         if let Some(state) = board_states.get(board_id) {
            if (state.is_dead == false) {
                socket.send_to(&heartbeat, SWITCHBOARD_ADDRESS);
            }
        }
    }
}
