// TODO: network discovery protocol, da mandare per inizializzare poi ogni tot ms e poi per ogni nack
// TODO: fragmentation of high level messages
// TODO: handle ACK, NACK
mod comunication;
mod test;

use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::process;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use message::{ChatRequest, ChatResponse, DroneSend, MediaRequest, MediaResponse};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType,
};

use comunication::{FromUiCommunication, MessageError, ServerType, ToUIComunication};

#[derive(Debug, Clone, PartialEq)]
enum Status {
    Sending,
    Ok,
}

// Send N fragment at the time
// every ack send another frag
// if dropped send again
// if other nack make again the flood_request and send from the ack not acked.
// TODO: introdurre un aging
#[allow(unused)]
#[derive(Debug, Clone)]
struct SentMessageStatus {
    session_id: u64,
    server: NodeId,
    total_n_fragments: u64, // total number of frag
    acked: HashSet<u64>,
    unreachable_direct: u64,
    unreachable_server: u64,
    fragments: Vec<Fragment>,
    status: Status,
}

#[allow(unused)]
#[derive(Debug, Clone)]
struct RecvMessageStatus {
    session_id: u64,
    server: NodeId,
    total_frag: u64, // total number of frag
    arrived: HashSet<u64>,
    fragments: Vec<Fragment>,
}

// Client make flood_request every GetServerList to upgrade, if a drone crashed or if a DestinationIsDrone/UnexpectedRecipient/ErrorInRouting, if Dropped send again
#[allow(unused)]
#[derive(Debug)]
pub struct Client {
    id: NodeId,
    node_type: NodeType,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    prec_flood_ids: HashSet<(u64, NodeId)>,

    // receive user interaction and send to user messages
    ui_recv: Receiver<FromUiCommunication>,
    ui_send: Sender<ToUIComunication>,

    // network status
    adj_list: HashMap<NodeId, HashSet<NodeId>>,
    nodes_type: HashMap<NodeId, NodeType>,
    servers_type: HashMap<NodeId, ServerType>,
    last_flood_id: u64,

    // current sended fragments status
    sent_message_status: HashMap<u64, SentMessageStatus>,
    recv_message_status: HashMap<u64, RecvMessageStatus>,
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
            prec_flood_ids: HashSet::new(),
            ui_recv,
            ui_send,
            adj_list: HashMap::new(),
            nodes_type: HashMap::new(),
            servers_type: HashMap::new(),
            last_flood_id: 0,
            sent_message_status: HashMap::new(),
            recv_message_status: HashMap::new(),
        }
    }

    pub fn run(&mut self) {
        self.send_flood_request();
        loop {
            select_biased! {
                recv(self.ui_recv) -> interaction => {
                    if let Ok(interaction) = interaction {
                        self.interaction_handler(interaction);
                    }
                },
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        self.packet_handler(packet);
                    }
                }
            }
        }
    }

    fn interaction_handler(&mut self, interaction: FromUiCommunication) {
        match interaction {
            FromUiCommunication::GetServerList => {
                // send the current server list and make a flood_request if is empty
                let servers: Option<Vec<u8>> = self.get_servers();
                servers.is_none().then(|| self.send_flood_request());
                let servers_type = if self.servers_type.is_empty() {
                    None
                } else {
                    Some(self.servers_type.clone())
                };
                self.send_to_ui(ToUIComunication::ServerList(servers, servers_type));
            }
            FromUiCommunication::AskServerType(session_id, server_id) => {
                self.send_raw_content("ServerType".to_string(), server_id, session_id)
            }
            FromUiCommunication::ReloadNetwork => self.send_flood_request(),
            FromUiCommunication::AskMedialist(session_id, server_id) => {
                self.send_raw_content(MediaRequest::MediaList.stringify(), server_id, session_id)
            }
            FromUiCommunication::AskMedia(session_id, server_id, media_id) => self
                .send_raw_content(
                    MediaRequest::Media(media_id).stringify(),
                    server_id,
                    session_id,
                ),
            FromUiCommunication::AskClientList(session_id, server_id) => {
                self.send_raw_content(ChatRequest::ClientList.stringify(), server_id, session_id)
            }
            FromUiCommunication::AskRegister(session_id, server_id) => self.send_raw_content(
                ChatRequest::Register(self.id).stringify(),
                server_id,
                session_id,
            ),
            FromUiCommunication::SendMessage {
                session_id,
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
                self.send_raw_content(raw_message, server_id, session_id);
            }
        }
    }

    fn packet_handler(&mut self, packet: Packet) {
        match &packet.pack_type {
            PacketType::Ack(ack) => self.ack_handler(ack, &packet),
            PacketType::FloodRequest(flood_request) => {
                self.flood_request_handler(flood_request, &packet)
            }
            PacketType::FloodResponse(flood_response) => {
                self.flood_response_handler(flood_response)
            }
            PacketType::Nack(nack) => self.nack_handler(nack, &packet),
            PacketType::MsgFragment(fragment) => {
                self.fragment_handler(fragment, &packet);
            }
        }
    }

    fn ack_handler(&mut self, ack: &Ack, packet: &Packet) {
        if let Some(mut message_status) = self.sent_message_status.remove(&packet.session_id) {
            if ack.fragment_index < message_status.total_n_fragments {
                message_status.acked.insert(ack.fragment_index);
                if message_status.acked.len() == message_status.total_n_fragments as usize {
                    self.send_to_ui(ToUIComunication::ServerReceivedAllSegment(
                        message_status.session_id,
                    ));
                } else {
                    self.sent_message_status
                        .insert(message_status.session_id, message_status);
                }
            }
        }
    }

    // TODO: fallisce invio del response, inviare al controller
    fn flood_request_handler(&mut self, flood_request: &FloodRequest, packet: &Packet) {
        if let Some((last_node_id, _)) = flood_request.path_trace.last() {
            let updated_flood_request = flood_request.get_incremented(self.id, self.node_type);

            if !self.prec_flood_ids.contains(&(
                updated_flood_request.flood_id,
                updated_flood_request.initiator_id,
            )) && self.packet_send.len() > 1
            {
                self.prec_flood_ids.insert((
                    updated_flood_request.flood_id,
                    updated_flood_request.initiator_id,
                ));
                let packet_to_send = Packet {
                    routing_header: packet.routing_header.clone(),
                    session_id: packet.session_id,
                    pack_type: PacketType::FloodRequest(updated_flood_request),
                };
                for (node_id, sender) in &self.packet_send {
                    if node_id != last_node_id {
                        let _ = sender.send(packet_to_send.clone());
                    }
                }
            } else {
                let mut response = updated_flood_request.generate_response(packet.session_id);
                response.routing_header.increase_hop_index();

                if let Some(sender) = response
                    .routing_header
                    .current_hop()
                    .and_then(|next_hop: u8| self.packet_send.get(&next_hop))
                {
                    if let Err(_e) = sender.send(response.clone()) {}
                } else {
                }
            }
        }
    }

    fn flood_response_handler(&mut self, flood_response: &FloodResponse) {
        if self.update_network(flood_response).is_ok() {
            let mut servers = false;
            let mut reachable_messages: Vec<u64> = Vec::new();

            for (node_id, node_type) in self.nodes_type.iter() {
                if *node_type == NodeType::Server {
                    self.send_to_ui(ToUIComunication::ServerReachable(*node_id));
                    servers = true;
                }
            }

            if servers {
                for (session_id, fragment_status) in self.sent_message_status.iter_mut() {
                    if self.nodes_type.contains_key(&fragment_status.server)
                        && fragment_status.status == Status::Sending
                    {
                        // server is reachable with this flood_response even if is not the best path
                        reachable_messages.push(*session_id);
                    }
                }

                for session_id in reachable_messages {
                    if let Some(message_status) = self.sent_message_status.remove(&session_id) {
                        if let Some(message_status) = self.send_fragments(message_status) {
                            self.sent_message_status
                                .insert(message_status.session_id, message_status);
                        }
                    }
                }
            }
        }
    }

    fn nack_handler(&mut self, nack: &Nack, packet: &Packet) {
        let fragment_index = nack.fragment_index;
        match nack.nack_type {
            NackType::Dropped => self.dropped_handler(packet.session_id, fragment_index),
            _ => {
                if let Some(mut message_status) =
                    self.sent_message_status.remove(&packet.session_id)
                {
                    message_status.status = Status::Sending;
                    self.sent_message_status
                        .insert(packet.session_id, message_status);
                    self.send_flood_request();
                }
            }
        }
    }

    fn dropped_handler(&mut self, packet_session_id: u64, fragment_index: u64) {
        if let Some(mut message_status) = self.sent_message_status.remove(&packet_session_id) {
            if message_status.unreachable_direct > 30 || message_status.unreachable_server > 30 {
                self.send_to_ui(ToUIComunication::Err(MessageError::TooManyErrors(
                    message_status.session_id,
                )));
            } else if let Some(fragment) = message_status.fragments.get(fragment_index as usize) {
                if let Some(path_trace) = self.elaborate_path_trace(message_status.server) {
                    message_status.unreachable_server = 0;
                    message_status.status = Status::Sending;
                    if !message_status.acked.contains(&fragment_index) {
                        let source_routing: SourceRoutingHeader =
                            SourceRoutingHeader::with_first_hop(path_trace);
                        let packet: Packet = Packet::new_fragment(
                            source_routing.clone(),
                            message_status.session_id,
                            fragment.clone(),
                        );

                        if let Some(next_hop) = source_routing.current_hop() {
                            if let Some(sender) = self.packet_send.get(&next_hop) {
                                if sender.send(packet).is_err() {
                                    // error during send
                                    self.send_to_ui(ToUIComunication::Err(
                                        MessageError::DirectConnectionDoNotWork(
                                            message_status.session_id,
                                            source_routing.current_hop().unwrap(),
                                        ),
                                    ));
                                    message_status.unreachable_direct += 1;
                                    self.send_flood_request();
                                } else {
                                    message_status.unreachable_direct = 0;
                                    message_status.status = Status::Ok;
                                }
                            } else {
                                self.send_to_ui(ToUIComunication::Err(
                                    MessageError::DirectConnectionDoNotWork(
                                        message_status.session_id,
                                        source_routing.current_hop().unwrap(),
                                    ),
                                ));
                                message_status.unreachable_direct += 1;
                                self.send_flood_request();
                            }
                        } else {
                            self.send_flood_request();
                        }
                    }
                } else {
                    message_status.unreachable_server += 1;
                    self.send_to_ui(ToUIComunication::Err(MessageError::ServerUnreachable(
                        message_status.session_id,
                        message_status.server,
                    )));
                    self.send_flood_request();
                }
            }
            self.sent_message_status
                .insert(packet_session_id, message_status);
        } else {
            self.send_to_ui(ToUIComunication::Err(MessageError::NoFragmentStatus(
                packet_session_id,
            )));
        }
    }

    fn fragment_handler(&mut self, fragment: &Fragment, packet: &Packet) {
        if let Some(server) = packet.routing_header.source() {
            let mut message_status = self
                .recv_message_status
                .remove(&packet.session_id)
                .unwrap_or_else(|| RecvMessageStatus {
                    session_id: packet.session_id,
                    server,
                    total_frag: fragment.total_n_fragments,
                    arrived: HashSet::new(),
                    fragments: vec![fragment.clone(); fragment.total_n_fragments as usize],
                });

            let index = fragment.fragment_index;
            message_status.fragments[index as usize] = fragment.clone();
            message_status.arrived.insert(index);

            self.send_ack(&message_status, index);

            if message_status.arrived.len() == message_status.total_frag as usize {
                let full_message: Vec<u8> = message_status
                    .fragments
                    .iter()
                    .flat_map(|frag: &Fragment| frag.data[..frag.length as usize].to_vec())
                    .collect();

                if let Ok(message_str) = String::from_utf8(full_message) {
                    self.process_complete_message(message_str, packet);
                }
            } else {
                self.recv_message_status
                    .insert(packet.session_id, message_status);
            }
        }
    }

    fn process_complete_message(&mut self, message_str: String, packet: &Packet) {
        if let Some(server_id) = packet.routing_header.source() {
            if message_str == "MediaServer" {
                self.servers_type.insert(server_id, ServerType::MediaServer);
                return;
            } else if message_str == "ChatServer" {
                self.servers_type.insert(server_id, ServerType::ChatServer);
                return;
            }
        }

        // Try parsing as MediaResponse first
        if let Ok(media_response) = MediaResponse::from_string(message_str.clone()) {
            match media_response {
                MediaResponse::MediaList(list) => {
                    self.send_to_ui(ToUIComunication::ResponseMediaList(packet.session_id, list));
                }
                MediaResponse::Media(bytes) => {
                    self.send_to_ui(ToUIComunication::ResponseMedia(packet.session_id, bytes));
                }
            }
            return;
        }

        // If not MediaResponse, try ChatResponse
        if let Ok(chat_response) = ChatResponse::from_string(message_str) {
            match chat_response {
                ChatResponse::ClientList(clients) => {
                    self.send_to_ui(ToUIComunication::ResponseClientList(
                        packet.session_id,
                        clients,
                    ));
                }
                ChatResponse::MessageFrom { from, message } => {
                    self.send_to_ui(ToUIComunication::MessageFrom {
                        session_id: packet.session_id,
                        from,
                        message,
                    });
                }
                ChatResponse::MessageSent => {
                    self.send_to_ui(ToUIComunication::ConfirmMessageSent(packet.session_id));
                }
            }
            return;
        }

        // Handle parsing error if both attempts fail
        self.send_to_ui(ToUIComunication::Err(MessageError::InvalidMessageReceived(
            packet.session_id,
        )));
    }

    fn send_ack(&self, message_status: &RecvMessageStatus, fragment_index: u64) {
        if let Some(path_trace) = self.elaborate_path_trace(message_status.server) {
            let source_routing = SourceRoutingHeader::with_first_hop(path_trace);
            let packet = Packet::new_ack(
                source_routing.clone(),
                message_status.session_id,
                fragment_index,
            );

            if let Some(next_hop) = source_routing.current_hop() {
                if let Some(sender) = self.packet_send.get(&next_hop) {
                    if let Ok(()) = sender.send(packet.clone()) {
                        return;
                    }
                }
            }

            // TODO: SHORTCUT DII A MARTINA CHE è PROBLEMA SUO ORA.
        }
    }

    fn send_fragments(
        &mut self,
        mut message_status: SentMessageStatus,
    ) -> Option<SentMessageStatus> {
        let mut make_flood_request = false;
        message_status.status = Status::Sending;

        if message_status.unreachable_direct > 30 || message_status.unreachable_server > 30 {
            self.send_to_ui(ToUIComunication::Err(MessageError::TooManyErrors(
                message_status.session_id,
            )));
            return None;
        }

        if let Some(path_trace) = self.elaborate_path_trace(message_status.server) {
            message_status.unreachable_server = 0;

            let source_routing: SourceRoutingHeader =
                SourceRoutingHeader::with_first_hop(path_trace);

            if let Some(next_hop) = source_routing.current_hop() {
                if let Some(sender) = self.packet_send.get(&next_hop) {
                    let mut sended = true;
                    for fragment in message_status.fragments.iter() {
                        if !message_status.acked.contains(&fragment.fragment_index) {
                            let packet: Packet = Packet::new_fragment(
                                source_routing.clone(),
                                message_status.session_id,
                                fragment.clone(),
                            );

                            if sender.send(packet).is_err() {
                                // error during send
                                self.send_to_ui(ToUIComunication::Err(
                                    MessageError::DirectConnectionDoNotWork(
                                        message_status.session_id,
                                        source_routing.current_hop().unwrap(),
                                    ),
                                ));
                                sended = false;
                                make_flood_request = true;
                                message_status.unreachable_direct += 1;
                                break;
                            } else {
                                message_status.unreachable_direct = 0;
                            }
                        }
                    }
                    if sended {
                        message_status.status = Status::Ok;
                    }
                } else {
                    self.send_to_ui(ToUIComunication::Err(
                        MessageError::DirectConnectionDoNotWork(
                            message_status.session_id,
                            source_routing.current_hop().unwrap(),
                        ),
                    ));
                    make_flood_request = true;
                    message_status.unreachable_direct += 1;
                }
            } else {
                make_flood_request = true;
            }
        } else {
            message_status.unreachable_server += 1;
            self.send_to_ui(ToUIComunication::Err(MessageError::ServerUnreachable(
                message_status.session_id,
                message_status.server,
            )));
            make_flood_request = true;
        }

        if make_flood_request {
            self.send_flood_request();
        }
        Some(message_status)
    }

    // fragment are sended in style of TCP ack too limitated the number of NACK in case of DestinationIsDrone/UnexpectedRecipient/ErrorInRouting
    fn send_raw_content(&mut self, raw_content: String, destination: NodeId, session_id: u64) {
        const FRAGMENT_SIZE: usize = 128;

        let raw_bytes = raw_content.into_bytes();
        let total_frag = raw_bytes.len().div_ceil(FRAGMENT_SIZE) as u64;

        let message_status = SentMessageStatus {
            session_id,
            server: destination,
            acked: HashSet::new(),
            total_n_fragments: total_frag,
            unreachable_direct: 0,
            unreachable_server: 0,
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

        if let Some(message_status) = self.send_fragments(message_status) {
            self.sent_message_status
                .insert(message_status.session_id, message_status);
        }
    }

    fn send_to_ui(&self, response: ToUIComunication) {
        if self.ui_send.send(response).is_err() {
            error!("{} cannot communicate with client UI", self.id);
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
        self.servers_type.clear();
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

    fn update_network(&mut self, flood_response: &FloodResponse) -> Result<(), String> {
        // check if flood_response is from the last sended flood_request
        if flood_response.flood_id == self.last_flood_id {
            let mut prev: NodeId = self.id;

            for (node_id, node_type) in flood_response.path_trace.iter() {
                self.nodes_type.insert(*node_id, *node_type); // insert the type

                if *node_id != self.id {
                    self.adj_list.entry(*node_id).or_default().insert(prev);

                    self.adj_list.entry(prev).or_default().insert(*node_id);
                }
                prev = *node_id;
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
