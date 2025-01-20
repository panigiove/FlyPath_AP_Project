use wg_2024::network::NodeId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Message<M: DroneSend> {
    pub source_id: NodeId,
    pub session_id: u64,
    pub content_type: ContentType,
    pub content: M,
}

pub trait DroneSend: Serialize + DeserializeOwned {
    fn stringify(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
    fn from_string(raw: String) -> Result<Self, String> {
        serde_json::from_str(raw.as_str()).map_err(|e| e.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ContentType {
    IdentificationRequest, // ask for server type, content can be empty
    IdentificationResponse, // server type response
    Request,
    Response,
    Status, // send to client errors or ok
} 

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerType{
    MediaServer,
    CommunicationServer,
}
impl DroneSend for ServerType {}

// ------------------ Media Messages ---------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaReq{
    List,
    Get(u8),
}
impl DroneSend for MediaReq{}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MediaRes{
    List(Vec<u8>),
    Wrapper(Vec<u8>), // media in bytes
}
impl DroneSend for MediaRes{}

// ------------------ Chat Messages ---------------------
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatReq{
    Register(NodeId),
    List,
    SendMessage{
        from:NodeId,
        to: NodeId,
        message: String,
    }
}
impl DroneSend for ChatReq{}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ChatRes{
    List(Vec<NodeId>),
    ReciveMessage{
        from:NodeId,
        message: Vec<u8>,
    },
}
impl DroneSend for ChatRes{}

// ------------------ Status Messages -------------------
pub trait Status {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Errors{
    WrongMessageType(ServerType),
}
impl DroneSend for Errors{}
impl Status for Errors{}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Ok{
    MessageSend(u64), // send the session
}
impl DroneSend for Ok{}
impl Status for Ok{}