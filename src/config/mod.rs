use crate::types::*;

use std::{
    borrow::{Borrow, BorrowMut},
    net::IpAddr,
    ops::Deref,
    sync::Arc,
};

use arc_swap::ArcSwap;
use bytes::Bytes;
use enum_map::EnumMap;
use netconfig::ipnet::IpAdd;
use once_cell::sync::Lazy;
use types::Config;

pub mod types;

pub static CONFIG: Lazy<ArcSwapConfig> = Lazy::new(|| ArcSwap::from_pointee(Config::default()));

pub struct Manager {
    config_rx: RxMessage,
    tx: EnumMap<ModuleId, Vec<TxMessage>>,
}

impl Manager {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let mut config: Config = (CONFIG.load_full().as_ref()).clone();

        config.manager_tx = tx;
        CONFIG.store(Arc::new(config));

        Self {
            config_rx: rx,

            tx: EnumMap::default(),
        }
    }
    pub async fn start(mut self) {
        let mut sleep_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        let mut buffer = Vec::with_capacity(1024);
        loop {
            sleep_interval.tick().await;

            let read_len = self.config_rx.recv_many(&mut buffer, 1024).await;
            println!("* manager rcv {buffer:?}");

            let mut config_ptr_cache: arc_swap::Cache<
                &arc_swap::ArcSwapAny<Arc<Config>>,
                Arc<Config>,
            > = arc_swap::Cache::new(&*CONFIG);

            let mut tx_cnt: usize = 0;

            for msg in buffer.drain(..) {
                match msg {
                    ManagerMessage::InsertTx(module, tx) => self.tx[module].push(tx),
                    ManagerMessage::TxPacket(addr, packet) => {
                        if let Some(reply) =
                            self.send_packet(config_ptr_cache.load(), &addr, packet)
                        {
                            let tx_list = &self.tx[ModuleId::Tun];
                            let tx_len = tx_list.len();
                            if tx_len == 0 {
                                continue;
                            }

                            tx_cnt = (tx_cnt + 1) % tx_len;
                            tx_list[tx_cnt].send(ManagerMessage::RxPacket(reply)).await;
                        }
                    }
                    _ => (),
                }
            }

            if read_len == 0 {
                break;
            }

            buffer.clear();
        }
    }

    fn send_packet(&mut self, config: &Arc<Config>, addr: &IpAddr, packet: Bytes) -> Option<Bytes> {
        if let Some(endpoints) = config.end_points.get(addr) {
            println!(" ---> send data");
            return None;
        } else {
            let data = Self::generate_icmp_no_route_to_host_reply(unsafe {
                packet[..28].try_into().unwrap_unchecked()
            });

            Some(Bytes::copy_from_slice(&data))
        }
    }

    fn generate_icmp_no_route_to_host_reply(original: [u8; 28]) -> [u8; 56] {
        let mut packet = [0u8; 56];

        // IP Header
        packet[0] = 0x45; // Version and IHL (IPv4, 20 bytes)
        packet[1] = 0x00; // Type of Service
        packet[2] = 0x00; // Total Length (2 bytes, will be calculated later)
        packet[3] = 56; //
        packet[4] = 0x00; // Identification (2 bytes)
        packet[5] = 0x00;
        packet[6] = 0x00; // Flags/Fragment Offset
        packet[7] = 0x00;
        packet[8] = 0x40; // TTL (64)
        packet[9] = 0x01; // Protocol (ICMP)
        packet[10] = 0x00; // Header Checksum (2 bytes, will be calculated later)
        packet[11] = 0x00;
        packet[12..16].copy_from_slice(&[172, 29, 0, 1]); // Source IP (Placeholder)
        packet[16..20].copy_from_slice(&[172, 29, 0, 2]); // Destination IP (Placeholder)

        // ICMP Header
        packet[20] = 0x03; // ICMP Type: Destination Unreachable
        packet[21] = 0x01; // ICMP Code: Host Unreachable
        packet[22] = 0x00; // Checksum (2 bytes, will be calculated later)
        packet[23] = 0x00;
        packet[24] = 0x00; // Unused (4 bytes)
        packet[25] = 0x00;
        packet[26] = 0x00;
        packet[27] = 0x00;

        // Original IP Header and first 8 bytes of the original data (Placeholder)
        packet[28..56].copy_from_slice(&original);

        // Calculate checksum for ICMP header and data (20: IPv4, 8: 실제 데이터)
        let checksum = Self::compute_checksum(&packet[20..28 + 20 + 8]);
        packet[22] = (checksum >> 8) as u8;
        packet[23] = (checksum & 0xff) as u8;

        // Calculate checksum for IP header
        let ip_checksum = Self::compute_checksum(&packet[0..20]);
        packet[10] = (ip_checksum >> 8) as u8;
        packet[11] = (ip_checksum & 0xff) as u8;

        packet
    }

    fn compute_checksum(data: &[u8]) -> u16 {
        let mut sum = 0u32;

        // Sum all 16-bit words
        for i in (0..data.len()).step_by(2) {
            let word = u16::from_be_bytes([data[i], data[i + 1]]);
            sum = sum.wrapping_add(u32::from(word));
        }

        // Fold 32-bit sum to 16 bits
        while (sum >> 16) > 0 {
            sum = (sum & 0xffff) + (sum >> 16);
        }

        !(sum as u16)
    }
}
