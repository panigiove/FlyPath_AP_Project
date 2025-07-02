use crate::comunication::{FromUiCommunication, ToUICommunication};
use crossbeam_channel::{Receiver, Sender};
use log::{info, warn};
use message::{NodeCommand, NodeEvent};
use std::collections::HashMap;
use std::thread;
use wg_2024::network::NodeId;
use wg_2024::packet::Packet;

pub struct ChannelManager {
    pub tx_drone: HashMap<NodeId, Sender<Packet>>,
    pub tx_ctrl: Sender<NodeEvent>,
    pub tx_ui: Sender<ToUICommunication>,
    pub rx_drone: Receiver<Packet>,
    pub rx_ctrl: Receiver<NodeCommand>,
    pub rx_ui: Receiver<FromUiCommunication>,
}

impl ChannelManager {
    pub fn new(
        tx_drone: HashMap<NodeId, Sender<Packet>>,
        tx_ctrl: Sender<NodeEvent>,
        tx_ui: Sender<ToUICommunication>,
        rx_drone: Receiver<Packet>,
        rx_ctrl: Receiver<NodeCommand>,
        rx_ui: Receiver<FromUiCommunication>,
    ) -> Self {
        ChannelManager {
            tx_drone,
            tx_ctrl,
            tx_ui,
            rx_drone,
            rx_ctrl,
            rx_ui,
        }
    }

    pub fn broadcast_packet(&mut self, packet: Packet) {
        let mut failed_nodes = Vec::new();

        for (&nid, tx) in &self.tx_drone {
            match tx.send(packet.clone()) {
                Ok(_) => {
                    self.tx_ctrl
                        .send(NodeEvent::PacketSent(packet.clone()))
                        .expect("Failed to transmit to CONTROLLER");
                    info!("{}: Packet sent successfully to node {}", thread::current().name().unwrap_or("unnamed"),nid);
                }
                Err(_) => {
                    warn!("{}: Failed to send packet to node {}", thread::current().name().unwrap_or("unnamed"), nid);
                    failed_nodes.push(nid);
                }
            }
        }

        for nid in failed_nodes {
            self.tx_drone.remove(&nid);
            info!("{}: Removed node {} from tx_drone",thread::current().name().unwrap_or("unnamed"), nid);
        }
    }
}
