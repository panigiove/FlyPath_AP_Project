use std::sync::mpsc::{Receiver, Sender};
use wg_2024::network::NodeId;

struct ButtonWindow{
    pub reciver_node_clicked: Receiver<NodeId>, //riceve dal grafo il nodo che è stato premuto
    pub sender_button_event: Sender<>
}