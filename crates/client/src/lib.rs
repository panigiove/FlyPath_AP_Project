// TODO: network discovery protocol, da mandare per inizializzare poi ogni tot ms e poi per ogni nack
// TODO: fragmentation of high level messages
// TODO: handle ACK, NACK
mod comunication;
mod test;

use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::process;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use message::{ChatRequest, DroneSend, MediaRequest};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, Fragment, NodeType, Packet};

use comunication::{FromUiCommunication, ToUIComunication};

#[derive(Debug, Clone)]
enum Status {
    Sending,
    Ok,
}

// Send N fragment at the time
// every ack send another frag
// if dropped send again
// if other nack make again the flood_request and send from the ack not acked.
#[allow(unused)]
#[derive(Debug, Clone)]
struct FragStatus {
    session_id: u64,
    destination: NodeId,
    total_frag: u64, // total number of frag
    acked: HashSet<u64>,
    fragments: Vec<Fragment>,
    status: Status,
}

// Client make flood_request every GetServerList to upgrade, if a drone crashed or if a DestinationIsDrone/UnexpectedRecipient/ErrorInRouting, if Dropped send again
#[allow(unused)]
#[derive(Debug)]
pub struct Client {
    id: NodeId,
    node_type: NodeType,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,

    // receive user interaction and send to user messages
    ui_recv: Receiver<FromUiCommunication>,
    ui_send: Sender<ToUIComunication>,

    // network status
    adj_list: HashMap<NodeId, HashSet<NodeId>>,
    nodes_type: HashMap<NodeId, NodeType>,
    last_flood_id: u64,
    last_session_id: u64,

    // current sended fragments status
    fragment_status: HashMap<u64, FragStatus>,
}

// #[allow(unused)]
impl Client {
    // run UI on a separated thread with bidirection channel and create an instance of Client
    pub fn new(
        id: NodeId,
        node_type: NodeType,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<u8, Sender<Packet>>,
    ) -> Self {
        debug!("Creating a new Client with id: {:?}", id);
        let (_to_logic, ui_recv) = unbounded();
        let (ui_send, _from_logic) = unbounded();

        // TODO: run the ui thread on a separated thread

        Self {
            id,
            node_type,
            packet_recv,
            packet_send,
            ui_recv,
            ui_send,
            adj_list: HashMap::new(),
            nodes_type: HashMap::new(),
            last_flood_id: 0,
            last_session_id: 0,
            fragment_status: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.ui_recv) -> interaction => {
                    if let Ok(interaction) = interaction {
                        self.interaction_handler(interaction);
                    }
                },
                // TODO: pick this up next time
                // recv(self.packet_recv) -> packet => {
                    // if let Ok(packet) = packet {
                        // self.packet_handler(packet);
                    // }
                // }
            }
        }
    }

    fn interaction_handler(&mut self, interaction: FromUiCommunication) {
        match interaction {
            FromUiCommunication::GetServerList => {
                // send the current server list and make a flood_request if is empty
                let servers: Option<Vec<u8>> = self.get_servers();
                if servers.is_none() {
                    self.send_flood_request();
                }
                self.send_to_ui(ToUIComunication::ServerList(servers));
            }
            FromUiCommunication::AskServerType(server_id) => {
                if let Err(e) = self.send_raw_content("ServerType".to_string(), server_id) {
                    self.send_flood_request();
                    error!("Failed to send ServerType request: {}", e);
                }
            }
            FromUiCommunication::ReloadNetwork => self.send_flood_request(),
            FromUiCommunication::AskMedialist(server_id) => {
                let raw_message: String = MediaRequest::MediaList.stringify();
                if let Err(e) = self.send_raw_content(raw_message, server_id) {
                    self.send_flood_request();
                    error!("failed to send servertype request: {}", e);
                }
            }
            FromUiCommunication::AskMedia(server_id, media_id) => {
                let raw_message: String = MediaRequest::Media(media_id).stringify();
                if let Err(e) = self.send_raw_content(raw_message, server_id) {
                    self.send_flood_request();
                    error!("failed to send servertype request: {}", e);
                }
            }
            FromUiCommunication::AskClientList(server_id) => {
                let raw_message: String = ChatRequest::ClientList.stringify();
                if let Err(e) = self.send_raw_content(raw_message, server_id) {
                    self.send_flood_request();
                    error!("Failed to send ServerType request: {}", e);
                }
            }
            FromUiCommunication::AskRegister(server_id) => {
                let raw_message: String = ChatRequest::Register(self.id).stringify();
                if let Err(e) = self.send_raw_content(raw_message, server_id) {
                    self.send_flood_request();
                    error!("Failed to send ServerType request: {}", e);
                }
            }
            FromUiCommunication::SendMessage {
                server_id,
                to: destination,
                message,
            } => {
                let raw_message: String = ChatRequest::SendMessage {
                    from: self.id,
                    to: destination,
                    message,
                }
                .stringify();
                if let Err(e) = self.send_raw_content(raw_message, server_id) {
                    self.send_flood_request();
                    error!("Failed to send ServerType request: {}", e);
                }
            }
        }
    }

    // fn packet_handler(&mut self, packet: Packet) {
    //     let packet_session_id = packet.session_id;
    //     let packet_routing_header = packet.routing_header;
    //     match packet.pack_type {
    //         _ => {}
    //     }
    // }

    // fragment are sended in style of TCP ack too limitated the number of NACK in case of DestinationIsDrone/UnexpectedRecipient/ErrorInRouting
    fn send_raw_content(&mut self, raw_content: String, destination: NodeId) -> Result<(), String> {
        const FRAGMENT_SIZE: usize = 128;

        let raw_bytes = raw_content.into_bytes();
        self.last_session_id += 1;
        let total_frag = raw_bytes.len().div_ceil(FRAGMENT_SIZE) as u64;
        let mut fragment_status = FragStatus {
            session_id: self.last_session_id,
            destination,
            acked: HashSet::new(),
            total_frag,
            fragments: raw_bytes
                .chunks(FRAGMENT_SIZE)
                .enumerate()
                .map(|(i, chunk)| {
                    let mut data = [0; FRAGMENT_SIZE];
                    data[..chunk.len()].copy_from_slice(chunk);
                    Fragment {
                        fragment_index: i as u64,
                        total_n_fragments: total_frag,
                        length: chunk.len() as u8,
                        data,
                    }
                })
                .collect(),
            status: Status::Sending,
        };

        self.fragment_status
            .insert(fragment_status.session_id, fragment_status.clone());

        // attempt to send fragments
        if let Some(path_trace) = self.elaborate_path_trace(destination) {
            let source_routing: SourceRoutingHeader =
                SourceRoutingHeader::with_first_hop(path_trace);

            for fragment in fragment_status.fragments.iter() {
                let packet: Packet = Packet::new_fragment(
                    source_routing.clone(),
                    fragment_status.session_id,
                    fragment.clone(),
                );

                match source_routing
                    .current_hop()
                    .and_then(|next_hop: u8| self.packet_send.get(&next_hop))
                {
                    Some(sender) if sender.send(packet).is_ok() => continue,
                    _ => {
                        return Err("Sender not found for next hop".to_string());
                    }
                }
            }
        } else {
            return Err(format!("Destination {:?} is unreachable", destination));
        }

        fragment_status.status = Status::Ok;
        self.fragment_status
            .insert(fragment_status.session_id, fragment_status.clone());
        Ok(())
    }

    fn send_to_ui(&self, response: ToUIComunication) {
        if self.ui_send.send(response).is_err() {
            process::exit(1); // problem with UI
        }
    }

    // -------------------------- other logic function ------------------------------------
    fn get_servers(&self) -> Option<Vec<NodeId>> {
        let mut servers = Vec::new();
        for (node_id, node_type) in self.nodes_type.iter() {
            if *node_type == NodeType::Server {
                servers.push(*node_id);
            }
        }

        if servers.is_empty() {
            None
        } else {
            Some(servers)
        }
    }

    fn elaborate_path_trace(&self, destination: NodeId) -> Option<Vec<NodeId>> {
        if !self.adj_list.contains_key(&destination) {
            return None;
        }

        let mut queue: Vec<NodeId> = vec![self.id];
        let mut visited: HashSet<NodeId> = HashSet::new();
        let mut predecessor: HashMap<NodeId, NodeId> = HashMap::new();

        visited.insert(self.id);

        while let Some(current) = queue.pop() {
            if current == destination {
                let mut path = Vec::new();
                let mut node = destination;
                while let Some(&prev) = predecessor.get(&node) {
                    path.push(node);
                    node = prev;
                }
                path.push(self.id);
                path.reverse();
                return Some(path);
            }

            // Visit neighbors
            if let Some(neighbors) = self.adj_list.get(&current) {
                for &neighbor in neighbors {
                    if !visited.contains(&neighbor) {
                        visited.insert(neighbor);
                        predecessor.insert(neighbor, current);
                        queue.push(neighbor);
                    }
                }
            }
        }

        None
    }

    // --------------------------- message with server -------------------------------------
    // pub fn ask_server_type(&self, server_id: NodeId)

    // --------------------------- flooding algorithm --------------------------------------
    // TODO: add a timer
    fn send_flood_request(&mut self) {
        // reset the adj list and node type
        self.reset_network_state();
        self.last_flood_id += 1;

        // generate the flood request
        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(self.last_flood_id, self.id, self.node_type),
        );

        // send broadcast the flood_request and remove if one channel is disconnected
        self.packet_send.retain(|node_id, send| {
            match send.send(flood_request.clone()) {
                Ok(_) => true, // Keep the channel if the send was successful
                Err(_) => {
                    // Log or handle the disconnected channel
                    warn!(
                        "Channel to node {:?} is disconnected and will be removed.",
                        node_id
                    );
                    false // Remove the channel
                }
            }
        });

        info!(
            "Flood request sent. Active channels: {}",
            self.packet_send.len()
        );
    }

    fn reset_network_state(&mut self) {
        self.nodes_type.clear();
        self.adj_list.clear();
        debug!("Reset network state: cleared nodes_type and adj_list");

        // insert itself
        self.nodes_type.insert(self.id, self.node_type);

        // add direct link
        for (node_id, _) in self.packet_send.iter() {
            self.adj_list.entry(*node_id).or_default().insert(self.id);

            self.adj_list.entry(self.id).or_default().insert(*node_id);

            debug!("Added connection between {:?} and {:?}", self.id, node_id);
        }
    }

    #[allow(unused)]
    fn update_network(&mut self, flood_response: FloodRequest) -> Result<(), String> {
        // check if flood_response is from the last sended flood_request
        if flood_response.initiator_id == self.id && flood_response.flood_id == self.last_flood_id {
            let mut prev: NodeId = self.id;

            for (node_id, node_type) in flood_response.path_trace {
                self.nodes_type.insert(node_id, node_type); // insert the type

                if node_id != self.id {
                    self.adj_list.entry(node_id).or_default().insert(prev);

                    self.adj_list.entry(prev).or_default().insert(node_id);
                }
                prev = node_id;
            }
            Ok(())
        } else {
            Err("Old or not my response".to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::controller::{DroneCommand, DroneEvent};
    use wg_2024::drone::Drone;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{FloodRequest, NodeType, Packet};

    use crossbeam_channel::unbounded;
    use crossbeam_channel::{Receiver, Sender};
    use std::collections::HashMap;
    use std::thread::{self, sleep};
    use std::time::Duration;

    // Importing all drone implementations
    use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
    use bagel_bomber::BagelBomber;
    use lockheedrustin_drone::LockheedRustin;
    use rolling_drone::RollingDrone;
    use rust_do_it::RustDoIt;
    use rust_roveri::RustRoveri;
    use rustastic_drone::RustasticDrone;
    use rustbusters_drone::RustBustersDrone;
    use LeDron_James::Drone as LeDronJames_drone;

    use flyPath::FlyPath;

    #[test]
    fn test_update_network() {
        let (_, recv) = unbounded();
        let mut client = Client::new(10, NodeType::Client, recv, HashMap::new());

        let flood_request = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(0, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone);

        let flood_request2 = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(0, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(11, NodeType::Server);

        client.update_network(flood_request).unwrap();
        client.update_network(flood_request2).unwrap();

        println!("{:?}", client);

        println!("{:?}", client.elaborate_path_trace(11));
    }

    // A-B-C-D-E-F-G-H-I-L
    fn build_complete_linear_network() -> (
        Vec<Sender<DroneCommand>>,
        Receiver<DroneEvent>,
        Receiver<Packet>,
        Receiver<Packet>,
        Vec<Sender<Packet>>,
        Vec<Box<dyn Drone>>,
    ) {
        let mut send_drones_command: Vec<Sender<DroneCommand>> = Vec::new();
        let mut recv_drones_command: Vec<Receiver<DroneCommand>> = Vec::new();

        let (send_drone_event, recv_drone_event) = unbounded();

        let mut sends_packet: Vec<Sender<Packet>> = Vec::new();
        let mut recvs_packet: Vec<Receiver<Packet>> = Vec::new();

        for _ in 0..10 {
            let (send_drone_command, recv_drone_command) = unbounded();
            send_drones_command.push(send_drone_command);
            recv_drones_command.push(recv_drone_command);

            let (send_packet, recv_packet) = unbounded();
            sends_packet.push(send_packet);
            recvs_packet.push(recv_packet);
        }

        let (client_send, client_recv) = unbounded();
        let (server_send, server_recv) = unbounded();

        let mut drones: Vec<Box<dyn Drone>> = Vec::new();

        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        packet_send_map.insert(10, client_send.clone());
        packet_send_map.insert(1, sends_packet[1].clone());
        drones.push(Box::new(LockheedRustin::new(
            0,
            send_drone_event.clone(),
            recv_drones_command[0].clone(),
            recvs_packet[0].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(0, send_drone_event.clone(), recv_drones_command[0].clone(), recvs_packet[0].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(0, sends_packet[0].clone());
        packet_send_map.insert(2, sends_packet[2].clone());
        drones.push(Box::new(RustBustersDrone::new(
            1,
            send_drone_event.clone(),
            recv_drones_command[1].clone(),
            recvs_packet[1].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(1, send_drone_event.clone(), recv_drones_command[1].clone(), recvs_packet[1].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(1, sends_packet[1].clone());
        packet_send_map.insert(3, sends_packet[3].clone());
        drones.push(Box::new(RustasticDrone::new(
            2,
            send_drone_event.clone(),
            recv_drones_command[2].clone(),
            recvs_packet[2].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(2, send_drone_event.clone(), recv_drones_command[2].clone(), recvs_packet[2].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(2, sends_packet[2].clone());
        packet_send_map.insert(4, sends_packet[4].clone());
        drones.push(Box::new(NoSoundDroneRIP::new(
            3,
            send_drone_event.clone(),
            recv_drones_command[3].clone(),
            recvs_packet[3].clone(),
            packet_send_map.clone(),
            0.0,
        ))); // not good enough
             // drones.push(Box::new(FlyPath::new(3, send_drone_event.clone(), recv_drones_command[3].clone(), recvs_packet[3].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(3, sends_packet[3].clone());
        packet_send_map.insert(5, sends_packet[5].clone());
        drones.push(Box::new(BagelBomber::new(
            4,
            send_drone_event.clone(),
            recv_drones_command[4].clone(),
            recvs_packet[4].clone(),
            packet_send_map.clone(),
            0.0,
        ))); // strange behavior
             // drones.push(Box::new(FlyPath::new(4, send_drone_event.clone(), recv_drones_command[4].clone(), recvs_packet[4].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(4, sends_packet[4].clone());
        packet_send_map.insert(6, sends_packet[6].clone());
        drones.push(Box::new(LeDronJames_drone::new(
            5,
            send_drone_event.clone(),
            recv_drones_command[5].clone(),
            recvs_packet[5].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(5, send_drone_event.clone(), recv_drones_command[5].clone(), recvs_packet[5].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(5, sends_packet[5].clone());
        packet_send_map.insert(7, sends_packet[7].clone());
        drones.push(Box::new(RollingDrone::new(
            6,
            send_drone_event.clone(),
            recv_drones_command[6].clone(),
            recvs_packet[6].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(6, send_drone_event.clone(), recv_drones_command[6].clone(), recvs_packet[6].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(6, sends_packet[6].clone());
        packet_send_map.insert(8, sends_packet[8].clone());
        drones.push(Box::new(RustBustersDrone::new(
            7,
            send_drone_event.clone(),
            recv_drones_command[7].clone(),
            recvs_packet[7].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(7, send_drone_event.clone(), recv_drones_command[7].clone(), recvs_packet[7].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(7, sends_packet[7].clone());
        packet_send_map.insert(9, sends_packet[9].clone());
        drones.push(Box::new(RustRoveri::new(
            8,
            send_drone_event.clone(),
            recv_drones_command[8].clone(),
            recvs_packet[8].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(8, send_drone_event.clone(), recv_drones_command[8].clone(), recvs_packet[8].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(8, sends_packet[8].clone());
        packet_send_map.insert(11, server_send.clone());
        drones.push(Box::new(RustDoIt::new(
            9,
            send_drone_event.clone(),
            recv_drones_command[9].clone(),
            recvs_packet[9].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(9, send_drone_event.clone(), recv_drones_command[9].clone(), recvs_packet[9].clone(), packet_send_map.clone(), 0.0)));

        (
            send_drones_command,
            recv_drone_event,
            client_recv,
            server_recv,
            sends_packet,
            drones,
        )
    }
}
