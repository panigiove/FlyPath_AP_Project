use wg_2024::network::NodeId;
use egui::{Color32};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Copy)]
pub enum NodeType{
    Drone,
    Server,
    Client
}

#[derive(Debug, Clone)]
pub enum GraphAction {
    AddNode(NodeId, NodeType),
    RemoveNode(NodeId),
    AddEdge(NodeId, NodeId),
    RemoveEdge(NodeId,NodeId)
}

#[derive(Debug, Clone)]
pub enum ButtonEvent {
    NewDrone(NodeId, f32),
    NewClient(NodeId),
    NewServerWithTwoConnections(NodeId, NodeId),
    NewConnection(NodeId, NodeId),
    Crash(NodeId),
    RemoveConection(NodeId, NodeId),
    ChangePdr(NodeId, f32),
}

#[derive(Debug, Clone)]
pub enum ButtonsMessages{
    UpdateSelection(Option<NodeId>, Option<NodeId>),
    ClearAllSelections,
}

pub enum MessageType{
    Error(String),
    Ok(String),
    Packet(String),
    Info(String),
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum DroneGroup{
    RustInPeace,
    BagelBomber,
    LockheedRustin,
    RollingDrone,
    RustDoIt,
    RustRoveri,
    Rustastic,
    RustBusters,
    LeDronJames,
    RustyDrones,
}

#[derive(Debug, Clone)]
pub enum Clicked{
    Node(NodeId),
    Edge(NodeId, NodeId)
}

pub const ORANGE: Color32 = Color32::from_rgb(200, 150, 100);
pub const LIGHT_BLUE: Color32 = Color32::from_rgb(140,182,188);
pub const DARK_BLUE: Color32 = Color32::from_rgb(14,137,146);
pub const LIGHT_ORANGE: Color32 = Color32::from_rgb(231,187,166);
