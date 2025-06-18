use message::{NodeCommand, NodeEvent};
use crossbeam_channel::{Receiver, Sender, unbounded};
use std::collections::HashMap;
use std::sync::Arc;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use rand::{thread_rng, Rng};

// Import dei droni
use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
use bagel_bomber::BagelBomber;
//use egui::debug_text::print;
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
use client::ui::{ClientState, UiState};
use client::worker::Worker;
use server::ChatServer;
use crate::utility::NodeType::{Client, Server};

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
    pub drones: HashMap<NodeId, Box<dyn wg_2024::drone::Drone>>,
    pub drones_types: HashMap<NodeId, DroneGroup>,
    pub packet_senders: HashMap<NodeId, Sender<Packet>>,
    pub clients: HashMap<NodeId, Worker>,
    pub servers: HashMap<NodeId, ChatServer>,
    pub connections: HashMap<NodeId, Vec<NodeId>>,
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    pub receiver_event: HashMap<NodeId, Receiver<DroneEvent>>,
    pub receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
    pub client_ui_state: UiState,

    // Controller's communication channels
    pub button_receiver: Receiver<ButtonEvent>,
    pub graph_action_sender: Sender<GraphAction>,
    pub message_sender: Sender<MessageType>,
    pub drones_counter: HashMap<DroneGroup, i8>,
}

impl ControllerHandler {
    pub fn new(
        drones: HashMap<NodeId, Box<dyn wg_2024::drone::Drone>>,
        drones_types: HashMap<NodeId, DroneGroup>,
        drone_senders: HashMap<NodeId, Sender<Packet>>,
        clients: HashMap<NodeId, Worker>,
        servers: HashMap<NodeId, ChatServer>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
        send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
        reciver_event: HashMap<NodeId, Receiver<DroneEvent>>,
        receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
        client_ui_state: UiState,
        button_receiver: Receiver<ButtonEvent>,
        graph_action_sender: Sender<GraphAction>,
        message_sender: Sender<MessageType>,
    ) -> Self {
        let drones_counter: HashMap<DroneGroup, i8> = HashMap::new();

        Self {
            drones,
            drones_types,
            packet_senders: drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event: reciver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
            drones_counter,
        }
    }

    pub fn run(&mut self) {
        loop {
            // Process drone events
            for (node_id, receiver) in self.receiver_event.clone() {
                if let Ok(event) = receiver.try_recv() {
                    self.handle_drone_event(event, node_id);
                }
            }

            // Process node events
            for (node_id, receiver) in self.receriver_node_event.clone() {
                if let Ok(event) = receiver.try_recv() {
                    self.handle_node_event(event, node_id);
                }
            }

            // Process button events
            if let Ok(command) = self.button_receiver.try_recv() {
                self.handle_button_event(command);
            }

            // Small pause to avoid intensive loop
            std::thread::yield_now();
        }
    }

    // ================================ Event Handlers ================================

    pub fn handle_button_event(&mut self, event: ButtonEvent) {
        let result = match event {
            ButtonEvent::NewDrone(id, pdr) => self.spawn_drone(&id, pdr),
            ButtonEvent::NewServer(id) => self.create_server(id),
            ButtonEvent::NewClient(id) => self.create_client(id),
            ButtonEvent::NewConnection(id1, id2) => self.add_connection(&id1, &id2),
            ButtonEvent::Crash(id) => self.crash_drone(&id),
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
                if let Err(_e) = self.message_sender.try_send(MessageType::Info(message)){
                    // gestisci errore se necessario
                }
            },
            NodeEvent::CreateMessage(_c) => {
                let message = "Message created".to_string();
                if let Err(_e) = self.message_sender.try_send(MessageType::Info(message)){
                    // gestisci errore se necessario
                }
            },
            NodeEvent::MessageRecv(_c) => {
                let message = "Message received".to_string();
                if let Err(_e) = self.message_sender.try_send(MessageType::Info(message)){
                    // gestisci errore se necessario
                }
            },
            NodeEvent::ControllerShortcut(packet) => {
                if let Err(e) = self.send_packet_to_client(packet) {
                    self.send_error_message(&format!("Failed to send packet to client: {}", e));
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

    // ================================ Drone Operations ================================

    fn spawn_drone(&mut self, first_connection: &NodeId, pdr: f32) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;

        if !self.check_network_before_add_drone(&id, &vec![*first_connection]) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add drone {} connected to {}", id, first_connection)
            ));
        }

        self.create_drone(id, first_connection, pdr)?;
        self.send_graph_update(AddNode(id, NodeType::Drone))?;
        self.send_success_message(&format!("Drone {} created successfully", id));

        Ok(())
    }

    fn create_drone(&mut self, id: NodeId, first_connection: &NodeId, pdr: f32) -> Result<(), ControllerError> {
        let (sender_event, receiver_event) = unbounded::<DroneEvent>();
        let (sender_drone_command, receiver_drone_command) = unbounded::<DroneCommand>();
        let (sender_packet, receiver_packet) = unbounded::<Packet>();

        let senders = HashMap::new();
        let (drone, drone_group) = self.create_balanced_drone(
            id, sender_event, receiver_drone_command, receiver_packet, senders, pdr
        )?;

        // Insert all drone-related data
        self.drones.insert(id, drone);
        self.drones_types.insert(id, drone_group);
        self.packet_senders.insert(id, sender_packet);
        self.connections.insert(id, Vec::new());
        self.send_command_drone.insert(id, sender_drone_command);
        self.receiver_event.insert(id, receiver_event);

        // Add initial connection
        self.add_connection(&id, first_connection)?;

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
    ) -> Result<(Box<dyn Drone>, DroneGroup), ControllerError> {
        let drone_group = self.select_drone_group()
            .ok_or_else(|| ControllerError::InvalidOperation("No drone group available".to_string()))?
            .clone();

        let drone: Box<dyn Drone> = match drone_group {
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
        Ok((drone, drone_group))
    }

    pub(crate) fn select_drone_group(&self) -> Option<&DroneGroup> {
        let min_value = self.drones_counter.values().min()?;
        let mut candidates: Vec<&DroneGroup> = self.drones_counter
            .iter()
            .filter_map(|(group, &count)| {
                if count == *min_value { Some(group) } else { None }
            })
            .collect();

        candidates.shuffle(&mut thread_rng());
        candidates.into_iter().next()
    }

    fn crash_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node {} is not a drone", id)
            ));
        }

        if !self.check_network_before_removing_drone(id) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot remove drone {} - would violate network constraints", id)
            ));
        }

        self.remove_drone(id)?;
        self.send_graph_update(RemoveNode(*id))?;
        self.send_success_message(&format!("Drone {} removed successfully", id));

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
        self.drones.remove(id);
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

        let mut packet_send: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();
        let (ui_comunication_send, ui_comunication_receiver) = unbounded::<ToUICommunication>();
        let (from_ui_comunication_send, from_ui_comunication_receiver) = unbounded::<FromUiCommunication>();

        if let Some(sender) = self.packet_senders.get(&id_connection) {
            packet_send.insert(id_connection, sender.clone());
        }

        let worker = Worker::new(
            id, packet_send, node_event_send, ui_comunication_send,
            p_receiver, node_command_receiver, from_ui_comunication_receiver
        );
        let client_state = ClientState::new(id, ui_comunication_receiver, from_ui_comunication_send);

        self.client_ui_state.add_client(id, client_state);
        self.packet_senders.insert(id, p_send);
        self.clients.insert(id, worker);
        self.connections.insert(id, vec![id_connection]);
        self.send_command_node.insert(id, node_command_send);
        self.receriver_node_event.insert(id, node_event_receiver);

        self.send_graph_update(AddNode(id, Client))?;
        self.send_success_message("New client added successfully");

        Ok(())
    }

    fn create_server(&mut self, id_connection: NodeId) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;

        if !self.is_drone(&id_connection) {
            return Err(ControllerError::InvalidOperation(
                "Server must connect to a drone".to_string()
            ));
        }

        let mut packet_send: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();

        if let Some(sender) = self.packet_senders.get(&id_connection) {
            packet_send.insert(id_connection, sender.clone());
        }

        let chat_server = ChatServer::new(
            id, node_event_send, node_command_receiver, p_receiver, packet_send
        );

        self.servers.insert(id, chat_server);
        self.connections.insert(id, vec![id_connection]);
        self.send_command_node.insert(id, node_command_send);
        self.receriver_node_event.insert(id, node_event_receiver);

        self.send_graph_update(AddNode(id, Server))?;
        self.send_success_message("New server added successfully");

        Ok(())
    }

    // ================================ Connection Management ================================

    fn add_connection(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {
        if !self.check_network_before_add_connection(id1, id2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add connection between {} and {}", id1, id2)
            ));
        }

        self.add_sender(id1, id2)?;
        self.add_sender(id2, id1)?;

        // Update connections
        if let Some(connections) = self.connections.get_mut(id1) {
            connections.push(*id2);
        }
        if let Some(connections) = self.connections.get_mut(id2) {
            connections.push(*id1);
        }

        self.send_graph_update(AddEdge(*id1, *id2))?;
        self.send_success_message(&format!("Connection added between {} and {}", id1, id2));

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
            connections.retain(|&id| id != *id2);
        }
        if let Some(connections) = self.connections.get_mut(id2) {
            connections.retain(|&id| id != *id1);
        }

        self.send_graph_update(RemoveEdge(*id1, *id2))?;
        self.send_success_message(&format!("Connection removed between {} and {}", id1, id2));

        Ok(())
    }

    // ================================ Helper Methods ================================

    fn add_sender(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {
        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;
            sender.send(DroneCommand::AddSender(*dst_id, dst_sender))
                .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;
            sender.send(NodeCommand::AddSender(*dst_id, dst_sender))
                .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
        }

        Ok(())
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

    pub(crate) fn send_packet_to_client(&self, packet: Packet) -> Result<(), ControllerError> {
        if let Some(destination) = packet.routing_header.hops.last() {
            if self.clients.contains_key(destination) {
                let sender = self.send_command_node.get(destination)
                    .ok_or_else(|| ControllerError::NodeNotFound(*destination))?;
                sender.try_send(FromShortcut(packet))
                    .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
            } else {
                return Err(ControllerError::NodeNotFound(*destination));
            }
        }
        Ok(())
    }

    // ================================ Utility Methods ================================

    pub(crate) fn is_drone(&self, id: &NodeId) -> bool {
        self.drones.contains_key(id)
    }

    pub(crate) fn generate_random_id(&self) -> Result<NodeId, ControllerError> {
        let used_ids: Vec<NodeId> = self.drones.keys()
            .chain(self.clients.keys())
            .chain(self.servers.keys())
            .cloned()
            .collect();

        let available_ids: Vec<NodeId> = (0..=255)
            .filter(|id| !self.packet_senders.contains_key(id))
            .collect();

        if available_ids.is_empty() {
            return Err(ControllerError::InvalidOperation("No available IDs".to_string()));
        }

        let random_index = rand::thread_rng().gen_range(0..available_ids.len());
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

    fn check_network_before_removing_drone(&self, drone_id: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();
        adj_list.remove(drone_id);

        for neighbors in adj_list.values_mut() {
            neighbors.retain(|&id| id != *drone_id);
        }

        self.validate_network_constraints(&adj_list) && is_connected_after_removal(drone_id, &adj_list)
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
            neighbors.retain(|&id| id != *id2);
        }
        if let Some(neighbors) = adj_list.get_mut(id2) {
            neighbors.retain(|&id| id != *id1);
        }

        self.validate_network_constraints(&adj_list) && is_connected_after_removal(id1, &adj_list)
    }

    pub(crate) fn validate_network_constraints(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
        // Check client constraints (1-2 connections)
        let clients_valid = self.clients.iter().all(|(&client, _)| {
            adj_list.get(&client)
                .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
        });

        // Check server constraints (at least 2 connections)
        let servers_valid = self.servers.iter().all(|(&server, _)| {
            adj_list.get(&server)
                .map_or(false, |neighbors| neighbors.len() >= 2)
        });

        clients_valid && servers_valid
    }

    // ================================ Message Helpers ================================

    fn send_graph_update(&self, action: GraphAction) -> Result<(), ControllerError> {
        self.graph_action_sender.try_send(action)
            .map_err(|e| ControllerError::ChannelSend(format!("Graph update failed: {}", e)))
    }

    pub(crate) fn send_success_message(&self, msg: &str) {
        let _ = self.message_sender.try_send(MessageType::Ok(msg.to_string()));
    }

    pub(crate) fn send_info_message(&self, msg: &str) {
        let _ = self.message_sender.try_send(PacketSent(msg.to_string()));
    }

    pub(crate) fn send_error_message(&self, msg: &str) {
        let _ = self.message_sender.try_send(Error(msg.to_string()));
    }
}

// ================================ Helper Functions ================================

pub fn is_connected_after_removal(removed_id: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let nodes: Vec<&NodeId> = adj_list.keys().collect();
    if nodes.is_empty() {
        return true;
    }

    let start_node = nodes.into_iter().find(|&&id| id != *removed_id);
    if let Some(start) = start_node {
        is_connected(start, adj_list)
    } else {
        true
    }
}

pub fn is_connected(start: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
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

    visited.len() == adj_list.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::collections::HashMap;
    use wg_2024::packet::{Packet, PacketType, Fragment};
    use wg_2024::network::SourceRoutingHeader;
    use wg_2024::controller::{DroneCommand, DroneEvent};

    // Test base per la creazione del controller
    #[test]
    fn test_controller_creation() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        assert_eq!(controller.drones.len(), 0);
        assert_eq!(controller.clients.len(), 0);
        assert_eq!(controller.servers.len(), 0);
    }

    // Test per l'enum ControllerError
    #[test]
    fn test_controller_error_display() {
        let error1 = ControllerError::ChannelSend("test message".to_string());
        assert_eq!(error1.to_string(), "Channel send error: test message");

        let error2 = ControllerError::NodeNotFound(42);
        assert_eq!(error2.to_string(), "Node not found: 42");

        let error3 = ControllerError::InvalidOperation("invalid op".to_string());
        assert_eq!(error3.to_string(), "Invalid operation: invalid op");

        let error4 = ControllerError::NetworkConstraintViolation("constraint violated".to_string());
        assert_eq!(error4.to_string(), "Network constraint violation: constraint violated");
    }

    // Test per is_drone method
    #[test]
    fn test_is_drone() {
        let mut drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        // Simula l'aggiunta di un drone
        // Per questo test, usiamo solo un placeholder dato che non possiamo creare droni reali

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Test con ID inesistente
        assert!(!controller.is_drone(&1));
        assert!(!controller.is_drone(&99));
    }

    // Test per generate_random_id con ID limitati
    #[test]
    fn test_generate_random_id_with_available_ids() {
        let mut drones = HashMap::new();
        let drones_types = HashMap::new();
        let mut packet_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        // Occupa alcuni ID
        for i in 0..10 {
            let (sender, _) = unbounded::<Packet>();
            packet_senders.insert(i, sender);
        }

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            packet_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let result = controller.generate_random_id();
        assert!(result.is_ok());
        let id = result.unwrap();
        assert!(id >= 10); // Dovrebbe essere >= 10 dato che 0-9 sono occupati
    }

    // Test per validate_network_constraints con rete vuota
    #[test]
    fn test_validate_network_constraints_empty() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let adj_list = HashMap::new();
        assert!(controller.validate_network_constraints(&adj_list));
    }

    // Test per change_packet_drop_rate con nodo inesistente
    #[test]
    fn test_change_packet_drop_rate_invalid_node() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let mut controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let result = controller.change_packet_drop_rate(&99, 0.5);
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ControllerError::InvalidOperation(_)));
        }
    }

    // Test per send_packet_to_client con client inesistente
    #[test]
    fn test_send_packet_to_client_not_found() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let packet = Packet {
            pack_type: PacketType::MsgFragment(Fragment {
                fragment_index: 0,
                total_n_fragments: 1,
                length: 10,
                data: [0; 128],
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 0,
                hops: vec![1, 99], // Client inesistente
            },
            session_id: 123,
        };

        let result = controller.send_packet_to_client(packet);
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ControllerError::NodeNotFound(_)));
        }
    }

    // Test per message helpers
    #[test]
    fn test_message_helpers() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        controller.send_success_message("test success");
        controller.send_info_message("test info");
        controller.send_error_message("test error");

        // Verifica che i messaggi siano stati inviati
        let mut messages = Vec::new();
        while let Ok(msg) = message_receiver.try_recv() {
            messages.push(msg);
        }

        assert_eq!(messages.len(), 3);

        // Verifica i tipi di messaggio
        assert!(messages.iter().any(|msg| matches!(msg, MessageType::Ok(_))));
        assert!(messages.iter().any(|msg| matches!(msg, PacketSent(_))));
        assert!(messages.iter().any(|msg| matches!(msg, Error(_))));
    }

    // Test per select_drone_group
    #[test]
    fn test_select_drone_group_empty() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Con counter vuoto dovrebbe restituire None
        let result = controller.select_drone_group();
        assert!(result.is_none());
    }

    // Test per select_drone_group con valori
    #[test]
    fn test_select_drone_group_with_values() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let mut controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Aggiungi alcuni contatori
        controller.drones_counter.insert(DroneGroup::RustInPeace, 3);
        controller.drones_counter.insert(DroneGroup::BagelBomber, 1);
        controller.drones_counter.insert(DroneGroup::LockheedRustin, 2);

        let result = controller.select_drone_group();
        assert!(result.is_some());

        // Dovrebbe selezionare il gruppo con il conteggio minimo (BagelBomber con 1)
        assert_eq!(*result.unwrap(), DroneGroup::BagelBomber);
    }
}

// Test separati per le funzioni helper globali
#[cfg(test)]
mod helper_function_tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_is_connected_empty_graph() {
        let adj_list = HashMap::new();
        // Un grafo vuoto è considerato connesso
        assert!(is_connected(&1, &adj_list));
    }

    #[test]
    fn test_is_connected_single_node() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, Vec::new());
        assert!(is_connected(&1, &adj_list));
    }

    #[test]
    fn test_is_connected_two_connected_nodes() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, vec![2]);
        adj_list.insert(2, vec![1]);

        assert!(is_connected(&1, &adj_list));
        assert!(is_connected(&2, &adj_list));
    }

    #[test]
    fn test_is_connected_two_disconnected_nodes() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, Vec::new());
        adj_list.insert(2, Vec::new());

        assert!(!is_connected(&1, &adj_list));
        assert!(!is_connected(&2, &adj_list));
    }

    #[test]
    fn test_is_connected_complex_connected() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, vec![2, 3]);
        adj_list.insert(2, vec![1, 4]);
        adj_list.insert(3, vec![1, 4]);
        adj_list.insert(4, vec![2, 3]);

        assert!(is_connected(&1, &adj_list));
        assert!(is_connected(&2, &adj_list));
        assert!(is_connected(&3, &adj_list));
        assert!(is_connected(&4, &adj_list));
    }

    #[test]
    fn test_is_connected_after_removal_disconnects() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, vec![2]);
        adj_list.insert(2, vec![1, 3]);
        adj_list.insert(3, vec![2]);

        // Rimuovendo il nodo 2 (bridge), dovrebbe disconnettere
        assert!(!is_connected_after_removal(&2, &adj_list));
    }

    #[test]
    fn test_is_connected_after_removal_stays_connected() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, vec![2, 3]);
        adj_list.insert(2, vec![1, 3]);
        adj_list.insert(3, vec![1, 2]);

        // Rimuovendo qualsiasi nodo da questo triangolo, dovrebbe rimanere connesso
        assert!(is_connected_after_removal(&1, &adj_list));
        assert!(is_connected_after_removal(&2, &adj_list));
        assert!(is_connected_after_removal(&3, &adj_list));
    }

    #[test]
    fn test_is_connected_after_removal_empty_result() {
        let mut adj_list = HashMap::new();
        adj_list.insert(1, Vec::new());

        // Rimuovendo l'unico nodo dovrebbe risultare in un grafo vuoto (connesso)
        assert!(is_connected_after_removal(&1, &adj_list));
    }
}