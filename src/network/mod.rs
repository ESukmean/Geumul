pub mod real_adapter;
pub mod tun_device;
pub mod packet_struct;
pub mod inner_router;

use real_adapter::*;
use tun_device::*;
use packet_struct::*;

use bytes::*;

type PacketTx = tokio::sync::mpsc::Sender<Packet>;
type PacketRx = tokio::sync::mpsc::Receiver<Packet>;
