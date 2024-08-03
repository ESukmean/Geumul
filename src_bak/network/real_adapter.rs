use super::{PacketRx, PacketTx};

struct Adapater {
    ip_address: String,
    tx_packet: PacketTx,
    rx_packet: PacketRx,
}