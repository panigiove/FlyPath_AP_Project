use message::{NodeCommand, NodeEvent};
use crossbeam_channel::{Receiver, Sender, unbounded, TryRecvError};
use std::collections::HashMap;
use std::thread;
use std::thread::JoinHandle;
use thread::sleep;
use wg_2024::network::{NodeId};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use rand::{thread_rng, Rng};

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
use rand::seq::SliceRandom;
use wg_2024::drone::Drone;
use message::NodeCommand::FromShortcut;

use client;
use client::communication::{FromUiCommunication, ToUICommunication};
use client::ui::{ClientState};
use client::worker::Worker;
use server::ChatServer;

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
    node_types: HashMap<NodeId, NodeType>,
    drones_types: HashMap<NodeId, DroneGroup>,

    packet_senders: HashMap<NodeId, Sender<Packet>>,
    connections: HashMap<NodeId, Vec<NodeId>>,
    send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    receiver_event: HashMap<NodeId, Receiver<DroneEvent>>,
    receiver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,

    button_receiver: Receiver<ButtonEvent>,
    graph_action_sender: Sender<GraphAction>,
    message_sender: Sender<MessageType>,
    client_state_sender: Sender<(NodeId, ClientState)>,

    drones_counter: HashMap<DroneGroup, i8>,

    thread_handler: HashMap<NodeId, JoinHandle<()>>,
}

impl ControllerHandler {
    pub fn new(
        node_types: HashMap<NodeId, NodeType>,
        drones_types: HashMap<NodeId, DroneGroup>,
        packet_senders: HashMap<NodeId, Sender<Packet>>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
        send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
        receiver_event: HashMap<NodeId, Receiver<DroneEvent>>,
        receiver_node_event: HashMap<NodeId, Receiver<NodeEvent>>,
        button_receiver: Receiver<ButtonEvent>,
        graph_action_sender: Sender<GraphAction>,
        message_sender: Sender<MessageType>,
        client_state_sender: Sender<(NodeId, ClientState)>,
        drones_counter: HashMap<DroneGroup, i8>,
        thread_handler: HashMap<NodeId, JoinHandle<()>>,
    ) -> Self {

        Self {
            node_types,
            drones_types,
            packet_senders,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receiver_node_event,
            button_receiver,
            graph_action_sender,
            message_sender,
            drones_counter,
            client_state_sender,
            thread_handler,
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

            let node_node_ids: Vec<NodeId> = self.receiver_node_event.keys().copied().collect();
            let mut node_events_processed = 0;
            for node_id in node_node_ids {
                let mut events_to_process = Vec::new();
                if let Some(receiver) = self.receiver_node_event.get(&node_id) {
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
                sleep(std::time::Duration::from_millis(1));
            } else {
                thread::yield_now();
            }
        }
    }

    // ================================ Event Handlers ================================

    pub fn handle_button_event(&mut self, event: ButtonEvent) {
        sleep(std::time::Duration::from_millis(10));

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
                self.add_connection(&id1, &id2)
            },
            ButtonEvent::Crash(id) => {
                self.crash_drone(&id)
            },
            ButtonEvent::RemoveConection(id1, id2) => {
                self.remove_connection(&id1, &id2)
            },
            ButtonEvent::ChangePdr(id, pdr) => {
                self.change_packet_drop_rate(&id, pdr)
            },
        };

        if let Err(e) = result {
            let error_msg = e.to_string();
            self.send_error_message(&error_msg);

            sleep(std::time::Duration::from_millis(50));
        } else {

            sleep(std::time::Duration::from_millis(50));
        }
    }

    fn handle_node_event(&mut self, event: NodeEvent, node_id: NodeId) {
        match event {
            NodeEvent::PacketSent(c) => {
                let message = format!("The node ID [{}] has sent a packet {}", node_id, c);
                let _ = self.message_sender.try_send(MessageType::Info(message));
            },
            NodeEvent::CreateMessage(_c) => {
                let message = "Message created".to_string();
                let _ = self.message_sender.try_send(MessageType::Info(message));
            },
            NodeEvent::MessageRecv(_c) => {
                let message = "Message received".to_string();
                let _ = self.message_sender.try_send(MessageType::Info(message));
            },
            NodeEvent::ControllerShortcut(packet) => {
                if let Err(e) = self.send_packet_to_client(packet) {
                    self.send_error_message(&format!("Failed to send shortcut packet [{}]", e));
                }
            }
        }
    }

    pub fn handle_drone_event(&mut self, event: DroneEvent, drone_id: NodeId) {
        match event {
            DroneEvent::PacketSent(packet) => {
                let msg = format!("Drone ID [{}] successfully sent packet with session ID [{}]",
                                  drone_id, packet.session_id);
                let _ = self.message_sender.try_send(MessageType::Packet(msg));
            }
            DroneEvent::PacketDropped(packet) => {
                let msg = format!("Drone ID [{}] dropped packet with session ID [{}]",
                                  drone_id, packet.session_id);
                let _ = self.message_sender.try_send(MessageType::Packet(msg));
            }
            DroneEvent::ControllerShortcut(packet) => {
                if let Err(e) = self.send_packet_to_client(packet) {
                    self.send_error_message(&format!("Failed to send shortcut packet [{}]", e));
                }
            }
        }
    }


    // ================================ Drone Operations ================================

    fn spawn_drone(&mut self, first_connection: &NodeId, pdr: f32) -> Result<(), ControllerError> {
        let id = self.generate_random_id()?;


        if !self.packet_senders.contains_key(first_connection) {
            return Err(ControllerError::NodeNotFound(*first_connection));
        }

        if !self.check_network_before_add_drone(&id, &vec![*first_connection]) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add a new drone connected to {}", first_connection)
            ));
        }

        self.node_types.insert(id, NodeType::Drone);

        //creating the drone without connections
        if let Err(e) = self.create_drone_without_connection(id, pdr) {
            self.node_types.remove(&id);
            return Err(e);
        }

        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Drone)) {
            self.cleanup_drone(&id);
            return Err(e);
        }

        // adding the first connection
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

        let senders = HashMap::new();
        let drone_group = self.create_balanced_drone(
            id, sender_event, receiver_drone_command, receiver_packet, senders, pdr
        )?;

        //insert all drone-related data
        self.drones_types.insert(id, drone_group);
        self.packet_senders.insert(id, sender_packet);
        self.connections.insert(id, Vec::new());
        self.send_command_drone.insert(id, sender_drone_command.clone());
        self.receiver_event.insert(id, receiver_event);

        sleep(std::time::Duration::from_millis(100));

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

        let handle = thread::Builder::new()
            .name(format!("Drone ID [{}]", id))
            .spawn(move || {
                drone.run();
            })
            .expect("Can't spawn Drone");
        self.thread_handler.insert(id, handle);
        Ok(drone_group)
    }

    pub fn select_drone_group(&self) -> Option<&DroneGroup> {

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

    fn crash_drone(&mut self, id: &NodeId) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node ID [{}] is not a drone", id)
            ));
        }

        if !self.check_network_before_removing_drone(id) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot remove drone ID [{}]", id)
            ));
        }

        if let Some(sender) = self.send_command_drone.get(id) {
            sender.send(DroneCommand::Crash)
                .map_err(|e| ControllerError::ChannelSend(format!("Failed to send crash command: {}", e)))?;
        }

        sleep(std::time::Duration::from_millis(150));

        if let Some(connections) = self.connections.get(id).cloned() {
            for neighbor_id in &connections {
                let _ = self.send_graph_update(RemoveEdge(*id, *neighbor_id));
            }
        }

        self.remove_all_connections(id)?;

        self.drain_event_channel_improved(id)?;

        if let Some(handle) = self.thread_handler.remove(id) {
            for attempt in 0..20 { // Max 1 s
                if handle.is_finished() {
                    let _ = handle.join();
                    break;
                }

                if attempt < 19 {
                    sleep(std::time::Duration::from_millis(50));
                }
            }
        }
        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_drone.remove(id);
        self.receiver_event.remove(id);

        if let Some(drone_group) = self.drones_types.remove(id) {
            if let Some(count) = self.drones_counter.get_mut(&drone_group) {
                *count -= 1;
            }
        }

        self.node_types.remove(id);

        self.send_graph_update(RemoveNode(*id))?;

        sleep(std::time::Duration::from_millis(50));

        self.send_success_message(&format!("Drone ID [{}] removed successfully", id));

        Ok(())
    }

    fn drain_event_channel_improved(&mut self, drone_id: &NodeId) -> Result<(), ControllerError> {
        if let Some(receiver) = self.receiver_event.get(drone_id) {
            let mut drained_count = 0;
            let max_iterations = 100;

            loop {
                match receiver.try_recv() {
                    Ok(_event) => {
                        drained_count += 1;

                        if drained_count >= max_iterations {
                            break;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => break,
                }
            }

            if drained_count > 0 {
                sleep(std::time::Duration::from_millis(100));

                loop {
                    match receiver.try_recv() {
                        Ok(_event) => {
                            drained_count += 1;
                        }
                        Err(_) => break,
                    }
                }
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

        if let Err(e) = self.create_client_without_connection(id) {
            return Err(e);
        }

        self.node_types.insert(id, NodeType::Client);

        if let Err(e) = self.send_graph_update(AddNode(id, NodeType::Client)) {
            self.cleanup_client(&id);
            return Err(e);
        }

        if let Err(e) = self.add_connection(&id, &id_connection) {
            let _ = self.send_graph_update(RemoveNode(id));
            self.cleanup_client(&id);
            return Err(e);
        }

        self.send_success_message(format!("New client added successfully with ID [{}]", id).as_str());
        Ok(())
    }

    fn create_client_without_connection(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();
        let (ui_communication_send, ui_communication_receiver) = unbounded::<ToUICommunication>();
        let (from_ui_communication_send, from_ui_communication_receiver) = unbounded::<FromUiCommunication>();

        let client_state = ClientState::new(id, ui_communication_receiver, from_ui_communication_send);

        if !self.send_client_state_safe(id, client_state) {
            return Err(ControllerError::InvalidOperation(
                "Client can't be added to the ui due to channel issue".to_string()
            ));
        }

        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send.clone());
        self.receiver_node_event.insert(id, node_event_receiver);

        let handle = thread::Builder::new()
            .name(format!("Client ID [{}]", id))
            .spawn(move || {
                let packet_send = HashMap::new();
                let mut worker = Worker::new(
                    id,
                    packet_send,
                    node_event_send,
                    ui_communication_send,
                    p_receiver,
                    node_command_receiver,
                    from_ui_communication_receiver
                );
                worker.run();
            })
            .expect("Can't spawn thread Worker");
        self.thread_handler.insert(id, handle);

        Ok(())
    }

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

        match self.add_server_connections(&id, &drone1, &drone2) {
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

    fn create_server_without_connection(&mut self, id: NodeId) -> Result<(), ControllerError> {
        let (p_send, p_receiver) = unbounded::<Packet>();
        let (node_event_send, node_event_receiver) = unbounded::<NodeEvent>();
        let (node_command_send, node_command_receiver) = unbounded::<NodeCommand>();

        self.packet_senders.insert(id, p_send);
        self.connections.insert(id, Vec::new());
        self.send_command_node.insert(id, node_command_send.clone());
        self.receiver_node_event.insert(id, node_event_receiver);

        let handle = thread::Builder::new()
            .name(format!("Server [{}]", id))
            .spawn(move || {
                let packet_send = HashMap::new();
                let mut chat_server = ChatServer::new(
                    id,
                    node_event_send,
                    node_command_receiver,
                    p_receiver,
                    packet_send
                );
                chat_server.run();
            })
            .expect("Can't spawn Server");

        self.thread_handler.insert(id, handle);

        Ok(())
    }

    // ================================ Connection Management ================================

    fn add_connection(&mut self, id1: &NodeId, id2: &NodeId) -> Result<(), ControllerError> {

        if !self.validate_connection_types(id1, id2) {
            let error_msg = self.get_connection_error_message(id1, id2);
            return Err(ControllerError::NetworkConstraintViolation(error_msg));
        }

        if !self.check_network_before_add_connection(id1, id2) {
            return Err(ControllerError::NetworkConstraintViolation(
                format!("Cannot add connection between Node ID [{}] and Node ID [{}]", id1, id2)
            ));
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
                sleep(std::time::Duration::from_millis(50));
            }
        }

        if !id1_ready {
            return Err(ControllerError::NodeNotFound(*id1));
        }
        if !id2_ready {
            return Err(ControllerError::NodeNotFound(*id2));
        }

        if let Err(e) = self.add_sender_safe(id1, id2) {
            return Err(e);
        }

        if let Err(e) = self.add_sender_safe(id2, id1) {
            //rollback first sender if second fails
            let _ = self.remove_sender(id1, id2);
            return Err(e);
        }

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

        self.send_graph_update(AddEdge(*id1, *id2))?;
        self.send_success_message(&format!("Connection added between Node ID [{}] and Node ID [{}]", id1, id2));

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

    // ================================ Connection Validation ================================

    fn validate_connection_types(&self, id1: &NodeId, id2: &NodeId) -> bool {
        let node1_type = self.node_types.get(id1);
        let node2_type = self.node_types.get(id2);

        match (node1_type, node2_type) {
            (Some(NodeType::Client), Some(NodeType::Drone)) => true,
            (Some(NodeType::Drone), Some(NodeType::Client)) => true,
            (Some(NodeType::Drone), Some(NodeType::Drone)) => true,
            (Some(NodeType::Drone), Some(NodeType::Server)) => true,
            (Some(NodeType::Server), Some(NodeType::Drone)) => true,

            //forbidden connections
            (Some(NodeType::Client), Some(NodeType::Client)) => false,
            (Some(NodeType::Client), Some(NodeType::Server)) => false,
            (Some(NodeType::Server), Some(NodeType::Client)) => false,
            (Some(NodeType::Server), Some(NodeType::Server)) => false,

            _ => false, //non existing nodes
        }
    }

    fn get_connection_error_message(&self, id1: &NodeId, id2: &NodeId) -> String {
        let node1_type = self.node_types.get(id1);
        let node2_type = self.node_types.get(id2);

        match (node1_type, node2_type) {
            (Some(NodeType::Client), Some(NodeType::Server)) |
            (Some(NodeType::Server), Some(NodeType::Client)) => {
                "Direct connections between Clients and Servers are not allowed.".to_string()
            }
            (Some(NodeType::Client), Some(NodeType::Client)) => {
                "Direct connections between Clients are not allowed.".to_string()
            }
            (Some(NodeType::Server), Some(NodeType::Server)) => {
                "Direct connections between Servers are not allowed.".to_string()
            }
            _ => "This type of connection is not permitted.".to_string()
        }
    }

    // ================================ Helper Methods ================================

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
                        sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Drone ID [{}] channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            Err(ControllerError::ChannelSend(
                format!("Failed to send command to Drone ID [{}] after 5 attempts", id)
            ))

        } else {
            let sender = self.send_command_node.get(id)
                .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

            for attempt in 0..5 {
                match sender.try_send(NodeCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        sleep(std::time::Duration::from_millis(25));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            if matches!(self.get_node_type(id), Some(NodeType::Server)) {
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

            Err(ControllerError::ChannelSend(
                format!("Failed to send command to node ID [{}] after 5 attempts", id)
            ))
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

        if connections.is_empty() {
            return Ok(());
        }

        for neighbor_id in connections {
            // Rimuovi i sender dal nodo verso il neighbor
            for attempt in 0..3 {
                match self.remove_sender(id, &neighbor_id) {
                    Ok(()) => break,
                    Err(_) => {
                        if attempt < 2 {
                            sleep(std::time::Duration::from_millis(50));
                        }
                    }
                }
            }

            for attempt in 0..3 {
                match self.remove_sender(&neighbor_id, id) {
                    Ok(()) => break,
                    Err(_) => {
                        if attempt < 2 {
                            sleep(std::time::Duration::from_millis(50));
                        }
                    }
                }
            }

            if let Some(neighbor_connections) = self.connections.get_mut(&neighbor_id) {
                neighbor_connections.retain(|&conn_id| conn_id != *id);
            }
        }

        Ok(())
    }

    pub(crate) fn change_packet_drop_rate(&mut self, id: &NodeId, new_pdr: f32) -> Result<(), ControllerError> {
        if !self.is_drone(id) {
            return Err(ControllerError::InvalidOperation(
                format!("Node ID [{}] is not a drone", id)
            ));
        }

        let sender = self.send_command_drone.get(id)
            .ok_or_else(|| ControllerError::NodeNotFound(*id))?;

        sender.send(DroneCommand::SetPacketDropRate(new_pdr))
            .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;

        self.send_success_message(&format!("PDR for Drone ID [{}] changed to {}", id, new_pdr));
        Ok(())
    }

    pub(crate) fn send_packet_to_client(&self, packet: Packet) -> Result<(), ControllerError> {
        let _ = packet.session_id;
        let destination = packet.routing_header.hops.last().copied();

        if let Some(destination) = destination {
            match self.get_node_type(&destination) {
                Some(NodeType::Client) => {
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                }
                Some(NodeType::Server) => {
                    let sender = self.send_command_node.get(&destination)
                        .ok_or_else(|| ControllerError::NodeNotFound(destination))?;
                    sender.try_send(FromShortcut(packet))
                        .map_err(|e| ControllerError::ChannelSend(e.to_string()))?;
                }
                Some(NodeType::Drone) => {
                    return Err(ControllerError::InvalidOperation(
                        format!("Cannot send shortcut packet to drone ID[{}]", destination)
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

    fn check_network_before_add_server_with_two_connections(&self, server_id: &NodeId, drone1: &NodeId, drone2: &NodeId) -> bool {
        let mut adj_list = self.connections.clone();

        adj_list.insert(*server_id, vec![*drone1, *drone2]);

        if let Some(drone1_connections) = adj_list.get_mut(drone1) {
            drone1_connections.push(*server_id);
        }
        if let Some(drone2_connections) = adj_list.get_mut(drone2) {
            drone2_connections.push(*server_id);
        }

        self.validate_network_constraints(&adj_list)
    }

    fn add_server_connections(&mut self, server_id: &NodeId, drone1: &NodeId, drone2: &NodeId) -> Result<(), ControllerError> {

        if !self.packet_senders.contains_key(server_id) {
            return Err(ControllerError::NodeNotFound(*server_id));
        }
        if !self.packet_senders.contains_key(drone1) {
            return Err(ControllerError::NodeNotFound(*drone1));
        }
        if !self.packet_senders.contains_key(drone2) {
            return Err(ControllerError::NodeNotFound(*drone2));
        }

        // add senders for server -> drones
        if let Err(e) = self.add_sender_without_validation(server_id, drone1) {
            return Err(e);
        }

        if let Err(e) = self.add_sender_without_validation(server_id, drone2) {
            // Rollback: remove first sender
            let _ = self.remove_sender(server_id, drone1);
            return Err(e);
        }

        // add senders for drones -> server
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

            for attempt in 0..5 {
                match sender.try_send(DroneCommand::AddSender(*dst_id, dst_sender.clone())) {
                    Ok(()) => {
                        return Ok(());
                    }
                    Err(crossbeam_channel::TrySendError::Full(_)) => {
                        sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Drone {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            Err(ControllerError::ChannelSend(
                format!("Failed to send command to drone {} after 5 attempts", id)
            ))

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
                        sleep(std::time::Duration::from_millis(50));
                        continue;
                    }
                    Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                        if attempt < 4 {
                            sleep(std::time::Duration::from_millis(100));
                            continue;
                        } else {
                            return Err(ControllerError::ChannelSend(
                                format!("Node {} channel permanently disconnected", id)
                            ));
                        }
                    }
                }
            }

            Err(ControllerError::ChannelSend(
                format!("Failed to send command to node {} after 5 attempts", id)
            ))
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
        self.receiver_node_event.remove(id);
    }

    fn complete_cleanup_server(&mut self, id: &NodeId) {

        self.node_types.remove(id);

        let connections = self.connections.get(id).cloned().unwrap_or_default();
        for neighbor_id in connections {
            if let Some(neighbor_connections) = self.connections.get_mut(&neighbor_id) {
                neighbor_connections.retain(|&conn_id| &conn_id != id);
            }

            let _ = self.remove_sender(&neighbor_id, id);
        }

        self.packet_senders.remove(id);
        self.connections.remove(id);
        self.send_command_node.remove(id);
        self.receiver_node_event.remove(id);
    }

    // ================================ Node Type Checkers ================================

    fn is_drone(&self, id: &NodeId) -> bool {
        self.node_types.get(id) == Some(&NodeType::Drone)
    }

    fn get_node_type(&self, id: &NodeId) -> Option<&NodeType> {
        self.node_types.get(id)
    }

    // ================================ Utility Methods ================================

    fn generate_random_id(&self) -> Result<NodeId, ControllerError> {
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

    fn check_network_before_removing_drone(&self, drone_id: &NodeId) -> bool {

        let mut adj_list = self.connections.clone();
        adj_list.remove(drone_id);

        // Remove drone from all neighbor connection lists
        for neighbors in adj_list.values_mut() {
            neighbors.retain(|&id| &id != drone_id);
        }

        let constraints_ok = self.validate_network_constraints(&adj_list);

        if !constraints_ok {
            return false;
        }
        let connectivity_ok = is_connected_after_removal_fixed(&adj_list);

        if !connectivity_ok {
            return false;
        }
        true
    }

    fn check_network_before_add_connection(&self, id1: &NodeId, id2: &NodeId) -> bool {
        if !self.validate_connection_types(id1, id2) {
            return false;
        }

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

        self.validate_network_constraints(&adj_list) && is_connected_after_removal_fixed(&adj_list)
    }

    fn validate_network_constraints(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
        // check client constraints (1-2 connections)
        let clients_valid = self.node_types.iter()
            .filter(|(_, &node_type)| node_type == NodeType::Client)
            .all(|(&client_id, _)| {
                adj_list.get(&client_id)
                    .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
            });

        // check server constraints (at least 2 connections)
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
        match self.graph_action_sender.try_send(action.clone()) {
            Ok(()) => Ok(()),
            Err(crossbeam_channel::TrySendError::Full(_)) => {
                // Channel pieno -> try multiple times
                for _ in 0..3 {
                    sleep(std::time::Duration::from_millis(10));
                    if self.graph_action_sender.try_send(action.clone()).is_ok() {
                        return Ok(());
                    }
                }
                Err(ControllerError::ChannelSend("Graph update channel is full".to_string()))
            }
            Err(crossbeam_channel::TrySendError::Disconnected(_)) => {
                Ok(()) // Silently ignore disconnected channels
            }
        }
    }

    fn send_success_message(&self, msg: &str) {
        match self.message_sender.try_send(MessageType::Ok(msg.to_string())) {
            Ok(()) => {
                sleep(std::time::Duration::from_millis(300));
            }
            _other => {}
        }

    }

    fn send_error_message(&self, msg: &str) {
        match self.message_sender.try_send(MessageType::Error(msg.to_string())) {
            Ok(()) => {
                sleep(std::time::Duration::from_millis(300));
            }
            _other => {}
        }
    }

    fn send_client_state_safe(&self, node_id: NodeId, client_state: ClientState) -> bool {
        self.client_state_sender.try_send((node_id, client_state)).is_ok()
    }

    pub fn test_message_channel(&self) {
        let _ = self.message_sender.try_send(MessageType::Info("Channel test message".to_string()));
    }
}

// ================================ Helper Functions ================================

pub fn is_connected_after_removal_fixed(adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let mut all_nodes = std::collections::HashSet::new();

    for &node_id in adj_list.keys() {
        all_nodes.insert(node_id);
    }

    for neighbors in adj_list.values() {
        for &neighbor_id in neighbors {
            all_nodes.insert(neighbor_id);
        }
    }

    let remaining_nodes: Vec<NodeId> = all_nodes.into_iter().collect();

    if remaining_nodes.is_empty() {
        return true;
    }

    if remaining_nodes.len() == 1 {
        return true;
    }

    let start_node = remaining_nodes[0];
    let reachable_count = count_reachable_nodes_robust(start_node, adj_list, &remaining_nodes);

    reachable_count == remaining_nodes.len()
}

fn count_reachable_nodes_robust(
    start: NodeId,
    adj_list: &HashMap<NodeId, Vec<NodeId>>,
    all_nodes: &[NodeId]
) -> usize {
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start];

    while let Some(current) = stack.pop() {
        if visited.insert(current) {
            if let Some(neighbors) = adj_list.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) && all_nodes.contains(&neighbor) {
                        stack.push(neighbor);
                    }
                }
            }
        }
    }

    visited.len()
}