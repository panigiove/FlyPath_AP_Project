use message::{NodeCommand, NodeEvent};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::HashMap;
use wg_2024::network::{NodeId};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use rand::{thread_rng, Rng};

// Import dei droni
use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
use bagel_bomber::BagelBomber;
use lockheedrustin_drone::LockheedRustin;
use rolling_drone::RollingDrone;
use rust_do_it::RustDoIt;
use rust_roveri::RustRoveri;
use rustastic_drone::RustasticDrone;
use rustbusters_drone::RustBustersDrone;
use LeDron_James::Drone as LeDronJames_drone;
use rusty_drones::RustyDrone;

use crate::utility::{ButtonEvent, GraphAction, NodeType, MessageType, DroneGroup};
use crate::utility::GraphAction::{AddEdge, AddNode, RemoveEdge, RemoveNode};
use crate::utility::MessageType::{Error, PacketSent};
use rand::seq::SliceRandom;
use wg_2024::drone::Drone;
use message::NodeCommand::FromShortcut;

use client;
use client::comunication::{FromUiCommunication, ToUICommunication};
use client::ui::{ClientState};
use client::worker::Worker;
use server::ChatServer;

// Tipi di errore personalizzati
#[derive(Debug)]
pub enum ControllerError {
    ChannelSend(String),
    NodeNotFound(NodeId),
    InvalidOperation(String),
    NetworkConstraintViolation(String),
}

impl std::fmt::Display for ControllerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ControllerError::ChannelSend(msg) => write!(f, "Channel send error: {}", msg),
            ControllerError::NodeNotFound(id) => write!(f, "Node not found: {}", id),
            ControllerError::InvalidOperation(msg) => write!(f, "Invalid operation: {}", msg),
            ControllerError::NetworkConstraintViolation(msg) => write!(f, "Network constraint violation: {}", msg),
        }
    }
}

impl std::error::Error for ControllerError {}

pub struct ControllerHandler {
    pub node_types: HashMap<NodeId, NodeType>,
    pub drones_types: HashMap<NodeId, DroneGroup>,
    
    pub packet_senders: HashMap<NodeId, Sender<Packet>>,
    pub connections: HashMap<NodeId, Vec<NodeId>>,
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    pub receiver_event: HashMap<NodeId, Receiver<DroneEvent>>,
    pub receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
    
    pub button_receiver: Receiver<ButtonEvent>,
    pub graph_action_sender: Sender<GraphAction>,
    pub message_sender: Sender<MessageType>,
    pub client_state_sender: Sender<(NodeId, ClientState)>,

    pub drones_counter: HashMap<DroneGroup, i8>,
}

impl ControllerHandler {
    pub fn new(
        node_types: HashMap<NodeId, NodeType>,
        drones_types: HashMap<NodeId, DroneGroup>,
        packet_senders: HashMap<NodeId, Sender<Packet>>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
        send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
        reciver_event: HashMap<NodeId, Receiver<DroneEvent>>,
        receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
        button_receiver: Receiver<ButtonEvent>,
        graph_action_sender: Sender<GraphAction>,
        message_sender: Sender<MessageType>,
        client_state_sender: Sender<(NodeId, ClientState)>,
        drones_counter: HashMap<DroneGroup, i8>,
    ) -> Self {

        Self {
            node_types,
            drones_types,
            packet_senders,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event: reciver_event,
            receriver_node_event,
            button_receiver,
            graph_action_sender,
            message_sender,
            drones_counter,
            client_state_sender,
        }
    }
    
    pub fn run(&mut self) {
        loop {
            let mut button_events_processed = 0;
            while let Ok(command) = self.button_receiver.try_recv() {
                self.handle_button_event(command);
                button_events_processed += 1;
                //in order to not have an infinite loop
                if button_events_processed >= 10 {
                    break;
                }
            }
            
            let drone_node_ids: Vec<NodeId> = self.receiver_event.keys().copied().collect();
            let mut drone_events_processed = 0;
            for node_id in drone_node_ids {
                let mut events_to_process = Vec::new();
                if let Some(receiver) = self.receiver_event.get(&node_id) {
                    for _ in 0..3 {
                        if let Ok(event) = receiver.try_recv() {
                            events_to_process.push(event);
                        } else {
                            break;
                        }
                    }
                }
                
                for event in events_to_process {
                    self.handle_drone_event(event, node_id);
                    drone_events_processed += 1;
                }

                //limit to the precessed events
                if drone_events_processed >= 20 {
                    break;
                }
            }
            
            let node_node_ids: Vec<NodeId> = self.receriver_node_event.keys().copied().collect();
            let mut node_events_processed = 0;
            for node_id in node_node_ids {
                let mut events_to_process = Vec::new();
                if let Some(receiver) = self.receriver_node_event.get(&node_id) {
                    for _ in 0..3 {
                        if let Ok(event) = receiver.try_recv() {
                            events_to_process.push(event);
                        } else {
                            break;
                        }
                    }
                }
                
                for event in events_to_process {
                    self.handle_node_event(event, node_id);
                    node_events_processed += 1;
                }
                
                if node_events_processed >= 20 {
                    break;
                }
            }

            //small pause to avoid intensive loop
            let total_events = button_events_processed + drone_events_processed + node_events_processed;
            if total_events == 0 {
                //longer pause
                std::thread::sleep(std::time::Duration::from_millis(1));
            } else {
                std::thread::yield_now();
            }
        }
    }

    // ================================ Event Handlers ================================
    
    pub fn handle_button_event(&mut self, event: ButtonEvent) {
        let result = match event {
            ButtonEvent::NewDrone(id, pdr) => {
                self.spawn_drone(&id, pdr)
            },
            ButtonEvent::NewServerWithTwoConnections(drone1, drone2) => {
                self.create_server_with_two_connections(drone1, drone2)
            },
            ButtonEvent::NewClient(id) => {
                self.create_client(id)
            },
            ButtonEvent::NewConnection(id1, id2) => {
                match self.add_connection(&id1, &id2) {
                    Err(ControllerError::InvalidOperation(msg)) if msg.contains("needs repair") => {
                        
                        if msg.contains(&format!("Server {} channel disconnected", id1)) {
                            if let Ok(()) = self.check_and_repair_node(&id1) {
                                self.add_connection(&id1, &id2)
                            } else {
                                Err(ControllerError::InvalidOperation(msg))
                            }
                        } else if msg.contains(&format!("Server {} channel disconnected", id2)) {
                            if let Ok(()) = self.check_and_repair_node(&id2) {
                                self.add_connection(&id1, &id2)
                            } else {
                                Err(ControllerError::InvalidOperation(msg))
                            }
                        } else {
                            Err(ControllerError::InvalidOperation(msg))
                        }
                    }
                    other => other
                }
            },
            ButtonEvent::Crash(id) => {
                self.smart_remove_drone(&id)
            },
            ButtonEvent::RemoveConection(id1, id2) => self.remove_connection(&id1, &id2),
            ButtonEvent::ChangePdr(id, pdr) => self.change_packet_drop_rate(&id, pdr),
        };

        if let Err(e) = result {
            self.send_error_message(&e.to_string());
        }
    }
    
    fn handle_node_event(&mut self, event: NodeEvent, node_id: NodeId) {
        match event {
            NodeEvent::PacketSent(c) => {
                let message = format!("The node ID {} has sent a packet {}", node_id, c);
                if let Err(_) = self.message_sender.try_send(MessageType::Info(message)){
                }
            },
            NodeEvent::CreateMessage(_c) => {
                let message = "Message created".to_string();
                if let Err(_) = self.message_sender.try_send(MessageType::Info(message)){
                }
            },
            NodeEvent::MessageRecv(_c) => {
                let message = "Message received".to_string();
                if let Err(_) = self.message_sender.try_send(MessageType::Info(message)){
                }
            },
            NodeEvent::ControllerShortcut(packet) => {
                if let Err(e) = self.send_packet_to_client(packet) {
                    self.send_error_message(&format!("Failed to send shortcut packet: {}", e));
                }
            }
        }
    }
    
    pub fn handle_drone_event(&mut self, event: DroneEvent, drone_id: NodeId) {
        match event {
            DroneEvent::PacketSent(packet) => {
                let msg = format!("Drone [{}] successfully sent packet with session ID [{}]",
                                  drone_id, packet.session_id);
                self.send_success_message(&msg);
            }
            DroneEvent::PacketDropped(packet) => {
                let msg = format!("Drone [{}] dropped packet with session ID [{}]",
                                  drone_id, packet.session_id);
                self.send_info_message(&msg);
            }
            DroneEvent::ControllerShortcut(packet) => {
                if let Err(e) = self.send_packet_to_client(packet) {
                    self.send_error_message(&format!("Failed to send shortcut packet: {}", e));
                }
            }
        }
    }

    // ================================ Node Health Management ================================
    
    //checks drone status
    fn is_node_healthy(&self, id: &NodeId) -> bool {
        if self.is_drone(id) {
            if let Some(sender) = self.send_command_drone.get(id) {
                // try to send a dummy command to check if channel is alive
                match sender.try_send(DroneCommand::SetPacketDropRate(0.0)) {
                    Ok(()) => true,
                    Err(crossbeam_channel::TrySendError::Full(_)) => true, //channel full but alive
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => false, //dead
                }
            } else {
                false
            }
        } else {
            if let Some(sender) = self.send_command_node.get(id) {
                // try to send a dummy command to check if channel is alive
                match sender.try_send(NodeCommand::RemoveSender(255)) {
                    Ok(()) => true,
                    Err(crossbeam_channel::TrySendError::Full(_)) => true, // channel full but alive
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => false, // dead
                }
            } else {
                false
            }
        }
    }

    // ✅ NEW: Check and repair corrupted nodes
    pub fn check_and_repair_node(&mut self, id: &NodeId) -> Result<(), ControllerError> {

        if !self.is_node_healthy(id) {

            match self.get_node_type(id) {
                Some(NodeType::Server) => {
                    self.repair_corrupted_server(id)
                }
                Some(NodeType::Client) => {
                    Err(ControllerError::InvalidOperation(
                        format!("Client {} is corrupted and cannot be automatically repaired", id)
                    ))
                }
                Some(NodeType::Drone) => {
                    Err(ControllerError::InvalidOperation(
                        format!("Drone {} is corrupted and cannot be automatically repaired", id)
                    ))
                }
                None => {
                    Err(ControllerError::NodeNotFound(*id))
                }
            }
        } else {
            Ok(())
        }
    }

    // ✅ NEW: Repair corrupted server by recreating it
    fn repair_corrupted_server(&mut self, server_id: &NodeId) -> Result<(), ControllerError> {
        if !matches!(self.get_node_type(server_id), Some(NodeType::Server)) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a server", server_id)
            ));
        }

        // Save current connections
        let current_connections = self.connections.get(server_id).cloned().unwrap_or_default();

        if current_connections.len() < 2 {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Server {} has insufficient connections to repair", server_id)
            ));
        }

        // Remove the corrupted server completely
        self.complete_cleanup_server(server_id);
        let _ = self.send_graph_update(RemoveNode(*server_id));

        // Recreate the server with the same ID
        if let Err(e) = self.recreate_server_with_id(*server_id) {
            return Err(e);
        }

        // Re-add all connections
        for &neighbor_id in &current_connections {
            if let Err(e) = self.add_connection_internal(server_id, &neighbor_id) {
                // println!("⚠️ Failed to restore connection {} -> {}: {}", server_id, neighbor_id, e);
            } else {
                // println!("✅ Restored connection {} -> {}", server_id, neighbor_id);
            }
        }

        self.send_success_message(&format!("Server {} has been repaired", server_id));

        Ok(())
    }
    fn recreate_server_with_id(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();

        // ✅ INSERT DATA STRUCTURES FIRST
        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send.clone());
        self.receriver_node_event.insert(id, node_event_receiver);
        self.node_types.insert(id, NodeType::Server);

        // ✅ Spawn the thread
        std::thread::spawn(move || {
            let packet_send = HashMap::new();
            let mut chat_server = ChatServer::new(
                id,
                node_event_send,
                node_command_receiver,
                p_receiver,
                packet_send
            );
            chat_server.run();
        });

        for attempt in 0..10 {
            std::thread::sleep(std::time::Duration::from_millis(50));

            match node_command_send.try_send(NodeCommand::RemoveSender(255)) {
                Ok(()) => {
                    let _ = self.send_graph_update(AddNode(id, NodeType::Server));
                    return Ok(());
                }
                Err(crossbeam_channel::TrySendError::Full(_)) => {
                    let _ = self.send_graph_update(AddNode(id, NodeType::Server));
                    return Ok(());
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    continue;
                }
            }
        }

        Err(ControllerError::InvalidOperation(
            format!("Failed to recreate server {} within timeout", id)
        ))
    }

    // ================================ Drone Operations ================================

    fn spawn_drone(&mut self, first_connection: &NodeId, pdr: f32) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;

        // Validazione: Verifica che il nodo di connessione esista
        if !self.packet_senders.contains_key(first_connection) {
            return Err(ControllerError::NodeNotFound(*first_connection));
        }

        if !self.check_network_before_add_drone(&id, &vec![*first_connection]) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add drone {} connected to {}", id, first_connection)
            ));
        }

        // Update node_types PRIMA di tutto
        self.node_types.insert(id, NodeType::Drone);

        // Crea il drone SENZA connessioni iniziali
        if let Err(e) = self.create_drone_without_connection(id, pdr) {
            self.node_types.remove(&id);
            return Err(e);
        }

        // ORDINE CORRETTO: Prima AddNode, poi AddEdge
        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Drone)) {
            self.cleanup_drone(&id);
            return Err(e);
        }

        // Usa add_connection che invierà AddEdge
        if let Err(e) = self.add_connection(&id, first_connection) {
            let _ = self.send_graph_update(RemoveNode(id));
            self.cleanup_drone(&id);
            return Err(e);
        }

        self.send_success_message(&format!("Drone {} created successfully", id));
        Ok(())
    }

    fn create_drone_without_connection(&mut self, id: NodeId, pdr: f32) -> Result<(), ControllerError> {
        let (sender_event, receiver_event) = unbounded::<DroneEvent>();
        let (sender_drone_command, receiver_drone_command) = unbounded::<DroneCommand>();
        let (sender_packet, receiver_packet) = unbounded::<Packet>();

        let senders = HashMap::new(); // Vuoto inizialmente
        let drone_group = self.create_balanced_drone(
            id, sender_event, receiver_drone_command, receiver_packet, senders, pdr
        )?;

        // Insert all drone-related data
        self.drones_types.insert(id, drone_group);
        self.packet_senders.insert(id, sender_packet);
        self.connections.insert(id, Vec::new()); // Vuoto inizialmente
        self.send_command_drone.insert(id, sender_drone_command.clone());
        self.receiver_event.insert(id, receiver_event);

        std::thread::sleep(std::time::Duration::from_millis(100));

        match sender_drone_command.try_send(DroneCommand::SetPacketDropRate(pdr)) {
            Ok(()) => {
            }
            Err(_) => {
                self.cleanup_drone(&id);
                return Err(ControllerError::InvalidOperation(
                    format!("Drone {} failed to initialize properly", id)
                ));
            }
        }

        Ok(())
    }

    fn create_balanced_drone(
        &mut self,
        id: NodeId,
        sender_event: Sender<DroneEvent>,
        receiver_command: Receiver<DroneCommand>,
        receiver_packet: Receiver<Packet>,
        packet_sender: HashMap<NodeId, Sender<Packet>>,
        drop_rate: f32,
    ) -> Result<DroneGroup, ControllerError> {
        let drone_group = self.select_drone_group()
            .ok_or_else(|| ControllerError::InvalidOperation("No drone group available".to_string()))?
            .clone();

        let mut drone: Box<dyn Drone> = match drone_group {
            DroneGroup::RustInPeace => Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::BagelBomber => Box::new(BagelBomber::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::LockheedRustin => Box::new(LockheedRustin::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::RollingDrone => Box::new(RollingDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::RustDoIt => Box::new(RustDoIt::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::RustRoveri => Box::new(RustRoveri::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::Rustastic => Box::new(RustasticDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::RustBusters => Box::new(RustBustersDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::LeDronJames => Box::new(LeDronJames_drone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
            DroneGroup::RustyDrones => Box::new(RustyDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)),
        };

        *self.drones_counter.entry(drone_group).or_insert(0) += 1;

        std::thread::spawn(move || {
            drone.run();
        });

        Ok(drone_group)
    }

    pub(crate) fn select_drone_group(&self) -> Option<&DroneGroup> {

        if self.drones_counter.is_empty() {
            return None;
        }

        let min_value = self.drones_counter.values().min()?;

        let mut candidates: Vec<&DroneGroup> = self.drones_counter
            .iter()
            .filter_map(|(group, &count)| {
                if count == *min_value {
                    Some(group)
                } else {
                    None
                }
            })
            .collect();

        candidates.shuffle(&mut thread_rng());
        let selected = candidates.into_iter().next();

        selected
    }

    // ✅ IMPROVED: Crash drone with better constraint checking
    fn crash_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }

        self.debug_drone_removal_impact(id);

        if !self.check_network_before_removing_drone(id) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot remove drone {} - would violate network constraints", id)
            ));
        }

        self.remove_drone(id)?;

        // Remove from node_types
        self.node_types.remove(id);

        self.send_graph_update(RemoveNode(*id))?;
        self.send_success_message(&format!("Drone {} removed successfully", id));

        Ok(())
    }

    // ✅ NEW: Debug method to show removal impact
    fn debug_drone_removal_impact(&self, drone_id: &NodeId) {

        if let Some(connections) = self.connections.get(drone_id) {
            for &neighbor_id in connections {
                if let Some(neighbor_connections) = self.connections.get(&neighbor_id) {
                    let remaining_connections = neighbor_connections.len() - 1; // -1 because we're removing this drone
                    let node_type = self.get_node_type(&neighbor_id).unwrap_or(&NodeType::Drone);

                    // println!("📊 Node {} ({:?}) will have {} connections remaining",
                    //          neighbor_id, node_type, remaining_connections);

                    match node_type {
                        NodeType::Client => {
                            if remaining_connections == 0 {
                                // println!("❌ Client {} would become isolated!", neighbor_id);
                            } else if remaining_connections > 2 {
                                // println!("❌ Client {} would have too many connections ({})!", neighbor_id, remaining_connections);
                            } else {
                                // println!("✅ Client {} constraints OK", neighbor_id);
                            }
                        }
                        NodeType::Server => {
                            if remaining_connections < 2 {
                                // println!("❌ Server {} would have insufficient connections ({})!", neighbor_id, remaining_connections);
                            } else {
                                // println!("✅ Server {} constraints OK", neighbor_id);
                            }
                        }
                        NodeType::Drone => {
                            // println!("✅ Drone {} no constraints", neighbor_id);
                        }
                    }
                }
            }
        }
    }

    pub fn smart_remove_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }
        match self.crash_drone(id) {
            Ok(()) => {
                // println!("✅ Normal removal successful");
                return Ok(());
            }
            Err(ControllerError::NetworkConstraintViolation(msg)) => {
                // println!("⚠️ Normal removal failed: {}", msg);

                // ✅ NEW: Check if connectivity is the only issue
                if msg.contains("would become disconnected") {
                    // println!("🔧 Connectivity issue detected, checking if it's a false positive...");

                    // Try a more thorough connectivity check
                    if self.try_advanced_connectivity_check(id) {
                        // println!("✅ Advanced check confirms network remains connected, forcing removal...");
                        return self.force_remove_drone_bypass_connectivity(id);
                    }
                }

                // Check if this is a newly added drone with minimal connections
                if let Some(connections) = self.connections.get(id) {
                    if connections.len() <= 2 {  // ✅ Increased threshold from 1 to 2
                        // println!("🔧 Trying force removal for minimally connected drone (≤2 connections)...");
                        return self.force_remove_drone(id);
                    }
                }

                return Err(ControllerError::NetworkConstraintViolation(
                    format!("Cannot remove drone {}: {}. Try removing other nodes first.", id, msg)
                ));
            }
            Err(e) => return Err(e),
        }
    }

    // ✅ NEW: Advanced connectivity check with multiple algorithms
    fn try_advanced_connectivity_check(&self, drone_id: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();
        adj_list.remove(drone_id);

        // Remove drone from all neighbor connection lists
        for neighbors in adj_list.values_mut() {
            neighbors.retain(|&id| &id != drone_id);
        }

        // Method 1: DFS from multiple starting points
        let nodes: Vec<NodeId> = adj_list.keys().copied().collect();
        if nodes.is_empty() {
            return true;
        }

        // Try DFS from the first 3 nodes to be sure
        let test_nodes = nodes.iter().take(3).collect::<Vec<_>>();
        for &start_node in test_nodes {
            let reachable = count_reachable_nodes(start_node, &adj_list);

            if reachable == nodes.len() {
                return true;
            }
        }

        // Method 2: Check if any isolated nodes exist
        let isolated_nodes: Vec<NodeId> = adj_list.iter()
            .filter(|(_, connections)| connections.is_empty())
            .map(|(&id, _)| id)
            .collect();

        if !isolated_nodes.is_empty() {
            return false;
        }

        // Method 3: Component analysis
        let components = find_connected_components(&adj_list);

        if components.len() == 1 {
            return true;
        } else {
            return false;
        }
    }

    // ✅ IMPROVED: More permissive drone removal for isolated drones
    pub fn force_remove_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }

        // Check if this drone has any connections that would violate constraints
        if let Some(connections) = self.connections.get(id) {
            if connections.is_empty() {
            } else {
                // Check if any connected node would be left in an invalid state
                let mut violations = Vec::new();

                for &neighbor_id in connections {
                    if let Some(neighbor_connections) = self.connections.get(&neighbor_id) {
                        let remaining_connections = neighbor_connections.len() - 1;
                        let node_type = self.get_node_type(&neighbor_id).unwrap_or(&NodeType::Drone);

                        match node_type {
                            NodeType::Client => {
                                if remaining_connections == 0 {
                                    violations.push(format!("Client {} would become isolated", neighbor_id));
                                }
                            }
                            NodeType::Server => {
                                if remaining_connections < 2 {
                                    violations.push(format!("Server {} would have only {} connections", neighbor_id, remaining_connections));
                                }
                            }
                            NodeType::Drone => {
                                // Drones have no constraints
                            }
                        }
                    }
                }

                if !violations.is_empty() {
                    return Err(ControllerError::NetworkConstraintViolation(
                        format!("Cannot force remove drone {}: {}", id, violations.join(", "))
                    ));
                }
            }
        }

        // Proceed with removal
        self.remove_drone(id)?;
        self.node_types.remove(id);
        self.send_graph_update(RemoveNode(*id))?;
        self.send_success_message(&format!("Drone {} force removed successfully", id));

        Ok(())
    }

    // ✅ NEW: Force remove drone bypassing connectivity (but not constraint) checks
    pub fn force_remove_drone_bypass_connectivity(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }

        // Still check basic constraints (client/server requirements)
        if let Some(connections) = self.connections.get(id) {
            let mut violations = Vec::new();

            for &neighbor_id in connections {
                if let Some(neighbor_connections) = self.connections.get(&neighbor_id) {
                    let remaining_connections = neighbor_connections.len() - 1;
                    let node_type = self.get_node_type(&neighbor_id).unwrap_or(&NodeType::Drone);

                    match node_type {
                        NodeType::Client => {
                            if remaining_connections == 0 {
                                violations.push(format!("Client {} would become isolated", neighbor_id));
                            }
                        }
                        NodeType::Server => {
                            if remaining_connections < 2 {
                                violations.push(format!("Server {} would have only {} connections", neighbor_id, remaining_connections));
                            }
                        }
                        NodeType::Drone => {
                            // Drones have no constraints
                        }
                    }
                }
            }

            if !violations.is_empty() {
                return Err(ControllerError::NetworkConstraintViolation(
                    format!("Cannot force remove drone {}: {}", id, violations.join(", "))
                ));
            }
        }

        // Proceed with removal
        self.remove_drone(id)?;
        self.node_types.remove(id);
        self.send_graph_update(RemoveNode(*id))?;
        self.send_success_message(&format!("Drone {} force removed successfully (connectivity bypassed)", id));

        Ok(())
    }

    fn remove_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        // Send crash command to drone
        if let Some(sender) = self.send_command_drone.get(id) {
            sender.send(DroneCommand::Crash)
                .map_err(|e| ControllerError::ChannelSend(format!("Failed to send crash command to drone {}: {}", id, e)))?;
        }

        // Remove all connections
        self.remove_all_connections(id)?;

        // Clean up data structures
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_drone.remove(id);
        self.receiver_event.remove(id);

        // Update counter
        if let Some(drone_group) = self.drones_types.remove(id) {
            if let Some(count) = self.drones_counter.get_mut(&drone_group) {
                *count -= 1;
            }
        }

        Ok(())
    }

    // ================================ Client/Server Operations ================================

    fn create_client(&mut self, id_connection: NodeId) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;

        if !self.is_drone(&id_connection) {
            return Err(ControllerError::InvalidOperation(
                "Client must connect to a drone".to_string()
            ));
        }

        // Crea il client SENZA connessioni iniziali
        if let Err(e) = self.create_client_without_connection(id) {
            return Err(e);
        }

        // Update node_types
        self.node_types.insert(id, NodeType::Client);

        // ORDINE CORRETTO: Prima AddNode, poi AddEdge
        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Client)) {
            self.cleanup_client(&id);
            return Err(e);
        }

        // USA add_connection che invierà AddEdge
        if let Err(e) = self.add_connection(&id, &id_connection) {
            let _ = self.send_graph_update(RemoveNode(id));
            self.cleanup_client(&id);
            return Err(e);
        }

        self.send_success_message("New client added successfully");
        Ok(())
    }

    // ✅ IMPROVED: Create client with robust initialization
    fn create_client_without_connection(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();
        let (ui_comunication_send, ui_comunication_receiver) = unbounded::<ToUICommunication>();
        let (from_ui_comunication_send, from_ui_comunication_receiver) = unbounded::<FromUiCommunication>();

        let client_state = ClientState::new(id, ui_comunication_receiver, from_ui_comunication_send);
        if let Err(_e) = self.client_state_sender.try_send((id, client_state)) {
            return Err(ControllerError::InvalidOperation(
                "Client can't be added to the ui".to_string()
            ));
        }

        // ✅ INSERT DATA STRUCTURES FIRST (before spawning thread)
        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send.clone());
        self.receriver_node_event.insert(id, node_event_receiver);

        // ✅ Now spawn the thread
        std::thread::spawn(move || {
            let packet_send = HashMap::new();
            let mut worker = Worker::new(
                id,
                packet_send,
                node_event_send,
                ui_comunication_send,
                p_receiver,
                node_command_receiver,
                from_ui_comunication_receiver
            );
            worker.run();
        });

        // ✅ VERIFY CLIENT IS READY with timeout and retry
        let mut ready = false;
        for attempt in 0..10 { // Max 10 attempts (500ms total)
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Try to send a dummy command to verify the client is responsive
            match node_command_send.try_send(NodeCommand::RemoveSender(255)) {
                Ok(()) => {
                    ready = true;
                    break;
                }
                Err(crossbeam_channel::TrySendError::Full(_)) => {
                    // Channel full but connected - client is ready
                    ready = true;
                    break;
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    // Client not ready yet, continue trying
                    continue;
                }
            }
        }

        if !ready {
            self.cleanup_client(&id);
            return Err(ControllerError::InvalidOperation(
                format!("Client {} failed to initialize within timeout", id)
            ));
        }

        Ok(())
    }

    // ✅ IMPROVED: Create server with two connections - Better error handling
    fn create_server_with_two_connections(&mut self, drone1: NodeId, drone2: NodeId) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;

        // Verifica che entrambi siano droni
        if !self.is_drone(&drone1) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", drone1)
            ));
        }
        if !self.is_drone(&drone2) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", drone2)
            ));
        }

        // Verifica che i droni esistano
        if !self.packet_senders.contains_key(&drone1) {
            return Err(ControllerError::NodeNotFound(drone1));
        }
        if !self.packet_senders.contains_key(&drone2) {
            return Err(ControllerError::NodeNotFound(drone2));
        }

        if !self.check_network_before_add_server_with_two_connections(&id, &drone1, &drone2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add server {} with connections to {} and {}", id, drone1, drone2)
            ));
        }

        if let Err(e) = self.create_server_without_connection(id) {
            return Err(e);
        }

        self.node_types.insert(id, NodeType::Server);

        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Server)) {
            self.complete_cleanup_server(&id);
            return Err(e);
        }

        match self.add_server_connections_atomic(&id, &drone1, &drone2) {
            Ok(()) => {
                self.send_success_message(&format!("Server {} created with connections to drones {} and {}", id, drone1, drone2));
                Ok(())
            }
            Err(e) => {
                let _ = self.send_graph_update(RemoveNode(id));
                self.complete_cleanup_server(&id);
                Err(e)
            }
        }
    }

    // ✅ IMPROVED: Create server with robust initialization
    fn create_server_without_connection(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();

        // ✅ INSERT DATA STRUCTURES FIRST (before spawning thread)
        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send.clone());
        self.receriver_node_event.insert(id, node_event_receiver);

        // ✅ Now spawn the thread
        std::thread::spawn(move || {
            let packet_send = HashMap::new();
            let mut chat_server = ChatServer::new(
                id,
                node_event_send,
                node_command_receiver,
                p_receiver,
                packet_send
            );

            chat_server.run();
        });

        // ✅ VERIFY SERVER IS READY with timeout and retry
        let mut ready = false;
        for attempt in 0..10 { // Max 10 attempts (500ms total)
            std::thread::sleep(std::time::Duration::from_millis(50));

            // Try to send a dummy command to verify the server is responsive
            match node_command_send.try_send(NodeCommand::RemoveSender(255)) {
                Ok(()) => {
                    ready = true;
                    break;
                }
                Err(crossbeam_channel::TrySendError::Full(_)) => {
                    // Channel full but connected - server is ready
                    ready = true;
                    break;
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    // Server not ready yet, continue trying
                    continue;
                }
            }
        }

        if !ready {
            self.cleanup_server(&id);
            return Err(ControllerError::InvalidOperation(
                format!("Server {} failed to initialize within timeout", id)
            ));
        }

        Ok(())
    }

    // ================================ Connection Management ================================

    // ✅ IMPROVED: Add connection with health checks and auto-repair
    fn add_connection(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {

        // ✅ NEW: Check if nodes are healthy
        if !self.is_node_healthy(id1) {
            if matches!(self.get_node_type(id1), Some(NodeType::Server)) {
                if let Err(e) = self.repair_corrupted_server(id1) {
                    return Err(ControllerError::InvalidOperation(
                        format!("Failed to repair server {}: {}", id1, e)
                    ));
                }
            } else {
                return Err(ControllerError::InvalidOperation(
                    format!("Node {} is unhealthy and cannot be repaired automatically", id1)
                ));
            }
        }

        if !self.is_node_healthy(id2) {
            if matches!(self.get_node_type(id2), Some(NodeType::Server)) {
                if let Err(e) = self.repair_corrupted_server(id2) {
                    return Err(ControllerError::InvalidOperation(
                        format!("Failed to repair server {}: {}", id2, e)
                    ));
                }
            } else {
                return Err(ControllerError::InvalidOperation(
                    format!("Node {} is unhealthy and cannot be repaired automatically", id2)
                ));
            }
        }

        let mut id1_ready = false;
        let mut id2_ready = false;

        // Check both nodes with multiple attempts
        for attempt in 0..5 {
            if !id1_ready {
                id1_ready = self.packet_senders.contains_key(id1) &&
                    (self.send_command_drone.contains_key(id1) || self.send_command_node.contains_key(id1));
            }

            if !id2_ready {
                id2_ready = self.packet_senders.contains_key(id2) &&
                    (self.send_command_drone.contains_key(id2) || self.send_command_node.contains_key(id2));
            }

            if id1_ready && id2_ready {
                break;
            }

            if attempt < 4 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Final verification with detailed error reporting
        if !id1_ready {
            return Err(ControllerError::NodeNotFound(*id1));
        }
        if !id2_ready {
            return Err(ControllerError::NodeNotFound(*id2));
        }

        if !self.check_network_before_add_connection(id1, id2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add connection between {} and {}", id1, id2)
            ));
        }

        // ✅ ADD SENDERS WITH IMPROVED ERROR HANDLING
        if let Err(e) = self.add_sender_safe(id1, id2) {
            return Err(e);
        }

        if let Err(e) = self.add_sender_safe(id2, id1) {
            // Rollback first sender if second fails
            let _ = self.remove_sender(id1, id2);
            return Err(e);
        }

        // ✅ UPDATE CONNECTIONS
        if let Some(connections) = self.connections.get_mut(id1) {
            if !connections.contains(id2) {
                connections.push(*id2);
            }
        }
        if let Some(connections) = self.connections.get_mut(id2) {
            if !connections.contains(id1) {
                connections.push(*id1);
            }
        }

        // ✅ SEND GRAPH UPDATE
        self.send_graph_update(AddEdge(*id1, *id2))?;
        self.send_success_message(&format!("Connection added between {} and {}", id1, id2));

        Ok(())
    }

    // ✅ NEW: Internal add connection without health checks (for repair)
    fn add_connection_internal(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {
        // ✅ ADD SENDERS
        self.add_sender(id1, id2)?;
        self.add_sender(id2, id1)?;

        // ✅ UPDATE CONNECTIONS
        if let Some(connections) = self.connections.get_mut(id1) {
            if !connections.contains(id2) {
                connections.push(*id2);
            }
        }
        if let Some(connections) = self.connections.get_mut(id2) {
            if !connections.contains(id1) {
                connections.push(*id1);
            }
        }

        // ✅ SEND GRAPH UPDATE
        self.send_graph_update(AddEdge(*id1, *id2))?;

        Ok(())
    }

    fn remove_connection(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {
        if !self.check_network_before_remove_connection(id1, id2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot remove connection between {} and {}", id1, id2)
            ));
        }

        self.remove_sender(id1, id2)?;
        self.remove_sender(id2, id1)?;

        // Update connections
        if let Some(connections) = self.connections.get_mut(id1) {
            connections.retain(|&id| &id != id2);
        }
        if let Some(connections) = self.connections.get_mut(id2) {
            connections.retain(|&id| &id != id1);
        }

        self.send_graph_update(RemoveEdge(*id1, *id2))?;
        self.send_success_message(&format!("Connection removed between {} and {}", id1, id2));

        Ok(())
    }

    // ================================ Helper Methods ================================

    // ✅ NEW: Safe add sender with better error handling and recovery
    fn add_sender_safe(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {
        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            for attempt in 0..5 {
                match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Drone {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to drone {} after 5 attempts", id)
            ));

        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            for attempt in 0..5 {
                match sender.try_send(NodeCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            // ✅ NEW: If it's a server, try to repair it
                            if matches!(self.get_node_type(id), Some(NodeType::Server)) {

                                // Note: We can't repair here as it would require mutable self again
                                // Just return a specific error for the calling function to handle
                                return Err(ControllerError::InvalidOperation(
                                    format!("Server {} channel disconnected - needs repair", id)
                                ));
                            }

                            return Err(ControllerError::ChannelSend(
                                format!("Node {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to node {} after 5 attempts", id)
            ));
        }
    }

    // ✅ IMPROVED: Add sender with better error handling
    fn add_sender(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {
        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            // ✅ IMPROVED: Multiple retry attempts for drone commands
            for attempt in 0..3 {
                match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 2 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Drone {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to drone {} after 3 attempts", id)
            ));

        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            // ✅ IMPROVED: Retry for node commands too
            for attempt in 0..3 {
                match sender.try_send(NodeCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 2 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Node {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to node {} after 3 attempts", id)
            ));
        }
    }

    fn remove_sender(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {
        if self.is_drone(id) {
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;
            sender.send(DroneCommand::RemoveSender(*dst_id))
                .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;
            sender.send(NodeCommand::RemoveSender(*dst_id))
                .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
        }

        Ok(())
    }

    fn remove_all_connections(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        let connections = self.connections.get(id).cloned().unwrap_or_default();

        for neighbor_id in connections {
            self.remove_sender(id, &neighbor_id)?;
        }

        Ok(())
    }

    pub(crate) fn change_packet_drop_rate(&mut self, id: &NodeId, new_pdr: f32) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }

        let sender = self.send_command_drone.get(id)
            .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

        sender.send(DroneCommand::SetPacketDropRate(new_pdr))
            .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;

        self.send_success_message(&format!("PDR for drone {} changed to {}", id, new_pdr));
        Ok(())
    }

    // ✅ FIX: send_packet_to_client gestisce sia Client che Server
    pub(crate) fn send_packet_to_client(&self, packet: Packet) -> Result<(), ControllerError> {
        // ✅ FIX: Salva i valori prima di spostare packet
        let _ = packet.session_id;
        let destination = packet.routing_header.hops.last().copied();


        if let Some(destination) = destination {

            match self.get_node_type(&destination) {
                Some(NodeType::Client) => {
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))  // ✅ packet moved qui
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                }
                Some(NodeType::Server) => {
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))  // ✅ packet moved qui
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                }
                Some(NodeType::Drone) => {
                    return Err(ControllerError::InvalidOperation(
                        format!("Cannot send shortcut packet to drone {}", destination)
                    ));
                }
                None => {
                    return Err(ControllerError::NodeNotFound(destination));
                }
            }
        } else {
            return Err(ControllerError::InvalidOperation(
                "Packet has no destination".to_string()
            ));
        }

        Ok(())
    }

    // ================================ Atomic Server Creation Helpers ================================

    // ✅ NUOVO: Validazione per server con 2 connessioni
    fn check_network_before_add_server_with_two_connections(&self, server_id: &NodeId, drone1: &NodeId, drone2: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();

        // Simula l'aggiunta del server con entrambe le connessioni
        adj_list.insert(*server_id, vec![*drone1, *drone2]);

        // Aggiorna le connessioni dei droni
        if let Some(drone1_connections) = adj_list.get_mut(drone1) {
            drone1_connections.push(*server_id);
        }
        if let Some(drone2_connections) = adj_list.get_mut(drone2) {
            drone2_connections.push(*server_id);
        }

        // Valida la rete risultante
        self.validate_network_constraints(&adj_list)
    }

    // ✅ IMPROVED: Add server connections with better error reporting
    fn add_server_connections_atomic(&mut self, server_id: &NodeId, drone1: &NodeId, drone2: &NodeId) -> Result<(), ControllerError> {
        // ✅ VERIFY ALL NODES EXIST BEFORE STARTING
        if !self.packet_senders.contains_key(server_id) {
            return Err(ControllerError::NodeNotFound(*server_id));
        }
        if !self.packet_senders.contains_key(drone1) {
            return Err(ControllerError::NodeNotFound(*drone1));
        }
        if !self.packet_senders.contains_key(drone2) {
            return Err(ControllerError::NodeNotFound(*drone2));
        }

        // ✅ STEP 1: Add senders for server -> drones
        if let Err(e) = self.add_sender_without_validation(server_id, drone1) {
            return Err(e);
        }

        if let Err(e) = self.add_sender_without_validation(server_id, drone2) {
            // Rollback: remove first sender
            let _ = self.remove_sender(server_id, drone1);
            return Err(e);
        }

        // ✅ STEP 2: Add senders for drones -> server
        if let Err(e) = self.add_sender_without_validation(drone1, server_id) {
            let _ = self.remove_sender(server_id, drone1);
            let _ = self.remove_sender(server_id, drone2);
            return Err(e);
        }

        if let Err(e) = self.add_sender_without_validation(drone2, server_id) {
            let _ = self.remove_sender(server_id, drone1);
            let _ = self.remove_sender(server_id, drone2);
            let _ = self.remove_sender(drone1, server_id);
            return Err(e);
        }

        if let Some(server_connections) = self.connections.get_mut(server_id) {
            server_connections.clear(); // Clear any existing connections
            server_connections.push(*drone1);
            server_connections.push(*drone2);
        }
        if let Some(drone1_connections) = self.connections.get_mut(drone1) {
            if !drone1_connections.contains(server_id) {
                drone1_connections.push(*server_id);
            }
        }
        if let Some(drone2_connections) = self.connections.get_mut(drone2) {
            if !drone2_connections.contains(server_id) {
                drone2_connections.push(*server_id);
            }
        }

        // ✅ STEP 4: Send AddEdge events
        if let Err(e) = self.send_graph_update(AddEdge(*server_id, *drone1)) {
            return Err(e);
        }

        if let Err(e) = self.send_graph_update(AddEdge(*server_id, *drone2)) {
            return Err(e);
        }

        Ok(())
    }

    fn add_sender_without_validation(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {

        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            // Multiple attempts for drone commands
            for attempt in 0..5 {
                match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Drone {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to drone {} after 5 attempts", id)
            ));

        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            // Multiple attempts for node commands
            for attempt in 0..5 {
                match sender.try_send(NodeCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Node {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            return Err(ControllerError::ChannelSend(
                format!("Failed to send command to node {} after 5 attempts", id)
            ));
        }
    }

    // ================================ Cleanup Methods ================================

    fn cleanup_drone(&mut self, id: &NodeId) {
        // Send crash command to drone
        if let Some(sender) = self.send_command_drone.get(id) {
            let _ = sender.send(DroneCommand::Crash);
        }

        // Remove from all data structures
        self.node_types.remove(id);
        if let Some(drone_group) = self.drones_types.remove(id) {
            if let Some(count) = self.drones_counter.get_mut(&drone_group) {
                *count = (*count - 1).max(0); // Evita valori negativi
            }
        }
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_drone.remove(id);
        self.receiver_event.remove(id);
    }

    fn cleanup_client(&mut self, id: &NodeId) {
        self.node_types.remove(id);
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receriver_node_event.remove(id);
    }

    fn cleanup_server(&mut self, id: &NodeId) {
        self.node_types.remove(id);
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receriver_node_event.remove(id);
    }

    // ✅ NEW: Complete cleanup method that removes ALL traces of a server
    fn complete_cleanup_server(&mut self, id: &NodeId) {

        // Remove from node_types FIRST
        self.node_types.remove(id);

        // Send crash command if channel exists
        if let Some(sender) = self.send_command_node.get(id) {
            let _ = sender.try_send(NodeCommand::RemoveSender(255)); // Dummy command to wake up thread
        }

        // Remove all connections BEFORE removing from data structures
        let connections = self.connections.get(id).cloned().unwrap_or_default();
        for neighbor_id in connections {
            // Remove this server from neighbor's connections
            if let Some(neighbor_connections) = self.connections.get_mut(&neighbor_id) {
                neighbor_connections.retain(|&conn_id| &conn_id != id);
            }

            // Remove sender from neighbor to this server
            let _ = self.remove_sender(&neighbor_id, id);
        }

        // Remove from all data structures
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receriver_node_event.remove(id);
    }

    // ================================ Node Type Checkers ================================

    pub(crate) fn is_drone(&self, id: &NodeId) -> bool {
        self.node_types.get(id) == Some(&NodeType::Drone)
    }

    pub(crate) fn get_node_type(&self, id: &NodeId) -> Option<&NodeType> {
        self.node_types.get(id)
    }

    // ================================ Utility Methods ================================

    pub(crate) fn generate_random_id(&self) -> Result<NodeId, ControllerError> {
        let available_ids: Vec<NodeId> = (0..=255)
            .filter(|id| !self.packet_senders.contains_key(id))
            .collect();

        if available_ids.is_empty() {
            return Err(ControllerError::InvalidOperation("No available IDs".to_string()));
        }

        let random_index = thread_rng().gen_range(0..available_ids.len());
        Ok(available_ids[random_index])
    }

    // ================================ Network Validation ================================

    fn check_network_before_add_drone(&self, drone_id: &NodeId, connections: &[NodeId]) -> bool {
        let mut adj_list = self.connections.clone();
        adj_list.insert(*drone_id, connections.to_vec());

        for neighbor in connections {
            if let Some(neighbors) = adj_list.get_mut(neighbor) {
                neighbors.push(*drone_id);
            }
        }

        self.validate_network_constraints(&adj_list)
    }

    // ✅ IMPROVED: Better network constraint validation with connectivity fix
    fn check_network_before_removing_drone(&self, drone_id: &NodeId) -> bool {

        let mut adj_list = self.connections.clone();
        adj_list.remove(drone_id);

        // Remove drone from all neighbor connection lists
        for neighbors in adj_list.values_mut() {
            neighbors.retain(|&id| &id != drone_id);
        }

        let constraints_ok = self.validate_network_constraints_with_logging(&adj_list);

        if !constraints_ok {
            return false;
        }

        self.debug_adjacency_list(&adj_list);
        let connectivity_ok = is_connected_after_removal_fixed(drone_id, &adj_list);

        if !connectivity_ok {
            return false;
        }

        true
    }

    // ✅ NEW: Debug adjacency list to see the actual connections
    fn debug_adjacency_list(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) {
        for (&node_id, connections) in adj_list {
            if !connections.is_empty() {
                let node_type = self.get_node_type(&node_id)
                    .map(|t| format!("{:?}", t))
                    .unwrap_or_else(|| "Unknown".to_string());
            }
        }
    }

    fn check_network_before_add_connection(&self, id1: &NodeId, id2: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();

        if let Some(neighbors) = adj_list.get_mut(id1) {
            neighbors.push(*id2);
        }
        if let Some(neighbors) = adj_list.get_mut(id2) {
            neighbors.push(*id1);
        }

        self.validate_network_constraints(&adj_list)
    }

    fn check_network_before_remove_connection(&self, id1: &NodeId, id2: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();

        if let Some(neighbors) = adj_list.get_mut(id1) {
            neighbors.retain(|&id| &id != id2);
        }
        if let Some(neighbors) = adj_list.get_mut(id2) {
            neighbors.retain(|&id| &id != id1);
        }

        self.validate_network_constraints(&adj_list) && is_connected_after_removal_fixed(id1, &adj_list)
    }

    pub(crate) fn validate_network_constraints(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
        // Check client constraints (1-2 connections)
        let clients_valid = self.node_types.iter()
            .filter(|(_, &node_type)| node_type == NodeType::Client)
            .all(|(&client_id, _)| {
                adj_list.get(&client_id)
                    .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
            });

        // Check server constraints (at least 2 connections)
        let servers_valid = self.node_types.iter()
            .filter(|(_, &node_type)| node_type == NodeType::Server)
            .all(|(&server_id, _)| {
                adj_list.get(&server_id)
                    .map_or(false, |neighbors| neighbors.len() >= 2)
            });

        clients_valid && servers_valid
    }

    // ✅ NEW: Network constraint validation with detailed logging
    fn validate_network_constraints_with_logging(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {

        // Check client constraints (1-2 connections)
        let clients_valid = self.node_types.iter()
            .filter(|(_, &node_type)| node_type == NodeType::Client)
            .all(|(&client_id, _)| {
                let connection_count = adj_list.get(&client_id)
                    .map_or(0, |neighbors| neighbors.len());

                connection_count > 0 && connection_count < 3
            });

        // Check server constraints (at least 2 connections)
        let servers_valid = self.node_types.iter()
            .filter(|(_, &node_type)| node_type == NodeType::Server)
            .all(|(&server_id, _)| {
                let connection_count = adj_list.get(&server_id)
                    .map_or(0, |neighbors| neighbors.len());
                connection_count >= 2
            });

        clients_valid && servers_valid
    }

    // ================================ Message Helpers ================================

    fn send_graph_update(&self, action: GraphAction) -> Result<(), ControllerError> {
        match self.graph_action_sender.try_send(action) {
            Ok(()) => {
                Ok(())
            }
            Err(e) => {
                Err(ControllerError::ChannelSend(format!("Graph update failed: {}", e)))
            }
        }
    }

    pub(crate) fn send_success_message(&self, msg: &str) {
        match self.message_sender.try_send(MessageType::Ok(msg.to_string())) {
            Ok(()) => {},
            Err(_) => {
            }
        }
    }

    pub(crate) fn send_info_message(&self, msg: &str) {
        match self.message_sender.try_send(PacketSent(msg.to_string())) {
            Ok(()) => {},
            Err(_) => {
            }
        }
    }

    pub(crate) fn send_error_message(&self, msg: &str) {
        match self.message_sender.try_send(Error(msg.to_string())) {
            Ok(()) => {},
            Err(_) => {
            }
        }
    }
}

// ================================ Helper Functions ================================

pub fn is_connected_after_removal_fixed(removed_id: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let remaining_nodes: Vec<NodeId> = adj_list.keys().copied().collect();

    if remaining_nodes.is_empty() {
        return true;
    }

    if remaining_nodes.len() == 1 {
        return true;
    }

    // Start DFS from the first remaining node
    let start_node = remaining_nodes[0];
    let reachable_count = count_reachable_nodes(start_node, adj_list);
    reachable_count == remaining_nodes.len()
}

fn count_reachable_nodes(start: NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> usize {
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start];

    while let Some(current) = stack.pop() {
        if visited.insert(current) {
            if let Some(neighbors) = adj_list.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    visited.len()
}

// ✅ NEW: Find all connected components
fn find_connected_components(adj_list: &HashMap<NodeId, Vec<NodeId>>) -> Vec<Vec<NodeId>> {
    let mut visited = std::collections::HashSet::new();
    let mut components = Vec::new();

    for &node in adj_list.keys() {
        if !visited.contains(&node) {
            let mut component = Vec::new();
            let mut stack = vec![node];

            while let Some(current) = stack.pop() {
                if visited.insert(current) {
                    component.push(current);
                    if let Some(neighbors) = adj_list.get(&current) {
                        for &neighbor in neighbors {
                            if !visited.contains(&neighbor) {
                                stack.push(neighbor);
                            }
                        }
                    }
                }
            }

            components.push(component);
        }
    }

    components
}

pub fn is_connected_after_removal(removed_id: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let nodes: Vec<&NodeId> = adj_list.keys().collect();
    if nodes.is_empty() {
        return true;
    }

    let start_node = nodes.into_iter().find(|&&id| &id != removed_id);
    if let Some(start) = start_node {
        is_connected(start, adj_list)
    } else {
        true
    }
}

// ✅ IMPROVED: Original connectivity check with better logging
pub fn is_connected(start: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let total_nodes = adj_list.len();
    if total_nodes <= 1 {
        return true;
    }

    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![*start];

    while let Some(current) = stack.pop() {
        if visited.insert(current) {
            if let Some(neighbors) = adj_list.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    visited.len() == total_nodes
}