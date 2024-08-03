use std::net::IpAddr;

use netconfig::{sys::InterfaceExt, Interface};
use tunio::{traits::*, *};

use crate::pre_types::*;

struct TunDeviceContext {}
impl TunDeviceContext {
    #[cfg(target_family = "unix")]
    pub fn open(
        &self,
        device_name: &String,
        mtu: u32,
        ip: IpAddr,
        subnet: u8,
    ) -> tunio::platform::linux::Interface {
        let mut driver = DefaultDriver::new().unwrap();
        // Preparing configuration for new interface. We use `Builder` pattern for this.
        let if_config = DefaultDriver::if_config_builder()
            .name(device_name.clone())
            .build()
            .unwrap();

        let iface = match DefaultInterface::new_up(&mut driver, if_config) {
            Ok(iface) => iface,
            Err(e) => {
                eprintln!("error while create TUN interface: {e:?}");
                panic!("error while create TUN interface");
            }
        };

        self.set_device_info(device_name, mtu, ip, subnet);

        iface
    }

    #[cfg(target_os = "windows")]
    pub fn open(
        &self,
        device_name: &String,
        mtu: u32,
        ip: IpAddr,
        subnet: u8,
    ) -> tunio::platform::wintun::Interface {
        let mut driver = DefaultDriver::new().unwrap();
        // Preparing configuration for new interface. We use `Builder` pattern for this.
        let if_config = DefaultDriver::if_config_builder()
            .name(device_name.clone())
            .build()
            .unwrap();

        let iface = match DefaultInterface::new_up(&mut driver, if_config) {
            Ok(iface) => iface,
            Err(e) => {
                eprintln!("error while create TUN interface: {e:?}");
                panic!("error while create TUN interface");
            }
        };

        self.set_device_info(device_name, mtu, ip, subnet);

        iface
    }

    fn set_device_info(&self, device_name: &String, mtu: u32, ip: IpAddr, subnet: u8) {
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
pub async fn start(config: ArcSwapConfig, manager_tx: TxMessage) {
    let mut tun_device_context = TunDeviceContext {};
    {
        let config = config.load();
        let iface = tun_device_context.open(
            &config.tun.device_name,
            config.tun.mtu,
            config.tun.ip,
            config.tun.subnet,
        );

        let (tx, rx) = tokio::sync::mpsc::channel(16384);
        manager_tx.send(ManagerMessage::InsertTx(tx)).await.unwrap();

        let mut interval = tokio::time::interval(std::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
        }
    }
}
