use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use super::TxMessage;
use tokio::{net::UdpSocket, sync::Mutex};
type SocketEndpointPair = (SocketAddr, SocketAddr);

pub struct SocketStrategy {
    seq_no: u32,

    sockets: HashMap<IpAddr, tokio::sync::mpsc::UnboundedSender<bytes::Bytes>>,
    connection_create_lock: Arc<Mutex<HashSet<SocketEndpointPair>>>,
}

impl SocketStrategy {
    pub fn select_socket(&mut self) -> Option<&mut UdpSocket> {
        todo!()
    }
    #[inline]
    pub fn get_sequence_no(&mut self) -> u32 {
        self.seq_no += 1;

        self.seq_no
    }

    pub async fn spawn_new_connection(
        &self,
        src_addr: SocketAddr,
        dst_addr: SocketAddr,
        tx: TxMessage,
    ) {
        let connection_create_lock_ptr = self.connection_create_lock.clone();
        let addr: SocketEndpointPair = (src_addr, dst_addr);

        let mut guard = connection_create_lock_ptr.lock().await;
        if guard.contains(&addr) {
            return;
        }

        guard.insert(addr);
        drop(guard);

        tokio::spawn(async move {
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                if let Ok(sock) = tokio::net::UdpSocket::bind(addr.0).await {
                    if let Err(_) = sock.connect(addr.1).await {
                        return;
                    }

                    let mut recv_buf = [0u8; 256];

                    // write handshake
                    sock.send(b"~~~~~").await;

                    // read handshake reply
                    sock.recv(&mut recv_buf).await;

                    // reply ack
                    sock.send(b"reply").await;
                }
            })
            .await;

            let mut guard = connection_create_lock_ptr.lock().await;
            guard.remove(&addr);
        });
    }
}
