use log::{info, warn};
use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, SystemTime};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodResponse, Nack, NackType, NodeType};

#[derive(Clone, Debug)]
pub struct NetworkManager {
    pub(crate) topology: HashMap<NodeId, (HashSet<NodeId>, f64, f64)>,
    pub(crate) routes: HashMap<NodeId, Vec<NodeId>>,
    pub(crate) client_list: HashSet<NodeId>,
    server_id: NodeId,
    pub(crate) n_errors: i64,
    pub(crate) n_dropped: i64,
    flood_interval: Duration,
    start_time: SystemTime,
}

impl NetworkManager {
    pub fn new(server_id: NodeId, flood_interval: Duration) -> Self {
        let mut topology = HashMap::new();
        topology.insert(server_id, (HashSet::new(), 1.0, 1.0));
        Self {
            topology,
            routes: HashMap::new(),
            client_list: HashSet::new(),
            server_id,
            n_errors: 0,
            n_dropped: 0,
            flood_interval,
            start_time: SystemTime::now(),
        }
    }
    pub fn update_topology(&mut self, flood_response: FloodResponse) {
        for n in 0..flood_response.path_trace.len() {
            if !self.topology.contains_key(&flood_response.path_trace[n].0) && !(flood_response.path_trace[n].1 == NodeType::Server) {
                    self.topology
                        .insert(flood_response.path_trace[n].0, (HashSet::new(), 1.0, 1.0));
                    if flood_response.path_trace[n].1 == NodeType::Client {
                        if !self.client_list.contains(&flood_response.path_trace[n].0) {
                            self.client_list.insert(flood_response.path_trace[n].0);
                        }
                }
            }
            if n > 0 {
                if flood_response.path_trace[n - 1].1 != NodeType::Server {
                    if let Some(node) = self.topology.get_mut(&flood_response.path_trace[n].0) {
                        node.0.insert(flood_response.path_trace[n - 1].0);
                    }
                }
            }
            if n < flood_response.path_trace.len() -1 {
                if flood_response.path_trace[n + 1].1 != NodeType::Server {
                    if let Some(node) = self.topology.get_mut(&flood_response.path_trace[n].0) {
                        node.0.insert(flood_response.path_trace[n + 1].0);
                    }
                }
            }
        }

        self.generate_all_routes();
    }
    pub fn update_errors(&mut self) {
        self.n_errors += 1;
    }
    pub fn update_from_nack(&mut self, hops: &Vec<NodeId>, nack: Nack) {
        let nack_source = hops[0];
        for hop in hops.iter() {
            if self.server_id != *hop || *hop != nack_source {
                self.topology.get_mut(hop).unwrap().2 += 1.0;
                self.topology.get_mut(hop).unwrap().1 += 1.0;
            }
        }
        match nack.nack_type {
            NackType::DestinationIsDrone => {
                info!("Destination is drone detected");
                self.n_errors += 1;
            }
            NackType::Dropped => {
                info!("Dropped detected");
                self.topology.get_mut(&nack_source).unwrap().2 += 1.0;
                self.n_dropped += 1;
            }
            NackType::ErrorInRouting(node) => {
                info!("Error in routing detected: {}", node);
                self.n_errors += 1;
                self.remove_node(node);
                self.generate_all_routes();
            }
            NackType::UnexpectedRecipient(node) => {
                info!("Unexpected recipient detected: {}", node);
                self.n_errors += 1;
            }
        }
    }

    pub fn update_from_ack(&mut self, hops: &Vec<NodeId>) {
        for hop in hops.iter() {
            if !self.client_list.contains(hop) || self.server_id != *hop {
                self.topology.get_mut(hop).unwrap().2 += 1.0;
                self.topology.get_mut(hop).unwrap().1 += 1.0;
            }
        }

        info!("Ack arrived from {}", hops[0]);
    }
    pub fn update_routing_path(&mut self, routing_header: &mut SourceRoutingHeader) -> bool{
        if let Some(dest) = routing_header.destination(){
            if self.generate_specific_route(&dest) {
                routing_header.hops = self.get_route(&dest).unwrap();
                routing_header.hop_index = 0;
                return true
            }
        }
        else{
            warn!("no destination found");
        }
        false
    }
    pub fn remove_node(&mut self, node: NodeId) {
        self.topology.remove(&node);
        let keys: Vec<NodeId> = self.topology.keys().cloned().collect();
        for key in keys.iter() {
            self.topology.get_mut(key).unwrap().0.remove(&node);
        }

        self.client_list.remove(&node);
    }

    fn calculate_path(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut path = vec![];
        let mut psp = HashMap::new();
        let mut dist = HashMap::new();
        let mut prev = HashMap::new();
        let mut queue = VecDeque::new();
        let mut current_node;

        for node in self.topology.keys() {
            let prob = self.topology.get(node).unwrap().1 / self.topology.get(node).unwrap().2;
            psp.insert(*node, -prob.ln());
            dist.insert(*node, f64::MAX);
        }

        queue.push_back(self.server_id);

        dist.insert(self.server_id, *psp.get(&self.server_id).unwrap());

        while !queue.is_empty() {
            current_node = queue.pop_front().unwrap();

            for vec_node_id in self.topology.get(&current_node).unwrap().0.iter() {
                if dist.get(&current_node).unwrap() + psp.get(vec_node_id).unwrap()
                    < *dist.get(vec_node_id).unwrap()
                {
                    dist.insert(
                        *vec_node_id,
                        dist.get(&current_node).unwrap() + psp.get(vec_node_id).unwrap(),
                    );
                    prev.insert(*vec_node_id, current_node);
                    queue.push_back(*vec_node_id);
                }
            }
        }

        current_node = node_id;
        while current_node != self.server_id {
            path.push(current_node);
            current_node = *prev.get(&current_node).unwrap();
        }

        path.push(self.server_id);
        path.reverse();

        path
    }

    pub fn generate_all_routes(&mut self) {
        for node in self.client_list.iter() {
            self.routes.insert(*node, self.calculate_path(*node));
        }
        info!("Generated all routes to clients");
    }
    fn generate_specific_route(&mut self, node_id: &NodeId) -> bool{
        if self.client_list.contains(node_id) {
            self.routes.insert(*node_id, self.calculate_path(*node_id));
            info!("Generated route to {}", node_id);
            true
        }
        else{
            warn!("{} is not on client list, unable to create route", node_id);
            false
        }
    }
    pub fn should_flood_request(&mut self) -> bool {
        let elapsed = self.start_time.elapsed().unwrap_or(Duration::from_secs(0));

        let res = elapsed > self.flood_interval || self.n_errors == 7 || self.n_dropped == 5;
        self.start_time = SystemTime::now();
        self.n_errors = 0;
        self.n_dropped = 0;

        res
    }
    pub fn get_client_list(&self) -> Vec<NodeId> {
        self.client_list.iter().cloned().collect()
    }
    pub fn get_route(&self, dest: &NodeId) -> Option<Vec<NodeId>> {
        self.routes.get(dest).cloned()
    }
}
