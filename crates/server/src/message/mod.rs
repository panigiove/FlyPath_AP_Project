use log::{info, warn};
use message::{ChatRequest, ChatResponse, RecvMessageWrapper, SentMessageWrapper};
use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{Ack, Fragment, Packet};

#[derive(Clone, Debug)]
pub struct ServerMessageManager {
    incoming_fragments: HashMap<(u64, NodeId), RecvMessageWrapper>,
    outgoing_packets: HashMap<u64, SentMessageWrapper>,
    registered_clients: HashSet<NodeId>,
}

impl ServerMessageManager {
    pub fn new() -> Self {
        Self {
            incoming_fragments: HashMap::new(),
            outgoing_packets: HashMap::new(),
            registered_clients: HashSet::new(),
        }
    }
    pub fn store_fragment(&mut self, key: &(u64, NodeId), fragment: Fragment) {
        if !self.incoming_fragments.contains_key(key) {
            self.incoming_fragments.insert(
                key.clone(),
                RecvMessageWrapper::new_from_fragment(key.0, key.1, fragment),
            );
        } else {
            self.incoming_fragments
                .get_mut(key)
                .unwrap()
                .add_fragment(fragment);
        }
    }
    pub fn is_registered(&self, client: &NodeId) -> bool {
        self.registered_clients.contains(client)
    }
    pub fn are_all_fragment_arrived(&self, key: &(u64, NodeId)) -> bool {
        self.incoming_fragments
            .get(key)
            .unwrap()
            .is_all_fragments_arrived()
    }
    pub fn add_to_registered_client(&mut self, client: NodeId) {
        self.registered_clients.insert(client);
    }
    pub fn get_from_registered_client(&self, client: &NodeId) -> Option<&NodeId> {
        self.registered_clients.get(client)
    }
    pub fn get_all_registered_clients(&self) -> Vec<NodeId> {
        self.registered_clients.iter().cloned().collect()
    }
    pub fn insert_ack(&mut self, ack: Ack, session_id: &u64) {
        self.outgoing_packets
            .get_mut(session_id)
            .unwrap()
            .add_acked(ack.fragment_index);
        
        if self.outgoing_packets.get(session_id).unwrap().is_all_fragment_acked() {
            self.outgoing_packets.remove(session_id);
        }
    }
    pub fn get_outgoing_packet(&self, session_id: &u64) -> Option<&SentMessageWrapper> {
        self.outgoing_packets.get(session_id)
    }
    pub fn get_incoming_fragments(&self, key: &(u64, NodeId)) -> Option<RecvMessageWrapper> {
        self.incoming_fragments.get(key).cloned()
    }
    //todo: MessageFrom e un altro di cui non ricordo il nome
    pub fn message_handling(
        &mut self,
        key: &(u64, NodeId),
        fragment: Fragment,
    ) -> Option<SentMessageWrapper> {
        self.store_fragment(key, fragment);

        let sent_msg_wrapper;
        
        if let Some(message) = self
            .incoming_fragments
            .get_mut(key)
            .unwrap()
            .try_deserialize::<ChatRequest>()
        {
            self.incoming_fragments.remove(key);
            
            match message {
                ChatRequest::ClientList => {
                    let client_list = self.get_all_registered_clients();
                    let msg = ChatResponse::ClientList(client_list);
                    sent_msg_wrapper = SentMessageWrapper::from_message(key.0, key.1, &msg);
                    Some(sent_msg_wrapper)
                }
                ChatRequest::Register(node_id) => {
                    self.add_to_registered_client(node_id);
                    info!("Client with {:?} id added to client list", node_id,);
                    None
                }
                ChatRequest::SendMessage { from, to, message } => {
                    sent_msg_wrapper =
                        SentMessageWrapper::new_from_raw_data(key.0, to, message);
                    self.outgoing_packets
                        .insert(key.0, sent_msg_wrapper.clone());
                    Some(sent_msg_wrapper)
                }
            }
        } else {
            warn!(
                "Error during deserialization of message from {:?} with session id: {:?}",
                key.1, key.0
            );
            None
        }
    }
}
