use std::sync::Arc;

use arc_swap::ArcSwap;
use enum_map::Enum;

use crate::config::con_struct::Config;

#[derive(Debug, Clone)]
pub enum ManagerMessage {
    InsertTx(Tx<ManagerMessage>),
}
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Enum)]
pub enum ModuleId {
    Tun,
}

pub type Tx<T> = tokio::sync::mpsc::Sender<T>;
pub type Rx<T> = tokio::sync::mpsc::Receiver<T>;

pub type TxMessage = Tx<ManagerMessage>;
pub type RxMessage = Rx<ManagerMessage>;

pub type ArcSwapConfig = Arc<ArcSwap<Config>>;
