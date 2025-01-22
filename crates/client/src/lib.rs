// TODO: network discovery protocol, da mandare per inizializzare poi ogni tot ms e poi per ogni nack
// TODO: fragmentation of high level messages
// TODO: handle ACK, NACK

use std::collections::{HashMap, HashSet};
use log::{debug, info, warn, error};

use crossbeam_channel::{Receiver, Sender, SendError};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{FloodRequest, NodeType, Packet, PacketType};

mod test;

#[derive(Debug)]
pub struct Client {
    id: NodeId,
    node_type: NodeType,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,

    adj_list: HashMap<NodeId, HashSet<NodeId>>,
    nodes_type: HashMap<NodeId, NodeType>,
    last_flood_id: u64,
}

impl Client {
    pub fn new(
        id: NodeId,
        node_type: NodeType,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<u8, Sender<Packet>>,
    ) -> Self {
        debug!("Creating a new Client with id: {:?}", id);
        Self {
            id: id,
            node_type: node_type,
            packet_recv: packet_recv,
            packet_send: packet_send,
            adj_list: HashMap::new(),
            nodes_type: HashMap::new(),
            last_flood_id: 0,
        }
    }

    pub fn get_servers (&self) -> Option<Vec<NodeId>>{
        let mut servers = Vec::new();
        for (node_id, node_type) in self.nodes_type.iter() {
            if *node_type == NodeType::Server {
                servers.push(node_id.clone());
            }
        }

        if servers.is_empty(){
            None
        }else{
            Some(servers)
        }
    }

    // --------------------------- message with server -------------------------------------
    // pub fn ask_server_type(&self, server_id: NodeId)

    // --------------------------- flooding algorithm --------------------------------------
    fn send_flood_request(&mut self) {
        // reset the adj list and node type
        self.reset_network_state();
        self.last_flood_id += 1;

        let flood_request = Packet::new_flood_request(
            SourceRoutingHeader::empty_route(),
            0,
            FloodRequest::initialize(self.last_flood_id, self.id, self.node_type),
        );

            // Rimuovi i canali disconnessi
        self.packet_send.retain(|node_id, send| {
            match send.send(flood_request.clone()) {
                Ok(_) => true, // Keep the channel if the send was successful
                Err(_) => {
                    // Log or handle the disconnected channel
                    warn!("Channel to node {:?} is disconnected and will be removed.", node_id);
                    false // Remove the channel
                }
            }
        });

        info!("Flood request sent. Active channels: {}", self.packet_send.len());
    }

    fn reset_network_state(&mut self) {
        self.nodes_type.clear();
        self.adj_list.clear();
        debug!("Reset network state: cleared nodes_type and adj_list");

        self.nodes_type.insert(self.id, self.node_type);

        for (node_id, _) in self.packet_send.iter(){
            self.adj_list
                .entry(node_id.clone())
                .or_insert_with(HashSet::new)
                .insert(self.id.clone());

            self.adj_list
                .entry(self.id.clone())
                .or_insert_with(HashSet::new)
                .insert(node_id.clone());

            debug!("Added connection between {:?} and {:?}", self.id, node_id);
        }
    }

    fn update_network(&mut self, flood_response: FloodRequest) -> Result<(), String> {
        if flood_response.initiator_id == self.id && flood_response.flood_id == self.last_flood_id {
            let mut prev: NodeId = self.id;

            for (node_id, node_type) in flood_response.path_trace {
                self.nodes_type.insert(node_id, node_type); // insert the type

                if node_id != self.id {
                    self.adj_list
                        .entry(node_id)
                        .or_insert_with(HashSet::new)
                        .insert(prev);

                    self.adj_list
                        .entry(prev)
                        .or_insert_with(HashSet::new)
                        .insert(node_id);
                }
                prev = node_id;
            }
            Ok(())
        } else {
            Err("Old or not my response".to_string())
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use wg_2024::controller::{DroneCommand, DroneEvent};
    use wg_2024::drone::Drone;
    use wg_2024::network::{NodeId, SourceRoutingHeader};
    use wg_2024::packet::{FloodRequest, NodeType, Packet};

    use crossbeam_channel::unbounded;
    use crossbeam_channel::{Receiver, Sender};
    use std::collections::HashMap;
    use std::thread::{self, sleep};
    use std::time::Duration;

    // Importing all drone implementations
    use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
    use bagel_bomber::BagelBomber;
    use lockheedrustin_drone::LockheedRustin;
    use rolling_drone::RollingDrone;
    use rust_do_it::RustDoIt;
    use rust_roveri::RustRoveri;
    use rustastic_drone::RustasticDrone;
    use rustbusters_drone::RustBustersDrone;
    use LeDron_James::Drone as LeDronJames_drone;

    use flyPath::FlyPath;

    #[test]
    fn test_update_network() {
        let (_, recv) = unbounded();
        let mut client = Client::new(10, NodeType::Client, recv, HashMap::new());

        let flood_request = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(0, NodeType::Drone)
            .get_incremented(2, NodeType::Drone)
            .get_incremented(3, NodeType::Drone);

        let flood_request2 = FloodRequest::initialize(0, 10, NodeType::Client)
            .get_incremented(0, NodeType::Drone)
            .get_incremented(1, NodeType::Drone)
            .get_incremented(3, NodeType::Drone)
            .get_incremented(11, NodeType::Server);

        client.update_network(flood_request).unwrap();
        client.update_network(flood_request2).unwrap();
        
        println!("{:?}", client);

        client.reset_network_state();

        println!("{:?}", client);
    }

    // A-B-C-D-E-F-G-H-I-L
    fn build_complete_linear_network() -> (
        Vec<Sender<DroneCommand>>,
        Receiver<DroneEvent>,
        Receiver<Packet>,
        Receiver<Packet>,
        Vec<Sender<Packet>>,
        Vec<Box<dyn Drone>>,
    ) {
        let mut send_drones_command: Vec<Sender<DroneCommand>> = Vec::new();
        let mut recv_drones_command: Vec<Receiver<DroneCommand>> = Vec::new();

        let (send_drone_event, recv_drone_event) = unbounded();

        let mut sends_packet: Vec<Sender<Packet>> = Vec::new();
        let mut recvs_packet: Vec<Receiver<Packet>> = Vec::new();

        for _ in 0..10 {
            let (send_drone_command, recv_drone_command) = unbounded();
            send_drones_command.push(send_drone_command);
            recv_drones_command.push(recv_drone_command);

            let (send_packet, recv_packet) = unbounded();
            sends_packet.push(send_packet);
            recvs_packet.push(recv_packet);
        }

        let (client_send, client_recv) = unbounded();
        let (server_send, server_recv) = unbounded();

        let mut drones: Vec<Box<dyn Drone>> = Vec::new();

        let mut packet_send_map: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        packet_send_map.insert(10, client_send.clone());
        packet_send_map.insert(1, sends_packet[1].clone());
        drones.push(Box::new(LockheedRustin::new(
            0,
            send_drone_event.clone(),
            recv_drones_command[0].clone(),
            recvs_packet[0].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(0, send_drone_event.clone(), recv_drones_command[0].clone(), recvs_packet[0].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(0, sends_packet[0].clone());
        packet_send_map.insert(2, sends_packet[2].clone());
        drones.push(Box::new(RustBustersDrone::new(
            1,
            send_drone_event.clone(),
            recv_drones_command[1].clone(),
            recvs_packet[1].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(1, send_drone_event.clone(), recv_drones_command[1].clone(), recvs_packet[1].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(1, sends_packet[1].clone());
        packet_send_map.insert(3, sends_packet[3].clone());
        drones.push(Box::new(RustasticDrone::new(
            2,
            send_drone_event.clone(),
            recv_drones_command[2].clone(),
            recvs_packet[2].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(2, send_drone_event.clone(), recv_drones_command[2].clone(), recvs_packet[2].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(2, sends_packet[2].clone());
        packet_send_map.insert(4, sends_packet[4].clone());
        drones.push(Box::new(NoSoundDroneRIP::new(
            3,
            send_drone_event.clone(),
            recv_drones_command[3].clone(),
            recvs_packet[3].clone(),
            packet_send_map.clone(),
            0.0,
        ))); // not good enough
             // drones.push(Box::new(FlyPath::new(3, send_drone_event.clone(), recv_drones_command[3].clone(), recvs_packet[3].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(3, sends_packet[3].clone());
        packet_send_map.insert(5, sends_packet[5].clone());
        drones.push(Box::new(BagelBomber::new(
            4,
            send_drone_event.clone(),
            recv_drones_command[4].clone(),
            recvs_packet[4].clone(),
            packet_send_map.clone(),
            0.0,
        ))); // strange behavior
             // drones.push(Box::new(FlyPath::new(4, send_drone_event.clone(), recv_drones_command[4].clone(), recvs_packet[4].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(4, sends_packet[4].clone());
        packet_send_map.insert(6, sends_packet[6].clone());
        drones.push(Box::new(LeDronJames_drone::new(
            5,
            send_drone_event.clone(),
            recv_drones_command[5].clone(),
            recvs_packet[5].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(5, send_drone_event.clone(), recv_drones_command[5].clone(), recvs_packet[5].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(5, sends_packet[5].clone());
        packet_send_map.insert(7, sends_packet[7].clone());
        drones.push(Box::new(RollingDrone::new(
            6,
            send_drone_event.clone(),
            recv_drones_command[6].clone(),
            recvs_packet[6].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(6, send_drone_event.clone(), recv_drones_command[6].clone(), recvs_packet[6].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(6, sends_packet[6].clone());
        packet_send_map.insert(8, sends_packet[8].clone());
        drones.push(Box::new(RustBustersDrone::new(
            7,
            send_drone_event.clone(),
            recv_drones_command[7].clone(),
            recvs_packet[7].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(7, send_drone_event.clone(), recv_drones_command[7].clone(), recvs_packet[7].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(7, sends_packet[7].clone());
        packet_send_map.insert(9, sends_packet[9].clone());
        drones.push(Box::new(RustRoveri::new(
            8,
            send_drone_event.clone(),
            recv_drones_command[8].clone(),
            recvs_packet[8].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(8, send_drone_event.clone(), recv_drones_command[8].clone(), recvs_packet[8].clone(), packet_send_map.clone(), 0.0)));

        packet_send_map.clear();
        packet_send_map.insert(8, sends_packet[8].clone());
        packet_send_map.insert(11, server_send.clone());
        drones.push(Box::new(RustDoIt::new(
            9,
            send_drone_event.clone(),
            recv_drones_command[9].clone(),
            recvs_packet[9].clone(),
            packet_send_map.clone(),
            0.0,
        )));
        // drones.push(Box::new(FlyPath::new(9, send_drone_event.clone(), recv_drones_command[9].clone(), recvs_packet[9].clone(), packet_send_map.clone(), 0.0)));

        (
            send_drones_command,
            recv_drone_event,
            client_recv,
            server_recv,
            sends_packet,
            drones,
        )
    }
}
