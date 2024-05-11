use bytes::Bytes;

pub enum Packet {
    ControlPacket(ControlPacket),
    DataPacket(DataPacket),
}

pub struct ControlPacket {
    src_node_id: u32,
    dst_node_id: u32,
    data: ControlPacketData,
}
pub enum ControlPacketData {
    HeartbeatRequest,
    HeartbeatResponse,
}
pub struct DataPacket {
    src_node_id: u32,
    dst_node_id: u32,
    current_group: u32,
    route_id: u32,

    data: Bytes,
}
