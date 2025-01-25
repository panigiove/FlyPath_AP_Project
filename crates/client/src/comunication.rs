use std::collections::HashMap;

use message::MessageError;
use wg_2024::network::NodeId;

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum ServerType {
    ChatServer,
    MediaServer,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum FromUiCommunication {
    AskServerType(u64, NodeId),

    AskMedialist(u64, NodeId),
    AskMedia(u64, NodeId, u64),

    AskClientList(u64, NodeId),
    AskRegister(u64, NodeId),
    SendMessage {
        session_id: u64,
        server_id: NodeId,
        to: NodeId,
        message: String,
    },

    GetServerList,
    ReloadNetwork,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum ToUIComunication {
    ResponseServerType(u64, ServerType),

    ResponseMediaList(u64, Vec<u64>),
    ResponseMedia(u64, Vec<u8>),

    ResponseClientList(u64, Vec<NodeId>),
    MessageFrom {
        session_id: u64,
        from: NodeId,
        message: Vec<u8>,
    },

    ConfirmMessageSent(u64), // server  confirm delivery of message

    ServerList(Option<Vec<NodeId>>, Option<HashMap<NodeId, ServerType>>),
    NewFloodRequest(),

    Err(MessageError),
    ServerReachable(NodeId),

    ServerReceivedAllSegment(u64),
}
