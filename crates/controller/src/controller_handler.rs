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
    // Unified node type tracking
    pub node_types: HashMap<NodeId, NodeType>,

    // Drone-specific data (only types, not instances)
    pub drones_types: HashMap<NodeId, DroneGroup>,

    // Common data
    pub packet_senders: HashMap<NodeId, Sender<Packet>>,
    pub connections: HashMap<NodeId, Vec<NodeId>>,
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    pub receiver_event: HashMap<NodeId, Receiver<DroneEvent>>,
    pub receriver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,

    // Controller's communication channels
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
            // Process drone events
            let drone_node_ids: Vec<NodeId> = self.receiver_event.keys().copied().collect();
            for node_id in drone_node_ids {
                if let Some(receiver) = self.receiver_event.get(&node_id) {
                    if let Ok(event) = receiver.try_recv() {
                        self.handle_drone_event(event, node_id);
                    }
                }
            }

            // Process node events
            let node_node_ids: Vec<NodeId> = self.receriver_node_event.keys().copied().collect();
            for node_id in node_node_ids {
                if let Some(receiver) = self.receriver_node_event.get(&node_id) {
                    if let Ok(event) = receiver.try_recv() {
                        self.handle_node_event(event, node_id);
                    }
                }
            }

            // Process button events
            if let Ok(command) = self.button_receiver.try_recv() {
                println!("🟢 ControllerHandler: Received button event: {:?}", command);
                self.handle_button_event(command);
            }

            // Small pause to avoid intensive loop
            std::thread::yield_now();
        }
    }

    // ================================ Event Handlers ================================

    pub fn handle_button_event(&mut self, event: ButtonEvent) {
        println!("🟢 CONTROLLER: Received button event: {:?}", event);
        let result = match event {
            ButtonEvent::NewDrone(id, pdr) => {
                println!("🤖 CONTROLLER: Creating new drone connected to {}", id);
                self.spawn_drone(&id, pdr)
            },
            ButtonEvent::NewServer(id) => {
                println!("🖥️ CONTROLLER: Creating new server connected to {} (legacy method)", id);
                // Legacy method - deprecated
                Err(ControllerError::InvalidOperation(
                    "Use 'New Server (2 connections)' button instead. Select 2 drones and create server.".to_string()
                ))
            },
            ButtonEvent::NewServerWithTwoConnections(drone1, drone2) => {
                println!("🖥️ CONTROLLER: Creating new server connected to {} and {}", drone1, drone2);
                self.create_server_with_two_connections(drone1, drone2)
            },
            ButtonEvent::NewClient(id) => {
                println!("💻 CONTROLLER: Creating new client connected to {}", id);
                self.create_client(id)
            },
            ButtonEvent::NewConnection(id1, id2) => {
                println!("🔗 CONTROLLER: Creating connection between {} and {}", id1, id2);
                self.add_connection(&id1, &id2)
            },
            ButtonEvent::Crash(id) => self.crash_drone(&id),
            ButtonEvent::RemoveConection(id1, id2) => self.remove_connection(&id1, &id2),
            ButtonEvent::ChangePdr(id, pdr) => self.change_packet_drop_rate(&id, pdr),
        };

        if let Err(e) = result {
            println!("❌ CONTROLLER: Error handling button event: {}", e);
            self.send_error_message(&e.to_string());
        } else {
            println!("✅ CONTROLLER: Button event handled successfully");
        }
    }

    // ✅ MIGLIORATO: Handle node event con debug
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
                println!("🔄 CONTROLLER: Received ControllerShortcut from node {}", node_id);
                println!("📦 CONTROLLER: Packet details - session_id: {}, destination: {:?}",
                         packet.session_id, packet.routing_header.hops.last());

                if let Err(e) = self.send_packet_to_client(packet) {
                    println!("❌ CONTROLLER: Failed to send shortcut packet: {}", e);
                    self.debug_existing_nodes();
                    self.send_error_message(&format!("Failed to send shortcut packet: {}", e));
                }
            }
        }
    }

    // ✅ MIGLIORATO: Handle drone event con debug
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
                println!("🔄 CONTROLLER: Received ControllerShortcut from drone {}", drone_id);
                println!("📦 CONTROLLER: Packet details - session_id: {}, destination: {:?}",
                         packet.session_id, packet.routing_header.hops.last());

                if let Err(e) = self.send_packet_to_client(packet) {
                    println!("❌ CONTROLLER: Failed to send shortcut packet: {}", e);
                    self.debug_existing_nodes();
                    self.send_error_message(&format!("Failed to send shortcut packet: {}", e));
                }
            }
        }
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
        println!("🚀 CONTROLLER: Sending AddNode({}, Drone)", id);
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

        // Health check
        println!("💤 CONTROLLER: Waiting for drone {} to initialize...", id);
        std::thread::sleep(std::time::Duration::from_millis(100));

        println!("🔍 CONTROLLER: Testing if drone {} is responsive...", id);
        match sender_drone_command.try_send(DroneCommand::SetPacketDropRate(pdr)) {
            Ok(()) => {
                println!("✅ CONTROLLER: Drone {} is responsive!", id);
            }
            Err(e) => {
                println!("❌ CONTROLLER: Drone {} is not responsive: {}", id, e);
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

        println!("🚀 CONTROLLER: Starting drone {} in separate thread", id);
        std::thread::spawn(move || {
            println!("🤖 DRONE {}: Thread started, calling run()", id);
            drone.run();
            println!("🤖 DRONE {}: Thread ended", id);
        });

        Ok(drone_group)
    }

    pub(crate) fn select_drone_group(&self) -> Option<&DroneGroup> {
        println!("🔍 select_drone_group: drones_counter = {:?}", self.drones_counter);

        if self.drones_counter.is_empty() {
            println!("❌ drones_counter is EMPTY!");
            return None;
        }

        let min_value = self.drones_counter.values().min()?;
        println!("🔍 min_value = {}", min_value);

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
        println!("🔍 selected = {:?}", selected);

        selected
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

        // Remove from node_types
        self.node_types.remove(id);

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
        println!("🚀 CONTROLLER: Sending AddNode({}, Client)", id);
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

    // ✅ FIX: Client creation con Worker nel thread
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

        // ✅ FIX: Crea il Worker all'interno del thread
        println!("🚀 CONTROLLER: Starting client worker {} in separate thread", id);
        std::thread::spawn(move || {
            println!("💻 CLIENT {}: Thread started, creating worker", id);

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

            println!("💻 CLIENT {}: Worker created, calling run()", id);
            worker.run();
            println!("💻 CLIENT {}: Thread ended", id);
        });

        // Insert dei dati DOPO aver avviato il thread
        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send);
        self.receriver_node_event.insert(id, node_event_receiver);

        println!("💤 CONTROLLER: Waiting for client {} to initialize...", id);
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(())
    }

    // ✅ NUOVO: Create server with two connections atomically
    fn create_server_with_two_connections(&mut self, drone1: NodeId, drone2: NodeId) -> Result<(), ControllerError> {
        println!("🖥️ CONTROLLER: Starting atomic server creation with 2 connections: {} and {}", drone1, drone2);

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

        // ✅ VERIFICA PRELIMINARE: Simula l'aggiunta per verificare che sarà valida
        println!("🔍 CONTROLLER: Pre-validating network with server {} and connections to {} and {}", id, drone1, drone2);
        if !self.check_network_before_add_server_with_two_connections(&id, &drone1, &drone2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add server {} with connections to {} and {}", id, drone1, drone2)
            ));
        }

        // Crea il server senza connessioni
        println!("🖥️ CONTROLLER: Creating server {} without connections", id);
        if let Err(e) = self.create_server_without_connection(id) {
            return Err(e);
        }

        // Update node_types PRIMA di tutto
        self.node_types.insert(id, NodeType::Server);

        // Invia AddNode
        println!("🚀 CONTROLLER: Sending AddNode({}, Server)", id);
        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Server)) {
            self.cleanup_server(&id);
            return Err(e);
        }

        // ✅ AGGIUNTA ATOMICA: Aggiungi entrambe le connessioni SENZA validazione intermedia
        println!("🔗 CONTROLLER: Adding both connections atomically for server {}", id);
        if let Err(e) = self.add_server_connections_atomic(&id, &drone1, &drone2) {
            println!("❌ CONTROLLER: Failed to add connections atomically, rolling back");
            let _ = self.send_graph_update(RemoveNode(id));
            self.cleanup_server(&id);
            return Err(e);
        }

        println!("✅ CONTROLLER: Server {} created successfully with 2 connections", id);
        self.send_success_message(&format!("Server {} created with connections to drones {} and {}", id, drone1, drone2));
        Ok(())
    }

    // ✅ FIX: Server creation con ChatServer nel thread
    fn create_server_without_connection(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();

        // ✅ FIX: Crea il ChatServer all'interno del thread
        println!("🚀 CONTROLLER: Starting chat server {} in separate thread", id);
        std::thread::spawn(move || {
            println!("🖥️ SERVER {}: Thread started, creating server", id);

            let packet_send = HashMap::new();
            let mut chat_server = ChatServer::new(
                id,
                node_event_send,
                node_command_receiver,
                p_receiver,
                packet_send
            );

            println!("🖥️ SERVER {}: Server created, calling run()", id);
            chat_server.run();
            println!("🖥️ SERVER {}: Thread ended", id);
        });

        // Insert dei dati DOPO aver avviato il thread
        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send);
        self.receriver_node_event.insert(id, node_event_receiver);

        println!("💤 CONTROLLER: Waiting for server {} to initialize...", id);
        std::thread::sleep(std::time::Duration::from_millis(100));

        Ok(())
    }

    // ================================ Connection Management ================================

    fn add_connection(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {
        println!("🔗 CONTROLLER: Adding connection between {} and {}", id1, id2);

        if !self.check_network_before_add_connection(id1, id2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add connection between {} and {}", id1, id2)
            ));
        }

        // Verifica che entrambi i nodi esistano
        if !self.packet_senders.contains_key(id1) {
            return Err(ControllerError::NodeNotFound(*id1));
        }
        if !self.packet_senders.contains_key(id2) {
            return Err(ControllerError::NodeNotFound(*id2));
        }

        self.add_sender(id1, id2)?;
        self.add_sender(id2, id1)?;

        // Update connections nelle strutture dati
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

        // IMPORTANTE: Invia AddEdge al UI
        println!("🚀 CONTROLLER: Sending AddEdge({}, {})", id1, id2);
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
        println!("📤 CONTROLLER: Adding sender from {} to {}", id, dst_id);

        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            println!("📤 CONTROLLER: {} is a drone, sending DroneCommand::AddSender", id);
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                Ok(()) => {
                    println!("✅ CONTROLLER: DroneCommand::AddSender sent successfully");
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    println!("❌ CONTROLLER: Drone {} channel disconnected! Trying with delay...", id);
                    std::thread::sleep(std::time::Duration::from_millis(50));

                    match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender)) {
                        Ok(()) => {
                            println!("✅ CONTROLLER: DroneCommand::AddSender sent successfully on retry");
                        }
                        Err(e) => {
                            println!("❌ CONTROLLER: Drone {} is completely dead: {}", id, e);
                            return Err(ControllerError::ChannelSend(format!("Drone {} channel disconnected: {}", id, e)));
                        }
                    }
                }
                Err(e) => {
                    println!("❌ CONTROLLER: FAILED to send DroneCommand::AddSender: {}", e);
                    return Err(ControllerError::ChannelSend(e.to_string()));
                }
            }
        } else {
            println!("📤 CONTROLLER: {} is a node, sending NodeCommand::AddSender", id);
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            match sender.send(NodeCommand::AddSender(*dst_id, dst_sender)) {
                Ok(()) => println!("✅ CONTROLLER: NodeCommand::AddSender sent successfully"),
                Err(e) => {
                    println!("❌ CONTROLLER: FAILED to send NodeCommand::AddSender: {}", e);
                    return Err(ControllerError::ChannelSend(e.to_string()));
                }
            }
        }

        println!("✅ CONTROLLER: Sender added from {} to {}", id, dst_id);
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

    // ✅ FIX: send_packet_to_client gestisce sia Client che Server
    pub(crate) fn send_packet_to_client(&self, packet: Packet) -> Result<(), ControllerError> {
        // ✅ FIX: Salva i valori prima di spostare packet
        let session_id = packet.session_id;
        let destination = packet.routing_header.hops.last().copied();

        println!("📦 CONTROLLER: Processing shortcut packet with session_id: {}", session_id);

        if let Some(destination) = destination {
            println!("📦 CONTROLLER: Packet destination: {}", destination);

            match self.get_node_type(&destination) {
                Some(NodeType::Client) => {
                    println!("📦 CONTROLLER: Destination {} is a CLIENT", destination);
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))  // ✅ packet moved qui
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                    println!("✅ CONTROLLER: Shortcut packet sent to client {}", destination);
                }
                Some(NodeType::Server) => {
                    println!("📦 CONTROLLER: Destination {} is a SERVER", destination);
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))  // ✅ packet moved qui
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                    println!("✅ CONTROLLER: Shortcut packet sent to server {}", destination);
                }
                Some(NodeType::Drone) => {
                    println!("⚠️ CONTROLLER: Destination {} is a DRONE - shortcut not applicable", destination);
                    return Err(ControllerError::InvalidOperation(
                        format!("Cannot send shortcut packet to drone {}", destination)
                    ));
                }
                None => {
                    println!("❌ CONTROLLER: Destination {} does not exist in the system", destination);
                    return Err(ControllerError::NodeNotFound(destination));
                }
            }
        } else {
            println!("❌ CONTROLLER: Packet has no destination in routing header");
            return Err(ControllerError::InvalidOperation(
                "Packet has no destination".to_string()
            ));
        }

        Ok(())
    }

    // ✅ NUOVO: Metodo di debug per visualizzare nodi esistenti
    pub fn debug_existing_nodes(&self) {
        println!("🔍 CONTROLLER: Current nodes in system:");
        for (&node_id, &node_type) in &self.node_types {
            let has_packet_sender = self.packet_senders.contains_key(&node_id);
            let has_command_sender = if node_type == NodeType::Drone {
                self.send_command_drone.contains_key(&node_id)
            } else {
                self.send_command_node.contains_key(&node_id)
            };

            println!("   Node {}: {:?}, packet_sender: {}, command_sender: {}",
                     node_id, node_type, has_packet_sender, has_command_sender);
        }
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

    // ✅ NUOVO: Aggiunge entrambe le connessioni senza validazione intermedia
    fn add_server_connections_atomic(&mut self, server_id: &NodeId, drone1: &NodeId, drone2: &NodeId) -> Result<(), ControllerError> {
        println!("🔗 CONTROLLER: Adding atomic connections for server {}", server_id);

        // Aggiungi senders per server -> droni
        if let Err(e) = self.add_sender_without_validation(server_id, drone1) {
            return Err(e);
        }
        if let Err(e) = self.add_sender_without_validation(server_id, drone2) {
            // Se fallisce la seconda, rimuovi la prima
            let _ = self.remove_sender(server_id, drone1);
            return Err(e);
        }

        // Aggiungi senders per droni -> server
        if let Err(e) = self.add_sender_without_validation(drone1, server_id) {
            // Rollback
            let _ = self.remove_sender(server_id, drone1);
            let _ = self.remove_sender(server_id, drone2);
            return Err(e);
        }
        if let Err(e) = self.add_sender_without_validation(drone2, server_id) {
            // Rollback completo
            let _ = self.remove_sender(server_id, drone1);
            let _ = self.remove_sender(server_id, drone2);
            let _ = self.remove_sender(drone1, server_id);
            return Err(e);
        }

        // ✅ AGGIORNA le strutture dati delle connessioni ALLA FINE
        if let Some(server_connections) = self.connections.get_mut(server_id) {
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

        // ✅ INVIA gli AddEdge ALLA FINE
        println!("🚀 CONTROLLER: Sending AddEdge({}, {})", server_id, drone1);
        if let Err(e) = self.send_graph_update(AddEdge(*server_id, *drone1)) {
            println!("❌ CONTROLLER: Failed to send first AddEdge");
            return Err(e);
        }

        println!("🚀 CONTROLLER: Sending AddEdge({}, {})", server_id, drone2);
        if let Err(e) = self.send_graph_update(AddEdge(*server_id, *drone2)) {
            println!("❌ CONTROLLER: Failed to send second AddEdge");
            return Err(e);
        }

        println!("✅ CONTROLLER: Both connections added successfully for server {}", server_id);
        Ok(())
    }

    // ✅ NUOVO: add_sender senza validazione della rete
    fn add_sender_without_validation(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(), ControllerError> {
        println!("📤 CONTROLLER: Adding sender from {} to {} (no validation)", id, dst_id);

        let dst_sender = self.packet_senders.get(dst_id)
            .ok_or_else(|| ControllerError::NodeNotFound(*dst_id))?
            .clone();

        if self.is_drone(id) {
            println!("📤 CONTROLLER: {} is a drone, sending DroneCommand::AddSender", id);
            let sender = self.send_command_drone.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                Ok(()) => {
                    println!("✅ CONTROLLER: DroneCommand::AddSender sent successfully");
                }
                Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                    println!("❌ CONTROLLER: Drone {} channel disconnected! Trying with delay...", id);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender)) {
                        Ok(()) => {
                            println!("✅ CONTROLLER: DroneCommand::AddSender sent successfully on retry");
                        }
                        Err(e) => {
                            println!("❌ CONTROLLER: Drone {} is completely dead: {}", id, e);
                            return Err(ControllerError::ChannelSend(format!("Drone {} channel disconnected: {}", id, e)));
                        }
                    }
                }
                Err(e) => {
                    println!("❌ CONTROLLER: FAILED to send DroneCommand::AddSender: {}", e);
                    return Err(ControllerError::ChannelSend(e.to_string()));
                }
            }
        } else {
            println!("📤 CONTROLLER: {} is a node, sending NodeCommand::AddSender", id);
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            match sender.send(NodeCommand::AddSender(*dst_id, dst_sender)) {
                Ok(()) => println!("✅ CONTROLLER: NodeCommand::AddSender sent successfully"),
                Err(e) => {
                    println!("❌ CONTROLLER: FAILED to send NodeCommand::AddSender: {}", e);
                    return Err(ControllerError::ChannelSend(e.to_string()));
                }
            }
        }

        println!("✅ CONTROLLER: Sender added from {} to {} (no validation)", id, dst_id);
        Ok(())
    }

    // ================================ Cleanup Methods ================================

    fn cleanup_drone(&mut self, id: &NodeId) {
        println!("🧹 CONTROLLER: Cleaning up drone {}", id);

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
        println!("🧹 CONTROLLER: Cleaning up client {}", id);

        self.node_types.remove(id);
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receriver_node_event.remove(id);
    }

    fn cleanup_server(&mut self, id: &NodeId) {
        println!("🧹 CONTROLLER: Cleaning up server {}", id);

        self.node_types.remove(id);
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receriver_node_event.remove(id);
    }

    // ================================ Node Type Checkers ================================

    pub(crate) fn is_drone(&self, id: &NodeId) -> bool {
        self.node_types.get(id) == Some(&NodeType::Drone)
    }

    pub(crate) fn is_client(&self, id: &NodeId) -> bool {
        self.node_types.get(id) == Some(&NodeType::Client)
    }

    pub(crate) fn is_server(&self, id: &NodeId) -> bool {
        self.node_types.get(id) == Some(&NodeType::Server)
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

    // ================================ Message Helpers ================================

    fn send_graph_update(&self, action: GraphAction) -> Result<(), ControllerError> {
        println!("🚀 CONTROLLER: Sending GraphAction: {:?}", action);
        match self.graph_action_sender.try_send(action) {
            Ok(()) => {
                println!("✅ CONTROLLER: GraphAction sent successfully");
                Ok(())
            }
            Err(e) => {
                println!("❌ CONTROLLER: FAILED to send GraphAction: {}", e);
                println!("❌ CONTROLLER: graph_action_sender disconnected!");
                Err(ControllerError::ChannelSend(format!("Graph update failed: {}", e)))
            }
        }
    }

    pub(crate) fn send_success_message(&self, msg: &str) {
        println!("📤 CONTROLLER: Attempting to send success message: {}", msg);
        match self.message_sender.try_send(MessageType::Ok(msg.to_string())) {
            Ok(()) => println!("✅ CONTROLLER: Success message sent successfully"),
            Err(e) => {
                println!("❌ CONTROLLER: FAILED to send success message: {}", e);
                println!("❌ CONTROLLER: message_sender disconnected!");
            }
        }
    }

    pub(crate) fn send_info_message(&self, msg: &str) {
        println!("📤 CONTROLLER: Attempting to send info message: {}", msg);
        match self.message_sender.try_send(PacketSent(msg.to_string())) {
            Ok(()) => println!("✅ CONTROLLER: Info message sent successfully"),
            Err(e) => {
                println!("❌ CONTROLLER: FAILED to send info message: {}", e);
                println!("❌ CONTROLLER: message_sender disconnected!");
            }
        }
    }

    pub(crate) fn send_error_message(&self, msg: &str) {
        println!("📤 CONTROLLER: Attempting to send error message: {}", msg);
        match self.message_sender.try_send(Error(msg.to_string())) {
            Ok(()) => println!("✅ CONTROLLER: Error message sent successfully"),
            Err(e) => {
                println!("❌ CONTROLLER: FAILED to send error message: {}", e);
                println!("❌ CONTROLLER: message_sender disconnected!");
            }
        }
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