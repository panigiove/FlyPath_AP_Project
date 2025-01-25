use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, Packet};
use std::collections::{HashMap, HashSet};

use crossbeam_channel::Sender;

pub const FRAGMENT_DSIZE: usize = 128;

// ------------------------------ CONTROLLER EVENTS
pub enum NodeCommand {
    RemoveSender(NodeId),
    AddSender(NodeId, Sender<Packet>),
    FromShortcut(Packet),
}
pub enum NodeEvent{
    PacketSent(Packet),
    CreateMessage(SentMessageWrapper), // try send message (every times is sended a stream of fragment)
    MessageRecv(RecvMessageWrapper), // received full message 
}


// ------------------------------ HIGH MESSAGE ERRORS
#[derive(Debug, Clone, PartialEq)]
pub enum MessageError {
    DirectConnectionDoNotWork(u64, NodeId),
    ServerUnreachable(u64, NodeId),
    NoFragmentStatus(u64),
    InvalidMessageReceived(u64),

    MessageNotComplete,
}

// ------------------------------ HIGH MESSAGE
// use this to store message and message State 
#[derive(Debug, Clone)]
pub struct SentMessageWrapper {
    pub session_id: u64,
    pub destination: NodeId,
    pub total_n_fragments: u64, 
    pub acked: HashSet<u64>,
    pub fragments: Vec<Fragment>,

    pub raw_data: String,
}

impl SentMessageWrapper {
    pub fn new_from_raw_data(session_id: u64, destination: NodeId, raw_data: String) -> Self {
        let (fragments, total_n_fragments) =SentMessageWrapper::fragmentation(&raw_data);
        Self{
            session_id,
            destination,
            total_n_fragments,
            acked: HashSet::new(),
            fragments,
            raw_data,
        }
    }


    pub fn is_all_fragment_acked(&self) -> bool{
        self.total_n_fragments == self.acked.len() as u64
    }

    pub fn get_fragment(&self, i: usize) -> Option<Fragment>{
        self.fragments.get(i).cloned()
    }

    pub fn fragment_acked(&self, index: u64) -> bool{
        self.acked.contains(&index)
    }

    pub fn add_acked(&mut self, index: u64){
        if index < self.total_n_fragments {
            self.acked.insert(index);
        }
    }

    pub fn fragmentation (raw_data: &String) -> (Vec<Fragment>, u64){
        let raw_bytes = raw_data.clone().into_bytes();
        let total_n_fragments = raw_bytes.len().div_ceil(FRAGMENT_DSIZE) as u64;
        let fragments = raw_bytes
            .chunks(FRAGMENT_DSIZE)
            .enumerate()
            .map(|(i, chunk)| {
                let mut data = [0; FRAGMENT_DSIZE];
                data[..chunk.len()].copy_from_slice(chunk);
                Fragment {
                    fragment_index: i as u64,
                    total_n_fragments,
                    length: chunk.len() as u8,
                    data,
                }
            }).collect();
        (fragments, total_n_fragments)
    }
}

// use this to save the arriving fragments
#[derive(Debug, Clone)]
pub struct RecvMessageWrapper {
    pub session_id: u64,
    pub source: NodeId,
    pub total_n_fragments:u64,
    pub arrived: HashSet<u64>,
    pub fragments: Vec<Option<Fragment>>,

    pub raw_data: String,
}

impl RecvMessageWrapper{
    pub fn new(session_id: u64, source: NodeId, total_n_fragments: u64) -> Self {
        Self {
            session_id,
            source,
            total_n_fragments,
            arrived: HashSet::new(),
            fragments: vec![None; total_n_fragments as usize], 
            raw_data: "".to_string(),
        }
    }

    pub fn is_all_fragments_arrived(&self) -> bool {
        self.arrived.len() as u64 == self.total_n_fragments
    }

    pub fn add_fragment(&mut self, fragment: Fragment) {
        if fragment.fragment_index < self.total_n_fragments {
            let index = fragment.fragment_index;
            self.arrived.insert(index); 
            self.fragments[index as usize] = Some(fragment); 
        }
    }

    pub fn try_generate_raw_data(&mut self) -> Result<(), MessageError> {
        if !self.is_all_fragments_arrived() {
            return Err(MessageError::MessageNotComplete);
        }
    
        let full_message: Vec<u8> = self
            .fragments
            .iter()
            .flat_map(|frag| {
                frag.as_ref()
                    .map(|f| f.data[..f.length as usize].to_vec())
                    .unwrap_or_else(|| {
                        Vec::new()
                    })
            })
            .collect();
    
        // funziona anche con i ?
        match String::from_utf8(full_message) {
            Ok(raw_data) => {
                self.raw_data = raw_data;
                Ok(())
            }
            Err(_) => Err(MessageError::InvalidMessageReceived(self.session_id)),
        }
    }
}



// ------------------------------ HIGH MESSAGE TYPE
pub trait DroneSend: Serialize + DeserializeOwned {
    fn stringify(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    fn from_string(raw: String) -> Result<Self, String> {
        serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())
    }
}

pub trait Request: DroneSend {}
pub trait Response: DroneSend {}

// -------------------- Messages --------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaRequest {
    MediaList,
    Media(u64),
}
impl DroneSend for MediaRequest {}
impl Request for MediaRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRequest {
    ClientList,
    Register(NodeId),
    SendMessage {
        from: NodeId,
        to: NodeId,
        message: String,
    },
}
impl DroneSend for ChatRequest {}
impl Request for ChatRequest {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaResponse {
    MediaList(Vec<u64>),
    Media(Vec<u8>), // should we use some other type?
}

impl DroneSend for MediaResponse {}
impl Response for MediaResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatResponse {
    ClientList(Vec<NodeId>),
    MessageFrom { from: NodeId, message: Vec<u8> },
    MessageSent,
}

impl DroneSend for ChatResponse {}
impl Response for ChatResponse {}
