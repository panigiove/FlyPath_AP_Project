use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use wg_2024::network::NodeId;
use wg_2024::packet::{Fragment, Packet};

use crate::ChatResponse::MessageFrom;
use crossbeam_channel::Sender;

pub const FRAGMENT_DSIZE: usize = 128;

// ------------------------------ CONTROLLER EVENTS
pub enum NodeCommand {
    RemoveSender(NodeId),
    AddSender(NodeId, Sender<Packet>),
    FromShortcut(Packet),
}
pub enum NodeEvent {
    PacketSent(Packet),
    CreateMessage(SentMessageWrapper), // try send message (every times is sended a stream of fragment)
    MessageRecv(RecvMessageWrapper),   // received full message
    ControllerShortcut(Packet),
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
        let (fragments, total_n_fragments) = SentMessageWrapper::fragmentation(raw_data.clone());
        Self {
            session_id,
            destination,
            total_n_fragments,
            acked: HashSet::new(),
            fragments,
            raw_data,
        }
    }

    /// Create `Wrapper` from a serializable message
    ///
    /// # Example
    ///
    /// ```
    /// use message::{ChatRequest, SentMessageWrapper};
    /// let nid = 1;
    /// let msg = ChatRequest::ClientList;
    /// let wrapper = SentMessageWrapper::from_message(1, nid, &msg);
    /// ```
    pub fn from_message<T: DroneSend>(session_id: u64, destination: NodeId, message: &T) -> Self {
        let raw_data = message.stringify();
        Self::new_from_raw_data(session_id, destination, raw_data)
    }

    pub fn is_all_fragment_acked(&self) -> bool {
        self.total_n_fragments == self.acked.len() as u64
    }

    pub fn get_fragment(&self, i: usize) -> Option<Fragment> {
        self.fragments.get(i).cloned()
    }

    pub fn fragment_acked(&self, index: u64) -> bool {
        self.acked.contains(&index)
    }

    pub fn add_acked(&mut self, index: u64) {
        if index < self.total_n_fragments {
            self.acked.insert(index);
        }
    }

    pub fn fragmentation(raw_data: String) -> (Vec<Fragment>, u64) {
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
            })
            .collect();
        (fragments, total_n_fragments)
    }
}

// use this to save the arriving fragments
#[derive(Debug, Clone)]
pub struct RecvMessageWrapper {
    pub session_id: u64,
    pub source: NodeId,
    pub total_n_fragments: u64,
    pub arrived: HashSet<u64>,
    pub fragments: Vec<Option<Fragment>>,

    pub raw_data: String,
}

impl RecvMessageWrapper {
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

    pub fn new_from_fragment(session_id: u64, source: NodeId, fragment: Fragment) -> Self {
        let mut wrapper = Self::new(session_id, source, fragment.total_n_fragments);
        wrapper.add_fragment(fragment);
        wrapper
    }

    pub fn is_all_fragments_arrived(&self) -> bool {
        self.arrived.len() as u64 == self.total_n_fragments
    }

    pub fn add_fragment(&mut self, fragment: Fragment) -> bool {
        if fragment.fragment_index < self.total_n_fragments {
            let index = fragment.fragment_index;
            if self.arrived.contains(&index) {
                return false;
            }

            self.arrived.insert(index);
            self.fragments[index as usize] = Some(fragment);
            true
        } else {
            false
        }
    }

    /// Try to deserialize received fragments in specified message type
    ///
    /// # Example
    ///
    /// ```
    /// let mut recv = RecvMessageWrapper::new(session_id, source, total_fragments);
    /// recv.add_fragment(fragment1);
    /// recv.add_fragment(fragment2);
    /// // ...add all fragments
    ///
    /// if let Some(msg) = recv.try_deserialize::<ChatRequest>() {
    ///     // use msg
    /// }
    /// ```
    pub fn try_deserialize<T: DroneSend>(&mut self) -> Option<T> {
        if !self.try_generate_raw_data() {
            return None;
        }
        DroneSend::from_string(self.raw_data.clone()).ok()
    }

    /// Generate self.raw_data if is possible
    pub fn try_generate_raw_data(&mut self) -> bool {
        if !self.is_all_fragments_arrived() {
            return false;
        }

        let full_message: Vec<u8> = self
            .fragments
            .iter()
            .flat_map(|frag| {
                frag.as_ref()
                    .map(|f| f.data[..f.length as usize].to_vec())
                    .unwrap_or_default()
            })
            .collect();

        match String::from_utf8(full_message) {
            Ok(raw_data) => {
                self.raw_data = raw_data;
                true
            }
            Err(_) => false,
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
pub enum ChatResponse {
    ClientList(Vec<NodeId>),
    MessageFrom { from: NodeId, message: Vec<u8> },
    ErrorWrongClientId(NodeId),
}

impl DroneSend for ChatResponse {}
impl Response for ChatResponse {}
