use std::{
    borrow::BorrowMut,
    collections::{HashMap, HashSet},
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use super::TxMessage;
use dashmap::DashMap;
use tokio::{net::UdpSocket, sync::Mutex};
type SocketEndpointPair = (SocketAddr, SocketAddr);
type SocketSink = tokio::sync::mpsc::Sender<bytes::Bytes>;

pub struct ConnectionBackend {
    /// seq_no < 10 일때, 특수 필드로서 작동함.
    /// seq_no == 0: syn, 1: syn-ack, 2: ack, 3: rst
    /// seq_no가 max가 된 이후로는 10 이후로 넘어가도록 제어 해야함.
    seq_no: u32,

    sockets: Arc<DashMap<SocketAddr, SocketSink>>,
    connection_create_lock: Arc<Mutex<HashSet<SocketEndpointPair>>>,
}

impl ConnectionBackend {
    /// payload: MSS (65535B) 이하인 패킷 데이터.
    /// UDP 안에 UDP가 실리는것이므로, UDP 헤더가 포함된 데이터가 64KB 이하이어야 함
    pub fn send_packet(&mut self, payload: &bytes::Bytes) -> Result<(), ()> {
        todo!();
    }
    #[inline]
    fn get_sequence_no(&mut self) -> u32 {
        self.seq_no += 1;

        self.seq_no
    }

    pub async fn spawn_new_connection(
        &self,
        src_addr: SocketAddr,
        dst_addr: SocketAddr,
        tx_manager: TxMessage,
    ) {
        let connection_create_lock_ptr = self.connection_create_lock.clone();
        let socket_sink_ptr = self.sockets.clone();

        let addr: SocketEndpointPair = (src_addr, dst_addr);

        let mut guard = connection_create_lock_ptr.lock().await;
        if guard.contains(&addr) {
            return;
        }

        guard.insert(addr);
        drop(guard);

        tokio::spawn(async move {
            // create_sock: infinte loop for listening
            let sock = Self::create_sock(addr).await;
            let Ok(sock) = sock else {
                let mut guard = connection_create_lock_ptr.lock().await;
                guard.remove(&addr);

                return;
            };

            let (tx, mut rx): (SocketSink, _) = tokio::sync::mpsc::channel(2048);
            if let Some(pre_sock) = socket_sink_ptr.insert(dst_addr, tx) {
                pre_sock.closed().await;
            }

            let mut read_buf = bytes::BytesMut::with_capacity(65535);
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            let mut payload_buf = Vec::with_capacity(1024);
            loop {
                tokio::select! {
                    payload = sock.recv_buf(&mut read_buf) => {
                        // TODO: 특수한 seq_no가 들어오면 소켓 초기화를 위해서 break;
                        break;
                    }
                    payload = rx.recv_many(&mut payload_buf, 1024) => {
                        for payload in payload_buf.drain(..) {
                            sock.send(&payload).await;
                        }
                    }
                    _ = interval.tick() => {
                        // TODO: n초 동안 ping 패킷이 안왔으면 socket broken 처리
                        break;
                    }
                }
            }

            // 정리
            let mut guard = connection_create_lock_ptr.lock().await;
            guard.remove(&addr);
            if let Some((_, tx)) = socket_sink_ptr.remove(&addr.1) {
                tx.closed().await;
            }
            drop(guard);

            // manager쪽에 socket을 새로 만들어라고 알림
            tx_manager
                .send(super::ManagerMessage::RevalidateConnection(Some(
                    addr.1.ip(),
                )))
                .await;
            while rx.recv_many(&mut payload_buf, 1024).await > 0 {}
        });
    }

    async fn create_sock(addr: SocketEndpointPair) -> Result<UdpSocket, ()> {
        let raw_sock =
            socket2::Socket::new(socket2::Domain::IPV4, socket2::Type::DGRAM, None).unwrap();

        raw_sock
            .bind(&socket2::SockAddr::from(addr.0))
            .and_then(|_| raw_sock.set_nonblocking(true))
            .and_then(|_| raw_sock.set_reuse_address(true))
            .and_then(|_| raw_sock.set_reuse_port(true))
            .unwrap();

        let Ok(sock) = tokio::net::UdpSocket::from_std(std::net::UdpSocket::from(raw_sock)) else {
            // TODO: Panic 말고 제대로 처리
            panic!("convert std udp to tokio udp failed");
        };

        if (sock.connect(addr.1).await).is_err() {
            return Err(());
        }

        let mut recv_buf = [0u8; 256];
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // 양쪽에서 syn을 보내면 경우의 수를 생각해야 함.
        // TODO: IP 주소가 낮은 쪽이 SYN을 보내고, IP 주소가 높은 쪽이 Server 역할을 하도록 수정
        while tokio::time::timeout(std::time::Duration::from_secs(6), async {
            // write handshake
            sock.send(b"~~~~~").await;

            // read handshake reply
            sock.recv(&mut recv_buf).await;
        })
        .await
        .is_err()
        {}

        // reply ack
        sock.send(b"reply").await;

        return Ok(sock);
    }
    //async fn process_sock()
}
