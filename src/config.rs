use std::{fmt, net::IpAddr};

type BoardId = &'static str;


#[derive(Debug)]
pub enum ConversionError {
    Ipv6Found,
    BoardIdNotFound {ip: [u8; 4]},
}


// TODO definetly a better way to do this
pub fn ip_to_id(ip: IpAddr) -> Result<BoardId, ConversionError> {
    let ip = match ip {
        IpAddr::V4(a) => a.octets(),
        IpAddr::V6(_) => return Err(ConversionError::Ipv6Found)
    };

    match ip {
        [192, 168, 1, 10] => Ok("servo-01"),
        [192, 168, 1, 11] => Ok("flight-01"),
        [192, 158, 1, 130] => Ok("ahrs-01"),
        [192, 168, 1, 131] => Ok("bms-01"),
        [192, 168, 1, 101] => Ok("sam-01"),
        [192, 168, 1, 102] => Ok("sam-02"),
        [192, 168, 1, 103] => Ok("sam-03"),
        [192, 168, 1, 104] => Ok("sam-04"),
        [192, 168, 1, 105] => Ok("sam-05"),
        _ => Err(ConversionError::BoardIdNotFound { ip }),
    }
}

impl fmt::Display for ConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversionError::Ipv6Found => write!(f, "Passed SocketAddr was IPv6, expected IPv4."),
            ConversionError::BoardIdNotFound { ip } => write!(f, "Couldn't find a board ID that maps to IP {}.{}.{}.{}.", ip[0], ip[1], ip[2], ip[3]),
        }
    }
}