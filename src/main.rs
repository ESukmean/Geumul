mod config;
mod tun;

mod types;
use arc_swap::access::Access;
use types::*;

fn main() {
    let manager = config::Manager::new();

    if let Ok(rt) = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        rt.block_on(async move {
            tun::start_async();

            manager.start().await;
        });
    }
}
