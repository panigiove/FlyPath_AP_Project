use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;
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