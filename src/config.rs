use std::{fmt, net::IpAddr};

pub type BoardId = &'static str;
pub enum Telemetry {}

// TODO definetly a better way to do this
pub fn ip_to_id(ip: IpAddr) -> Result<BoardId, ConversionError> {
    let ip = match ip {
        IpAddr::V4(a) => a.octets(),
        IpAddr::V6(_) => return Err(ConversionError::Ipv6Found)
    };

    match ip {
        [1, 2, 3, 4] => Ok("flight-01"),
        _ => Err(ConversionError::BoardIdNotFound { ip }),
    }
}



#[derive(Debug, Clone)]
pub enum ConversionError {
    Ipv6Found,
    BoardIdNotFound {ip: [u8; 4]},
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::Ipv6Found => write!(f, "Passed SocketAddr was IPv6, expected IPv4."),
            ConversionError::BoardIdNotFound { ip } => write!(f, "Couldn't find a board ID that maps to IP {}.{}.{}.{}.", ip[0], ip[1], ip[2], ip[3]),
        }
    }
}