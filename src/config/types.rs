use std::{
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use super::TxMessage;
#[derive(Debug, Clone)]
pub struct Config {
    pub secret_key: [u8; 16],
    pub device_id: [u8; 6],
    pub end_points: Vec<EndPoint>,

    pub listen: Vec<SocketAddr>,

    pub tun: MyTunDevice,

    pub manager_tx: TxMessage,
}
#[derive(Debug, Clone)]
pub struct EndPoint {
    pub addr: SocketAddr,
    pub secret_key: String,
}
#[derive(Debug, Clone)]
pub struct MyTunDevice {
    pub device_name: String,
    pub ip: IpAddr,

    pub subnet: u8,
    pub mtu: u32,

    pub queue_length: Option<usize>,
}

impl Default for MyTunDevice {
    fn default() -> Self {
        Self {
            device_name: "esm".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(172, 29, 0, 1)),
            subnet: 24,
            mtu: 9000,

            queue_length: Some(num_cpus::get()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        let tx = unsafe {
            let tx: MaybeUninit<TxMessage> = MaybeUninit::uninit();
            tx.assume_init()
        };

        Self {
            device_id: [0, 1, 2, 3, 4, 5],
            secret_key: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
            end_points: Vec::new(),
            listen: Vec::new(),
            tun: MyTunDevice::default(),
            manager_tx: tx,
        }
    }
}