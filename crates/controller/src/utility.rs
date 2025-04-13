use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;
use std::ptr::NonNull;
use wg_2024::config::{Client, Drone, Server};
use wg_2024::network::NodeId;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;

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

pub enum NodeType{
    Drone,
    Server,
    Client
}

pub enum GraphAction {
    AddNode(NodeId, NodeType),
    RemoveNode(NodeId),
    AddEdge(NodeId, NodeId),
    RemoveEdge(NodeId,NodeId)
}

pub enum ButtonEvent{
    NewNode(),
    NewConnection(NodeId, NodeId),
    Crash(NodeId),
    RemoveConection(),
    ChangePdr(NodeId, f32),
}

pub enum ButtonsMessages{
    DeselectNode(NodeId), //dopo che abbiamo fatto un'operazione deselezioniamo il nodo
    MultipleSelectionAllowed //per quando diciamo di voler aggiungere un edge
}
