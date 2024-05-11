use std::collections::HashMap;

use avl::AvlTreeMap;

use crate::type_alias;

// struct NodeLookupTable {
//     routing_table: AvlTreeMap<type_alias::NodeId, RoutingTableEntry>,
// }
enum InnerControl {
    RequestConnect
}
type InnerTx = tokio::sync::broadcast::Sender<InnerControl>;
type InnerRx = tokio::sync::broadcast::Receiver<InnerControl>;

struct RoutingTable {
    routing_table: AvlTreeMap<type_alias::NodeId, RoutingTableEntry>,    
}
enum RoutingTableEntry {
    Node(type_alias::NodeId),
    Group(Vec<type_alias::NodeId>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RoutingGroupStrategy {
    ActiveStandby,
    RoundRobin(usize),
    Replication(Option<usize>),
}
struct RoutingGroup {
    group_id: type_alias::NodeId,
    node_ids: Vec<type_alias::NodeId>,
    connected_nodes: Vec<type_alias::NodeId>,
    strategy: RoutingGroupStrategy,

    inner_tx: InnerTx,
    inner_rx: InnerRx,

    last_refuel: std::time::Instant,
}
impl RoutingGroup {
    pub fn new(group_id: type_alias::NodeId, routing_strategy: RoutingGroupStrategy, tx: InnerTx, rx: InnerRx) -> Self {
        Self {
            group_id,

            node_ids: Vec::new(),
            connected_nodes: Vec::new(),
            strategy: routing_strategy,

            inner_tx: tx,
            inner_rx: rx,

            last_refuel: std::time::Instant::now(),
        }
    }

    fn request_refuel(&mut self) {
        if self.last_refuel.duration_since(std::time::Instant::now()).as_secs() < 1 {
            return;
        }

        self.last_refuel = std::time::Instant::now();

        // TODO: send connection request
        self.inner_tx.send(InnerControl::RequestConnect);
    }

    pub fn pickup(&mut self) -> Option<Vec<type_alias::NodeId>> {
        if self.connected_nodes.len() == 0 {
            return None;
        }

        match self.strategy {
            RoutingGroupStrategy::ActiveStandby => {
                return Some(vec![self.connected_nodes[0]]);
            }
            RoutingGroupStrategy::RoundRobin(mut idx) => {
                idx = (idx + 1) % self.connected_nodes.len();
                return Some(vec![self.connected_nodes[idx]]);
            }
            RoutingGroupStrategy::Replication(pull_size) => {
                if let Some(mut size) = pull_size {
                    if size < self.connected_nodes.len() {
                        size = self.connected_nodes.len()
                    }

                    return Some(self.connected_nodes[0..size].to_vec());
                } else {
                    return Some(self.connected_nodes.clone());
                }
            }
        }
    }
}
struct ConnectionTable {
    connection: AvlTreeMap<type_alias::NodeId, UdpConnection>,
}
impl ConnectionTable {
    pub fn new() -> Self {
        Self {
            connection: AvlTreeMap::new(),
        }
    }

    pub fn list_connections(&self, req: Vec<type_alias::NodeId>) -> Vec<&UdpConnection> {
        req.iter().filter_map(|node_id| self.connection.get(node_id)).collect()
    }
    pub fn list_connections_mut(&mut self, req: Vec<type_alias::NodeId>, func: impl Fn(&mut UdpConnection) -> ()) {
        req.iter().for_each(|node_id| { if let Some(conn) = self.connection.get_mut(node_id) { func(conn); } });
    }
    
}
struct UdpConnection {

}

pub struct InnerRouter {
    routing_table: RoutingTable,


}
