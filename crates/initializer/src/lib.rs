use client::comunication::{FromUiCommunication, ToUICommunication};
use client::worker::Worker;
use crossbeam_channel::{unbounded, Receiver, Sender};
use message::{NodeCommand, NodeEvent};
use server::ChatServer;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::{fs, thread};
use std::thread::JoinHandle;
use thiserror::Error;
use wg_2024::config::{Config, Drone};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

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
use controller::{ButtonEvent, DroneGroup, GraphAction, MessageType, NodeType};
use wg_2024::drone::Drone as DroneTrait;
use client::ui::{ClientState};
use controller::controller_handler::ControllerHandler;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("I/O error reading file: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parsing error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("Validation error: {0}")]
    Validation(String),
}

pub fn parse_config<P: AsRef<Path>>(path: P) -> Result<Config, ConfigError> {
    let content = fs::read_to_string(path)?;
    let cfg: Config = toml::from_str(&content)?;
    validate(&cfg)?;
    Ok(cfg)
}

//checks if the Network Initialization File meets the needed requirements
fn validate(cfg: &Config) -> Result<(), ConfigError> {
    are_ids_unique(cfg)?;

    check_drone_requirement(cfg)?;

    check_client_requirements(cfg)?;

    check_server_requirements(cfg)?;

    if !is_connected(cfg) {
        return Err(ConfigError::Validation(
            "The graph is not connected".to_string(),
        ));
    }

    if !is_bidirectional(cfg) {
        return Err(ConfigError::Validation(
            "The graph is not bidirectional".to_string(),
        ));
    }

    if !are_client_server_at_edge(cfg) {
        return Err(ConfigError::Validation(
            "Clients and/ or severs aren't at the edge of the network".to_string(),
        ));
    }

    Ok(())
}

pub fn start<P: AsRef<Path>>(config_path: P) -> Result<(
    HashMap<NodeId, (Sender<ToUICommunication>, Receiver<ToUICommunication>)>,
    HashMap<NodeId, (Sender<FromUiCommunication>, Receiver<FromUiCommunication>)>,
    Sender<ButtonEvent>, Receiver<GraphAction>, Receiver<MessageType>,
    Sender<MessageType>,Receiver<(NodeId, ClientState)>, HashMap<NodeId, Vec<NodeId>>,
    HashMap<NodeId, NodeType>), Box<dyn std::error::Error>>{
    let cfg = parse_config(config_path)?;
    let mut packet_senders: HashMap<NodeId, Sender<Packet>> = HashMap::new();

    let mut sender_receiver_pair_drone_event: HashMap<NodeId, (Sender<DroneEvent>, Receiver<DroneEvent>)> = HashMap::new();
    let mut sender_receiver_drone_command: HashMap<NodeId, (Sender<DroneCommand>, Receiver<DroneCommand>)> = HashMap::new();
    let mut sender_receiver_packet: HashMap<NodeId, (Sender<Packet>, Receiver<Packet>)> = HashMap::new();
    let mut sender_receiver_node_command: HashMap<NodeId, (Sender<NodeCommand>, Receiver<NodeCommand>)> = HashMap::new();
    let mut sender_receiver_node_event: HashMap<NodeId, (Sender<NodeEvent>, Receiver<NodeEvent>)> = HashMap::new();
    let mut sender_receiver_node_to_ui_communication: HashMap<NodeId, (Sender<ToUICommunication>, Receiver<ToUICommunication>)> = HashMap::new();
    let mut sender_receiver_node_from_ui_communication: HashMap<NodeId, (Sender<FromUiCommunication>, Receiver<FromUiCommunication>)> = HashMap::new();


    // Create channels for drones
    for node in cfg.drone.iter() {
        let (sender_drone_event, receiver_drone_event) = unbounded::<DroneEvent>();
        let (sender_drone_command, receiver_drone_command) = unbounded::<DroneCommand>();
        let (sender_packet, receiver_packet) = unbounded::<Packet>();
        sender_receiver_pair_drone_event.insert(node.id, (sender_drone_event, receiver_drone_event));
        sender_receiver_drone_command.insert(node.id, (sender_drone_command, receiver_drone_command));
        sender_receiver_packet.insert(node.id, (sender_packet.clone(), receiver_packet));
        packet_senders.insert(node.id, sender_packet);
    }

    // Create channels for clients
    for node in cfg.client.iter() {
        let (sender_node_event, receiver_node_event) = unbounded::<NodeEvent>();
        let (sender_node_command, receiver_node_command) = unbounded::<NodeCommand>();
        let (sender_packet, receiver_packet) = unbounded::<Packet>();
        let (sender_to_ui_communication, receiver_to_ui_communication) = unbounded::<ToUICommunication>();
        let (sender_from_ui_communication, receiver_from_ui_communication) = unbounded::<FromUiCommunication>();
        sender_receiver_node_event.insert(node.id, (sender_node_event, receiver_node_event));
        sender_receiver_node_command.insert(node.id, (sender_node_command, receiver_node_command));
        sender_receiver_packet.insert(node.id, (sender_packet.clone(), receiver_packet));
        sender_receiver_node_to_ui_communication.insert(node.id, (sender_to_ui_communication, receiver_to_ui_communication));
        sender_receiver_node_from_ui_communication.insert(node.id, (sender_from_ui_communication, receiver_from_ui_communication));
        packet_senders.insert(node.id, sender_packet);
    }

    // Create channels for servers
    for node in cfg.server.iter() {
        let (sender_node_event, receiver_node_event) = unbounded::<NodeEvent>();
        let (sender_node_command, receiver_node_command) = unbounded::<NodeCommand>();
        let (sender_packet, receiver_packet) = unbounded::<Packet>();
        sender_receiver_node_event.insert(node.id, (sender_node_event, receiver_node_event));
        sender_receiver_node_command.insert(node.id, (sender_node_command, receiver_node_command));
        sender_receiver_packet.insert(node.id, (sender_packet, receiver_packet));
    }
    let mut thread_handles: HashMap<NodeId, JoinHandle<()>> = HashMap::new();

    //let mut drones: HashMap<NodeId, Box<dyn wg_2024::drone::Drone>> = HashMap::new();
    let mut drones_types: HashMap<NodeId, DroneGroup> = HashMap::new();
    let mut drones_counter: HashMap<DroneGroup, i8> = HashMap::new();
    let mut connections: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut send_command_drone: HashMap<NodeId, Sender<DroneCommand>> = HashMap::new();
    let mut send_command_node: HashMap<NodeId, Sender<NodeCommand>> = HashMap::new();
    let mut receivers_drone_event: HashMap<NodeId, Receiver<DroneEvent>> = HashMap::new();
    let mut receivers_node_event: HashMap<NodeId, Receiver<NodeEvent>> = HashMap::new();
    let mut nodes: HashMap<NodeId, NodeType> = HashMap::new();

    let mut index = 0;

    drones_counter.insert(DroneGroup::RustInPeace, 0);
    drones_counter.insert(DroneGroup::BagelBomber, 0);
    drones_counter.insert(DroneGroup::LockheedRustin, 0);
    drones_counter.insert(DroneGroup::RollingDrone, 0);
    drones_counter.insert(DroneGroup::RustDoIt, 0);
    drones_counter.insert(DroneGroup::RustRoveri, 0);
    drones_counter.insert(DroneGroup::Rustastic, 0);
    drones_counter.insert(DroneGroup::RustBusters, 0);
    drones_counter.insert(DroneGroup::LeDronJames, 0);
    drones_counter.insert(DroneGroup::RustyDrones, 0);

    // Start drone threads
    for node in cfg.drone.iter() {
        let seder_drone_event = sender_receiver_pair_drone_event.get(&node.id)
            .ok_or(format!("Drone event receiver not found for node {}", node.id))?
            .0.clone();
        let receiver_drone_command = sender_receiver_drone_command.get(&node.id)
            .ok_or(format!("Drone command receiver not found for node {}", node.id))?
            .1.clone();
        let receiver_packet = sender_receiver_packet.get(&node.id)
            .ok_or(format!("Packet sender not found for node {}", node.id))?
            .1.clone();

        packet_senders.insert(node.id, sender_receiver_packet.get(&node.id)
            .ok_or(format!("Packet sender not found for node {}", node.id))?
            .0.clone());

        send_command_drone.insert(node.id, sender_receiver_drone_command.get(&node.id)
            .ok_or(format!("Drone command receiver not found for node {}", node.id))?
            .0.clone());

        let mut sender_packet: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        receivers_drone_event.insert(node.id, sender_receiver_pair_drone_event.get(&node.id)
            .ok_or(format!("Drone event receiver not found for node {}", node.id))?
            .1.clone());

        let node_id = node.id;

        for id in node.connected_node_ids.clone(){
            let _ = sender_packet.insert(id, sender_receiver_packet.get(&id).ok_or(format!("Packet sender not found for node {}", node.id))?.0.clone());
            connections.entry(node.id).or_insert_with(Vec::new).push(id);
        }

        nodes.insert(node.id, NodeType::Drone);

        let drone: Option<Box <dyn DroneTrait>> = match index {
            0 => {
                drones_types.insert(node_id, DroneGroup::RustInPeace);
                index = index + 1;
                *drones_counter.entry(DroneGroup::RustInPeace).or_insert(0) += 1;
                Some(Box::new(NoSoundDroneRIP::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            1 => {
                drones_types.insert(node_id, DroneGroup::BagelBomber);
                index = index + 1;
                *drones_counter.entry(DroneGroup::BagelBomber).or_insert(0) += 1;
                Some(Box::new(BagelBomber::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            2 => {
                drones_types.insert(node_id, DroneGroup::LockheedRustin);
                index = index + 1;
                *drones_counter.entry(DroneGroup::LockheedRustin).or_insert(0) += 1;
                Some(Box::new(LockheedRustin::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            3 => {
                drones_types.insert(node_id, DroneGroup::RollingDrone);
                index = index + 1;
                *drones_counter.entry(DroneGroup::RollingDrone).or_insert(0) += 1;
                Some(Box::new(RollingDrone::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            4 => {
                drones_types.insert(node_id, DroneGroup::RustDoIt);
                index = index + 1;
                *drones_counter.entry(DroneGroup::RustDoIt).or_insert(0) += 1;
                Some(Box::new(RustDoIt::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            5 => {
                drones_types.insert(node_id, DroneGroup::RustRoveri);
                index = index + 1;
                *drones_counter.entry(DroneGroup::RustRoveri).or_insert(0) += 1;
                Some(Box::new(RustRoveri::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            6 => {
                drones_types.insert(node_id, DroneGroup::Rustastic);
                index = index + 1;
                *drones_counter.entry(DroneGroup::Rustastic).or_insert(0) += 1;
                Some(Box::new(RustasticDrone::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            7 => {
                drones_types.insert(node_id, DroneGroup::RustBusters);
                index = index + 1;
                *drones_counter.entry(DroneGroup::RustBusters).or_insert(0) += 1;
                Some(Box::new(RustBustersDrone::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            8 => {
                drones_types.insert(node_id, DroneGroup::LeDronJames);
                index = index + 1;
                *drones_counter.entry(DroneGroup::LeDronJames).or_insert(0) += 1;
                Some(Box::new(LeDronJames_drone::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            }
            9 => {
                drones_types.insert(node_id, DroneGroup::RustyDrones);
                index = 0;
                *drones_counter.entry(DroneGroup::RustyDrones).or_insert(0) += 1;
                Some(Box::new(RustyDrone::new(
                    node_id,
                    seder_drone_event,
                    receiver_drone_command,
                    receiver_packet,
                    sender_packet,
                    node.pdr,
                )))
            },
            _ => {None}
        };

        if let Some(mut drone) = drone {
            let handle = thread::Builder::new()
                .name(format!("Client ID [{}]", node_id))
                .spawn(move || {
                    drone.run();
                })
                .expect("Can't spawn Drone");

            thread_handles.insert(node_id, handle);
        } else {
            eprintln!("Drone not created for index {}", index);
        }
    }

    // Start client threads
    for node in cfg.client.iter() {
        let mut sender_hash = HashMap::new();
        for &adj in &node.connected_drone_ids {
            let sender = sender_receiver_packet.get(&adj)
                .ok_or(format!("Packet sender not found for drone {}", adj))?
                .0.clone();
            sender_hash.insert(adj, sender);
            connections.entry(node.id).or_insert_with(Vec::new).push(adj);
        }

        let receiver_packet = sender_receiver_packet.get(&node.id)
            .ok_or(format!("Packet receiver not found for node {}", node.id))?
            .1.clone();
        let sender_node_event = sender_receiver_node_event.get(&node.id)
            .ok_or(format!("Node event sender not found for node {}", node.id))?
            .0.clone();
        let sender_node_to_ui_communication = sender_receiver_node_to_ui_communication.get(&node.id)
            .ok_or(format!("UI communication sender not found for node {}", node.id))?
            .0.clone();
        let receiver_node_command = sender_receiver_node_command.get(&node.id)
            .ok_or(format!("Node command receiver not found for node {}", node.id))?
            .1.clone();
        let receiver_node_from_ui_communication = sender_receiver_node_from_ui_communication.get(&node.id)
            .ok_or(format!("UI communication receiver not found for node {}", node.id))?
            .1.clone();

        send_command_node.insert(node.id, sender_receiver_node_command.get(&node.id)
            .ok_or(format!("Node command receiver not found for node {}", node.id))?
            .0.clone());

        receivers_node_event.insert(node.id, sender_receiver_node_event.get(&node.id)
            .ok_or(format!("Node event sender not found for node {}", node.id))?
            .1.clone());

        nodes.insert(node.id, NodeType::Client);

        let node_id = node.id;

        let handle = thread::Builder::new()
            .name(format!("Client ID [{}]", node_id))
            .spawn(move || {
                let mut node = Worker::new(
                    node_id,
                    sender_hash,
                    sender_node_event,
                    sender_node_to_ui_communication,
                    receiver_packet,
                    receiver_node_command,
                    receiver_node_from_ui_communication,
                );
                node.run();
            })
            .expect("Impossibile spawnare il thread Worker");

        thread_handles.insert(node_id, handle);
    }
    
    for node in cfg.server.iter() {
        let mut sender_hash = HashMap::new();
        for &adj in &node.connected_drone_ids {
            let sender = sender_receiver_packet.get(&adj)
                .ok_or(format!("Packet sender not found for drone {}", adj))?
                .0.clone();
            sender_hash.insert(adj, sender);
            connections.entry(node.id).or_insert_with(Vec::new).push(adj);
        }

        let receiver_packet = sender_receiver_packet.get(&node.id)
            .ok_or(format!("Packet receiver not found for node {}", node.id))?
            .1.clone();
        let sender_node_event = sender_receiver_node_event.get(&node.id)
            .ok_or(format!("Node event sender not found for node {}", node.id))?
            .0.clone();
        let receiver_node_command = sender_receiver_node_command.get(&node.id)
            .ok_or(format!("Node command receiver not found for node {}", node.id))?
            .1.clone();

        send_command_node.insert(node.id, sender_receiver_node_command.get(&node.id)
            .ok_or(format!("Node command receiver not found for node {}", node.id))?
            .0.clone());

        receivers_node_event.insert(node.id, sender_receiver_node_event.get(&node.id)
            .ok_or(format!("Node event sender not found for node {}", node.id))?
            .1.clone());
        
        packet_senders.insert(node.id, sender_receiver_packet.get(&node.id)
            .ok_or(format!("Packet sender not found for node {}", node.id))?
            .0.clone());

        nodes.insert(node.id, NodeType::Server);

        let node_id = node.id;
        let handle = thread::Builder::new().name(format!("Server ID [{}]", node_id)).spawn(move || {
            let mut node = ChatServer::new(
                node_id,
                sender_node_event,
                receiver_node_command,
                receiver_packet,
                sender_hash
            );
            node.run();
        }).expect("Can't spawn thread ChatServer");
        thread_handles.insert(node_id, handle);
    }

    let (button_sender, button_receiver) = unbounded::<ButtonEvent>();
    let (graph_action_sender, graph_action_receiver) = unbounded::<GraphAction>();
    let (message_sender, message_receiver) = unbounded::<MessageType>();
    let (client_state_sender, client_state_receiver) = unbounded::<(NodeId, ClientState)>();

    let cloned_node = nodes.clone();
    let cloned_connections = connections.clone();

    let mut controller_handler: ControllerHandler = ControllerHandler::new(
        cloned_node, drones_types, packet_senders, cloned_connections,
        send_command_drone, send_command_node, receivers_drone_event,
        receivers_node_event, button_receiver, graph_action_sender,
        message_sender.clone(), client_state_sender,
        drones_counter,
        thread_handles,
    );
    
    let controller_handle = thread::spawn(move || {
        controller_handler.run();
    });

    Ok((sender_receiver_node_to_ui_communication, sender_receiver_node_from_ui_communication,button_sender, graph_action_receiver,
        message_receiver,message_sender, client_state_receiver, connections, nodes))
}

//the Network Initialization File should represent a connected graph
fn is_connected(config: &Config) -> bool {

    let mut all_existing_nodes: HashSet<NodeId> = HashSet::new();
    let mut node_connections: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();


    for drone in &config.drone {
        all_existing_nodes.insert(drone.id);
        node_connections.insert(drone.id, HashSet::new());
    }
    for client in &config.client {
        all_existing_nodes.insert(client.id);
        node_connections.insert(client.id, HashSet::new());
    }
    for server in &config.server {
        all_existing_nodes.insert(server.id);
        node_connections.insert(server.id, HashSet::new());
    }

    for drone in &config.drone {
        let connections = node_connections.get_mut(&drone.id).unwrap();
        for &connected_id in &drone.connected_node_ids {
            if all_existing_nodes.contains(&connected_id) {
                connections.insert(connected_id);
            }
        }
    }
    for client in &config.client {
        let connections = node_connections.get_mut(&client.id).unwrap();
        for &connected_id in &client.connected_drone_ids {
            if all_existing_nodes.contains(&connected_id) {
                connections.insert(connected_id);
            }
        }
    }
    for server in &config.server {
        let connections = node_connections.get_mut(&server.id).unwrap();
        for &connected_id in &server.connected_drone_ids {
            if all_existing_nodes.contains(&connected_id) {
                connections.insert(connected_id);
            }
        }
    }

    //if there are no nodes is connected
    if all_existing_nodes.is_empty() {
        return true;
    }

    // DFS
    let start_node = *all_existing_nodes.iter().next().unwrap();

    let mut visited = HashSet::new();
    let mut to_visit = vec![start_node];

    while let Some(current) = to_visit.pop() {
        if visited.insert(current) {
            if let Some(neighbors) = node_connections.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        to_visit.push(neighbor);
                    }
                }
            }
        }
    }

    visited == all_existing_nodes
}


//The Network Initialization File should represent a bidirectional graph
fn is_bidirectional(cfg: &Config) -> bool {
    let mut edges: HashMap<NodeId, HashSet<NodeId>> = HashMap::new();

    for drone in &cfg.drone {
        edges
            .entry(drone.id)
            .or_insert_with(HashSet::new)
            .extend(&drone.connected_node_ids);
    }
    for client in &cfg.client {
        edges
            .entry(client.id)
            .or_insert_with(HashSet::new)
            .extend(&client.connected_drone_ids);
    }
    for server in &cfg.server {
        edges
            .entry(server.id)
            .or_insert_with(HashSet::new)
            .extend(&server.connected_drone_ids);
    }
    for (node1, connections1) in &edges {
        for node2 in connections1 {
            if let Some(connections2) = edges.get(node2) {
                if !connections2.contains(node1) {
                    return false;
                }
            } else {
                return false;
            }
        }
    }
    true
}

//the Network Initialization File should represent a network where clients and servers are at the edges of the network
fn are_client_server_at_edge(cfg: &Config) -> bool {
    let drone_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();

    let cleaned_drones = cfg
        .drone
        .iter()
        .map(|drone| {
            let filtered_ids = drone
                .connected_node_ids
                .iter()
                .cloned()
                .filter(|id| drone_ids.contains(id))
                .collect();

            Drone {
                id: drone.id,
                connected_node_ids: filtered_ids,
                pdr: drone.pdr,
            }
        })
        .collect();

    let config_only_drones = Config {
        drone: cleaned_drones,
        client: vec![],
        server: vec![],
    };

    is_connected(&config_only_drones)
}

//The Network Initialization File should never contain two nodes with the same node_id value
fn are_ids_unique(cfg: &Config) -> Result<(), ConfigError> {
    let mut seen = HashSet::new();

    for drone in &cfg.drone {
        if !seen.insert(drone.id) {
            return Err(ConfigError::Validation(format!(
                "The id = [{}] is duplicated",
                drone.id
            )));
        }
    }

    for client in &cfg.client {
        if !seen.insert(client.id) {
            return Err(ConfigError::Validation(format!(
                "The id = [{}] is duplicated",
                client.id
            )));
        }
    }

    for server in &cfg.server {
        if !seen.insert(server.id) {
            return Err(ConfigError::Validation(format!(
                "The id = [{}] is duplicated",
                server.id
            )));
        }
    }

    Ok(())
}

fn check_drone_requirement(cfg: &Config) -> Result<(), ConfigError> {
    let all_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for drone in &cfg.drone {
        let mut seen = HashSet::new();
        for id in &drone.connected_node_ids {
            if !seen.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] has a duplicate in the connected_node_ids list: [{}]",
                    drone.id, id
                )));
            }

            if drone.id == *id {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] cannot be connected to itself",
                    drone.id
                )));
            }

            if !all_ids.contains(id) && !client_ids.contains(id) && !server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The drone with id = [{}] is connected to an unknown node id = [{}]",
                    drone.id, id
                )));
            }
        }

        // You might want to define a minimum number of connections (example: at least 1)
        // if drone.connected_node_ids.is_empty() {
        //     return Err(ConfigError::Validation(format!(
        //         "The drone with id = [{}] must be connected to at least one node",
        //         drone.id
        //     )));
        // }
    }

    Ok(())
}

fn check_client_requirements(cfg: &Config) -> Result<(), ConfigError> {
    let drone_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for client in &cfg.client {
        let mut seen_ids = HashSet::new();

        //a client cannot connect to other clients or servers
        for id in &client.connected_drone_ids {
            if client_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] cannot be connected to another client (with id = [{}])",
                    client.id, id
                )));
            }

            if server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] cannot be connected to a server (with id = [{}])",
                    client.id, id
                )));
            }

            //connected_drone_ids cannot contain repetitions
            if !seen_ids.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] has a duplicate in the drone's list, which is: id = [{}]",
                    client.id, id
                )));
            }

            //checks if the node really exists
            if !drone_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The client with id = [{}] is connected to the id = [{}] which is not valid",
                    client.id, id
                )));
            }
        }

        //a client can be connected to at least one and at most two drones
        let count = client.connected_drone_ids.len();
        if count == 0 || count > 2 {
            return Err(ConfigError::Validation(format!(
                "The client with id = [{}] can be connected to at least one and at most two drones but found: {}",
                client.id, count
            )));
        }
    }

    Ok(())
}

fn check_server_requirements(cfg: &Config) -> Result<(), ConfigError> {
    let drone_ids: HashSet<_> = cfg.drone.iter().map(|d| d.id).collect();
    let client_ids: HashSet<_> = cfg.client.iter().map(|c| c.id).collect();
    let server_ids: HashSet<_> = cfg.server.iter().map(|s| s.id).collect();

    for server in &cfg.server {
        let mut seen_ids = HashSet::new();

        //a server cannot connect to other clients or servers
        for id in &server.connected_drone_ids {
            if client_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] cannot be connected to a client (with id = [{}])",
                    server.id, id
                )));
            }

            if server_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] cannot be connected to another server (with id = [{}])",
                    server.id, id
                )));
            }

            //connected_drone_ids cannot contain repetitions
            if !seen_ids.insert(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] has a duplicate in the drone's list, which is: id = [{}]",
                    server.id, id
                )));
            }

            //checks if the node really exists
            if !drone_ids.contains(id) {
                return Err(ConfigError::Validation(format!(
                    "The server with id = [{}] is connected to the id = [{}] which is not valid",
                    server.id, id
                )));
            }
        }

        //a server should be connected to at least two drones
        let count = server.connected_drone_ids.len();
        if count < 2 {
            return Err(ConfigError::Validation(format!(
                "The server with id = [{}] should be connected to at least two drones but found: {}",
                server.id, count
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::config::{Client, Drone, Server};

    //TODO: complete TEST after they approve the PartialEq
    #[test]
    fn parse_test() {
        const FILE_CORRECT: &str = "src/test_data/input1.toml";
        // const FILE_INVALID: &str = "src/test_data/input2.toml";
        // const FILE_EMPTY: &str = "src/test_data/input3.toml";
        // test correct file
        let result = parse_config(FILE_CORRECT);
        assert!(result.is_ok(), "Failed to parse the config file");
        let config = result.unwrap();
        assert_eq!(config.drone.len(), 3);
        assert_eq!(config.drone[0].id, 1);
        assert_eq!(config.drone[0].connected_node_ids, vec![2, 3, 5]);
        assert_eq!(config.drone[0].pdr, 0.05);
        assert_eq!(config.drone[1].id, 2);
        assert_eq!(config.drone[1].connected_node_ids, vec![1, 3, 4]);
        assert_eq!(config.drone[1].pdr, 0.03);
        assert_eq!(config.drone[2].id, 3);
        assert_eq!(config.drone[2].connected_node_ids, vec![2, 1, 4]);
        assert_eq!(config.drone[2].pdr, 0.14);
        assert_eq!(config.client.len(), 2);
        assert_eq!(config.client[0].id, 4);
        assert_eq!(config.client[0].connected_drone_ids, vec![3, 2]);
        assert_eq!(config.client[1].id, 5);
        assert_eq!(config.client[1].connected_drone_ids, vec![1]);
        assert_eq!(config.server.len(), 1);
        assert_eq!(config.server[0].id, 6);
        assert_eq!(config.server[0].connected_drone_ids, vec![2, 3]);

        //TODO: parse empty and invalid file
    }

    #[test]
    fn test_is_connected_empty_graph() {
        let config = Config {
            drone: vec![],
            client: vec![],
            server: vec![],
        };
        assert!(
            is_connected(&config),
            "Empty graph should be considered connected."
        );
    }

    #[test]
    fn test_is_connected_single_node() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![],
            server: vec![],
        };
        assert!(
            is_connected(&config),
            "Single-node graph should be considered connected."
        );
    }

    #[test]
    fn test_is_connected_connected_graph() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![2],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![3],
                    pdr: 0.1,
                },
                Drone {
                    id: 3,
                    connected_node_ids: vec![1],
                    pdr: 0.1,
                },
            ],
            client: vec![],
            server: vec![],
        };
        assert!(is_connected(&config), "Graph should be connected.");
    }

    #[test]
    fn test_is_connected_disconnected_graph() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![2],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![],
            }],
            server: vec![],
        };
        assert!(!is_connected(&config), "Graph should not be connected.");
    }

    #[test]
    fn test_non_unique_ids() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 1,
                connected_drone_ids: vec![],
            }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The id = [1] is duplicated"));
        }
    }

    #[test]
    fn test_drone_duplicate_connection() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![2, 2],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![],
            }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains(
                    "The drone with id = [1] has a duplicate in the connected_node_ids list: [2]"
                ),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_drone_self_connection() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![1],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![],
            }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains("The drone with id = [1] cannot be connected to itself"),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_drone_unknown_connection() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![42],
                pdr: 0.1,
            }],
            client: vec![],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains("The drone with id = [1] is connected to an unknown node id = [42]"),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_client_connected_to_client() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![
                Client {
                    id: 2,
                    connected_drone_ids: vec![3],
                },
                Client {
                    id: 3,
                    connected_drone_ids: vec![],
                },
            ],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] cannot be connected to another client (with id = [3])"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_with_invalid_drone_count() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 3,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 4,
                connected_drone_ids: vec![1, 2, 3],
            }],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [4] can be connected to at least one and at most two drones but found: 3"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_connected_to_server() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![3],
            }],
            server: vec![Server {
                id: 3,
                connected_drone_ids: vec![1],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains(
                    "The client with id = [2] cannot be connected to a server (with id = [3])"
                ),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_duplicate_client_connection() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![1, 1],
            }],
            server: vec![Server {
                id: 3,
                connected_drone_ids: vec![1],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The client with id = [2] has a duplicate in the drone's list, which is: id = [1]"), "got: {msg}");
        }
    }

    #[test]
    fn test_invalid_client_connection() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![1, 4],
            }],
            server: vec![Server {
                id: 3,
                connected_drone_ids: vec![1],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains(
                    "The client with id = [2] is connected to the id = [4] which is not valid"
                ),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_server_connected_to_client() {
        let config = Config {
            drone: vec![Drone {
                id: 1,
                connected_node_ids: vec![],
                pdr: 0.1,
            }],
            client: vec![Client {
                id: 2,
                connected_drone_ids: vec![1],
            }],
            server: vec![Server {
                id: 3,
                connected_drone_ids: vec![2],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains(
                    "The server with id = [3] cannot be connected to a client (with id = [2])"
                ),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_server_connected_to_server() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1],
            }],
            server: vec![
                Server {
                    id: 4,
                    connected_drone_ids: vec![1, 2],
                },
                Server {
                    id: 5,
                    connected_drone_ids: vec![4],
                },
            ],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [5] cannot be connected to another server (with id = [4])"), "got: {msg}");
        }
    }

    #[test]
    fn test_duplicate_server_connection() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1],
            }],
            server: vec![
                Server {
                    id: 4,
                    connected_drone_ids: vec![1, 1],
                },
                Server {
                    id: 5,
                    connected_drone_ids: vec![4],
                },
            ],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [4] has a duplicate in the drone's list, which is: id = [1]"), "got: {msg}");
        }
    }

    #[test]
    fn test_invalid_server_connection() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1],
            }],
            server: vec![Server {
                id: 4,
                connected_drone_ids: vec![1, 5],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(
                msg.contains(
                    "The server with id = [4] is connected to the id = [5] which is not valid"
                ),
                "got: {msg}"
            );
        }
    }

    #[test]
    fn test_server_with_invalid_drone_count() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1, 2],
            }],
            server: vec![Server {
                id: 4,
                connected_drone_ids: vec![1],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The server with id = [4] should be connected to at least two drones but found: 1"), "got: {msg}");
        }
    }

    #[test]
    fn test_non_bidirectional_graph_should_fail() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![2],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![],
                    pdr: 0.1,
                },
            ],
            client: vec![],
            server: vec![],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("The graph is not bidirectional"), "got: {msg}");
        }
    }

    #[test]
    fn test_client_server_not_at_edge() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![3, 4],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![3, 4],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1, 2],
            }],
            server: vec![Server {
                id: 4,
                connected_drone_ids: vec![1, 2],
            }],
        };

        let result = validate(&config);
        assert!(result.is_err());
        if let Err(ConfigError::Validation(msg)) = result {
            assert!(msg.contains("Clients and/ or severs aren't at the edge of the network"));
        }
    }

    #[test]
    fn test_valid_config() {
        let config = Config {
            drone: vec![
                Drone {
                    id: 1,
                    connected_node_ids: vec![2, 3, 4],
                    pdr: 0.1,
                },
                Drone {
                    id: 2,
                    connected_node_ids: vec![1, 4],
                    pdr: 0.1,
                },
            ],
            client: vec![Client {
                id: 3,
                connected_drone_ids: vec![1],
            }],
            server: vec![Server {
                id: 4,
                connected_drone_ids: vec![1, 2],
            }],
        };

        let result = validate(&config);
        assert!(result.is_ok());
    }
}
