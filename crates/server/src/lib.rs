mod message;
mod network;

use crate::message::ServerMessageManager;
use crate::network::NetworkManager;
use ::message::NodeEvent::ControllerShortcut;
use ::message::{NodeCommand, NodeEvent};
use crossbeam_channel::select_biased;
use crossbeam_channel::{Receiver, Sender};
use log::{info, warn};
use std::collections::HashMap;
use std::time::Duration;
use wg_2024::network::*;
use wg_2024::packet::{FloodRequest, NodeType, Packet, PacketType};

#[derive(Clone, Debug)]
pub struct ChatServer {
    pub id: NodeId,
    pub controller_send: Sender<NodeEvent>,
    pub controller_recv: Receiver<NodeCommand>,
    pub packet_recv: Receiver<Packet>,
    pub packet_send: HashMap<NodeId, Sender<Packet>>,
    pub last_session_id: u64,
    pub network_manager: NetworkManager,
    pub server_message_manager: ServerMessageManager,
    pub server_buffer: HashMap<NodeId, Vec<Packet>>,
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
            network_manager: NetworkManager::new(id, Duration::new(10, 0)),
            server_message_manager: ServerMessageManager::new(),
            server_buffer: HashMap::new(),
        }
    }

    fn run(&mut self) {
        self.flood_initializer();

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
            
            self.try_resend();
        }
    }

    fn command_handler(&mut self, packet: NodeCommand) {
        match packet {
            NodeCommand::AddSender(id, sender) => {
                self.packet_send.insert(id, sender);
                self.flood_initializer();
            }
            NodeCommand::RemoveSender(id) => {
                self.packet_send.remove(&id);
                self.network_manager.remove_node(id);
                self.network_manager.generate_all_routes();
            }
            NodeCommand::FromShortcut(pack) => {
                self.packet_handler(pack);
            }
        }
    }

    fn packet_handler(&mut self, packet: Packet) {
        match packet.pack_type {
            //da completare
            PacketType::MsgFragment(fragment) => {
                let key = &(packet.session_id, packet.routing_header.source().unwrap());
                //todo controparte in message, poi cacellare questo
                if !self.server_message_manager.is_registered(&key.1) {
                    warn!("Client {} not registered", key.1);
                    
                    return;
                }
                
                self.server_message_manager.store_fragment(key, fragment.clone());

                if self.server_message_manager.are_all_fragment_arrived(key) {
                    let recv_msg = self
                        .server_message_manager
                        .get_incoming_fragments(key)
                        .unwrap();
                    info!("Message {:?} received", recv_msg);
                    self.send_event(NodeEvent::MessageRecv(recv_msg));

                    if let Some(wrapper) =
                        self.server_message_manager.message_handling(&key, self.last_session_id + 1)
                    {
                        self.last_session_id += 1;
                        self.send_event(NodeEvent::CreateMessage(wrapper.clone()));
                        for frag in wrapper.fragments {
                            self.send_packet(&mut Packet {
                                routing_header: SourceRoutingHeader::initialize(
                                    self.network_manager
                                        .get_route(&wrapper.destination)
                                        .unwrap(),
                                ),
                                session_id: wrapper.session_id, //todo decidere come metterci il last_session_id
                                pack_type: PacketType::MsgFragment(frag),
                            })
                        }
                    }
                }
            }
            //da completare, mancano controlli (?)
            PacketType::Ack(ack) => {
                self.network_manager
                    .update_from_ack(packet.routing_header.source().unwrap());
                self.server_message_manager
                    .insert_ack(ack, &packet.session_id);
            }
            //da controllare
            PacketType::Nack(nack) => {
                self.network_manager
                    .update_from_nack(packet.routing_header.hops[0], nack.clone());

                let wrapper = self
                    .server_message_manager
                    .get_outgoing_packet(&packet.session_id)
                    .unwrap();
                let fragment_to_resend =
                    wrapper.get_fragment(nack.fragment_index as usize).unwrap();
                let mut packet_to_send = Packet {
                    routing_header: SourceRoutingHeader::initialize(
                        self.network_manager.get_route(&wrapper.destination).unwrap()
                    ),
                    session_id: wrapper.session_id,
                    pack_type: PacketType::MsgFragment(fragment_to_resend),
                };

                self.send_packet(&mut packet_to_send);

                if self.network_manager.should_flood_request() {
                    self.flood_initializer();
                }
            }
            PacketType::FloodRequest(flood_request) => {
                let updated_flood_request =
                    flood_request.get_incremented(self.id, NodeType::Server);
                let mut response = updated_flood_request.generate_response(packet.session_id);
                self.send_packet(&mut response);
            }
            PacketType::FloodResponse(flood_response) => {
                self.network_manager
                    .update_from_flood_response(flood_response);
            }
        }
    }
    fn try_resend(&mut self) {
        if !self.server_buffer.is_empty(){
            let keys = self.server_buffer.keys().cloned().collect::<Vec<_>>();

            for key in keys.iter() {
                for mut packet in self.server_buffer.remove(key).unwrap().clone() {
                    self.network_manager.update_routing_path(&mut packet.routing_header);
                    self.send_packet(&mut packet);
                }
            }
        }
    }
    //da modificare: non gestisco il caso in cui il drone o il sender siano non raggiungibili e il messaggio rimane nel buffer
    fn send_packet(&mut self, packet: &mut Packet) {
        packet.routing_header.increase_hop_index();
        if let Some(next_hop) = packet.routing_header.current_hop() {
            if let Some(sender) = self.packet_send.get(&next_hop) {
                if sender.send(packet.clone()).is_err() {
                    warn!("Failed to send packet, Drone {} unreachable", next_hop);
                    self.add_to_buffer(packet.clone());
                    match packet.pack_type {
                        PacketType::Ack(_) | PacketType::FloodResponse(_) => {
                            self.send_event(ControllerShortcut(packet.clone()));
                        }
                        _ => {
                            //todo (?)
                        }
                    }

                    self.network_manager.remove_node(next_hop);
                    self.network_manager.generate_all_routes();
                    
                    if self.network_manager.should_flood_request(){
                        self.flood_initializer();
                    }
                } else {
                    info!(
                        "Packet with session id {} sent successfully to {}",
                        packet.session_id, next_hop
                    );
                    let event = NodeEvent::PacketSent(packet.clone());
                    self.send_event(event);
                }
            } else {
                warn!("Sender for {} drone is unreachable", next_hop);
                self.network_manager.remove_adj(next_hop);
                self.flood_initializer();
            }
        }
    }
    //chiarire flood_id come impostarlo
    fn flood_initializer(&mut self) {
        let request = FloodRequest::initialize(0, self.id, NodeType::Server);
        let source_routing = SourceRoutingHeader::initialize(vec![self.id]);
        let packet = Packet::new_flood_request(source_routing, self.last_session_id, request);
        self.last_session_id += 1; //forse non va bene? Non ne ho idea
        for (_, sender) in &self.packet_send {
            let _ = sender.send(packet.clone());
        }
    }
    fn add_to_buffer(&mut self, packet: Packet) {
        let dest = &packet.routing_header.source().unwrap();
        if !self.server_buffer.contains_key(dest) {
            self.server_buffer.insert(*dest, Vec::new());
        }
        self.server_buffer.get_mut(dest).unwrap().push(packet);
    }
    fn send_event(&self, event: NodeEvent) {
        if self.controller_send.send(event).is_err() {
            panic!("Controller is unreaceable");
        }
    }
}
