mod tests_network_manager;
mod tests_network_state;

use crate::channel::ChannelManager;
use log::{debug, error, info, warn};
use message::NodeEvent::PacketSent;
use petgraph::algo::dijkstra;
use petgraph::graph::{Graph, NodeIndex};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, SystemTime};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, FloodResponse, Nack, NackType, NodeType, Packet};

type Weight = u32;
type Session = u64;

const NEW_STATE_GRACE_PERIOD: Duration = Duration::from_secs(3); // TODO: is too much? evaluate

#[derive(Clone)]
pub struct NetworkState {
    topology: Graph<NodeId, Weight>,
    id_to_idx: HashMap<NodeId, NodeIndex>,
    start_idx: NodeIndex,
    pub server_list: HashSet<NodeId>,
    routing_table: HashMap<NodeId, Vec<NodeId>>, // destination -> path

    creation_time: SystemTime,
    flood_interval: Duration, // default 10 seconds,
    failed_error_count: u8,
    failed_drop_count: u8,
    error_scale: u32,
    drop_scale: u32,
}

impl NetworkState {
    pub fn new(
        start_id: NodeId,
        flood_interval: Duration,
        error_scale: u32,
        drop_scale: u32,
    ) -> Self {
        let mut topology = Graph::<NodeId, Weight>::new();
        let idx = topology.add_node(start_id);
        let mut id_to_idx = HashMap::new();
        id_to_idx.insert(start_id, idx);
        Self {
            topology,
            id_to_idx,
            start_idx: idx,
            server_list: HashSet::new(),
            routing_table: HashMap::new(),
            creation_time: SystemTime::now(),
            flood_interval,
            failed_error_count: 0,
            failed_drop_count: 0,
            error_scale,
            drop_scale,
        }
    }

    /// Determines whether the flood protocol should be triggered.
    ///
    /// Triggers flooding when:
    /// - The state is older than the configured flood interval.
    /// - The number of errors exceeds the acceptable error threshold.
    /// - The number of dropped packets exceeds the acceptable drop threshold.
    ///
    pub fn should_flood(&self) -> bool {
        let edge_count = self.topology.edge_count() as u32;

        let error_threshold = (edge_count * self.error_scale / 100).clamp(10, 100) as u8;
        let drop_threshold = (edge_count * self.drop_scale / 100).clamp(5, 50) as u8;

        let elapsed = self
            .creation_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0));

        elapsed > self.flood_interval
            || self.failed_error_count > error_threshold
            || self.failed_drop_count > drop_threshold
    }

    fn should_flood_after_missing(&self) -> bool {
        let elapsed = self
            .creation_time
            .elapsed()
            .unwrap_or(Duration::from_secs(0));
        elapsed >= NEW_STATE_GRACE_PERIOD
    }

    /// Add link
    /// If nodes does not exist, they will be created
    ///
    /// No Client will be added to the topology, except self
    pub fn add_link(
        &mut self,
        a: NodeId,
        b: NodeId,
        a_type: NodeType,
        b_type: NodeType,
        mut weight: Weight,
    ) {
        if (a_type == NodeType::Client && a != self.topology[self.start_idx])
            || b_type == NodeType::Client
        {
            return;
        }

        weight = if weight > 0 { weight } else { 1 };

        let a_idx = *self
            .id_to_idx
            .entry(a)
            .or_insert_with(|| self.topology.add_node(a));

        let b_idx = *self
            .id_to_idx
            .entry(b)
            .or_insert_with(|| self.topology.add_node(b));

        if a_type == NodeType::Server {
            self.server_list.insert(a);
        }

        if b_type == NodeType::Server {
            self.server_list.insert(b);
        }

        match (a_type, b_type) {
            (NodeType::Drone, NodeType::Drone)
            | (NodeType::Client, NodeType::Drone)
            | (NodeType::Drone, NodeType::Client) => {
                self.topology.add_edge(a_idx, b_idx, weight);
                self.topology.add_edge(b_idx, a_idx, weight);
            }
            (NodeType::Drone, NodeType::Server) => {
                self.topology.add_edge(a_idx, b_idx, weight);
            }
            (NodeType::Server, NodeType::Drone) => {
                self.topology.add_edge(b_idx, a_idx, weight);
            }
            _ => {}
        }
    }

    /// Add node to `topology` and `id_to_idx`
    ///
    /// No Client will be added to the topology
    pub fn add_node(&mut self, nid: NodeId, node_type: NodeType) {
        if node_type == NodeType::Client && nid != self.topology[self.start_idx] {
            return;
        }

        if self.id_to_idx.contains_key(&nid) {
            debug!("Node {:?} already present, no other action.", nid);
            return;
        }

        let idx = self.topology.add_node(nid);
        self.id_to_idx.insert(nid, idx);
        info!("Add node {:?} with type {:?}", nid, node_type);

        if node_type == NodeType::Server {
            self.server_list.insert(nid);
        }
    }

    /// Remove node from `topology` and `id_to_idx`
    pub fn remove_node(&mut self, nid: &NodeId) {
        if !self.server_list.contains(nid) {
            if let Some(&idx) = self.id_to_idx.get(nid) {
                self.topology.remove_node(idx);
                self.id_to_idx.remove(nid);
            }
        }
    }

    pub fn increment_weight_around_node(&mut self, nid: &NodeId, increment: i32) {
        if let Some(&node_idx) = self.id_to_idx.get(nid) {
            let outgoing_neighbors: Vec<_> = self.topology.neighbors(node_idx).collect();
            for neighbor_idx in outgoing_neighbors {
                if let Some(edge_idx) = self.topology.find_edge(node_idx, neighbor_idx) {
                    if let Some(&current_weight) = self.topology.edge_weight(edge_idx) {
                        let new_weight = if increment >= 0 {
                            current_weight.saturating_add(increment as u32)
                        } else {
                            let decrement = (-increment) as u32;
                            if current_weight > decrement {
                                current_weight - decrement
                            } else {
                                1
                            }
                        };
                        self.topology
                            .update_edge(node_idx, neighbor_idx, new_weight);
                    }
                }
            }

            let all_nodes: Vec<_> = self.topology.node_indices().collect();
            for other_idx in all_nodes {
                if other_idx != node_idx {
                    if let Some(edge_idx) = self.topology.find_edge(other_idx, node_idx) {
                        if let Some(&current_weight) = self.topology.edge_weight(edge_idx) {
                            let new_weight = if increment >= 0 {
                                current_weight.saturating_add(increment as u32)
                            } else {
                                let decrement = (-increment) as u32;
                                if current_weight > decrement {
                                    current_weight - decrement
                                } else {
                                    1
                                }
                            };
                            self.topology.update_edge(other_idx, node_idx, new_weight);
                        }
                    }
                }
            }
        }
    }

    /// Attempts to elaborate or recompute a group of paths a group of path
    ///
    /// If some paths are missing, it will try to generate them.
    /// You can optionally filter by a specific `NodeId` to restrict the computation
    /// to paths involving that node.
    ///
    ///
    ///
    /// # Arguments
    ///
    /// * `nid` - An optional to filter only path with this nodeId
    ///
    /// # Returns
    ///
    /// - false - flooding required
    pub fn recompute_all_routes_to_server(&mut self, nid: Option<&NodeId>) -> bool {
        let distances = dijkstra(&self.topology, self.start_idx, None, |e| *e.weight());

        for sid in &self.server_list {
            let should_recompute = match self.routing_table.get(sid) {
                Some(old_path) => nid.is_none_or(|n| old_path.contains(n)),
                None => true,
            };
            if should_recompute {
                if let Some(sidx) = self.id_to_idx.get(sid) {
                    if distances.contains_key(sidx) {
                        if let Some(path) = self._reconstruct_path(&distances, *sidx) {
                            debug!("New path computed {:?}", path);
                            self.routing_table.insert(*sid, path);
                        }
                    } else if self.should_flood_after_missing() {
                        debug!("Should flood after missing a route");
                        return false;
                    } else {
                        debug!("No path elaborated but should not flood yet");
                    }
                } else {
                    warn!("Index of Server {:?} doesnt exist", sid)
                }
            }
        }
        true
    }

    /// Retrieves a cached path or computes a new path to the specified server.
    ///
    /// # Arguments
    ///
    /// * `server` - Target Server NodeId
    ///
    /// # Returns
    ///
    /// * - `Some(path)` - If a path was found
    /// * - `None` - If no route founded or determined
    pub fn get_server_path(&mut self, sid: &NodeId) -> Option<Vec<NodeId>> {
        if !self.server_list.contains(sid) {
            warn!("Server is not present inside the list {:?}", sid);
            return None;
        }

        if let Some(path) = self.routing_table.get(sid) {
            debug!("Return Cached path to {:?}: {:?}", sid, path);
            return Some(path.clone());
        }

        if let Some(sidx) = self.id_to_idx.get(sid) {
            let distances = dijkstra(&self.topology, self.start_idx, Some(*sidx), |e| *e.weight());
            if distances.contains_key(sidx) {
                if let Some(path) = self._reconstruct_path(&distances, *sidx) {
                    self.routing_table.insert(*sid, path.clone());
                    debug!("Return Elaborated path to {:?}: {:?}", sid, path);
                    return Some(path.clone());
                }
                warn!("No path founded for {:?}", sid);
            }
        }
        None
    }

    fn _reconstruct_path(
        &self,
        distances: &hashbrown::HashMap<NodeIndex, Weight>,
        target_idx: NodeIndex,
    ) -> Option<Vec<NodeId>> {
        let mut path = Vec::new();
        let mut current = target_idx;
        let mut visited = HashSet::new();

        while current != self.start_idx {
            if !visited.insert(current) {
                return None;
            }
            path.push(self.topology[current]);

            let mut best_prev = None;
            let mut best_distance = u32::MAX;
            for node_idx in self.topology.node_indices() {
                if let Some(edge_idx) = self.topology.find_edge(node_idx, current) {
                    if let Some(&edge_weight) = self.topology.edge_weight(edge_idx) {
                        if let Some(&node_dist) = distances.get(&node_idx) {
                            if node_dist + edge_weight < best_distance {
                                best_distance = node_dist + edge_weight;
                                best_prev = Some(node_idx);
                            }
                        }
                    }
                }
            }

            current = best_prev?;
        }

        path.push(self.topology[self.start_idx]);
        path.reverse();
        Some(path)
    }
}

pub struct NetworkManager {
    my_id: NodeId,

    pub state: NetworkState,
    pub old_state: NetworkState,

    channels: Rc<RefCell<ChannelManager>>,
    last_flood: Session,
}

impl NetworkManager {
    pub fn new(my_id: NodeId, channels: Rc<RefCell<ChannelManager>>) -> Self {
        let flood_interval = Duration::from_secs(10);
        let error_scale = 30;
        let drop_scale = 20;
        let mut state = NetworkState::new(my_id, flood_interval, error_scale, drop_scale);
        state.add_node(my_id, NodeType::Client);
        Self {
            my_id,
            state,
            old_state: NetworkState::new(my_id, flood_interval, error_scale, drop_scale),
            channels,
            last_flood: 0,
        }
    }

    // <--------------------------------------Flood Protocol--------------------------------------->
    /// Sends a flood request via broadcast.
    ///
    /// Before sending, the current state is saved as a backup (`old_state`)
    /// in case the new state proves incomplete or invalid.
    pub fn send_flood_request(&mut self) {
        self.old_state = self.state.clone();
        self.last_flood += 1;
        debug!("Sending flood request with session {}", self.last_flood);
        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(self.last_flood, self.my_id, NodeType::Client),
        );
        self.channels.borrow_mut().broadcast_packet(flood_request);
    }

    /// # Returns
    /// - Option(NewServers)
    pub fn update_network_from_flood_response(
        &mut self,
        flood_response: &FloodResponse,
    ) -> Option<Vec<NodeId>> {
        let Some((mut prev, mut prev_type)) = flood_response.path_trace.first().copied() else {
            error!("Invalid path_trace: empty in flood response");
            return None;
        };

        let mut new_servers: Vec<NodeId> = Vec::new();

        for &(nid, ntype) in &flood_response.path_trace[1..] {
            debug!(
                "Processing link: {:?} ({:?}) -> {:?} ({:?})",
                prev, prev_type, nid, ntype
            );
            if ntype == NodeType::Server && !self.state.server_list.contains(&nid) {
                info!("Discovered new server node: {:?}", nid);
                new_servers.push(nid);
            }
            self.state.add_link(prev, nid, prev_type, ntype, 1);
            prev = nid;
            prev_type = ntype;
        }

        if new_servers.is_empty() {
            debug!("No new servers discovered in flood response.");
            return None;
        }

        debug!(
            "New servers discovered: {:?}. Recomputing routes...",
            new_servers
        );

        if !self.state.recompute_all_routes_to_server(None) {
            warn!("Route recomputation failed. Sending new flood request.");
            self.send_flood_request();
        } else {
            info!("Route recomputation succeeded.");
        }

        Some(new_servers)
    }

    // <--------------------------------------Send Packet--------------------------------------->
    /// Send a packet to a server.
    ///
    /// Starts the flooding protocol if the grace period has expired.
    ///
    /// # Arguments
    ///
    /// * `packet` - Packet with an empty routing path.
    /// * `server` - Target server NodeId.
    ///
    /// # Returns
    ///
    /// * `true` if the packet was successfully sent.
    /// * `false` otherwise (e.g. no route found, flooding triggered).
    pub fn send_packet(&mut self, packet: &Packet, server: &NodeId) -> bool {
        if let Some(path) = self.state.get_server_path(server) {
            if self._send_packet_actual(packet.clone(), path.clone(), server, true) {
                return true;
            }
        }

        if !self.state.should_flood_after_missing() {
            self.old_state
                .get_server_path(server)
                .map(|path| self._send_packet_actual(packet.clone(), path, server, false))
                .unwrap_or(false)
        } else {
            self.send_flood_request();
            false
        }
    }

    /// If a path fail try another ona and removes the neighbour.
    fn _send_packet_actual(
        &mut self,
        mut packet: Packet,
        mut path: Vec<NodeId>,
        server: &NodeId,
        use_current_state: bool,
    ) -> bool {
        let mut retry_count = 0;
        const MAX_RETRIES: usize = 3;
        loop {
            if retry_count >= MAX_RETRIES {
                return false;
            }
            retry_count += 1;

            packet.routing_header = SourceRoutingHeader::with_first_hop(path.clone());
            if let Some(drone) = packet.routing_header.current_hop() {
                if let Some(tx_drone) = self.channels.borrow().tx_drone.get(&drone) {
                    if tx_drone.send(packet.clone()).is_ok() {
                        self.channels
                            .borrow()
                            .tx_ctrl
                            .send(PacketSent(packet.clone()))
                            .expect("Failed to transmit to CONTROLLER");
                        return true;
                    }
                }

                let state = if use_current_state {
                    &mut self.state
                } else {
                    &mut self.old_state
                };

                state.remove_node(&drone);
                state.routing_table.remove(server);
                self.channels.borrow_mut().tx_drone.remove(&drone);
                if let Some(new_path) = state.get_server_path(server) {
                    path = new_path;
                    continue;
                }
            }
            return false;
        }
    }

    // <--------------------------------------Nack protocol---------------------------------------->
    pub fn update_network_from_nack(&mut self, nack: &Nack, origin: &NodeId) {
        match nack.nack_type {
            NackType::ErrorInRouting(faulty) | NackType::UnexpectedRecipient(faulty) => {
                self.state.failed_error_count = self.state.failed_error_count.saturating_add(1);
                debug!(
                    "Routing error involving node {:?}, error count now {}",
                    faulty, self.state.failed_error_count
                );

                self.state.remove_node(&faulty);
                if !self.state.recompute_all_routes_to_server(Some(&faulty)) {
                    self.send_flood_request();
                }
            }

            NackType::Dropped => {
                self.state.failed_drop_count = self.state.failed_drop_count.saturating_add(1);
                debug!(
                    "Message dropped by node {:?}, drop count now {}",
                    origin, self.state.failed_drop_count
                );

                self.state.increment_weight_around_node(origin, 1);
                if !self.state.recompute_all_routes_to_server(Some(origin)) {
                    self.send_flood_request();
                }
            }

            NackType::DestinationIsDrone => {
                debug!(
                    "Removing {:?} from server list — identified as drone.",
                    origin
                );
                self.state.server_list.remove(origin);
                self.state.id_to_idx.remove(origin);
            }
        }
    }

    pub fn update_network_from_ack(&mut self, origin: &NodeId) {
        debug!("Decrement weight around {:?}", origin);
        self.state.increment_weight_around_node(origin, -1);
        if !self.state.recompute_all_routes_to_server(Some(origin)) {
            self.send_flood_request();
        }
    }
}
