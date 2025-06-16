mod tests;

use crate::channel::ChannelManager;
use crate::comunication::ToUICommunication::ChatResponse;
use hashbrown::{HashMap, HashSet};
use log::{debug, error, info, warn};
use message::ChatResponse::{ClientList, ErrorWrongClientId};
use message::NodeEvent::{CreateMessage, MessageRecv};
use message::{ChatRequest, RecvMessageWrapper, SentMessageWrapper};
use std::cell::RefCell;
use std::rc::Rc;
use wg_2024::network::NodeId;
use wg_2024::packet::{Ack, Fragment, Nack, NackType, Packet};

type Session = u64;

pub struct MessagerManager {
    my_id: NodeId,

    channels: Rc<RefCell<ChannelManager>>,

    pub clients: HashMap<NodeId, HashSet<NodeId>>, // client -> server

    buffer: HashMap<NodeId, Vec<Packet>>, // server -> buffer
    msg_wrapper: HashMap<Session, SentMessageWrapper>,
    rcv_wrapper: HashMap<(Session, NodeId), RecvMessageWrapper>,
    last_session: Session,
}

impl MessagerManager {
    pub fn new(my_id: NodeId, channels: Rc<RefCell<ChannelManager>>) -> Self {
        Self {
            my_id,
            channels,
            clients: HashMap::new(),
            buffer: HashMap::new(),
            msg_wrapper: HashMap::new(),
            rcv_wrapper: HashMap::new(),
            last_session: 0,
        }
    }
    pub fn create_and_store_wrapper(
        &mut self,
        destination: &NodeId,
        msg: ChatRequest,
    ) -> &SentMessageWrapper {
        self.last_session += 1;
        let wrapper = SentMessageWrapper::from_message(self.last_session, *destination, &msg);
        self.channels
            .borrow()
            .tx_ctrl
            .send(CreateMessage(wrapper.clone()))
            .expect("Failed to transmit to CONTROLLER");
        self.msg_wrapper.insert(self.last_session, wrapper);
        self.msg_wrapper.get(&self.last_session).unwrap()
    }

    pub fn add_packets_to_buffer(&mut self, server: &NodeId, packets: Vec<Packet>) {
        self.buffer.entry(*server).or_default().extend(packets);
    }

    pub fn get_server_buffer(&self, server: &NodeId) -> Vec<Packet> {
        self.buffer.get(server).cloned().unwrap_or_default()
    }

    pub fn set_server_buffer(&mut self, server: &NodeId, new_buffer: Vec<Packet>) {
        self.buffer.insert(*server, new_buffer);
    }

    pub fn clear_server_buffer(&mut self, server: &NodeId) {
        if let Some(buffer) = self.buffer.get_mut(server) {
            buffer.clear();
        }
    }

    pub fn get_dropped_fragment(&mut self, nack: &Nack, session: Session) -> Option<&Fragment> {
        match nack.nack_type {
            NackType::UnexpectedRecipient(_) => {
                if let Some(wrapper) = self.msg_wrapper.get(&session) {
                    error!(
                        "UnexpectedRecipient, delete the following message {:?}",
                        wrapper
                    );
                }
                self.msg_wrapper.remove(&session);
                None
            }
            _ => self
                .msg_wrapper
                .get(&session)
                .and_then(|wrapper| wrapper.fragments.get(nack.fragment_index as usize)),
        }
    }

    pub fn get_destination(&self, session: &Session) -> Option<NodeId> {
        self.msg_wrapper
            .get(session)
            .map(|wrapper| wrapper.destination)
    }

    pub fn ack_and_build_message(&mut self, ack: &Ack, session: Session) {
        debug!(
            "ACK received: session={:?}, fragment_index={}",
            session, ack.fragment_index
        );
        if let Some(wrapper) = self.msg_wrapper.get_mut(&session) {
            wrapper.acked.insert(ack.fragment_index);
            if wrapper.is_all_fragment_acked() {
                info!(
                    "All fragments acknowledged for session {:?}, removing wrapper",
                    session
                );
                self.msg_wrapper.remove(&session);
            }
        } else {
            warn!(
                "Received ACK for unknown session {:?}, no sent by client {:?}",
                session, self.my_id
            );
        }
    }

    pub fn save_received_message(
        &mut self,
        fragment: Fragment,
        session: Session,
        source: NodeId,
    ) -> bool {
        let session_key = (session, source);
        let is_new_session = !self.rcv_wrapper.contains_key(&session_key);

        if is_new_session {
            self.rcv_wrapper.insert(
                session_key,
                RecvMessageWrapper::new_from_fragment(session, source, fragment),
            );
            true
        } else {
            let wrapper = self.rcv_wrapper.get_mut(&session_key).unwrap();
            let is_not_duplicate = wrapper.add_fragment(fragment);

            if wrapper.is_all_fragments_arrived() {
                if let Some(msg) = wrapper.try_deserialize() {
                    if let ClientList(list) = &msg {
                        for client in list {
                            self.clients
                                .entry(*client)
                                .or_insert_with(HashSet::new)
                                .insert(source);
                        }
                    } else if let ErrorWrongClientId(client) = &msg {
                        if let Some(servers) = self.clients.get_mut(client) {
                            servers.remove(&source);
                            if servers.is_empty() {
                                self.clients.remove(client);
                            }
                        }
                    }
                    self.channels
                        .borrow()
                        .tx_ctrl
                        .send(MessageRecv(wrapper.clone()))
                        .expect("Failed to transmit to CONTROLLER");
                    self.channels
                        .borrow()
                        .tx_ui
                        .send(ChatResponse { response: msg })
                        .expect("Failed to transmit to UI");
                    self.rcv_wrapper.remove(&session_key);
                }
            }

            is_not_duplicate
        }
    }
}
