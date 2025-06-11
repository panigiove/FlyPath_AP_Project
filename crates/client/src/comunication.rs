use message::ChatResponse;
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
    SendChatMessage { to_client: NodeId, message: String },
    RefreshTopology,
    AskClientList,
}

#[allow(unused)]
#[derive(Debug, Clone)]
pub enum ToUICommunication {
    ChatResponse { response: ChatResponse },
    MessageDeliveredToServer(u64),
}
