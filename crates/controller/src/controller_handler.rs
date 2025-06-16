use message::{NodeCommand};

use crossbeam_channel::{select, select_biased, Receiver, Sender, unbounded, TrySendError};
use std::collections::HashMap;
use std::fmt::{format, Pointer};
use std::sync::mpsc;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;

use rand::{thread_rng, Rng};
// use utility::UIcommand;
// use utility::Operation;
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
use crate::utility::{ButtonEvent, GraphAction, NodeType, MessageType, DroneGroup};
use crate::utility::GraphAction::{AddEdge, AddNode, RemoveEdge, RemoveNode};
use crate::utility::MessageType::{Error, PacketSent};
use egui_graphs::Node;
use rand::seq::SliceRandom;
use wg_2024::drone::Drone;
use message::NodeCommand::FromShortcut;
use crate::utility::Operation::{AddSender, RemoveSender};

use client::Client;
use server::ChatServer;

pub struct ControllerHandler {
    pub drones: HashMap<NodeId, Box<dyn wg_2024::drone::Drone>>,
    pub drones_types: HashMap<NodeId, DroneGroup>,
    pub drone_senders: HashMap<NodeId, Sender<Packet>>, //the set of all the senders
    pub clients: HashMap<NodeId, Client>, //devo importare robe di daniele
    pub servers: HashMap<NodeId, ChatServer>,
    pub connections: HashMap<NodeId, Vec<NodeId>>, //viene passato dall'initializer
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>, // da controller a drone
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>, // da client a controller (anche server?) TODO io non avevo il reciver?
    pub receiver_event: HashMap<NodeId, Receiver<DroneEvent>>, // da dorne a controller
    //pub send_packet_server: HashMap<NodeId, Sender<Packet>>, //canali diversi per client e server vedi nodeCommand
    //pub send_packet_client: HashMap<NodeId, Sender<Packet>>,
    //LE VARIE COMPONENTI DEL CONTROLLER
    pub button_receiver: Receiver<ButtonEvent>, // the buttons window says what the user wants to do
    //TODO delete
    pub graph_connections_sender: Sender<HashMap<NodeId, Vec<NodeId>>>, // sends the connections to the graph (only at the first stage)
    //TODO delete
    pub graph_node_type_sender: Sender<HashMap<NodeId, NodeType>>, //sends the type of nodes to the graph (only at the first stage),
    pub graph_action_sender: Sender<GraphAction>, //sendes connection's updates to the graph
    pub message_sender: Sender<MessageType>, //sends to the message window what it has to shoh
    pub drones_counter: HashMap<DroneGroup, i8>,
}

//initializes the drones, distributing the implementations bought from the other groups(impl)
// as evenly as possible, having at most a difference of 1 between the group with the most drones
// running and the one with the least:



impl ControllerHandler {
    pub fn new(
        drones: HashMap<NodeId, Box<dyn wg_2024::drone::Drone>>,
        drones_types: HashMap<NodeId, DroneGroup>,
        drone_senders: HashMap<NodeId, Sender<Packet>>,
        clients: HashMap<NodeId, Client>,
        servers: HashMap<NodeId, ChatServer>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
        send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
        reciver_event: HashMap<NodeId, Receiver<DroneEvent>>,
        //send_packet_server: HashMap<NodeId, Sender<Packet>>,
        //send_packet_client: HashMap<NodeId, Sender<Packet>>,
        button_receiver: Receiver<ButtonEvent>,
        graph_connections_sender: Sender<HashMap<NodeId, Vec<NodeId>>>,
        graph_node_type_sender: Sender<HashMap<NodeId, NodeType>>,
        graph_action_sender: Sender<GraphAction>,
        message_sender: Sender<MessageType>,

    ) -> Self {
        let mut drones_counter: HashMap<DroneGroup, i8> = HashMap::new();

        Self {
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event: reciver_event,
            // send_packet_server,
            // send_packet_client,
            button_receiver,
            graph_connections_sender,
            graph_node_type_sender,
            graph_action_sender,
            message_sender,
            drones_counter,
        }
    }

    //TODO complete run
    pub fn run(&mut self) {
        loop {
            for (node_id, receiver) in self.receiver_event.clone() {
                if let Ok(event) = receiver.try_recv() {
                    self.drone_event_handlrer(event, node_id);
                }
            }

            if let Ok(command) = self.button_receiver.try_recv() {
                self.button_event_handler(command);
            }
            
            //TODO aggiungere il meccanismo per fermare loop

            // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }
    
    //—————————————————————————————————————————— Handlers ——————————————————————————————————————————
    
    pub fn button_event_handler(&mut self, event: ButtonEvent) {
        match event {
            ButtonEvent::NewDrone(id, pdr) => {
                self.spawn(&id, pdr);
            }
            ButtonEvent::NewServer(id) => {
                self.new_server(id);
            }
            ButtonEvent::NewClient(id) => {
                self.new_client(id);
            }
            ButtonEvent::NewConnection(id1, id2) => {
                self.add_connection(&id1, &id2);

            }

            ButtonEvent::Crash(id) => {
                self.crash(&id);

            }

            ButtonEvent::RemoveConection(id1, id2) => {
                //TODO verificare se è corretto interrompere la comunicazione da entrambi i lati
                self.remove_sender(&id1, &id2);

            }

            ButtonEvent::ChangePdr(id, pdr) => {
                self.change_packet_drop_rate(&id, pdr);
            }
            _ => {
                
            }

        }
    }

    pub fn drone_event_handlrer(&mut self, event: DroneEvent, drone_sender_id: NodeId){
        match event{
            DroneEvent::PacketSent(packet) => {
                if let Err(e) = self.message_sender.try_send(PacketSent(format!("The drone with ID [{}] has successfully sent the packet with session ID [{}]", drone_sender_id, packet.session_id))){
                    eprint!("Simulation Controller: The message confirming the successful reception of the PacketSent sent by the drone with ID [{}] could not be sent to the Message Window.", drone_sender_id)
                }
            }
            DroneEvent::PacketDropped(packet) => {
                if let Err(e) = self.message_sender.try_send(PacketSent(format!("The drone with ID [{}] has not successfully sent the packet with session ID [{}]", drone_sender_id, packet.session_id))){
                    eprint!("Simulation Controller: The message indicating the unsuccessful reception of the PacketSent from the drone with ID [{}] could not be sent to the Message Window.", drone_sender_id)
                }
            }

            DroneEvent::ControllerShortcut(packet) => {
                self.send_packet_to_client(packet).unwrap();
            }
        }
    }

    //——————————————————————————————————————— Useful metods ————————————————————————————————————————

    //TODO fare in modo che ne restituisca l'id
    pub fn new_drone_balanced(&mut self, id: NodeId,
                              sender_event: Sender<DroneEvent>,
                              receiver_command: Receiver<DroneCommand>,
                              receiver_packet: Receiver<Packet>,
                              packet_sender: HashMap<NodeId, Sender<Packet>>, drop_rate: f32) -> Option<(Box<dyn Drone>, DroneGroup)> {
        if let Some(drone_group) = self.select_drone_group().cloned(){
            let drone: Box<dyn Drone> = match drone_group {
                DroneGroup::RustInPeace => {
                    Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::BagelBomber => {
                    Box::new(BagelBomber::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::LockheedRustin => {
                    Box::new(LockheedRustin::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::RollingDrone => {
                    Box::new(RollingDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::RustDoIt => {
                    Box::new(RustDoIt::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::RustRoveri => {
                    Box::new(RustRoveri::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::Rustastic => {
                    Box::new(RustasticDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::RustBusters => {
                    Box::new(RustBustersDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::LeDronJames => {
                    Box::new(LeDronJames_drone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
                DroneGroup::RustyDrones => {
                    Box::new(RustyDrone::new(id, sender_event, receiver_command, receiver_packet, packet_sender, drop_rate))},
            };
            *self.drones_counter.entry(drone_group).or_insert(0) += 1;
            Some((drone, drone_group))
        }
        else {
            None
        }
    }

    pub fn select_drone_group(&self) -> Option<&DroneGroup>{
        let min_value = self.drones_counter.values().min()?;
        let mut candidates: Vec<&DroneGroup> = self.drones_counter.iter().filter_map(|(group, &count)|{
            if count == *min_value{
                Some(group)
            }
            else{
                None
            }
        }).collect();
        candidates.shuffle(&mut thread_rng());
        candidates.into_iter().next()
    }
    
    pub fn send_packet_to_client(&self, mut packet: Packet) -> Result<(), TrySendError<NodeCommand>> {
        if let Some(destination) = packet.routing_header.hops.clone().pop() {
            if self.clients.contains_key(&destination){
                if let Some(channel) = self.send_command_node.get(&destination){
                    channel.try_send(FromShortcut(packet))?;
                }
                //TODO stampare il fatto che c'è stato un errore nella comunicazione con il canale del client
            }
            
            else{
                //TODO messagio errore da stampare che il client non risulta esistente
            }
        }
        Ok(())
    }

    // pub fn notify_packet_dropped(&self, packet: Packet) {
    //     self.ui_sender.send(UIcommand::PacketDropped(packet));
    // }


    //
    // TODO vedere se funzione sotto deve essere migliorata in qualche modo

pub fn add_new_drone(&mut self, id: NodeId, first_connection: &NodeId, pdr: f32) -> Result<(), &str> {

    let mut connections:Vec<NodeId> = Vec::new();
    let mut senders = HashMap::new();

    let (sender_event, receiver_event) = unbounded::<DroneEvent>();
    let (sender_drone_command, receiver_drone_command) = unbounded::<DroneCommand>();
    let (sender_packet, receiver_packet) = unbounded::<Packet>();

    if let Some((drone, drone_group)) = self.new_drone_balanced(id, sender_event, receiver_drone_command, receiver_packet, senders, pdr) {
        self.drones.insert(id, drone);
        self.drones_types.insert(id, drone_group);
        self.drone_senders.insert(id, sender_packet);
        self.connections.insert(id, connections);
        self.send_command_drone.insert(id, sender_drone_command);
        self.receiver_event.insert(id,receiver_event);

        self.add_connection(&id, first_connection);

        match self.graph_action_sender.try_send(AddNode(id, NodeType::Drone)){
            Ok(_) => {
                print!("Controller: The message to the graph has been correctly sent")
            }

            Err(e) => {
                eprintln!("Controller: The message to the graph couldn't be sent correctly");
                return Err("Controller: The message to the graph couldn't be sent correctly")
                //TODO fare qualcosa?
            }

        }

        match self.message_sender.try_send(MessageType::Ok(format!("The new drone has been correctly created with id = {}", id))){
            Ok(_) => {
                print!("Controller: The message to the message window has been correctly sent")
            }

            Err(e) => {
                eprintln!("Controller: The message to the message window couldn't be sent correctly");
                return Err("Controller: The message to the message window couldn't be sent correctly")
                //TODO fare qualcosa?
            }
        }

    }
    else{
        eprintln!("Controller: the new drone couldn't be generated");
        return Err("Controller: the new drone couldn't be generated")
    }
    Ok(())
}

    //adds a new dorne to the network
    pub fn spawn(&mut self, first_connection: &NodeId, pdr: f32) {
        //TODO nella generazione random bisogna considerare anche gli id dei client e dei server
        let Some(id) = get_random_key(&self.drones) else {
            eprintln!("Controller: Couldn't get a random key");
            let _ = self.message_sender.try_send(Error("Controller: Couldn't get a random key".to_string()));
            return;
        };
        if !self.check_network_before_add_drone(&id, &vec![first_connection.clone() as NodeId]) {
            eprintln!("Controller: Drone with id = {} and connected to drone with id = {} can't be added to the network due to a violation of the network requirement", id, first_connection);
            let _ = self.message_sender.try_send(Error(format!("Controller: Drone with id = {} and connected to drone with id = {} can't be added to the network due to a violation of the network requirement", id, first_connection)));
            return;
        }

        //TODO capire se too much messaggi di errore
        let message = match self.add_new_drone(id, first_connection, pdr) {
            Ok(_) => return,
            Err(msg) => msg.to_string(), // cloni il messaggio ora
        };

        let _ = self.message_sender.try_send(Error(message));
    }
    
    pub fn new_client(&mut self, id_connection: NodeId){
        let Some(id) = get_random_key(&self.drones) else {
            eprintln!("Controller: Couldn't get a random key");
            let _ = self.message_sender.try_send(Error("Controller: Couldn't get a random key".to_string()));
            return;
        };
        if !self.is_drone(&id_connection){
            eprintln!("Controller: The first connection of a client must be a drone");
            let _ = self.message_sender.try_send(Error("Controller: The first connection of a client must be a drone".to_string()));
            return;
        }
    }

    // id: NodeId,
    // controller_send: Sender<NodeEvent>,
    // controller_recv: Receiver<NodeCommand>,
    // packet_recv: Receiver<Packet>,
    // packet_send: HashMap<NodeId, Sender<Packet>>,
    // ) -> Self {
    pub fn new_server(&mut self, id_connection: NodeId){
        let Some(id) = get_random_key(&self.drones) else {
            eprintln!("Controller: Couldn't get a random key");
            let _ = self.message_sender.try_send(Error("Controller: Couldn't get a random key".to_string()));
            return;
        };
        if !self.is_drone(&id_connection){
            eprintln!("Controller: The first connection of a client must be a drone");
            let _ = self.message_sender.try_send(Error("Controller: The first connection of a client must be a drone".to_string()));
            return;
        }
        let (controller_send, controller_recv_for_server) = unbounded::<DroneEvent>();
        let (controller_send_for_server, controller_recv) = unbounded::<DroneEvent>();
        let (packet_send_for_server, packet_recv) = unbounded::<DroneEvent>();

        // Crea l'HashMap per packet_send (inizialmente vuoto)
        let mut packet_send: HashMap<NodeId, Sender<Packet>> = HashMap::new();
        let (p_send, p_receiver) = unbounded::<Packet>();
        packet_send.insert(id_connection, p_send);
        
        //TODO sistemare connessioni nel drone connesso
        
        // let chat_server = ChatServer::new(
        //     id,
        //     controller_send,
        //     controller_recv,
        //     packet_recv,
        //     packet_send,
        // );
        
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
        match self.remove_drone(id){
            Ok(_) => {
                self.drone_senders.remove(id);

                //updating the counter
                if let Some (drone_group) = self.drones_types.get(id){
                    if let Some (count) = self.drones_counter.get_mut(drone_group){
                        *count = -1;
                    }
                }

                let _ = self.graph_action_sender.try_send(RemoveNode(*id));
                let _ = self.message_sender.try_send(MessageType::Ok(format!("The drone with id = {} has been correctly removed", id)));
            }
            Err(message) => {
                let _ = self.message_sender.try_send(Error(message));
            }
        }
    }

    pub fn add_connection(&mut self, id1: &NodeId, id2: &NodeId) {
        if !self.is_drone(id1) {
            eprintln!("Controller: Can't be added a new sender to node with id = {} cause it isn't a drone", id1);
            return;
        }
        if !self.is_drone(id2) {
            eprintln!("Controller: Node with id = {} can't be added as a new sender cause it isn't a drone", id2);
            return;
        }

        if let Ok(_) = self.new_sender(id1, id2){
            if let Ok(_) = self.new_sender(id2, id1){
                if let Err(_) = self.graph_action_sender.try_send(AddEdge(*id1, *id2)){
                    eprintln!("Controller: The update couldn't be sent correctly to the GraphView due to channel issue");
                    let _ = self.message_sender.try_send(Error("Controller: The update couldn't be sent correctly to the GraphView due to channel issue".to_string()));
                }
                else{
                    if let Err(_) = self.graph_action_sender.try_send(AddEdge(*id1, *id2)) {
                        eprintln!("Controller: The update couldn't be sent correctly to the GraphView due to channel issue");
                        let _ = self.message_sender.try_send(Error("Controller: The update couldn't be sent correctly to the GraphView due to channel issue".to_string()));
                    }
                }
            }
        }

    }

    pub fn remove_sender(&mut self, id: &NodeId, nghb_id: &NodeId) {
        if !self.check_network_remove_sender(id, nghb_id){
            return
        }

        if let Err(e) = self.close_sender(id, nghb_id){
            let _ = self.message_sender.try_send(Error(format!("The connection between the drone with id = {} and the drone with id = {} can't be removed", id, nghb_id)));
            //messaggio
        }
        else{
            //let Ok(_) = self.graph_action_sender.try_send(RemoveEdge(*id, *nghb_id));
            //TODO capire se solo tra droni
            let _ = self.message_sender.try_send(MessageType::Ok(format!("The connection between the drone with id = {} and the drone with id = {} has been correctly removed", id, nghb_id)));
        }
    }

    //sends also a remove sender to its neighbours
    //TODO capire quando far rimuovere il drone dalle connection del controller
    pub fn remove_drone(&mut self, id: &NodeId) -> Result<(), String>{
        if let Some(sender) = self.send_command_drone.get(id) {
            if let Err(e) = sender.send(DroneCommand::Crash) {
                eprintln!("Controller: Node with id = {} doesen't recive correctly the DroneCommand", id);
                return Err(format!("Controller: Node with id = {} doesen't recive correctly the DroneCommand", id))
            }
        }
        match self.remove_all_senders(id){
            Err(nodes) => {
                let node_list = nodes
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                eprintln!(
                    "Controller: The nodes with id = [{}] couldn't be disconnected from the drone with id = {}",
                    node_list, id
                );
                return Err(format!("Controller: The nodes with id = [{}] couldn't be disconnected from the drone with id = {}", node_list, id));
            }
            Ok(_) => {
                let _ = self.drones.remove(id);
                let _ = self.drone_senders.remove(id);
                let _ = self.connections.remove(id);
                let _ = self.send_command_drone.remove(id);
                let _ = self.receiver_event.remove(id);
            }
        }
        Ok(())
    }


    //close the channel with all neighbours of a drone
    fn remove_all_senders(&mut self, id: &NodeId) -> Result<(), Vec<NodeId>> {
        let mut vec_err: Vec<NodeId> = Vec::new();
        if let Some(drone_connections) = self.connections.get(id).cloned() {
            for i in drone_connections {
                if let Err(_) = self.close_sender(id, &i){
                    vec_err.push(i);
                };

            }
        }
        //TODO per client e server

        if !vec_err.is_empty(){
            return Err(vec_err)
        }
        Ok(())
    }

    //close the channel with a neighbour drone
    fn close_sender(&mut self, id: &NodeId, nghb_id: &NodeId) -> Result<(), String>{
        if let Some(sender) = self.send_command_drone.get(id) {
            if let Err(e) = sender.send(DroneCommand::RemoveSender(*nghb_id)) {
                eprintln!("Controller: The DroneCommand RemoveSender to the drone with id = {} hasn't been sent correctly", nghb_id);
                return Err(format!("Controller: The DroneCommand RemoveSender to the drone with id = {} hasn't been sent correctly", nghb_id))
            }
        }
        // if let Some(sender) = self.send_command_node.get(nghb_id) {
        //     if let Err(e) = sender.send(NodeCommand::RemoveSender(*id)) {
        //         eprintln!("Controller: The DroneCommand RemoveSender to the node with id = {} hasn't been sent correctly", id);
        //         return Err(e)
        //     }
        // }
        else{
            //qui rimuovere la connessione?
            if let Some(connection) = self.connections.get_mut(id){
                if let Some(index) = connection.iter().position(|&id| id == *nghb_id){
                    connection.remove(index);
                    if let Some(connection) = self.connections.get_mut(nghb_id){
                        if let Some(index) = connection.iter().position(|&id| id == id){
                            connection.remove(index);
                            let _ = self.graph_action_sender.try_send(RemoveEdge(*id,*nghb_id)); //TODO capire se farci qualcosa
                        }
                    }
                }
            }

        }

        Ok(())
    }

    //adds dst_id to the drone neighbors (with dst_id crossbeam::Sender)
    fn new_sender(&mut self, id: &NodeId, dst_id: &NodeId) -> Result<(),String>{
        if let Some(sender1) = self.send_command_drone.get(id) {
            if let Some(sender2) = self.drone_senders.get(dst_id){
                if let Err(e) = sender1.send(DroneCommand::AddSender(*dst_id, sender2.clone())) {
                    eprintln!("Controller: The DroneCommand AddSender to the drone with id = {} hasn't been sent correctly", id);
                    Err(format!("Controller: The DroneCommand AddSender to the drone with id = {} hasn't been sent correctly", id))
                }
                else{
                    if let Some (connection) = self.connections.get_mut(id){
                        connection.push(*dst_id);
                    }
                    Ok(())
                }
            }
            else{
                Err(format!("Controller: Couldn't find the DroneCommand sender for the drone with id = {}", id))
            }
        }
        else{
            Err(format!("Controller: Couldn't find the DroneCommand sender for the drone with id = {}", id))
        }
    }

    //TODO sistemare la funzione is_drone in modo tale che sia lei a far stampare il messaggio

    //alter the pdr of a drone
    fn change_packet_drop_rate(&mut self, id: &NodeId, new_pdr: f32){
    match self.set_packet_drop_rate(id,new_pdr){
        Ok(pdr) => {
            let _ = self.message_sender.try_send(MessageType::Ok(format!("The packet drop rate of the drone with id = {} has been correctly changed to {}", id, pdr)));
        },
        Err(message) => {
            let _ = self.message_sender.try_send(Error(message));
        }
    }
}


    fn set_packet_drop_rate(&mut self, id: &NodeId, new_pdr: f32) -> Result<f32, String> {
        if !self.is_drone(id) {
            let err_msg = format!("The packet drop rate of the node with id = {} can't be modified cause it isn't a drone", id);
            eprintln!("{}", err_msg);
            return Err(err_msg)
        }
        if let Some(sender) = self.send_command_drone.get(id) {
            match sender.send(DroneCommand::SetPacketDropRate(new_pdr)) {
                Ok(_) => {
                    Ok(new_pdr)
                },
                Err(e) => {
                    let err_msg = format!("Failed to send DroneCommand to drone with id = {}: {}", id, e);
                    eprintln!("{}", err_msg);
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
        fn are_server_and_clients_requirements_respected(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool {
            self.clients.iter().all(|(&client, _)| {
                adj_list
                    .get(&client)
                    .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
            }) && self.servers.iter().all(|(&server, _)| {
                adj_list
                    .get(&server)
                    .map_or(false, |neighbors| neighbors.len() >= 2)
            });
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

pub fn get_random_key(d: &HashMap<NodeId, Box<dyn Drone>>, c: &HashMap<NodeId, Box<Client>>, s: &HashMap<NodeId, Box<Server>>) -> Option<NodeId>{
    let mut possible_id: Vec<NodeId> = (0..=255).collect();
    possible_id.retain(|id| !d.contains_key(id));

    if possible_id.is_empty(){
        return None
    }

    let random_index = rand::thread_rng().gen_range(0..possible_id.len());

    Some(possible_id[random_index])
}

