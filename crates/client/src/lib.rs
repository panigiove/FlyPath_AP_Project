// TODO: network discovery protocol, da mandare per inizializzare poi ogni tot ms e poi per ogni nack
mod comunication;
mod test;
mod worker;

use log::{debug, error, info, warn};
use std::collections::{HashMap, HashSet};
use std::process;

use crossbeam_channel::{select_biased, unbounded, Receiver, Sender};
use message::{ChatRequest, ChatResponse, DroneSend, MediaRequest, MediaResponse};
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType,
};

use comunication::{FromUiCommunication, ServerType, ToUIComunication};

// Send N fragment at the time
// every ack send another frag
// if dropped send again
// if other nack make again the flood_request and send from the ack not acked.
// TODO: introdurre un aging
#[derive(Debug)]
pub struct Client {}

// #[allow(unused)]
impl Client {}

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

    // #[test]
    // fn test_update_network() {
    //     let (_, recv) = unbounded();
    //     let mut client = Client::new(10, NodeType::Client, recv, HashMap::new());

    //     let flood_request = FloodRequest::initialize(0, 10, NodeType::Client)
    //         .get_incremented(0, NodeType::Drone)
    //         .get_incremented(2, NodeType::Drone)
    //         .get_incremented(3, NodeType::Drone);

    //     let flood_request2 = FloodRequest::initialize(0, 10, NodeType::Client)
    //         .get_incremented(0, NodeType::Drone)
    //         .get_incremented(1, NodeType::Drone)
    //         .get_incremented(3, NodeType::Drone)
    //         .get_incremented(11, NodeType::Server);
    // }

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
