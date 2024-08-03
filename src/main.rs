mod config;
mod tun;

mod pre_types;
use pre_types::*;

fn main() {
    let manager = config::Manager::new();
    let (config, manager_tx) = manager.get_config();

    if let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        rt.block_on(async move {
            tokio::spawn(tun::start(config, manager_tx));

            manager.start().await;
        });
    }
}
