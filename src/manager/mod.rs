use crate::types::*;

use std::{
    borrow::{Borrow, BorrowMut},
    collections::HashMap,
    net::{IpAddr, SocketAddr, UdpSocket},
    ops::Deref,
    sync::Arc,
};

use crate::OptionHelper;
use arc_swap::ArcSwap;
use bytes::Bytes;
use enum_map::EnumMap;
use once_cell::sync::Lazy;
use socket_strategy::ConnectionBackend;
use types::Config;

mod helper;
mod socket_strategy;
pub mod types;

pub static CONFIG: Lazy<ArcSwapConfig> = Lazy::new(|| ArcSwap::from_pointee(Config::default()));

pub struct Manager {
    config_rx: RxMessage,
    tx: EnumMap<ModuleId, Vec<TxMessage>>,

    tx_cnt: usize,

    socket: HashMap<IpAddr, ConnectionBackend>,
}

impl Manager {
    pub fn new() -> Self {
        let (tx, rx) = tokio::sync::mpsc::channel(1024);
        let mut config: Config = (CONFIG.load_full().as_ref()).clone();

        let endpoint_cnt = config.end_points.capacity();

        config.manager_tx = tx;
        CONFIG.store(Arc::new(config));

        Self {
            config_rx: rx,

            socket: HashMap::with_capacity(endpoint_cnt),

            tx: EnumMap::default(),
            tx_cnt: 0,
        }
    }
    pub async fn start(mut self) {
        let mut sleep_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        let mut buffer = Vec::with_capacity(1024);

        loop {
            tokio::select! {
                _ = sleep_interval.tick() => {
                    self.do_interval_work().await;
                }
                len = self.config_rx.recv_many(&mut buffer, 1024) => {
                    if len == 0 {
                        break;
                    }

                    self.process_message(&mut buffer);
                }
            }
        }
    }

    async fn do_interval_work(&mut self) {
        let mut config_ptr_cache: arc_swap::Cache<&arc_swap::ArcSwapAny<Arc<Config>>, Arc<Config>> =
            arc_swap::Cache::new(&*CONFIG);
    }
    fn process_message(&mut self, msgs: &mut Vec<ManagerMessage>) {
        println!("* manager rcv {msgs:?}");

        let mut config_ptr_cache: arc_swap::Cache<&arc_swap::ArcSwapAny<Arc<Config>>, Arc<Config>> =
            arc_swap::Cache::new(&*CONFIG);

        for msg in msgs.drain(..) {
            match msg {
                ManagerMessage::InsertTx(module, tx) => self.tx[module].push(tx),
                ManagerMessage::TxPacket(addr, packet) => {
                    if let Err(pkt_icmp) = self.send_packet(config_ptr_cache.load(), &addr, packet)
                    {
                        self.tx_packet(pkt_icmp);
                    }
                }
                ManagerMessage::RxPacket(pkt) => self.tx_packet(pkt),
                _ => (),
            }
        }
    }

    fn tx_packet(&mut self, packet: Bytes) {
        let tx_list = &self.tx[ModuleId::Tun];
        let tx_len = tx_list.len();
        if tx_len == 0 {
            return;
        }

        self.tx_cnt = (self.tx_cnt + 1) % tx_len;
        tx_list[self.tx_cnt].try_send(ManagerMessage::RxPacket(packet));
    }

    fn send_packet(
        &mut self,
        config: &Arc<Config>,
        addr: &IpAddr,
        packet: HeaderAllocatedPayload<bytes::Bytes>,
    ) -> Result<(), Bytes> {
        let packet = packet.0;

        // 애초에 등록이 안되어 있는 EndPoint 주소
        if !config.end_points.contains_key(addr) {
            let data = helper::generate_icmp_no_route_to_host_reply(
                unsafe { packet[..28].try_into().unwrap_unchecked() },
                helper::ICMPDestinationUnreachableCode::DestinationHostUnknown,
            );

            return Err(Bytes::copy_from_slice(&data));
        }

        return match self.socket.get_mut(addr).map(|ep| ep.send_packet(&packet)) {
            Some(Ok(())) => Ok(()),
            Some(_) => {
                // error while sending - maybe TX channel queue full by NIC queue full
                let data = helper::generate_icmp_no_route_to_host_reply(
                    unsafe { packet[..28].try_into().unwrap_unchecked() },
                    helper::ICMPDestinationUnreachableCode::DestinationHostUnknown,
                );

                Err(Bytes::copy_from_slice(&data))
            }
            _ => {
                // no socket available
                let data = helper::generate_icmp_no_route_to_host_reply(
                    unsafe { packet[..28].try_into().unwrap_unchecked() },
                    helper::ICMPDestinationUnreachableCode::SourceRouteFailed,
                );

                Err(Bytes::copy_from_slice(&data))
            }
        };
    }
}
