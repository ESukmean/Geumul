use array_macro::array;
use etherparse::SlicedPacket;
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use bytes::BytesMut;
use futures_util::AsyncWrite;
use netconfig::{sys::InterfaceExt, Interface};
use tunio::{traits::*, *};

use crate::{
    config::{Manager, CONFIG},
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

    for _ in 0..config.tun.queue_length.unwrap_or(1) {
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

    let buffer_alloc_size = {
        let config = CONFIG.load();
        std::cmp::max(
            (config.tun.mtu * 32) as usize,
            config.expect_byte_per_sec / (100 * num_cpus::get()),
        )
    };
    let mut packet_buf = BytesMut::with_capacity(buffer_alloc_size);

    let mut manager_rx_buf = Vec::with_capacity(1000);
    loop {
        tokio::select! {
            _ = rx.recv_many(&mut manager_rx_buf, 1000) => {
                tun_queue_process_rx_from_manager(&mut tun_iface, &manager_rx_buf).await;

                manager_rx_buf.clear();
            }
            read_len = tun_iface.read_buf(&mut packet_buf) => {
                if let Ok(len) = read_len {
                    tun_queue_process_tx_to_manager(&mut tun_iface, &mut packet_buf, len).await;
                } else {
                    rx.close();

                    while !rx.is_empty() {
                        rx.recv_many(&mut manager_rx_buf, 1000).await;

                        manager_tx.send(ManagerMessage::RePushPacketToTunQueue(manager_rx_buf.clone())).await;
                        manager_rx_buf.clear();
                    }

                    return;
                }
            }
        }
    }
}

async fn tun_queue_process_rx_from_manager(
    tun_iface: &mut TunInterface,
    buf: &Vec<ManagerMessage>,
) {
    for msg in buf {
        if let ManagerMessage::Packet(data) = msg {
            tun_iface.write_all(data).await;

            todo!("the queue could be full. we should wait - make no drop :)");
            todo!("we may merge all packets into one, so make only 1 syscall");
        } else {
            unsafe {
                std::hint::unreachable_unchecked();
            }
        }
    }
}

#[derive(Debug, Clone)]
struct OffloadCache {
    dst: Option<SocketAddr>,
    data: BytesMut,
}
async fn tun_queue_process_tx_to_manager(
    tun_iface: &mut TunInterface,
    buf: &mut BytesMut,
    len: usize,
) {
    // n개의 dst (IP, Port)에 대해 패킷 offload를 실시함

    let arr = array![_ => OffloadCache{ dst:None, data: BytesMut::with_capacity(131072)}; 4];

    let parsed_packet = SlicedPacket::from_ethernet(&buf);
    println!("--> {buf:?} => {parsed_packet:?}");
}
