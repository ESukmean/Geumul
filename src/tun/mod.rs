use array_macro::array;
use etherparse::{err::packet::SliceError, SlicedPacket};
use std::{
    net::{IpAddr, SocketAddr},
    ops::DerefMut,
};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use futures_util::AsyncWrite;
use netconfig::{sys::InterfaceExt, Interface};
use tunio::{traits::*, *};

#[derive(Error, Debug)]
pub enum TunDeviceError {
    #[error("Packet Proto is not valid (it seems not IPv4, IPv6)")]
    NotValidProto,
    #[error("Packet is not valid")]
    NotValidPacket,
}

use crate::{
    manager::{Manager, CONFIG},
    types::*,
};

struct TunDeviceContext;
impl TunDeviceContext {
    #[cfg(target_family = "unix")]
    pub fn open(&self, device_name: String) -> TunInterface {
        let mut driver = DefaultDriver::new().unwrap();
        // Preparing configuration for new interface. We use `Builder` pattern for this.

        let if_config = DefaultAsyncInterface::config_builder()
            .name(device_name)
            // .layer(Layer::L2)
            .build()
            .unwrap();

        let iface = match DefaultAsyncInterface::new_up(&mut driver, if_config) {
            Ok(iface) => iface,
            Err(e) => {
                eprintln!("error while create TUN interface: {e:?}");
                panic!("error while create TUN interface");
            }
        };

        iface
    }

    pub fn set_device_info(device_name: &String, mtu: u32, ip: IpAddr, subnet: u8) {
        let iface_setting = match netconfig::Interface::try_from_name(device_name) {
            Ok(iface) => iface,
            Err(e) => {
                eprint!(
                    "error while get TUN interface info. is the network interface really created?\n\t - {e:?}"
                );
                panic!("error while get TUN interface info");
            }
        };

        // let mac_addr = advmac::MacAddr6::new(device_id);
        // if let Err(e) = iface_setting.set_hwaddress(mac_addr) {
        //     eprintln!(
        //         "error while setting MAC address of TUN interface ({device_name} << {mac_addr:?} ({device_id:?}))\n\t - {e:?}",
        //     );
        //     panic!("error while setting MAC address of TUN interface");
        // }
        if let Err(e) = iface_setting.add_address(match ip {
            IpAddr::V4(ip) => {
                netconfig::ipnet::IpNet::V4(netconfig::ipnet::Ipv4Net::new(ip, subnet).unwrap())
            }
            IpAddr::V6(ip) => {
                netconfig::ipnet::IpNet::V6(netconfig::ipnet::Ipv6Net::new(ip, subnet).unwrap())
            }
        }) {
            eprintln!(
                "error while setting IP address of TUN interface ({device_name} << {ip:?}/{subnet}))\n\t - {e:?}",
            );
            panic!("error while setting IP address of TUN interface");
        }

        if let Err(e) = iface_setting.set_mtu(mtu) {
            eprintln!(
                "error while setting MTU of TUN interface ({device_name} << MTU {mtu})\n\t - {e:?}",
            );
            panic!("error while setting MTU of TUN interface");
        }

        #[cfg(not(target_family = "windows"))]
        if let Err(e) = iface_setting.set_up(true) {
            eprintln!("error while setting STATE=UP of TUN interface ({device_name})\n\t - {e:?}",);
            panic!("error while setting STATE=UP of TUN interface");
        }
        #[cfg(not(target_family = "windows"))]
        if let Err(e) = iface_setting.set_running(true) {
            eprintln!(
                "error while setting STATE=RUNNING of TUN interface ({device_name})\n\t - {e:?}",
            );
            panic!("error while setting STATE=RUNNING of TUN interface");
        }
    }
}

pub fn start_async() {
    let mut tun_device_context = TunDeviceContext;

    let config = CONFIG.load();

    // for _ in 0..config.tun.queue_length.unwrap_or(1) {
    for _ in 0..4 {
        let tun = tun_device_context.open(config.tun.device_name.clone());

        tokio::spawn(tun_queue_process(tun, config.manager_tx.clone()));
    }

    TunDeviceContext::set_device_info(
        &config.tun.device_name,
        config.tun.mtu,
        config.tun.ip,
        config.tun.subnet,
    );
}

async fn tun_queue_process(mut tun_iface: TunInterface, manager_tx: TxMessage) {
    let (tx, mut rx) = tokio::sync::mpsc::channel(16384);

    manager_tx
        .send(ManagerMessage::InsertTx(ModuleId::Tun, tx))
        .await
        .unwrap();

    // 단순한 TUN이면 MTU값 어치만 들고있으면 되는데, Offload를 쓰기 때문에 버퍼를 맞춰줘야 함
    // https://blog.cloudflare.com/virtual-networking-101-understanding-tap
    // these features the application must be ready to receive much larger buffers - up to 65507 bytes for IPv4 and 65527 for IPv6.

    // 단순 TUN 장치일때 버퍼크기
    // let buffer_alloc_size = {
    //     let config = CONFIG.load();
    //     // MTU 크기
    //     std::cmp::max((config.tun.mtu * 32) as usize, 9216)
    // };

    let buffer_alloc_size = (65535 * 4) as usize;

    let mut packet_buf = BytesMut::with_capacity(buffer_alloc_size);
    let mut packet_typed = HeaderAllocatedPayload::from(packet_buf);

    let mut manager_rx_buf = Vec::with_capacity(1000);
    loop {
        tokio::select! {
            _ = rx.recv_many(&mut manager_rx_buf, 1000) => {
                tun_queue_process_rx_from_manager(&mut tun_iface, &mut manager_rx_buf).await;

                manager_rx_buf.clear();
            }
            Ok(read_len) = tun_iface.read_buf(packet_typed.deref_mut()) => {
                if read_len == 0 {
                    cleanup(rx, manager_tx).await;
                    return;
                }

                let (pkt, remain) = packet_typed.into();
                packet_buf = remain;
                packet_typed = HeaderAllocatedPayload::from(packet_buf);

                tun_queue_process_tx_to_manager(&manager_tx, &mut tun_iface, pkt, read_len);
            }
        }

        // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    }
}
async fn cleanup(mut rx: RxMessage, tx: TxMessage) {
    rx.close();

    let mut manager_rx_buf = Vec::with_capacity(1000);

    while !rx.is_empty() {
        rx.recv_many(&mut manager_rx_buf, 1000).await;

        tx.send(ManagerMessage::RePushPacketToTunQueue(
            manager_rx_buf.clone(),
        ))
        .await;
        manager_rx_buf.clear();
    }

    return;
}

async fn tun_queue_process_rx_from_manager(
    tun_iface: &mut TunInterface,
    buf: &mut Vec<ManagerMessage>,
) {
    for msg in buf.drain(..) {
        if let ManagerMessage::RxPacket(data) = msg {
            let packet = etherparse::IpSlice::from_slice(&data);
            if let Ok(pkt) = packet {
                println!(
                    "read buf {:?} -> {:?} / {pkt:?}",
                    pkt.source_addr(),
                    pkt.destination_addr()
                );
            }

            tun_iface.write_all(&data).await;

            continue;

            todo!("the queue could be full. we should wait - make no drop :)");
            todo!("we may merge all packets into one, so make only 1 syscall");
        } else {
            unsafe {
                std::hint::unreachable_unchecked();
            }
        }
    }
}

// #[derive(Debug, Clone)]
// struct OffloadCache {
//     dst: Option<SocketAddr>,
//     data: BytesMut,
// }
fn tun_queue_process_tx_to_manager(
    tx_manager: &TxMessage,
    tun_iface: &mut TunInterface,
    buf: HeaderAllocatedPayload<Bytes>,
    len: usize,
) -> Result<(), TunDeviceError> {
    // n개의 dst (IP, Port)에 대해 패킷 offload를 실시함

    // let arr = array![_ => OffloadCache{ dst:None, data: BytesMut::with_capacity(131072)}; 4];

    // match etherparse::Ethernet2Slice::from_slice_with_crc32_fcs(buf) {
    //     Ok(pkt) => println!("read pkt {pkt:?}"),
    //     Err(e) => eprintln!("parse eth2 frame err {e:?}"),
    // };

    // println!("readbuf {buf:#02x}");

    let packet = etherparse::IpSlice::from_slice(&buf[6..]);
    let packet = match packet {
        Ok(pkt) => pkt,
        Err(etherparse::err::ip::SliceError::Len(_)) => return Ok(()),
        Err(err) => {
            eprintln!("TUN parse E => {err:?}");
            return Err(TunDeviceError::NotValidPacket);
        }
    };

    let addr = packet.destination_addr();
    tx_manager.try_send(ManagerMessage::TxPacket(addr, buf));

    // TODO: 같은 주소에 대해서 패킷 합치기를 해야함
    // TUN MultiQueue를 통해서 여러개의 큐를 사용중임. manager는 단일코어이기 때문에,,, 여기서 잡다한것을 다 해서 보내야 함.

    // match packet {
    //     etherparse::IpSlice::Ipv4(packet) => {
    //         // let packet_len = packet.header().total_len() as usize;

    //         // buf.advance(packet_len);
    //     }
    //     etherparse::IpSlice::Ipv6(packet) => {
    //         // let packet_len =
    //         //     packet.header().header_len() + packet.header().payload_length() as usize;

    //         tx_manager.try_send(ManagerMessage::TxPacket(addr, buf));
    //         // buf.advance(packet_len);
    //     }
    // };

    return Ok(());
    // println!("--> {buf:?} => {parsed_packet:?}");
}
