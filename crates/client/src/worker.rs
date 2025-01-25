use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::process::{self, exit};
use wg_2024::controller;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use message::{
    ChatRequest, ChatResponse, DroneSend, MediaRequest, MediaResponse, MessageError, NodeCommand,
    NodeEvent, RecvMessageWrapper, SentMessageWrapper, TransmissionStatus,
};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType,
};

use crate::comunication::{FromUiCommunication, ServerType, ToUIComunication};

#[derive(Debug)]
pub struct Worker {
    id: NodeId,
    node_type: NodeType,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    prec_flood_ids: HashSet<(u64, NodeId)>,

    // receive user interaction and send to user messages
    ui_recv: Receiver<FromUiCommunication>,
    ui_send: Sender<ToUIComunication>,

    controller_recv: Receiver<NodeCommand>,
    controller_send: Sender<NodeEvent>,

    // network status
    adj_list: HashMap<NodeId, HashSet<NodeId>>,
    nodes_type: HashMap<NodeId, NodeType>,
    servers_type: HashMap<NodeId, ServerType>,
    last_flood_id: u64,

    // current sended fragments status
    sent_message_status: HashMap<u64, SentMessageWrapper>,
    recv_message_status: HashMap<u64, RecvMessageWrapper>,
}

impl Worker {
    pub fn new(
        id: NodeId,
        node_type: NodeType,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<u8, Sender<Packet>>,
        controller_recv: Receiver<NodeCommand>,
        controller_send: Sender<NodeEvent>,
    ) -> Self {
        debug!("Creating a new Client with id: {:?}", id);
        let (_to_logic, ui_recv) = unbounded();
        let (ui_send, _from_logic) = unbounded();

        Self {
            id,
            node_type,
            packet_recv,
            packet_send,
            prec_flood_ids: HashSet::new(),
            ui_recv,
            ui_send,
            controller_recv,
            controller_send,
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
                recv (self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        self.command_handler(command);
                    }
                }
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

    fn command_handler(&mut self, command: NodeCommand) {
        match command {
            NodeCommand::RemoveSender(node_id) => {
                self.packet_send.remove(&node_id);
            }
            NodeCommand::AddSender(node_id, sender) => {
                self.packet_send.insert(node_id, sender);
            }
            NodeCommand::FromShortcut(packet) => self.packet_handler(packet),
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
        if let Some(message_status) = self.sent_message_status.get_mut(&packet.session_id) {
            message_status.add_acked(ack.fragment_index);
            if message_status.is_all_fragment_acked() {
                let session_id = message_status.session_id;
                self.send_to_ui(ToUIComunication::ServerReceivedAllSegment(session_id));
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
                        if self
                            .controller_send
                            .send(NodeEvent::PacketSent(packet_to_send.clone()))
                            .is_err()
                        {
                            exit(1); // CRASH
                        }
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
                    if let Err(_e) = sender.send(response.clone()) {
                        if self
                            .controller_send
                            .send(NodeEvent::ControllerShortcut(response.clone()))
                            .is_err()
                        {
                            exit(1); // CRASH
                        }
                    } else if self
                            .controller_send
                            .send(NodeEvent::PacketSent(response.clone()))
                            .is_err()
                    {
                        exit(1); // CRASH
                    }
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
                for (session_id, message_status) in self.sent_message_status.iter_mut() {
                    if self.nodes_type.contains_key(&message_status.destination)
                        && message_status.transmission_status == TransmissionStatus::Pending
                    {
                        reachable_messages.push(*session_id);
                    }
                }
                for session_id in reachable_messages {
                    if let Err(e) = self.send_fragments(session_id) {
                        self.send_to_ui(ToUIComunication::Err(e));
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
                if let Some(message_status) = self.sent_message_status.get_mut(&packet.session_id) {
                    message_status.transmission_status = TransmissionStatus::Pending;
                    self.send_flood_request();
                }
            }
        }
    }

    fn dropped_handler(&mut self, packet_session_id: u64, fragment_index: u64) {
        if let Some(mut message_status) = self.sent_message_status.remove(&packet_session_id) {
            message_status.transmission_status = TransmissionStatus::Pending;
            if message_status.evaluate_error_threshold() {
                self.send_to_ui(ToUIComunication::Err(MessageError::TooManyErrors(
                    message_status.session_id,
                )));
                return;
            }

            // get fragment and elaborate path trace
            if let Some(fragment) = message_status.get_fragment(fragment_index as usize) {
                if let Some(path_trace) = self.elaborate_path_trace(message_status.destination) {
                    message_status.n_unreachable_server = 0;

                    // check if not acked
                    if !message_status.fragment_acked(fragment_index) {
                        let source_routing = SourceRoutingHeader::with_first_hop(path_trace);
                        let packet: Packet = Packet::new_fragment(
                            source_routing.clone(),
                            message_status.session_id,
                            fragment.clone(),
                        );

                        let next_hop = source_routing.current_hop().unwrap();
                        // try to send
                        if let Some(sender) = self.packet_send.get(&next_hop) {
                            if sender.send(packet).is_err() {
                                self.send_to_ui(ToUIComunication::Err(
                                    MessageError::DirectConnectionDoNotWork(
                                        message_status.session_id,
                                        source_routing.current_hop().unwrap(),
                                    ),
                                ));
                                message_status.n_unreachable_direct += 1;
                                self.send_flood_request();
                            } else {
                                message_status.n_unreachable_direct = 0;
                                message_status.transmission_status = TransmissionStatus::Completed;
                            }
                        } else {
                            self.send_to_ui(ToUIComunication::Err(
                                MessageError::DirectConnectionDoNotWork(
                                    message_status.session_id,
                                    source_routing.current_hop().unwrap(),
                                ),
                            ));
                            message_status.n_unreachable_direct += 1;
                            self.send_flood_request();
                        }
                    }
                } else {
                    message_status.n_unreachable_server += 1;
                    self.send_to_ui(ToUIComunication::Err(MessageError::ServerUnreachable(
                        message_status.session_id,
                        message_status.destination,
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
            let message_status = self
                .recv_message_status
                .entry(packet.session_id)
                .or_insert_with(|| {
                    RecvMessageWrapper::new(
                        packet.session_id,
                        server,
                        self.id,
                        fragment.total_n_fragments,
                    )
                });

            let index = fragment.fragment_index;
            message_status.add_fragment(fragment.clone());

            if let Err(e) = message_status.try_generate_raw_data() {
                if let MessageError::InvalidMessageReceived(session_id) = e {
                    self.recv_message_status.remove(&session_id);
                    self.send_to_ui(ToUIComunication::Err(MessageError::InvalidMessageReceived(
                        session_id,
                    )));
                    return;
                }
                self.send_ack(packet.session_id, index);
            } else {
                self.send_ack(packet.session_id, index);
                let message_status = self.recv_message_status.remove(&packet.session_id).unwrap();
                self.process_complete_message(message_status.raw_data, packet);
            }
        }
    }

    fn process_complete_message(&mut self, raw_data: String, packet: &Packet) {
        if let Some(server_id) = packet.routing_header.source() {
            if raw_data == "MediaServer" {
                self.servers_type.insert(server_id, ServerType::MediaServer);
                return;
            } else if raw_data == "ChatServer" {
                self.servers_type.insert(server_id, ServerType::ChatServer);
                return;
            }
        }

        // Try parsing as MediaResponse first
        if let Ok(media_response) = MediaResponse::from_string(raw_data.clone()) {
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
        if let Ok(chat_response) = ChatResponse::from_string(raw_data) {
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

    fn send_ack(&self, session_id: u64, fragment_index: u64) {
        if let Some(message_status) = self.recv_message_status.get(&session_id) {
            if let Some(path_trace) = self.elaborate_path_trace(message_status.source) {
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
            }
            // TODO: SHORTCUT DII A MARTINA CHE è PROBLEMA SUO ORA.
        }
    }

    fn send_raw_content(&mut self, raw_data: String, destination: NodeId, session_id: u64) {
        self.sent_message_status.insert(
            session_id,
            SentMessageWrapper::new_from_raw_data(session_id, self.id, destination, raw_data),
        );

        if let Err(e) = self.send_fragments(session_id) {
            self.send_to_ui(ToUIComunication::Err(e));
        }
    }

    fn send_fragments(&mut self, session_id: u64) -> Result<(), MessageError> {
        if let Some(mut message_status) = self.sent_message_status.remove(&session_id) {
            message_status.transmission_status = TransmissionStatus::Pending;

            if message_status.evaluate_error_threshold() {
                return Err(MessageError::TooManyErrors(message_status.session_id));
            }

            if let Some(path_trace) = self.elaborate_path_trace(message_status.destination) {
                message_status.n_unreachable_server = 0;
                let source_routing = SourceRoutingHeader::with_first_hop(path_trace);
                let next_hop = source_routing.current_hop().unwrap(); // safe unwrap cus elaborate path trace doesnt return a invalid path

                if let Some(sender) = self.packet_send.get(&next_hop) {
                    for fragment in message_status.fragments.iter() {
                        if !message_status.fragment_acked(fragment.fragment_index) {
                            let packet: Packet = Packet::new_fragment(
                                source_routing.clone(),
                                session_id,
                                fragment.clone(),
                            );

                            if sender.send(packet).is_err() {
                                message_status.n_unreachable_direct += 1;
                                self.sent_message_status.insert(session_id, message_status);
                                return Err(MessageError::DirectConnectionDoNotWork(
                                    session_id,
                                    source_routing.current_hop().unwrap(),
                                ));
                            } else {
                                message_status.n_unreachable_direct = 0;
                            }
                        }
                    }
                    message_status.transmission_status = TransmissionStatus::Completed;
                    self.sent_message_status
                        .insert(message_status.session_id, message_status);
                    Ok(())
                } else {
                    message_status.n_unreachable_direct += 1;
                    self.sent_message_status.insert(session_id, message_status);
                    return Err(MessageError::DirectConnectionDoNotWork(
                        session_id,
                        source_routing.current_hop().unwrap(),
                    ));
                }
            } else {
                let destination = message_status.destination;
                message_status.n_unreachable_server += 1;
                self.sent_message_status.insert(session_id, message_status);
                Err(MessageError::ServerUnreachable(session_id, destination))
            }
        } else {
            Err(MessageError::NoFragmentStatus(session_id))
        }
    }

    fn send_to_ui(&self, response: ToUIComunication) {
        if self.ui_send.send(response).is_err() {
            error!("{} cannot communicate with client UI", self.id);
            process::exit(1); // problem with UI
        }
    }

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

    fn send_flood_request(&mut self) {
        self.send_to_ui(ToUIComunication::NewFloodRequest());

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
