use crate::pre_types::*;

use std::sync::Arc;

use arc_swap::ArcSwap;
use con_struct::Config;
use enum_map::EnumMap;

pub mod con_struct;

pub struct Manager {
    config: ArcSwapConfig,
    config_tx: TxMessage,
    config_rx: RxMessage,
    tx: EnumMap<ModuleId, Vec<TxMessage>>,
}

impl Manager {
    pub fn new() -> Self {
        let config = Config::default();
        let config_arc = Arc::new(ArcSwap::from_pointee(config));

        let (tx, rx) = tokio::sync::mpsc::channel(1024);

        Self {
            config: config_arc,
            config_tx: tx,
            config_rx: rx,

            tx: EnumMap::default(),
        }
    }
    pub fn get_config(&self) -> (ArcSwapConfig, TxMessage) {
        (self.config.clone(), self.config_tx.clone())
    }
    pub async fn start(mut self) {
        let mut sleep_interval = tokio::time::interval(std::time::Duration::from_secs(5));
        let mut buffer = Vec::with_capacity(1024);
        loop {
            sleep_interval.tick().await;

            if self.config_rx.recv_many(&mut buffer, 0).await == 0 {
                continue;
            }

            // todo!("implete manager cycle");

            buffer.clear();
        }
    }
}
