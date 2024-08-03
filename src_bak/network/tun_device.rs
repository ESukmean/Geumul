use super::{PacketRx, PacketTx};

pub struct TunDevice {
    pub name: String,
    pub ip: String,

    tx: PacketRx,
    rx: PacketTx,
}

impl TunDevice {
    pub fn new(name: String, ip: String, tx: PacketRx, rx: PacketTx) -> Self {
        Self {
            name,
            ip,
            tx,
            rx,
        }
    }
}
