use crossbeam_channel::select_biased;
use crossbeam_channel::{Receiver, Sender};
use message::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f64::MAX;
use wg_2024::network::*;
use wg_2024::packet::NodeType::{Client, Server};
use wg_2024::packet::{FloodRequest, NackType, NodeType, Packet, PacketType};
/*pub trait Server {
    type RequestType: Request;
    type ResponseType: Response;

    fn compose_message(
        source_id: NodeId,
        session_id: u64,
        raw_content: String,
    ) -> Result<Message<Self::RequestType>, String> {
        let content = Self::RequestType::from_string(raw_content)?;
        Ok(Message {
            session_id,
            source_id,
            content,
        })
    }

    fn on_request_arrived(&mut self, source_id: NodeId, session_id: u64, raw_content: String) {
        if raw_content == "ServerType" {
            let _server_type = Self::get_sever_type();
            // send response
            return;
        }
        match Self::compose_message(source_id, session_id, raw_content) {
            Ok(message) => {
                let response = self.handle_request(message.content);
                self.send_response(response);
            }
            Err(str) => panic!("{}", str),
        }
    }

    fn send_response(&mut self, _response: Self::ResponseType) {
        // send response
    }

    fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType;

    fn get_sever_type() -> ServerType;
}*/

#[derive(Clone, Debug)]
pub struct ChatServer {
    pub id: NodeId,
    pub controller_send: Sender<NodeEvent>,
    pub controller_recv: Receiver<NodeCommand>,
    pub packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    pub last_session_id: u64,

    pub topology: HashMap<NodeId, (HashSet<NodeId>, f64, f64)>,
    //da rivedere
    pub incoming_fragments: HashMap<u64, VecDeque<Packet>>,
    pub outgoing_fragments: HashMap<u64, VecDeque<Packet>>,
}

impl ChatServer {
    fn new(
        id: NodeId,
        controller_send: Sender<NodeEvent>,
        controller_recv: Receiver<NodeCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
    ) -> Self {
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            last_session_id: 0,
            topology: HashMap::new(),
            incoming_fragments: HashMap::new(),
            outgoing_fragments: HashMap::new(),
        }
    }

    fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.controller_recv) -> packet =>{
                    if let Ok(packet) = packet {
                        self.command_handler(packet);
                    }
                },
                recv(self.packet_recv) -> packet =>{
                    if let Ok(packet) = packet {
                        self.packet_handler(packet);
                    }
                }
            }
        }
    }

    fn command_handler(&mut self, packet: NodeCommand) {
        match packet {
            NodeCommand::AddSender(id, sender) => {
                self.packet_send.insert(id, sender);
            }
            NodeCommand::RemoveSender(id) => {
                self.packet_send.remove(&id);
            }
            NodeCommand::FromShortcut(pack) => {
                self.packet_handler(pack);
            }
        }
    }

    fn packet_handler(&mut self, mut packet: Packet) {
        match packet.pack_type {
            //da completare
            PacketType::MsgFragment(fragment) => {}
            //da completare, mancano controlli
            PacketType::Ack(ack) => {
                self.topology
                    .get_mut(&packet.routing_header.hops[0])
                    .unwrap()
                    .2 += 1.0;
                self.topology
                    .get_mut(&packet.routing_header.hops[0])
                    .unwrap()
                    .1 += 1.0;
            }
            //da completare
            PacketType::Nack(nack) => {}
            PacketType::FloodRequest(flood_request) => {
                if let Some((last_nodeId, _)) = flood_request.path_trace.last() {
                    let updated_flood_request =
                        flood_request.get_incremented(self.id, NodeType::Server);
                    let mut response = updated_flood_request.generate_response(packet.session_id);
                    self.send_packet(&mut response);
                }
            }
            PacketType::FloodResponse(floodresponse) => {
                for n in 0..floodresponse.path_trace.len() - 2 {
                    if !self.topology.contains_key(&floodresponse.path_trace[n].0) {
                        if floodresponse.path_trace[n].1 == Server
                            || floodresponse.path_trace[n].1 == Client
                        {
                            self.topology
                                .insert(floodresponse.path_trace[n].0, (HashSet::new(), 1.0, 1.0));
                        } else {
                            self.topology
                                .insert(floodresponse.path_trace[n].0, (HashSet::new(), 0.0, 0.0));
                        }
                    }
                    self.topology
                        .get_mut(&floodresponse.path_trace[n].0)
                        .unwrap()
                        .0
                        .insert(floodresponse.path_trace[n + 1].0.clone());
                }
            }
        }
    }
    //da modificare
    fn send_packet(&mut self, packet: &mut Packet) {
        packet.routing_header.increase_hop_index();
        if let Some(next_hop) = packet.routing_header.current_hop() {
            if let Some(sender) = self.packet_send.get_mut(&next_hop) {
                if sender.send(packet.clone()).is_err() {
                } else if let PacketType::MsgFragment(_) = packet.pack_type {
                    let event = NodeEvent::PacketSent(packet.clone());

                    self.send_event(event);
                }
            } else {
            }
        }
    }
    //chiarire flood_id come impostarlo
    fn flood_initializer(&mut self) {
        let request = FloodRequest::initialize(0, self.id, NodeType::Server);
        let sourcerouting = SourceRoutingHeader::initialize(vec![self.id]);
        let packet = Packet::new_flood_request(sourcerouting, self.last_session_id, request);
        self.last_session_id += 1; //forse non va bene? Non ne ho idea
        for (node_id, sender) in &self.packet_send {
            let _ = sender.send(packet.clone());
        }
    }
    //da modificare (i client e gli altri server non hanno pdp!!)
    fn calculate_path(&self, node_id: NodeId) -> Vec<NodeId> {
        let mut path = vec![0, self.id];
        let mut psp = HashMap::new();
        let mut dist = HashMap::new();
        let mut prev = HashMap::new();
        let mut queue = VecDeque::new();
        let mut current_node;

        for node in self.topology.keys() {
            let prob = self.topology.get(node).unwrap().1 / self.topology.get(node).unwrap().2;
            psp.insert(*node, -prob.ln());
            dist.insert(*node, MAX);
        }

        for node in self.topology.get(&self.id).unwrap().0.iter() {
            queue.push_back(*node);
        }

        dist.insert(self.id, *psp.get(&self.id).unwrap());

        while !queue.is_empty() {
            current_node = queue.pop_front().unwrap();

            for vec_node_id in self.topology.get(&current_node).unwrap().0.iter() {
                if dist.get(&current_node).unwrap() + psp.get(&vec_node_id).unwrap()
                    < *dist.get(&vec_node_id).unwrap()
                {
                    dist.insert(
                        *vec_node_id,
                        dist.get(&current_node).unwrap() + psp.get(&vec_node_id).unwrap(),
                    );
                    prev.insert(*vec_node_id, current_node);
                    queue.push_back(*vec_node_id);
                }
            }
        }

        current_node = node_id;
        while prev.get(&current_node).unwrap() != &self.id {
            path.push(current_node);
            current_node = *prev.get(&current_node).unwrap();
        }

        path.push(self.id);
        path.reverse();

        path
    }

    fn packet_assembler() {}

    fn send_event(&self, event: NodeEvent) {
        if self.controller_send.send(event).is_err() {
            panic!("Controller is unreaceable");
        }
    }
}

/*impl Server for ChatServer {
    type RequestType = ChatRequest;
    type ResponseType = ChatResponse;

    fn handle_request(&mut self, request: Self::RequestType) -> Self::ResponseType {
        match request {
            ChatRequest::ClientList => {
                println!("Sending ClientList");
                ChatResponse::ClientList(vec![1, 2])
            }
            ChatRequest::Register(id) => {
                println!("Registering {}", id);
                ChatResponse::ClientList(vec![1, 2])
            }
            ChatRequest::SendMessage {
                message,
                to,
                from: _,
            } => {
                println!("Sending message \"{}\" to {}", message, to);
                // effectively forward message
                ChatResponse::MessageSent
            }
        }
    }

    fn get_sever_type() -> ServerType {
        ServerType::Chat
    }
}*/
