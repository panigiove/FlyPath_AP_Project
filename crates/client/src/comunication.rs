use wg_2024::network::NodeId;

#[allow(unused)]
pub enum ServerType {
    ChatServer,
    MediaServer,
}

#[allow(unused)]
pub enum FromUiCommunication {
    AskServerType(NodeId),

    AskMedialist(NodeId),
    AskMedia(NodeId, u64),

    AskClientList(NodeId),
    AskRegister(NodeId),
    SendMessage {
        server_id: NodeId,
        to: NodeId,
        message: String,
    },

    GetServerList,
    ReloadNetwork,
}

#[allow(unused)]
pub enum ToUIComunication {
    ResponseServerType(ServerType),

    ResponseMediaList(NodeId, Vec<u64>),
    ResponseMedia(Vec<u8>),

    ResponseClientList(Vec<NodeId>),
    MessageFrom { from: NodeId, message: Vec<u8> },

    ConfirmMessageSent, // server  confirm delivery of message

    ServerList(Option<Vec<NodeId>>),

    Error(String), // generic error
}
