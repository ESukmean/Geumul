use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub struct Config {
    pub secret_key: [u8; 16],
    pub device_id: [u8; 6],
    pub end_points: Vec<EndPoint>,

    pub listen: Vec<SocketAddr>,

    pub tun: MyTunDevice,
}
pub struct EndPoint {
    pub addr: SocketAddr,
    pub secret_key: String,
}
pub struct MyTunDevice {
    pub device_name: String,
    pub ip: IpAddr,

    pub subnet: u8,
    pub mtu: u32,
}

impl Default for MyTunDevice {
    fn default() -> Self {
        Self {
            device_name: "esm".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(172, 29, 0, 1)),
            subnet: 24,
            mtu: 9000,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_id: [0, 1, 2, 3, 4, 5],
            secret_key: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            end_points: Vec::new(),
            listen: Vec::new(),
            tun: MyTunDevice::default(),
        }
    }
}
