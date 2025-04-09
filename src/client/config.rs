use std::net::SocketAddr;
use getset::{CopyGetters, Getters};
use iso13400_2::{LogicAddress, TCP_SERVER_PORT};

#[derive(Clone, Debug, Getters, CopyGetters)]
pub struct Configuration {
    #[get = "pub"]
    pub(crate) server_ip: String,
    #[get_copy = "pub"]
    pub(crate) address: LogicAddress,
}

impl Configuration {
    pub fn new(
        server_ip: &str,
        address: LogicAddress,
    ) -> Option<Self> {
        match format!("{}:{}", server_ip, TCP_SERVER_PORT).parse::<SocketAddr>() {
            Ok(_) => Some(Self {
                server_ip: server_ip.to_owned(),
                address
            }),
            Err(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use iso13400_2::LogicAddress;
    use super::Configuration;

    #[test]
    fn test_configuration() {
        let cfg = Configuration::new("127.0.0.1", LogicAddress::from(0x0E00)).unwrap();
        assert_eq!(cfg.server_ip(), "127.0.0.1");
        assert_eq!(cfg.address(), LogicAddress::from(0x0E00));
    }
}
