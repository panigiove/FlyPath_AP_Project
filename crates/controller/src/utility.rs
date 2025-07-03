use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;
use std::ptr::NonNull;
use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
use wg_2024::config::{Client, Drone, Server};
use wg_2024::network::NodeId;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use egui::{Color32};

pub enum UIcommand{
    Spawn(Vec<NodeId>),
    Crash(NodeId),
    RemoveSender(NodeId, NodeId), //removing to the Drone with the first id the drone with the second id
    AddSender (NodeId, NodeId), //removing to the Drone with the first id the drone with the second id
    SetPacketDropRate(NodeId, f32),
    PacketSent(Packet),
    PacketDropped(Packet),
}

pub enum Operation{
    AddDrone,
    RemoveDrone,
    AddSender,
    RemoveSender
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Copy)]
pub enum NodeType{
    Drone,
    Server,
    Client
}

#[derive(Debug)]
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
    NewServer(NodeId),
    NewServerWithTwoConnections(NodeId, NodeId),
    NewConnection(NodeId, NodeId),
    Crash(NodeId),
    RemoveConection(NodeId, NodeId),
    ChangePdr(NodeId, f32),
}

#[derive(Debug, Clone)]
pub enum ButtonsMessages{
    DeselectNode(NodeId), //dopo che abbiamo fatto un'operazione deselezioniamo il nodo
    MultipleSelectionAllowed, //per quando diciamo di voler aggiungere un edge
    UpdateSelection(Option<NodeId>, Option<NodeId>), // NUOVO: per sincronizzare la selezione
    UpdateEdgeSelection(NodeId, NodeId),
    ClearAllSelections, // NUOVO: per pulire tutte le selezioni
}

pub enum MessageType{
    Error(String),
    Ok(String),
    PacketSent(String),
    PacketDropped(String),
    Info(String),
    //TODO vedere se aggiungere un tipo di messaggi per il drone
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
type NodePayload = (NodeId, NodeType);

pub const ORANGE: Color32 = Color32::from_rgb(200, 150, 100);
pub const LIGHT_BLUE: Color32 = Color32::from_rgb(140,182,188);
pub const DARK_BLUE: Color32 = Color32::from_rgb(14,137,146);
pub const LIGHT_ORANGE: Color32 = Color32::from_rgb(231,187,166);
pub const DARK_GREEN: Color32 = Color32::from_rgb(13,49,58);
