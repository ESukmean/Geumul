use std::{
    collections::HashMap,
    mem::MaybeUninit,
    net::{IpAddr, Ipv4Addr, SocketAddr, SocketAddrV4},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndPointStrategy {
    FullMirror,
}

use super::{SocketEndpointPair, TxMessage};
#[derive(Debug, Clone)]
pub struct Config {
    pub secret_key: [u8; 32],
    pub device_id: [u8; 6],
    pub end_points: HashMap<IpAddr, EndPoint>,

    // pub listen: Vec<SocketAddr>,
    // listen port, bind port 바꾸는건 프로그램 재시작이 필요함
    // TODO: listen port를 바꿀 수 있도록 함 (먼 미래에)
    pub listen_port: u16,

    pub tun: MyTunDevice,

    pub manager_tx: TxMessage,
    pub expect_byte_per_sec: usize,
}
#[derive(Debug, Clone)]
pub struct EndPoint {
    pub addr: Vec<SocketEndpointPair>,
    pub strategy: EndPointStrategy,
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
            ip: IpAddr::V4(Ipv4Addr::new(172, 29, 0, 2)),
            subnet: 24,
            mtu: 9000,

            queue_length: Some(num_cpus::get()),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        // MaybeUninit로 남겨두려 했는뎅... mpsc::channel에 Atomic Counter가 있어서 drop때 Segment fault가 발생함ㅠ
        // 그러므로, 비어있는 channel을 하나 만들어서 사용함. (length = 0도 assert에 걸림)
        let mut end_points = HashMap::with_capacity(32);
        end_points.insert(
            IpAddr::V4(Ipv4Addr::new(172, 29, 0, 3)),
            EndPoint {
                addr: vec![(
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 33888)),
                    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 33389)),
                )],
                strategy: EndPointStrategy::FullMirror,
                secret_key: "1234567890".to_string(),
            },
        );

        Self {
            device_id: [0, 1, 2, 3, 4, 5],
            secret_key: [
                0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22,
                23, 24, 25, 26, 27, 28, 29, 30, 31,
            ],
            end_points,
            // listen: Vec::new(),
            listen_port: 33600,
            tun: MyTunDevice::default(),
            manager_tx: tokio::sync::mpsc::channel(1).0,
            expect_byte_per_sec: 1_000_000_000,
        }
    }
}
