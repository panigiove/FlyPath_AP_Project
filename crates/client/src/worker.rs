use crate::comunication::{FromUiCommunication, ServerType, ToUIComunication};
use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use log::{debug, error, info, warn};
use message::*;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::{path, process};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType,
};

// non rimandare i flood request rispondere e basta

type Weight = u32;

#[derive(Debug)]
pub struct Worker {
    id: NodeId,
    node_type: NodeType,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,

    // receive user interaction and send to user messages
    ui_recv: Receiver<FromUiCommunication>,
    ui_send: Sender<ToUIComunication>,

    // controller
    controller_recv: Receiver<NodeCommand>,
    controller_send: Sender<NodeEvent>,

    // network status
    adj_list: HashMap<NodeId, HashMap<NodeId, Weight>>,
    nodes_type: HashMap<NodeId, NodeType>,
    servers_type: HashMap<NodeId, ServerType>,
    last_flood_id: u64,

    sent_message_wrappers: HashMap<u64, SentMessageWrapper>,
    recv_message_wrapper: HashMap<(u64, NodeId), RecvMessageWrapper>,

    buffer: Vec<Packet>,
}

impl Worker {
    pub fn new(
        id: NodeId,
        node_type: NodeType,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<u8, Sender<Packet>>,
        controller_recv: Receiver<NodeCommand>,
        controller_send: Sender<NodeEvent>,
        ui_recv: Receiver<FromUiCommunication>,
        ui_send: Sender<ToUIComunication>,
    ) -> Self {
        Self {
            id,
            node_type,
            packet_recv,
            packet_send,
            ui_recv,
            ui_send,
            controller_recv,
            controller_send,
            adj_list: HashMap::new(),
            nodes_type: HashMap::new(),
            servers_type: HashMap::new(),
            last_flood_id: 0,
            sent_message_wrappers: HashMap::new(),
            recv_message_wrapper: HashMap::new(),
            buffer: Vec::new(),
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

    // ----------------------------------------------------------- HANDLER FUNCTIONS
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
                if let Err(e) = self.send_content(session_id, server_id, "ServerType".to_string()) {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
            FromUiCommunication::ReloadNetwork => self.send_flood_request(),
            FromUiCommunication::AskMedialist(session_id, server_id) => {
                if let Err(e) =
                    self.send_content(session_id, server_id, MediaRequest::MediaList.stringify())
                {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
            FromUiCommunication::AskMedia(session_id, server_id, media_id) => {
                if let Err(e) = self.send_content(
                    session_id,
                    server_id,
                    MediaRequest::Media(media_id).stringify(),
                ) {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
            FromUiCommunication::AskClientList(session_id, server_id) => {
                if let Err(e) =
                    self.send_content(session_id, server_id, ChatRequest::ClientList.stringify())
                {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
            FromUiCommunication::AskRegister(session_id, server_id) => {
                if let Err(e) = self.send_content(
                    session_id,
                    server_id,
                    ChatRequest::Register(self.id).stringify(),
                ) {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
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
                if let Err(e) = self.send_content(session_id, server_id, raw_message) {
                    self.send_to_ui(ToUIComunication::Err(e));
                }
            }
        }
    }

    fn packet_handler(&mut self, packet: Packet) {
        match &packet.pack_type {
            PacketType::FloodRequest(flood_request) => {
                self.flood_request_handler(flood_request, &packet)
            }
            PacketType::FloodResponse(flood_response) => {
                self.flood_response_handler(flood_response)
            }
            PacketType::Ack(ack) => self.ack_handler(ack, &packet),
            PacketType::Nack(nack) => self.nack_handler(nack, &packet),
            PacketType::MsgFragment(fragment) => {
                self.fragment_handler(fragment, &packet);
            }
        }
    }

    fn flood_request_handler(&mut self, flood_request: &FloodRequest, packet: &Packet) {
        let updated_flood_request = flood_request.get_incremented(self.id, self.node_type);
        let mut response = updated_flood_request.generate_response(packet.session_id);

        response.routing_header.increase_hop_index();

        println!("{:?}", response);

        if let Some(sender) = response
            .routing_header
            .current_hop()
            .and_then(|next_hop: u8| self.packet_send.get(&next_hop))
        {
            if let Err(_e) = sender.send(response.clone()) {
                self.controller_send
                    .send(NodeEvent::ControllerShortcut(response.clone()))
                    .unwrap(); // PANIC
            } else {
                self.controller_send
                    .send(NodeEvent::PacketSent(response.clone()))
                    .unwrap(); // PANIC
            }
        } else {
            self.controller_send
                .send(NodeEvent::ControllerShortcut(response.clone()))
                .unwrap(); // PANIC
        }
    }

    fn flood_response_handler(&mut self, flood_response: &FloodResponse) {
        if self.update_network(flood_response).is_ok() {
            let mut servers = false;

            for (node_id, node_type) in self.nodes_type.iter() {
                if *node_type == NodeType::Server {
                    self.send_to_ui(ToUIComunication::ServerReachable(*node_id));
                    servers = true;
                }
            }
            if servers {
                // iter the buffer and try to send the fragment
                let buffer_iter = self.buffer.clone().into_iter().enumerate();
                for (index, fragment) in buffer_iter {
                    if self
                        .nodes_type
                        .contains_key(&fragment.routing_header.destination().unwrap())
                    {
                        self.send_buffer_fragment(index);
                    }
                }
            }
        }
    }

    fn ack_handler(&mut self, ack: &Ack, packet: &Packet) {
        if let Some(message_status) = self.sent_message_wrappers.get_mut(&packet.session_id) {
            message_status.add_acked(ack.fragment_index);
            if message_status.is_all_fragment_acked() {
                let session_id = message_status.session_id;
                self.send_to_ui(ToUIComunication::ServerReceivedAllSegment(session_id));
            }
        }
    }

    fn nack_handler(&mut self, nack: &Nack, packet: &Packet) {
        let fragment_index = nack.fragment_index;
        if let Some(source) = packet.routing_header.source() {
            match nack.nack_type {
                NackType::Dropped => {
                    self.dropped_handler(packet.session_id, fragment_index, source)
                }
                _ => {
                    if let Some(message_wrapper) =
                        self.sent_message_wrappers.get(&packet.session_id)
                    {
                        let destination = message_wrapper.destination;
                        let session_id = message_wrapper.session_id;
                        for fragment in message_wrapper.fragments.clone().iter() {
                            self.add_fragment_to_buffer_and_send_to_ui_unreachable(
                                fragment.clone(),
                                destination,
                                session_id,
                            );
                        }
                        self.send_flood_request();
                    }
                }
            }
        }
    }

    // send again the fragment if errors occors sent to UI
    fn dropped_handler(&mut self, packet_session_id: u64, fragment_index: u64, dropper: NodeId) {
        if let Some(message_wrapper) = self.sent_message_wrappers.get(&packet_session_id) {
            if let Some(neighbours) = self.adj_list.get_mut(&dropper) {
                let neighbours_ids: Vec<NodeId> = neighbours
                    .iter_mut()
                    .map(|(&node_id, weight)| {
                        *weight += 1;
                        node_id
                    })
                    .collect();
                for node_id in neighbours_ids {
                    if let Some(reverse_neighbours) = self.adj_list.get_mut(&node_id) {
                        reverse_neighbours.entry(dropper).and_modify(|e| *e += 1);
                    }
                }

                let destination = message_wrapper.destination;
                if let Some(fragment) = message_wrapper.get_fragment(fragment_index as usize) {
                    if let Some(path_trace) = self.elaborate_path_trace(destination) {
                        if self
                            .send_packet(Packet::new_fragment(
                                SourceRoutingHeader::initialize(path_trace),
                                message_wrapper.session_id,
                                fragment.clone(),
                            ))
                            .is_ok()
                        {
                            return;
                        }
                    }
                    self.add_fragment_to_buffer_and_send_to_ui_unreachable(
                        fragment,
                        destination,
                        packet_session_id,
                    );
                }
                self.send_to_ui(ToUIComunication::Err(MessageError::InvalidFragmentIndex(
                    packet_session_id,
                    fragment_index,
                )));
            }
        } else {
            self.send_to_ui(ToUIComunication::Err(MessageError::NoFragmentWrapper(
                packet_session_id,
            )));
        }
    }

    fn fragment_handler(&mut self, fragment: &Fragment, packet: &Packet) {
        if let Some(server) = packet.routing_header.source() {
            let recv_message_wrapper = self
                .recv_message_wrapper
                .entry((packet.session_id, server))
                .or_insert_with(|| {
                    RecvMessageWrapper::new(packet.session_id, server, fragment.total_n_fragments)
                });

            let index = fragment.fragment_index;
            recv_message_wrapper.add_fragment(fragment.clone());

            if let Err(e) = recv_message_wrapper.try_generate_raw_data() {
                if let MessageError::InvalidMessageReceived(session_id) = e {
                    self.recv_message_wrapper.remove(&(session_id, server));
                    self.send_to_ui(ToUIComunication::Err(MessageError::InvalidMessageReceived(
                        session_id,
                    )));
                    return;
                }
            } else {
                let recv_message_wrapper = self
                    .recv_message_wrapper
                    .remove(&(packet.session_id, server))
                    .unwrap();
                self.process_complete_message(recv_message_wrapper.raw_data, packet);
            }
            self.send_ack(packet.session_id, server, index);
        }
    }

    // ----------------------------------------------------------- SEND FUNCTIONS

    // take a packet, INCREASE HOP INDEX, send to the next hop
    fn send_packet(&mut self, mut packet: Packet) -> Result<(), String> {
        packet.routing_header.increase_hop_index();
        if let Some(next_hop) = packet.routing_header.current_hop() {
            if let Some(sender) = self.packet_send.get(&next_hop) {
                if sender.send(packet.clone()).is_err() {
                    Err("Error with packet send".to_string())
                } else {
                    self.send_to_controller(NodeEvent::PacketSent(packet));
                    Ok(())
                }
            } else {
                Err("Error with path trace".to_string())
            }
        } else {
            Err("Error with path trace".to_string())
        }
    }

    // PANIC if controller doesnt exist
    fn send_to_controller(&self, node_event: NodeEvent) {
        self.controller_send.send(node_event).unwrap();
    }

    // buffer fragment path_trace MUST have the destination, REMOVE IT IF SUCCESSFUL
    fn send_buffer_fragment(&mut self, i: usize) {
        if let Some(packet) = self.buffer.get(i) {
            let fragment = packet.pack_type.clone();
            if let PacketType::MsgFragment(fragment) = fragment {
                if let Some(path_trace) =
                    self.elaborate_path_trace(packet.routing_header.destination().unwrap())
                {
                    let packet_fragment = Packet::new_fragment(
                        SourceRoutingHeader::initialize(path_trace),
                        packet.session_id,
                        fragment,
                    );
                    if self.send_packet(packet_fragment.clone()).is_ok() {
                        self.send_to_controller(NodeEvent::PacketSent(packet_fragment));
                        self.buffer.remove(i);
                    }
                }
            }
        }
    }

    // if impossible to send fragments, insert in buffer and return error
    fn send_content(
        &mut self,
        session_id: u64,
        destination: NodeId,
        raw_data: String,
    ) -> Result<(), MessageError> {
        let wrapper = SentMessageWrapper::new_from_raw_data(session_id, destination, raw_data);
        self.sent_message_wrappers
            .insert(session_id, wrapper.clone());

        self.send_to_controller(NodeEvent::CreateMessage(wrapper.clone()));

        let path_trace = self.elaborate_path_trace(destination);

        let source_routing = if let Some(trace) = path_trace {
            SourceRoutingHeader::initialize(trace)
        } else {
            for fragment in &wrapper.fragments {
                self.add_fragment_to_buffer_and_send_to_ui_unreachable(
                    fragment.clone(),
                    destination,
                    session_id,
                );
            }
            return Err(MessageError::ServerUnreachable(session_id, destination));
        };

        let mut send_success = true;

        for fragment in &wrapper.fragments {
            let packet = Packet::new_fragment(source_routing.clone(), session_id, fragment.clone());

            if self.send_packet(packet).is_err() {
                send_success = false;
                break;
            }
        }

        if !send_success {
            for fragment in &wrapper.fragments {
                self.add_fragment_to_buffer_and_send_to_ui_unreachable(
                    fragment.clone(),
                    destination,
                    session_id,
                );
            }
            return Err(MessageError::DirectConnectionDoNotWork(
                session_id,
                source_routing.next_hop().unwrap(),
            ));
        }

        Ok(())
    }

    fn send_to_ui(&self, response: ToUIComunication) {
        if self.ui_send.send(response).is_err() {
            error!("{} cannot communicate with client UI", self.id);
            process::exit(1); // problem with UI
        }
    }

    fn send_ack(&mut self, session_id: u64, source: NodeId, fragment_index: u64) {
        if let Some(path_trace) = self.elaborate_path_trace(source) {
            let packet = Packet::new_ack(
                SourceRoutingHeader::initialize(path_trace),
                session_id,
                fragment_index,
            );
            if self.send_packet(packet.clone()).is_err() {
                self.send_to_controller(NodeEvent::ControllerShortcut(Packet::new_ack(
                    SourceRoutingHeader::with_first_hop(vec![self.id, source]),
                    session_id,
                    fragment_index,
                )));
            }
        } else {
            self.send_to_controller(NodeEvent::ControllerShortcut(Packet::new_ack(
                SourceRoutingHeader::with_first_hop(vec![self.id, source]),
                session_id,
                fragment_index,
            )));
        }
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
        let mut disconnected_nodes = Vec::new();
        for (&node_id, sender) in &self.packet_send.clone() {
            match sender.send(flood_request.clone()) {
                Ok(_) => {
                    self.send_to_controller(NodeEvent::PacketSent(flood_request.clone()));
                }
                Err(_) => {
                    warn!(
                        "Channel to node {:?} is disconnected and will be removed.",
                        node_id
                    );
                    disconnected_nodes.push(node_id);
                }
            }
        }

        for node_id in disconnected_nodes {
            self.packet_send.remove(&node_id);
        }

        info!(
            "Flood request sent. Active channels: {}",
            self.packet_send.len()
        );
    }

    fn elaborate_path_trace(&self, destination: NodeId) -> Option<Vec<NodeId>> {
        let mut dist: HashMap<NodeId, Weight> = HashMap::new();
        let mut predecessor: HashMap<NodeId, NodeId> = HashMap::new();
        let mut pq = BinaryHeap::new();

        for &node in self.adj_list.keys() {
            if self.nodes_type.get(&node) == Some(&NodeType::Drone)
                || (self.nodes_type.get(&node) == Some(&NodeType::Server) && node == destination)
            {
                dist.insert(node, Weight::MAX);
            }
        }
        dist.insert(self.id, 0);

        pq.push(State {
            node: self.id,
            cost: 0,
        });

        while let Some(State {
            node: current,
            cost,
        }) = pq.pop()
        {
            if current == destination {
                let mut path = Vec::new();
                let mut node = destination;
                while node != self.id {
                    path.push(node);
                    node = *predecessor.get(&node).unwrap();
                }
                path.push(self.id);
                path.reverse();
                return Some(path);
            }

            if let Some(&stored_cost) = dist.get(&current) {
                if cost > stored_cost {
                    continue;
                }
            }

            if let Some(neighbors) = self.adj_list.get(&current) {
                for (&neighbor, &edge_cost) in neighbors {
                    // Ensure only drone nodes are considered in the path
                    if let Some(node_type) = self.nodes_type.get(&neighbor) {
                        if *node_type != NodeType::Drone && neighbor != destination {
                            continue;
                        }
                    }

                    let next_cost = cost + edge_cost;
                    if next_cost < *dist.get(&neighbor).unwrap_or(&Weight::MAX) {
                        dist.insert(neighbor, next_cost);
                        predecessor.insert(neighbor, current);
                        pq.push(State {
                            node: neighbor,
                            cost: next_cost,
                        });
                    }
                }
            }
        }
        None
    }

    // add and send Server is unrechable
    fn add_fragment_to_buffer_and_send_to_ui_unreachable(
        &mut self,
        fragment: Fragment,
        destination: NodeId,
        session_id: u64,
    ) {
        let packet = Packet::new_fragment(
            SourceRoutingHeader::initialize(vec![self.id, destination]),
            session_id,
            fragment,
        );
        self.buffer.push(packet);
        self.send_to_ui(ToUIComunication::Err(MessageError::ServerUnreachable(
            session_id,
            destination,
        )));
    }

    fn update_network(&mut self, flood_response: &FloodResponse) -> Result<(), String> {
        // check if flood_response is from the last sended flood_request
        if flood_response.flood_id == self.last_flood_id {
            let mut prev: NodeId = self.id;

            for (node_id, node_type) in flood_response.path_trace.iter() {
                self.nodes_type.insert(*node_id, *node_type); // insert the type

                if *node_id != self.id {
                    self.adj_list.entry(*node_id).or_default().insert(prev, 1);

                    self.adj_list.entry(prev).or_default().insert(*node_id, 1);
                }
                prev = *node_id;
            }
            Ok(())
        } else {
            Err("Old or not my response".to_string())
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

    fn reset_network_state(&mut self) {
        self.nodes_type.clear();
        self.adj_list.clear();
        self.servers_type.clear();
        debug!("Reset network state: cleared nodes_type and adj_list");

        // insert itself
        self.nodes_type.insert(self.id, self.node_type);

        // add direct link
        for node_id in self.packet_send.keys() {
            self.adj_list
                .entry(self.id)
                .or_default()
                .insert(*node_id, 1);
            self.adj_list
                .entry(*node_id)
                .or_default()
                .insert(self.id, 1);

            debug!("Added connection between {:?} and {:?}", self.id, node_id);
        }
    }
}

#[derive(Eq, PartialEq)]
struct State {
    node: NodeId,
    cost: Weight,
}
impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost)
    }
}
impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[allow(unused)]
#[cfg(test)]
mod tests {
    use std::{time::Duration, vec};

    use super::*;
    #[test]
    fn test_elaborate_path_trace_easy() {
        let (mut worker, _, _, _, _, _, _, _) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (12, 20)])),
            (1, HashMap::from([(10, 1), (2, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(12, 10), (1, 1), (11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
            (12, HashMap::from([(10, 20), (2, 10), (11, 1)])),
            (11, HashMap::from([(2, 1), (12, 1)])), // Nodo 11 connesso al nodo 2 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (12, NodeType::Server),
            (11, NodeType::Server),
            (10, NodeType::Client),
        ]);

        let expected = Some(vec![10, 1, 2, 12]);
        let actual = worker.elaborate_path_trace(12);

        assert_eq!(expected, actual);
    }

    #[test]
    fn test_elaborate_path_trace_complex() {
        let (mut worker, _, _, _, _, _, _, _) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(3, 1), (4, 1), (10, 1)])),
            (2, HashMap::from([(3, 1), (5, 1), (10, 1)])),
            (3, HashMap::from([(1, 1), (2, 1), (4, 1), (5, 1)])),
            (4, HashMap::from([(1, 1), (3, 1), (6, 1)])),
            (5, HashMap::from([(2, 1), (3, 1), (7, 1)])),
            (6, HashMap::from([(4, 1), (8, 1), (11, 1)])),
            (7, HashMap::from([(5, 1), (9, 1), (12, 1)])),
            (8, HashMap::from([(6, 1), (11, 1)])),
            (9, HashMap::from([(7, 1), (12, 1)])),
            (11, HashMap::from([(6, 1), (8, 1)])),
            (12, HashMap::from([(7, 1), (9, 1)])),
        ]);

        worker.nodes_type.extend([
            (10, NodeType::Client),
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (4, NodeType::Drone),
            (5, NodeType::Drone),
            (6, NodeType::Drone),
            (7, NodeType::Drone),
            (8, NodeType::Drone),
            (9, NodeType::Drone),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);

        let expected_path_1 = Some(vec![10, 1, 4, 6, 11]);
        let actual_path_1 = worker.elaborate_path_trace(11);
        assert_eq!(expected_path_1, actual_path_1, "Test 1 fallito");

        let expected_path_2 = Some(vec![10, 2, 5, 7, 12]);
        let actual_path_2 = worker.elaborate_path_trace(12);
        assert_eq!(expected_path_2, actual_path_2, "Test 2 fallito");

        let expected_path_3 = None;
        let actual_path_3 = worker.elaborate_path_trace(99);
        assert_eq!(expected_path_3, actual_path_3, "Test 3 fallito");
    }

    #[test]
    fn test_send_packet_success() {
        let (mut worker, _, _, drone2_receiver, controller_recv, _, _, _) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let input = Packet::new_ack(SourceRoutingHeader::initialize(vec![10, 2]), 1, 1);
        let mut expected = input.clone();
        expected.routing_header.increase_hop_index();

        assert!(worker.send_packet(input.clone()).is_ok());
        if let Ok(receved) = drone2_receiver.recv_timeout(Duration::from_secs(1)) {
            assert!(receved == expected);
            assert!(controller_recv.recv_timeout(Duration::from_secs(1)).is_ok());
        } else {
            panic!("Failed to receive the expected packet within the timeout");
        }
    }

    #[test]
    fn test_send_packet_fails() {
        let (mut worker, _, drone1_receiver, _, _, _, _, _) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let input = Packet::new_ack(SourceRoutingHeader::initialize(vec![10, 10]), 1, 1);
        let result = worker.send_packet(input.clone());
        if let Err(e) = &result {
            println!("Error: {e}");
        }
        assert!(result.is_err());

        let input = Packet::new_ack(SourceRoutingHeader::initialize(vec![10]), 1, 1);
        let result = worker.send_packet(input.clone());
        if let Err(e) = &result {
            println!("Error: {e}");
        }
        assert!(result.is_err());

        drop(drone1_receiver);
        worker.adj_list.remove(&1);
        let input = Packet::new_ack(SourceRoutingHeader::initialize(vec![10, 1]), 1, 1);
        let result = worker.send_packet(input.clone());
        if let Err(e) = &result {
            println!("Error: {e}");
        }
        assert!(result.is_err());
    }

    #[test]
    fn test_send_buffer_fragment_success() {
        let (mut worker, _, drone1_receiver, _, controller_recv, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let input = Fragment::from_string(19, 1, "Test".to_string());
        let expected = Packet::new_fragment(
            SourceRoutingHeader::with_first_hop(vec![10, 1]),
            0,
            input.clone(),
        );

        worker.add_fragment_to_buffer_and_send_to_ui_unreachable(input, 1, 0);
        worker.send_buffer_fragment(0);

        let result = drone1_receiver.recv_timeout(Duration::from_secs(1));
        if let Ok(actual) = result {
            assert_eq!(actual, expected);
            assert!(controller_recv.recv_timeout(Duration::from_secs(1)).is_ok());
            assert!(ui_receiver.recv_timeout(Duration::from_secs(1)).is_ok());
            assert!(worker.buffer.is_empty());
        } else {
            panic!("Failed to receive the expected packet within the timeout");
        }
    }

    #[test]
    fn test_send_buffer_fragment_fails() {
        let (mut worker, _, _, _, _, _, _, ui_receiver) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let input = Fragment::from_string(19, 1, "Test".to_string());

        worker.send_buffer_fragment(2);
        assert!(true);

        worker.add_fragment_to_buffer_and_send_to_ui_unreachable(input, 3, 0);
        worker.send_buffer_fragment(0);
        assert!(true);
    }

    #[test]
    fn test_send_content_success() {
        let (mut worker, _, drone1_receiver , _, controller_recv, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let result = worker.send_content(0, 1, "TestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTest".to_string());
        assert!(!worker.sent_message_wrappers.is_empty());
        match result {
            Ok(()) => {
                assert!(worker.buffer.is_empty());
                for _ in 0..5 {
                    assert!(drone1_receiver.recv_timeout(Duration::from_secs(1)).is_ok());
                }
            }
            Err(e) => panic!("Failed to send content: {:?}", e),
        }
    }

    #[test]
    fn test_send_content_fails() {
        let (mut worker, _, _, _, controller_recv, _, _, ui_receiver) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        let result = worker.send_content(0, 3, "ciao".to_string());

        assert!(!worker.sent_message_wrappers.is_empty());
        match result {
            Ok(()) => panic!("Should be error!"),
            Err(e) => {
                assert_eq!(e, MessageError::ServerUnreachable(0, 3));
                assert!(!worker.buffer.is_empty());
            }
        }

        let result = worker.send_content(0, 1, "ciao".to_string());

        assert!(!worker.sent_message_wrappers.is_empty());
        match result {
            Ok(()) => panic!("Should be error!"),
            Err(e) => {
                assert_eq!(e, MessageError::DirectConnectionDoNotWork(0, 1));
                assert!(!worker.buffer.is_empty());
            }
        }
    }

    #[test]
    fn test_send_ack_success() {
        let (mut worker, _, drone1_receiver, _, controller_receiver, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        worker.send_ack(0, 1, 10);

        assert!(controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .is_ok());
    }

    #[test]
    fn test_send_ack_fails() {
        let (mut worker, _, _, _, controller_receiver, _, _, _) = create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        worker.send_ack(0, 1, 10);
        let result = controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if let NodeEvent::ControllerShortcut(packet) = result {
            assert_eq!(packet.routing_header.current_hop(), Some(1));
        } else {
            panic!("Wront Node Event");
        }

        worker.send_ack(0, 5, 10);
        let result = controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if let NodeEvent::ControllerShortcut(packet) = result {
            assert_eq!(packet.routing_header.current_hop(), Some(5));
        } else {
            panic!("Wront Node Event");
        }
    }

    #[test]
    fn test_flood_request_one_valid_and_one_not_valid() {
        let (mut worker, _, node1_received, _, controller_receiver, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::new()),           // Nodo 1 senza connessioni
            (2, HashMap::from([(11, 1)])), // Nodo 2 connesso al nodo 11 con peso 1
        ]);

        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (10, NodeType::Client),
        ]);

        worker.send_flood_request();

        assert!(node1_received.recv_timeout(Duration::from_secs(1)).is_ok());
        assert!(ui_receiver.recv_timeout(Duration::from_secs(1)).is_ok());
        assert!(controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .is_ok());
        assert!(!worker.packet_send.contains_key(&2));
    }

    #[test]
    fn test_all_nteraction_handler() {
        let (mut worker, _, node1_receiver, node2_receiver, controller_receiver, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (11, HashMap::from([(1, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (12, HashMap::from([(2, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (10, NodeType::Client),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);

        worker.interaction_handler(FromUiCommunication::AskServerType(0, 11));

        let result = node1_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            result,
            Packet::new_fragment(
                SourceRoutingHeader::with_first_hop(vec![10, 1, 11]),
                0,
                Fragment::from_string(0, 1, "ServerType".to_string())
            )
        );

        worker.interaction_handler(FromUiCommunication::ReloadNetwork);

        let result1 = node1_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let result2 = node2_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(
            result1,
            Packet::new_flood_request(
                SourceRoutingHeader::empty_route(),
                0,
                FloodRequest::initialize(1, 10, NodeType::Client)
            )
        );
        assert_eq!(
            result2,
            Packet::new_flood_request(
                SourceRoutingHeader::empty_route(),
                0,
                FloodRequest::initialize(1, 10, NodeType::Client)
            )
        );
    }

    #[test]
    fn test_flood_request_handler() {
        let (mut worker, _, node1_received, _, controller_receiver, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (11, HashMap::from([(1, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (12, HashMap::from([(2, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (10, NodeType::Client),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);

        // success
        let mut flood_request =
            FloodRequest::initialize(0, 11, NodeType::Server).get_incremented(1, NodeType::Drone);
        let input =
            Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request.clone());
        worker.packet_handler(input);
        flood_request = flood_request.get_incremented(10, NodeType::Client);
        let mut expect = flood_request.generate_response(0);
        expect.routing_header.increase_hop_index();
        let result = node1_received.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(result, expect);
        assert!(controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .is_ok());

        // shortcut
        let mut flood_request =
            FloodRequest::initialize(0, 11, NodeType::Server).get_incremented(2, NodeType::Drone);
        let input =
            Packet::new_flood_request(SourceRoutingHeader::empty_route(), 0, flood_request.clone());
        worker.packet_handler(input);
        flood_request = flood_request.get_incremented(10, NodeType::Client);
        let mut expect = flood_request.generate_response(0);
        expect.routing_header.increase_hop_index();
        let result = controller_receiver
            .recv_timeout(Duration::from_secs(1))
            .unwrap();
        if let NodeEvent::ControllerShortcut(result) = result {
            assert_eq!(expect, result);
        } else {
            panic!("wrong event");
        }
    }

    #[test]
    fn test_flood_response_handler() {
        let (mut worker, _, node1_received, _, controller_receiver, _, _, ui_receiver) =
            create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker
            .nodes_type
            .extend([(1, NodeType::Drone), (2, NodeType::Drone)]);
        let flood_response1: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(11, NodeType::Server)
            .generate_response(1);
        let flood_response2: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(12, NodeType::Server)
            .generate_response(1);
        let flood_response3: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(11, NodeType::Server)
            .generate_response(1);
        let flood_response4: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .generate_response(1);
        let flood_response5: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .generate_response(2);

        let flood_response6: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .generate_response(1);
        let flood_response7: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(11, NodeType::Server)
            .generate_response(1);
        let flood_response8: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(12, NodeType::Server)
            .generate_response(1);
        let flood_response9: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .generate_response(1);
        let flood_response10: Packet = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(12, NodeType::Server)
            .generate_response(1);

        worker.packet_handler(flood_response1);
        worker.packet_handler(flood_response2);
        worker.packet_handler(flood_response3);
        worker.packet_handler(flood_response4);
        worker.packet_handler(flood_response5);
        worker.packet_handler(flood_response6);
        worker.packet_handler(flood_response7);
        worker.packet_handler(flood_response8);
        worker.packet_handler(flood_response9);
        worker.packet_handler(flood_response10);

        let expected_adj_list: HashMap<u8, HashMap<u8, Weight>> = HashMap::from([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])),
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])),
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])),
            (11, HashMap::from([(1, 1), (3, 1)])),
            (12, HashMap::from([(2, 1), (3, 1)])),
        ]);

        if worker.adj_list.len() != expected_adj_list.len() {
            assert!(false);
        }

        for (key, value) in worker.adj_list {
            if let Some(value2) = expected_adj_list.get(&key) {
                if value != *value2 {
                    assert!(true);
                }
            } else {
                assert!(false);
            }
        }
    }

    #[test]
    fn test_nack_dropped_handler(){
        let (mut worker, _, node1_receiver, node2_receiver, controller_receiver, _, _, ui_receiver) =
        create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (11, HashMap::from([(1, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (12, HashMap::from([(2, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (10, NodeType::Client),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);
        worker.sent_message_wrappers.insert(0, SentMessageWrapper::new_from_raw_data(0, 11, "Test".to_string()));

        worker.nack_handler(&Nack { fragment_index: 0, nack_type: NackType::Dropped }, &Packet::new_fragment(SourceRoutingHeader::new(vec![10, 1, 11], 1), 0, worker.sent_message_wrappers.get(&0).unwrap().get_fragment(0).unwrap()));
        node1_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        controller_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(worker.adj_list.get(&10).unwrap().get(&1), Some(&2));
        assert_eq!(worker.adj_list.get(&1).unwrap().get(&10), Some(&2));

        worker.nack_handler(&Nack { fragment_index: 0, nack_type: NackType::Dropped }, &Packet::new_fragment(SourceRoutingHeader::new(vec![10, 1, 20], 1), 0, worker.sent_message_wrappers.get(&0).unwrap().get_fragment(0).unwrap()));
        controller_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(worker.adj_list.get(&10).unwrap().get(&1), Some(&3));
        assert_eq!(worker.adj_list.get(&1).unwrap().get(&10), Some(&3));
        assert!(!worker.buffer.is_empty());
    }

    #[test]
    fn test_nack_other_handler(){
        let (mut worker, _, node1_receiver, node2_receiver, controller_receiver, _, _, ui_receiver) =
        create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (11, HashMap::from([(1, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (12, HashMap::from([(2, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (10, NodeType::Client),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);
        worker.sent_message_wrappers.insert(0, SentMessageWrapper::new_from_raw_data(0, 11, "Test".to_string()));
        worker.nack_handler(&Nack { fragment_index: 0, nack_type: NackType::ErrorInRouting(8) }, &Packet::new_fragment(SourceRoutingHeader::new(vec![10, 1, 8, 11], 1), 0, worker.sent_message_wrappers.get(&0).unwrap().get_fragment(0).unwrap()));

        assert!(!worker.buffer.is_empty())
    }

    #[test]
    fn test_fragment_handler_invalid_message(){
        let (mut worker, _, node1_receiver, node2_receiver, controller_receiver, _, _, ui_receiver) =
        create_worker();

        worker.adj_list.extend([
            (10, HashMap::from([(1, 1), (2, 1)])),
            (1, HashMap::from([(10, 1), (2, 1), (3, 1), (11, 1)])), // Nodo 1 senza connessioni
            (2, HashMap::from([(10, 1), (1, 2), (3, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (3, HashMap::from([(1, 1), (2, 1), (11, 1), (12, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (11, HashMap::from([(1, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
            (12, HashMap::from([(2, 1), (3, 1)])), // Nodo 2 connesso al nodo 11 con peso 1,
        ]);
        worker.nodes_type.extend([
            (1, NodeType::Drone),
            (2, NodeType::Drone),
            (3, NodeType::Drone),
            (10, NodeType::Client),
            (11, NodeType::Server),
            (12, NodeType::Server),
        ]);
        let fragment_wrapper = SentMessageWrapper::new_from_raw_data(0, 10, "TestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTestTest".to_string());

        worker.fragment_handler(&fragment_wrapper.get_fragment(0).unwrap(), &Packet::new_fragment(SourceRoutingHeader::new(vec![11,1,10], 2), 0, fragment_wrapper.get_fragment(0).unwrap()));
        node1_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        controller_receiver.recv_timeout(Duration::from_secs(1)).unwrap();

        worker.fragment_handler(&fragment_wrapper.get_fragment(1).unwrap(), &Packet::new_fragment(SourceRoutingHeader::new(vec![11,1,10], 2), 0, fragment_wrapper.get_fragment(1).unwrap()));
        node1_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        controller_receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let uimesg = ui_receiver.recv_timeout(Duration::from_secs(1)).unwrap();

    }

    fn create_worker() -> (
        Worker,
        Sender<Packet>,              // Per inviare pacchetti al Worker
        Receiver<Packet>,            // Per ricevere pacchetti dal Drone 1
        Receiver<Packet>,            // Per ricevere pacchetti dal Drone 2
        Receiver<NodeEvent>,         // Per ricevere eventi dal Worker (controller)
        Sender<NodeCommand>,         // Per inviare comandi al Worker (controller)
        Sender<FromUiCommunication>, // Per inviare messaggi all'UI
        Receiver<ToUIComunication>,  // Per ricevere messaggi dall'UI
    ) {
        let id = 10;

        // Canali per la comunicazione dei pacchetti
        let (packet_sender, packet_receiver) = unbounded();

        // Canali per la comunicazione con i droni
        let (drone1_sender, drone1_receiver) = unbounded();
        let (drone2_sender, drone2_receiver) = unbounded();

        // Mappa per i canali di invio ai droni
        let mut drone_senders = HashMap::new();
        drone_senders.insert(1, drone1_sender);
        drone_senders.insert(2, drone2_sender);

        // Canali per la comunicazione con il controller
        let (controller_event_sender, controller_event_receiver) = unbounded(); // Worker invia eventi al controller
        let (controller_command_sender, controller_command_receiver) = unbounded(); // Worker riceve comandi dal controller

        // Canali per la comunicazione con l'interfaccia utente (UI)
        let (ui_sender, ui_receiver) = unbounded(); // Worker invia messaggi all'UI
        let (logic_sender, logic_receiver) = unbounded(); // Worker riceve messaggi dall'UI

        // Creiamo il worker
        let worker = Worker::new(
            id,
            NodeType::Client,
            packet_receiver,             // Worker riceve pacchetti da questo canale
            drone_senders,               // Worker invia pacchetti ai droni attraverso questa mappa
            controller_command_receiver, // Worker riceve comandi dal controller
            controller_event_sender,     // Worker invia eventi al controller
            logic_receiver,              // Worker riceve messaggi dall'UI
            ui_sender,                   // Worker invia messaggi all'UI
        );

        // Restituiamo il worker e l'altra parte dei canali che il worker non prende
        (
            worker,
            packet_sender,             // Canale per inviare pacchetti al Worker
            drone1_receiver,           // Canale per ricevere pacchetti dal Drone 1
            drone2_receiver,           // Canale per ricevere pacchetti dal Drone 2
            controller_event_receiver, // Canale per ricevere eventi dal Worker (controller)
            controller_command_sender, // Canale per inviare comandi al Worker (controller)
            logic_sender,              // Canale per inviare messaggi all'UI
            ui_receiver,               // Canale per ricevere messaggi dall'UI
        )
    }
}
