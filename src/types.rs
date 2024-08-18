use std::{ops::DerefMut, sync::Arc};

use arc_swap::ArcSwap;
use bytes::BufMut;
use enum_map::Enum;

use crate::manager::types::Config;

#[derive(Debug, Clone)]
pub enum ManagerMessage {
    InsertTx(ModuleId, Tx<ManagerMessage>),
    TxPacket(std::net::IpAddr, HeaderAllocatedPayload<bytes::Bytes>),
    RxPacket(bytes::Bytes),
    RePushPacketToTunQueue(Vec<ManagerMessage>),
    RevalidateConnection(Option<std::net::IpAddr>),
}
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Enum)]
pub enum ModuleId {
    Tun,
}

pub type Tx<T> = tokio::sync::mpsc::Sender<T>;
pub type Rx<T> = tokio::sync::mpsc::Receiver<T>;

pub type TxMessage = Tx<ManagerMessage>;
pub type RxMessage = Rx<ManagerMessage>;

pub type ArcSwapConfig = ArcSwap<Config>;

#[cfg(target_family = "unix")]
pub type TunInterface = tunio::DefaultAsyncInterface;
#[cfg(target_family = "windows")]
pub type TunInterface = tunio::DefaultAsyncInterface;

pub trait OptionHelper<T> {
    fn then<F>(self, func: F)
    where
        F: FnOnce(T);
}
impl<T> OptionHelper<T> for Option<T> {
    #[inline(always)]
    fn then<F>(self, func: F)
    where
        F: FnOnce(T),
    {
        if let Some(x) = self {
            func(x);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct HeaderAllocatedPayload<T>(pub T);
impl<T> core::ops::Deref for HeaderAllocatedPayload<T>
where
    T: core::ops::Deref,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl From<bytes::BytesMut> for HeaderAllocatedPayload<bytes::BytesMut> {
    fn from(mut value: bytes::BytesMut) -> Self {
        unsafe { value.advance_mut(6) };
        Self(value)
    }
}
impl DerefMut for HeaderAllocatedPayload<bytes::BytesMut> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Into<(HeaderAllocatedPayload<bytes::Bytes>, bytes::BytesMut)>
    for HeaderAllocatedPayload<bytes::BytesMut>
{
    fn into(mut self) -> (HeaderAllocatedPayload<bytes::Bytes>, bytes::BytesMut) {
        let data = self.0.split();

        (
            HeaderAllocatedPayload::<bytes::Bytes>(data.freeze()),
            self.0,
        )
    }
}
