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
use types::{Config, EndPoint};

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

        tx.blocking_send(ManagerMessage::RevalidateConnection(None));

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

                    self.process_message(&mut buffer).await;
                }
            }
        }
    }

    async fn do_interval_work(&mut self) {
        let mut config_ptr_cache: arc_swap::Cache<&arc_swap::ArcSwapAny<Arc<Config>>, Arc<Config>> =
            arc_swap::Cache::new(&*CONFIG);
    }
    async fn process_message(&mut self, msgs: &mut Vec<ManagerMessage>) {
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
                ManagerMessage::RevalidateConnection(endpoint) => {
                    match endpoint {
                        None => {
                            // check if there's added or removed endpoint
                            // signal connect them

                            self.revalidate_connection(config_ptr_cache.load()).await;
                        }
                        Some(addr) => {
                            // signal connect them
                        }
                    }
                }
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
        payload: HeaderAllocatedPayload<bytes::Bytes>,
    ) -> Result<(), Bytes> {
        let packet = payload.0;

        // 애초에 등록이 안되어 있는 EndPoint 주소
        if !config.end_points.contains_key(addr) {
            let data = helper::generate_icmp_no_route_to_host_reply(
                unsafe { packet[..28].try_into().unwrap_unchecked() },
                helper::ICMPDestinationUnreachableCode::DestinationHostUnknown,
            );

            return Err(Bytes::copy_from_slice(&data));
        }

        // send complete packet

        return if let Some(conn_backend) = self.socket.get_mut(addr) {
            let seq_no = conn_backend.get_sequence_no();
            let len = (packet.len() - 6) as u16;

            let packet = if let Ok(mut bmut) = packet.try_into_mut() {
                bmut[0..4].copy_from_slice(&seq_no.to_be_bytes());
                bmut[4..6].copy_from_slice(&len.to_be_bytes());

                bmut.freeze()
            } else {
                panic!("convert to bytes -> bytesmut failed");
            };

            if conn_backend.send_complete_packet(&packet).is_err() {
                // error while sending - maybe TX channel queue full by NIC queue full
                let data = helper::generate_icmp_no_route_to_host_reply(
                    unsafe { packet[..28].try_into().unwrap_unchecked() },
                    helper::ICMPDestinationUnreachableCode::DestinationHostUnknown,
                );

                Err(Bytes::copy_from_slice(&data))
            } else {
                Ok(())
            }
        } else {
            // no socket available
            let data = helper::generate_icmp_no_route_to_host_reply(
                unsafe { packet[..28].try_into().unwrap_unchecked() },
                helper::ICMPDestinationUnreachableCode::SourceRouteFailed,
            );

            Err(Bytes::copy_from_slice(&data))
        };
    }

    async fn revalidate_connection(&mut self, config: &Arc<Config>) {
        println!("==> revalidate");

        let mut keys_connected: Vec<_> = self.socket.iter_mut().collect();
        let mut keys_new: Vec<_> = config.end_points.iter().collect();

        keys_connected.sort_by_key(|v| v.0);
        keys_new.sort_by_key(|v| v.0);

        let mut key_connected_idx: usize = 0;
        let mut key_new_idx: usize = 0;

        let mut to_insert: Vec<(IpAddr, EndPoint)> = Vec::with_capacity(8);
        let mut to_remove: Vec<IpAddr> = Vec::with_capacity(8);

        loop {
            match (
                keys_connected.get_mut(key_connected_idx),
                keys_new.get(key_new_idx),
            ) {
                (Some((connected, connected_item)), Some((new, new_item))) => {
                    if connected == new {
                        connected_item.load_config_and_init(new_item).await;

                        key_connected_idx += 1;
                        key_new_idx += 1;
                    } else if *connected < *new {
                        connected_item.shutdown().await;
                        to_remove.push(**connected);

                        key_connected_idx += 1;
                    } else if *connected > *new {
                        to_insert.push((**new, (*new_item).clone()));

                        key_new_idx += 1;
                    }
                }
                (Some((connected, connected_item)), None) => {
                    connected_item.shutdown().await;
                    to_remove.push(**connected);

                    key_connected_idx += 1;
                }
                (None, Some((new, new_item))) => {
                    to_insert.push((**new, (*new_item).clone()));

                    key_new_idx += 1;
                }
                (None, None) => break,
            }
        }

        println!("revalidate {to_remove:?} / {to_insert:?}");
        for rm in to_remove.drain(..) {
            self.socket.remove(&rm);
        }
        for add in to_insert.drain(..) {
            let mut connection_backend = ConnectionBackend::default();
            connection_backend.load_config_and_init(&add.1).await;

            self.socket.insert(add.0, connection_backend);
        }
    }
}
