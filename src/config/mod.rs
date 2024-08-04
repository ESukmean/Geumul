use crate::types::*;

use std::{
    borrow::{Borrow, BorrowMut},
    sync::Arc,
};

use arc_swap::ArcSwap;
use enum_map::EnumMap;
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

            if self.config_rx.recv_many(&mut buffer, 0).await == 0 {
                continue;
            }

            // todo!("implete manager cycle");

            buffer.clear();
        }
    }
}
