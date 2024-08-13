use std::sync::Arc;

use arc_swap::ArcSwap;
use enum_map::Enum;

use crate::config::types::Config;

#[derive(Debug, Clone)]
pub enum ManagerMessage {
    InsertTx(ModuleId, Tx<ManagerMessage>),
    TxPacket(std::net::IpAddr, bytes::Bytes),
    RxPacket(bytes::Bytes),
    RePushPacketToTunQueue(Vec<ManagerMessage>),
}
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Enum)]
pub enum ModuleId {
    Tun,
}

pub type Tx<T> = tokio::sync::mpsc::Sender<T>;
pub type Rx<T> = tokio::sync::mpsc::Receiver<T>;

pub type TxMessage = Tx<ManagerMessage>;
pub type RxMessage = Rx<ManagerMessage>;

pub type ArcSwapConfig = ArcSwap<Config>;

#[cfg(target_family = "unix")]
pub type TunInterface = tunio::DefaultAsyncInterface;
#[cfg(target_family = "windows")]
pub type TunInterface = tunio::DefaultAsyncInterface;
