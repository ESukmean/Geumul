#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub u32);
impl From<u32> for NodeId {
    fn from(value: u32) -> Self {
        Self(value)
    }
}