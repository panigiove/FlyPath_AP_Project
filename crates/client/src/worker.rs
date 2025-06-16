use crate::channel::ChannelManager;
use crate::comunication::FromUiCommunication::{AskClientList, RefreshTopology, SendChatMessage};
use crate::comunication::{FromUiCommunication, ToUICommunication};
use crate::message::MessagerManager;
use crate::network::NetworkManager;
use crossbeam_channel::{select_biased, Receiver, Sender};
use log::{debug, error, info, warn};
use message::NodeEvent::CreateMessage;
use message::{ChatRequest, NodeCommand, NodeEvent};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::NodeType::{Client, Drone};
use wg_2024::packet::Packet;
use wg_2024::packet::PacketType::{Ack, FloodRequest, FloodResponse, MsgFragment, Nack};

pub struct Worker {
    my_id: NodeId,
    network: NetworkManager,
    message: MessagerManager,
    channels: Rc<RefCell<ChannelManager>>,
}

impl Worker {
    pub fn new(
        my_id: NodeId,
        tx_drone: HashMap<NodeId, Sender<Packet>>,
        tx_ctrl: Sender<NodeEvent>,
        tx_ui: Sender<ToUICommunication>,
        rx_drone: Receiver<Packet>,
        rx_ctrl: Receiver<NodeCommand>,
        rx_ui: Receiver<FromUiCommunication>,
    ) -> Self {
        let channel_manager =
            ChannelManager::new(tx_drone, tx_ctrl, tx_ui, rx_drone, rx_ctrl, rx_ui);
        let channels = Rc::new(RefCell::new(channel_manager));
        let network = NetworkManager::new(my_id, channels.clone());
        let message = MessagerManager::new(my_id, channels.clone());
        Self {
            my_id,
            network,
            message,
            channels,
        }
    }

    pub fn run(&mut self) {
        info!("Worker {:?} starting", self.my_id);
        loop {
            let (mut cmd, mut inter, mut pack) = (None, None, None);
            select_biased! {
                recv (self.channels.borrow().rx_ctrl) -> res => if let Ok(c) = res { cmd = Some(c) } ,
                recv (self.channels.borrow().rx_ui) -> res => if let Ok(i) = res { inter = Some(i) },
                recv (self.channels.borrow().rx_drone) -> res => if let Ok(p) = res { pack = Some(p) },
            };

            if let Some(cmd) = cmd {
                info!("handling CMD");
                match cmd {
                    NodeCommand::RemoveSender(nid) => {
                        debug!("CMD: Remove Sender {:?}", nid);
                        self.channels.borrow_mut().tx_drone.remove(&nid);
                        self.network.state.remove_node(&nid);
                        if !self
                            .network
                            .state
                            .recompute_all_routes_to_server(Some(&nid))
                        {
                            self.network.send_flood_request();
                        }
                    }
                    NodeCommand::AddSender(nid, tx_drone) => {
                        debug!("CMD: Add Sender {:?}", nid);
                        self.channels.borrow_mut().tx_drone.insert(nid, tx_drone);
                        self.network
                            .state
                            .add_link(self.my_id, nid, Client, Drone, 1);
                        if !self.network.state.recompute_all_routes_to_server(None) {
                            self.network.send_flood_request();
                        }
                    }
                    NodeCommand::FromShortcut(pack) => {
                        self._packet_handler(pack);
                    }
                }
            }
            if let Some(inter) = inter {
                info!("handling UI interaction");
                match inter {
                    RefreshTopology => {
                        debug!("UI: Send Flood Request");
                        self.network.send_flood_request();
                    }
                    AskClientList => {
                        debug!("UI: Broadcast ClientList Request");
                        let servers: Vec<_> =
                            self.network.state.server_list.iter().cloned().collect();
                        for server in servers {
                            self._send_message(&server, ChatRequest::ClientList);
                        }
                    }
                    SendChatMessage {
                        to_client: destination,
                        message: body,
                    } => {
                        if let Some(sids) = self.message.clients.get(&destination) {
                            if let Some(&sid) = sids.iter().next() {
                                debug!(
                                    "UI: Send Message to: {:?} with message {:?} through: {:?}",
                                    destination, body, sid
                                );
                                let request = ChatRequest::SendMessage {
                                    from: self.my_id,
                                    to: sid,
                                    message: body,
                                };
                                self._send_message(&sid, request);
                            } else {
                                warn!("Client {:?} has no known servers", destination);
                            }
                        }
                    }
                }
            }
            if let Some(pack) = pack {
                info!("handling PACKET");
                self._packet_handler(pack);
            }

            if self.network.state.should_flood() {
                info!("Network State expired, ask for flooding");
                self.network.send_flood_request();
            }
        }
    }

    fn _packet_handler(&mut self, packet: Packet) {
        let session = packet.session_id;
        if let Some(from) = packet.routing_header.source() {
            match &packet.pack_type {
                FloodRequest(request) => {
                    debug!("PACK: flood request handling from {:?}", from);
                    // let channels = self.channels.borrow();

                    let mut response = request
                        .get_incremented(self.my_id, Client)
                        .generate_response(packet.session_id);
                    response.routing_header.increase_hop_index();
                    debug!("Generated FloodResponse: {:?}", response.routing_header);

                    // upgrade the topology throw generate flood response
                    if let FloodResponse(response) = &response.pack_type {
                        if let Some(server_reach) =
                            self.network.update_network_from_flood_response(response)
                        {
                            info!("New servers discovered: {:?}", server_reach);
                            self._send_buffer(&server_reach);
                            self._registry_and_client_list(&server_reach);
                        } else {
                            debug!("No new servers found in flood response.");
                        }
                    }

                    if let Some(nid) = response.routing_header.current_hop() {
                        if let Some(tx_drone) = self.channels.borrow().tx_drone.get(&nid) {
                            if tx_drone.send(response.clone()).is_ok() {
                                debug!("Flood response sent to drone {:?}", nid);
                                self.channels
                                    .borrow()
                                    .tx_ctrl
                                    .send(NodeEvent::PacketSent(response.clone()))
                                    .expect("Failed to transmit to CONTROLLER");
                                return;
                            } else {
                                warn!("Failed to send flood response to drone {:?}", nid);
                            }
                        } else {
                            warn!("No drone channel found for {:?}", nid);
                            self.network.state.remove_node(&nid);
                        }

                        info!("Sending packet to controller as fallback for {:?}", nid);
                        self.channels
                            .borrow()
                            .tx_ctrl
                            .send(NodeEvent::ControllerShortcut(response.clone()))
                            .expect("Failed to transmit to CONTROLLER");
                        self.channels.borrow_mut().tx_drone.remove(&nid);
                    } else {
                        error!("No next hop found in routing header");
                    }
                }
                FloodResponse(flood_response) => {
                    if let Some(server_reach) = self
                        .network
                        .update_network_from_flood_response(flood_response)
                    {
                        self._send_buffer(&server_reach);
                        self._registry_and_client_list(&server_reach);
                    }
                }
                Ack(ack) => {
                    self.network.update_network_from_ack(&from);
                    self.message.ack_and_build_message(ack, session);
                }
                Nack(nack) => {
                    self.network.update_network_from_nack(nack, &from);
                    if let Some(fragment) = self.message.get_dropped_fragment(nack, session) {
                        let packet = Packet::new_fragment(
                            SourceRoutingHeader::empty_route(),
                            session,
                            fragment.clone(),
                        );

                        if let Some(sid) = self.message.get_destination(&session) {
                            if !self.network.send_packet(&packet, &sid) {
                                self.message.add_packets_to_buffer(&sid, vec![packet]);
                            }
                        }
                    }
                }
                MsgFragment(frag) => {
                    self.message
                        .save_received_message(frag.clone(), session, from);
                }
            }
        }
    }

    fn _send_message(&mut self, sid: &NodeId, msg: ChatRequest) {
        let wrapper = self.message.create_and_store_wrapper(sid, msg);
        let mut unsent = Vec::new();

        self.channels
            .borrow()
            .tx_ctrl
            .send(CreateMessage(wrapper.clone()))
            .expect("Failed to transmit to Controller");

        for (index, frag) in wrapper.fragments.iter().enumerate() {
            let packet = Packet::new_fragment(
                SourceRoutingHeader::empty_route(),
                wrapper.session_id,
                frag.clone(),
            );

            if !self.network.send_packet(&packet, sid) {
                unsent = wrapper.fragments[index..]
                    .iter()
                    .map(|f| {
                        Packet::new_fragment(
                            SourceRoutingHeader::empty_route(),
                            wrapper.session_id,
                            f.clone(),
                        )
                    })
                    .collect();
                break;
            }
        }

        if !unsent.is_empty() {
            self.message.add_packets_to_buffer(sid, unsent);
        }
    }

    fn _send_buffer(&mut self, reachable: &[NodeId]) {
        for server in reachable {
            let packets = self.message.get_server_buffer(server);

            if packets.is_empty() {
                continue;
            }

            let mut sent_count = 0;

            for packet in &packets {
                if self.network.send_packet(packet, server) {
                    sent_count += 1;
                } else {
                    break;
                }
            }

            if sent_count == packets.len() {
                self.message.clear_server_buffer(server);
            } else {
                let remaining = packets[sent_count..].to_vec();
                self.message.set_server_buffer(server, remaining);
            }
        }
    }

    fn _registry_and_client_list(&mut self, reachable: &[NodeId]) {
        for server in reachable {
            self._send_message(server, ChatRequest::Register(self.my_id));
            self._send_message(server, ChatRequest::ClientList);
        }
    }
}
