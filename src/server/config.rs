use std::net::SocketAddr;
use getset::Getters;
use iso13400_2::{Eid, Gid, LogicAddress, LENGTH_OF_VIN, TCP_SERVER_PORT};

#[derive(Clone, Debug, Getters)]
pub struct Configuration {
    #[get = "pub"]
    pub(crate) ip_address: String,
    #[get = "pub"]
    pub(crate) vin: String,
    #[get_copy = "pub"]
    pub(crate) address: LogicAddress,
    #[get_copy = "pub"]
    pub(crate) eid: Eid,
    #[get_copy = "pub"]
    pub(crate) gid: Gid,
}

impl Configuration {
    pub fn new(
        ip: &str,
        vin: &str,
        address: LogicAddress,
        eid: Eid,
        gid: Gid,
    ) -> Option<Self> {
        match vin.len() {
            LENGTH_OF_VIN => match format!("{}:{}", ip, TCP_SERVER_PORT).parse::<SocketAddr>() {
                Ok(_) => Some(Self {
                    ip_address: ip.to_owned(),
                    vin: vin.to_owned(),
                    address,
                    eid,
                    gid,
                }),
                Err(_) => None,
            },
            _ => None,
        }
    }
}

