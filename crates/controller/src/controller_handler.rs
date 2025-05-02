mod controller_test;
mod utility;
mod graph;
mod buttons;

use message::{NodeCommand};

use crossbeam_channel::{select, select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;

use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::drone::Drone;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;

use rand::Rng;
use std::cmp;
use std::ops::Deref;
use std::process::id;
use utility::UIcommand;
use utility::Operation;
use ap2024_rustinpeace_nosounddrone::NoSoundDroneRIP;
use bagel_bomber::BagelBomber;
use lockheedrustin_drone::LockheedRustin;
use rolling_drone::RollingDrone;
use rust_do_it::RustDoIt;
use rust_roveri::RustRoveri;
use rustastic_drone::RustasticDrone;
use rustbusters_drone::RustBustersDrone;
use LeDron_James::Drone as LeDronJames_drone;
use rusty_drones::RustyDrone;
use wg_2024::config::Server;
use crate::utility::{ButtonEvent, GraphAction, NodeType, MessageType};
use crate::utility::GraphAction::RemoveEdge;
use crate::utility::MessageType::Error;
use crate::utility::Operation::{AddSender, RemoveSender};

pub struct ControllerHandler {
    pub drones: HashMap<NodeId, Box<dyn Drone>>,
    pub drone_senders: HashMap<NodeId, Sender<Packet>>, //the set of all the senders
    //pub clients: HashMap<NodeId, Box<dyn Client>>, //devo importare robe di daniele
    //pub servers: HashMap<NodeId, Box<dyn Server>>,
    pub connections: HashMap<NodeId, Vec<NodeId>>, //viene passato dall'initializer
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>, // da controller a drone
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>, // da client a controller (anche server?) TODO io non avevo il reciver?
    pub receive_event: HashMap<NodeId, Receiver<DroneEvent>>, // da dorne a controller
    pub send_packet_server: HashMap<NodeId, Sender<Packet>>, //canali diversi per client e server vedi nodeCommand
    pub send_packet_client: HashMap<NodeId, Sender<Packet>>,
    //LE VARIE COMPONENTI DEL CONTROLLER
    pub button_receiver: Receiver<ButtonEvent>, // the buttons window says what the user wants to do
    pub graph_connections_sender: Sender<HashMap<NodeId, Vec<NodeId>>>, // sends the connections to the graph (only at the first stage)
    pub graph_node_type_sender: Sender<HashMap<NodeId, NodeType>>, //sends the type of nodes to the graph (only at the first stage),
    pub graph_action_sender: Sender<GraphAction>, //sendes connection's updates to the graph

    pub message_sender: Sender<MessageType>, //sends to the message window what it has to shoh



    //TODO! da eliminare?
    pub ui_receiver: Receiver<UIcommand>, //TODO cosa invia controller a sender
    pub ui_sender: Sender<UIcommand>,
    pub counter: i8,
}

//initializes the drones, distributing the implementations bought from the other groups(impl)
// as evenly as possible, having at most a difference of 1 between the group with the most drones
// running and the one with the least:
//TODO funzione per decidere che drone creare

pub fn chose_the_drone(){

}
// pub fn drone_random(id: NodeId,
//                     sender_event: Sender<DroneEvent>,
//                     receiver_command: Receiver<DroneCommand>,
//                     receiver_packet: Receiver<DroneCommand>,
//                     packet_sender: HashMap<NodeId, Sender<Packet>>) -> Option<Box<dyn Drone>> {
//     let mut rng = rand::thread_rng();
//     let rand = rng.gen_range(1..11);
//     let drop_rate = rng.gen_range(0.0 .. 1.1);
//     match rand{
//         1 => Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         2 => Box::new(BagelBomber::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         3 => Box::new(LockheedRustin::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         4 => Box::new(RollingDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         5 => Box::new(RustDoIt::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         6 => Box::new(RustRoveri::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         7 => Box::new(RustasticDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         8 => Box::new(RustBustersDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         9 => Box::new(LeDronJames_drone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         10 => Box::new(RustyDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
//         _ => None,
//     }
// }



impl ControllerHandler {
    pub fn new(
        drones: HashMap<NodeId, Box<dyn Drone>>,
        //clients: HashMap<NodeId, Box<dyn Client>>,
        //servers: HashMap<NodeId, Box<dyn Server>>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
        send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
        recive_event: HashMap<NodeId, Receiver<DroneEvent>>,
        send_packet_server: HashMap<NodeId, Sender<Packet>>,
        send_packet_client: HashMap<NodeId, Sender<Packet>>,

        ui_reciver: Receiver<UIcommand>, //TODO cosa invia controller a sender
        ui_sender: Sender<UIcommand>
    ) -> Self {
        Self {
            drones,
            //clients,
            //servers,
            connections,
            send_command_drone,
            send_command_node,
            receive_event: recive_event,
            send_packet_server,
            send_packet_client,
            ui_receiver: ui_reciver,
            ui_sender,
            counter: 1,
        }
    }

    //TODO complete run
    pub fn run(&mut self) {
        loop {
            select_biased! {
                recv(self.ui_receiver) -> command =>{
                    if let Ok(command) = command{
                        self.ui_command_handler(command); //TODO completare una volta fatta la UI
                    }
                }
                default => {
                    for (_, reciver) in self.receive_event.clone(){
                        if let Ok(event) = reciver.try_recv(){
                            self.event_handler(event);
                        }
                    }
                }
            }
            if let Ok(command) = self.ui_receiver.try_recv() {
                self.ui_command_handler(command);
                continue;
            }

            if let Ok(command) = self.button_receiver.try_recv() {
                self.button_event_handler(command);
                continue;
            }

            for (_, i) in self.receive_event.clone() {
                if let Ok(event) = i.try_recv() {
                    self.event_handler(event);
                }
            }

            if let Ok(command) = self.button_reciver.try_recv() {
                self.button_handler(command);
            }

            // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }

    //TODO fare in modo che ne restituisca l'id
    pub fn new_drone_balanced(&mut self, id: NodeId,
                              sender_event: Sender<DroneEvent>,
                              receiver_command: Receiver<DroneCommand>,
                              receiver_packet: Receiver<Packet>,
                              packet_sender: HashMap<NodeId, Sender<Packet>>, drop_rate: f32) -> Option<Box<dyn Drone>> {
        match self.counter {
            1 => {
                self.counter = 2;
                Some(Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            }
            2 => {
                self.counter = 3;
                Some(Box::new(BagelBomber::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            3 => {
                self.counter = 4;
                Some(Box::new(LockheedRustin::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            4 => {
                self.counter = 5;
                Some(Box::new(RollingDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            5 => {
                self.counter = 6;
                Some(Box::new(RustDoIt::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            6 => {
                self.counter = 7;
                Some(Box::new(RustRoveri::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            7 => {
                self.counter = 8;
                Some(Box::new(RustasticDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            8 => {
                self.counter = 9;
                Some(Box::new(RustBustersDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            9 => {
                self.counter = 10;
                Some(Box::new(LeDronJames_drone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            10 => {
                self.counter = 1;
                Some(Box::new(RustyDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate)))
            },
            _ => None,
        }
    }

    pub fn ui_command_handler(&mut self, ui_command: UIcommand) {
        match ui_command {
            UIcommand::Spawn(connections) => self.add_new_drone(&connections),
            UIcommand::Crash(id) => self.crash(&id),
            UIcommand::RemoveSender(id1, id2) => self.remove_sender(&id1, &id2),
            UIcommand::AddSender(id1, id2) => self.add_new_sender(&id1, &id2),
            UIcommand::SetPacketDropRate(id, pdr) => self.set_packet_drop_rate(&id, pdr),
            UIcommand::PacketSent(_) | UIcommand::PacketDropped(_) => todo!(),
            //_ => None,
        }
    }

    pub fn button_event_handler(&mut self, event: ButtonEvent) {
        match event {
            ButtonEvent::NewDrone(id) => {
                //TODO inviare aggiornamento a grafo
            }
            ButtonEvent::NewConnection(id1, id2) => {
                //TODO capire come sistemare i sender, conviene fare un hashmap con i node id e i sender così li copio da lì?
                self.add_connection(&id1, &id2);

                //TODO inviare aggiornamento a grafo

            }

            ButtonEvent::Crash(id) => {
                //TODO rivedere come funziona il meccanismo, per capire quando e dove mandare il messaggio al grafica del grafo
                //non credo che noi abbiamo modo di capire quando definitivamente crasha
                self.crash(&id);
                //TODO inviare aggiornamento a grafo

            }

            ButtonEvent::RemoveConection(id1, id2) => {
                //TODO verificare se è corretto interrompere la comunicazione da entrambi i lati
                self.remove_sender(&id1, &id2);
                //TODO inviare aggiornamento a grafo
                if let r = self.graph_action_sender.try_send(RemoveEdge(id1, id2)) {
                    //TODO facciamo qualcosa qui?
                };

                //TODO inviamo messaggio in message window per corretta rimozione
            }

            ButtonEvent::ChangePdr(id, pdr) => {
                let result = self.set_packet_drop_rate(&id, pdr);
                match result {
                    Ok(pdr) => {
                        let _ = self.message_sender.try_send(MessageType::Ok(format!("The packet drop rate of the drone with id = {} has been correctly changed to {}", id, pdr)));
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                        let _ = self.message_sender.try_send(Error(format!("{}", e)));
                    }
                }
            }

        }
    }


    pub fn event_handler(&mut self, event: DroneEvent) {
        match event {
            DroneEvent::PacketSent(packet) => self.notify_packet_sent(packet),
            DroneEvent::PacketDropped(packet) => self.notify_packet_dropped(packet),
            DroneEvent::ControllerShortcut(packet) => self.send_packet(packet).unwrap(), //TODO togliere unwrap
        }
    }


    // TODO sistemare questa funzione: il controller si occupa di mandare pacchetti che non possono essere persi a client/server
    pub fn send_packet(&self, mut packet: Packet) -> Result<(), ()> {
        if let Some(destination) = packet.routing_header.hops.pop() {
            if let Some(sender) = self.send_packet_client.get(&destination) {
                //sender.send(packet).map_err(|_| Err(()))?; //abbiamo modificato il tipo di errorezZ
                return Ok(());
            } else if let Some(sender) = self.send_packet_server.get(&destination) {
                //sender.send(packet).map_err(|_| Err(()))?;
                return Ok(());
            }
        }
        Err((()))
        // let destination = hops.pop();
        // match destination{
        //     Ok(id) => self.send_packet_client.get(id).unwrap().send(packet).unwrap(),
        //     _ => None
        // }
    }

    pub fn notify_packet_sent(&self, packet: Packet) {
        // self.ui_sender.send(PacketDropped(packet));
    }

    pub fn notify_packet_dropped(&self, packet: Packet) {
        self.ui_sender.send(UIcommand::PacketDropped(packet));
    }


pub fn add_new_drone(&mut self, first_connection: &NodeId) {
    let mut connections = vec![*first_connection];
    if let id = Some(get_random_key(&self.drones)){
        //TODO creare i campi insetiti dentro al new drone
    }
        if let drone = Some(self.new_drone_balanced(id, sender_event, reciver_drone_command, reciver_packet, sender_packet, pdr)){

        }

    //adds a new dorne to the network
    pub fn spawn(&mut self, id: &NodeId, connections: &Vec<NodeId>, drone: Box<dyn Drone>) {
        if !self.check_network_before_add_drone(id, connections) {
            eprintln!("Controller: Drone with id = {} can't be added to the network due to a violation of the network requirement", id);
            return;
        }
        self.add_drone(id, connections);
    }

    pub fn crash(&mut self, id: &NodeId) {
        if !self.is_drone(id) {
            eprintln!("Controller: Node with id = {} can't be removed to the network cause it isn't a drone", id);
            return;
        }
        if !self.check_network_before_removing_drone(id) {
            eprintln!("Controller: Node with id = {} can't be removed to the network due to a violation of the network requirement", id);
            return;
        }
        self.remove_drone(id);
    }

    pub fn add_new_sender(&mut self, id: &NodeId, dst_id: &NodeId) {}

    pub fn add_connection(&mut self, id: &NodeId, dst_id: &NodeId, sender1: Sender<Packet>, sender2: Sender<Packet>) {
        if !self.is_drone(id) {
            eprintln!("Controller: Can't be added a new sender to node with id = {} cause it isn't a drone", id);
            return;
        }
        if !self.is_drone(dst_id) {
            eprintln!("Controller: Node with id = {} can't be added as a new sender cause it isn't a drone", dst_id);
            return;
        }

        // if !self.check_network(AddSender, (id, dst_id)){
        //     eprintln!("Controller: Node with id = {} can't be removed to the network due to a violation of the network requirement", id);
        //     return;
        // }
        self.new_sender(id, dst_id, sender1);
        self.new_sender(dst_id, id, sender2);
    }

    pub fn remove_sender(&mut self, id: &NodeId, nghb_id: &NodeId) {
        // if !self.check_network(RemoveSender, (id, nghb_id)) || !self.is_drone(id) || !self.is_drone(nghb_id){
        //     return;
        // }
        self.close_sender(id, nghb_id);
    }

    //sends also a remove sender to its neighbours
    //TODO capire quando far rimuovere il drone dalle connection del controller
    pub fn remove_drone(&mut self, id: &NodeId) {
        if let Some(sender) = self.send_command_drone.get(id) {
            if let Err(e) = sender.send(DroneCommand::Crash) {
                eprintln!("Controller: Node with id = {} doesen't recive correctly the DroneCommand", id);
            }
        }
        self.remove_all_senders(id);
    }

    //This command adds dst_id to the drone neighbors, with dst_id crossbeam::Sender


    pub fn add_drone(&mut self, id: &NodeId, connections: &Vec<NodeId>) {
        let (drone, sender_command, receiver_event) = self.new_drone(&id, &connections);

        self.drones.insert(*id, drone); //insieriamo il drone nell'elenco

        self.send_command_drone.insert(*id, sender_command);
        self.receive_event.insert(*id, receiver_event);
    }


    //close the channel with all neighbours of a drone
    fn remove_all_senders(&mut self, id: &NodeId) {
        if let Some(drone_connections) = self.connections.get(id) {
            for i in drone_connections {
                self.close_sender(id, i);
            }
        }
        //TODO per client e server
        //ci pensa il drone ad aggiornare le connessioni?
    }

    //close the channel with a neighbour drone
    fn close_sender(&mut self, id: &NodeId, nghb_id: &NodeId) {
        if let Some(sender) = self.send_command_drone.get(id) {
            if let Err(e) = sender.send(DroneCommand::RemoveSender(*nghb_id)) {
                eprintln!("Controller: The DroneCommand RemoveSender to the drone with id = {} hasn't been sent correctly", nghb_id);
            }
        }
        if let Some(sender) = self.send_command_node.get(nghb_id) {
            if let Err(e) = sender.send(NodeCommand::RemoveSender(*id)) {
                eprintln!("Controller: The DroneCommand RemoveSender to the node with id = {} hasn't been sent correctly", id);
            }
        }
    }

    //adds dst_id to the drone neighbors (with dst_id crossbeam::Sender)
    fn new_sender(&self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>) {
        if let Some(s) = self.send_command_drone.get(id) {
            if let Err(e) = s.send(DroneCommand::AddSender(*dst_id, sender)) {
                eprintln!("Controller: The DroneCommand AddSender to the drone with id = {} hasn't been sent correctly", id);
            }
        }
    }

    //TODO sistemare la funzione is_drone in modo tale che sia lei a far stampare il messaggio

    //alter the pdr of a drone
    fn set_packet_drop_rate(&mut self, id: &NodeId, new_pdr: f32) -> Result<f32, Err> {
        if !self.is_drone(id) {
            let err_msg = format!("The packet drop rate of the node with id = {} can't be modified cause it isn't a drone", id);
            return Err(err_msg);
        }
        if let Some(sender) = self.send_command_drone.get(id) {
            match sender.send(DroneCommand::SetPacketDropRate(new_pdr)) {
                Ok(_) => {
                    Ok(new_pdr)
                },
                Err(e) => {
                    let err_msg = format!("Failed to send DroneCommand to drone with id = {}: {}", id, e);
                    Err(err_msg)
                }
            }
        }
        else{
            let err_msg = format!("No sender found for drone with id = {}", id);
            eprintln!("{}", err_msg);
            Err(err_msg)
        }
    }

    fn is_drone(&self, id: &NodeId) -> bool {
            self.drones.contains_key(id)
        }

        fn check_network_before_add_drone(&self, drone_id: &NodeId, connection: &Vec<NodeId>) -> bool {
            let mut adj_list = self.connections.clone();

            adj_list.insert(*drone_id, connection.clone());
            for neighbor in connection {
                if let Some(neighbor) = adj_list.get_mut(&neighbor) {
                    neighbor.push(*drone_id);
                };
            }

            self.are_server_and_clients_requirements_respected(&adj_list)
        }

        fn check_network_before_removing_drone(&self, drone_id: &NodeId) -> bool {
            let mut adj_list = self.connections.clone();

            if adj_list.remove(drone_id).is_none() {
                println!("Controller: Drone with id = {} can't be removed cause it doesen't exist", drone_id);
            }
            for neighbors in adj_list.values_mut() {
                neighbors.retain(|&id| id != *drone_id);
            }

            self.are_server_and_clients_requirements_respected(&adj_list) && is_connected(drone_id, &adj_list)
        }

        fn check_network_before_add_sender(&self, drone1_id: &NodeId, drone2_id: &NodeId) -> bool {
            let mut adj_list = self.connections.clone();

            if let Some(neighbors) = adj_list.get_mut(drone1_id) {
                neighbors.push(*drone2_id);
            }
            if let Some(neighbors) = adj_list.get_mut(drone2_id) {
                neighbors.push(*drone1_id);
            }

            self.are_server_and_clients_requirements_respected(&adj_list)
        }

        fn check_network_remove_sender(&self, drone1_id: &NodeId, drone2_id: &NodeId) -> bool {
            let mut adj_list = self.connections.clone();

            if let Some(neighbors) = adj_list.get_mut(drone1_id) {
                neighbors.retain(|&id| id != *drone2_id);
            }
            if let Some(neighbors) = adj_list.get_mut(&drone2_id) {
                neighbors.retain(|&id| id != *drone1_id);
            }

            self.are_server_and_clients_requirements_respected(&adj_list) && is_connected(drone1_id, &adj_list)
        }

        // each client must remain connected to at least one and at most two drones
        // and each server must remain connected to at least two drones
        fn are_server_and_clients_requirements_respected(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
            // self.clients.iter().all(|(&client, _)| {
            //     adj_list
            //         .get(&client)
            //         .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
            // }) && self.servers.iter().all(|(&server, _)| {
            //     adj_list
            //         .get(&server)
            //         .map_or(false, |neighbors| neighbors.len() >= 2)
            // })
            true
        }
    }

pub fn is_connected(id: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
    let n_nodes = adj_list.len();
    if n_nodes == 0 {
        return true; // an empty graph is connected
    }
    let mut visited = vec![false; n_nodes];
    dfs(id, adj_list, &mut visited);
    visited.into_iter().all(|v| v) // evry drone has been visited?
}
pub fn dfs(id: &NodeId, adj_list: &HashMap<NodeId, Vec<NodeId>>, visited: &mut Vec<bool>) {
    visited[*id as usize] = true; //TODO controllare la correttezza dell'algoritmo
    if let Some(neighbors) = adj_list.get(id){
        for n in neighbors{
            if !visited[*n as usize]{
                dfs(n, adj_list, visited);
            }
        }
    }
}

pub fn get_random_key(m: &HashMap<NodeId, Box<dyn Drone>>) -> Option<NodeId>{
    let mut possible_id: Vec<NodeId> = (0..=255).collect();
    possible_id.retain(|id| !m.contains_key(id));

    if possible_id.is_empty(){
        return None
    }

    let random_index = rand::thread_rng().gen_range(0..possible_id.len());

    Some(possible_id[random_index])
}

