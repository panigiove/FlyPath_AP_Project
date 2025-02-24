mod controller_test;
mod utility;

use std::ascii::Char::Null;
use crossbeam_channel::{select, select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;

use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::packet::{
    Ack, FloodRequest, FloodResponse, Fragment, Nack, NackType, NodeType, Packet, PacketType,
};

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
use wg_2024::drone::Drone;
use crate::utility::Operation::{AddSender, RemoveSender};

#[derive(Debug, Clone)]
pub struct Controller{
    pub drones: HashMap<NodeId, Box<dyn Drone>>,
    pub clients: HashMap<NodeId, Box<dyn Client>>,
    pub servers: HashMap<NodeId, Box<dyn Server>>,
    pub connections: HashMap<NodeId, Vec<NodeId>>, //viene passato dall'initializer
    pub send_command_drone: HashMap<NodeId, Sender<DroneCommand>>,
    pub send_command_node: HashMap<NodeId, Sender<NodeCommand>>,
    pub recive_event: HashMap<NodeId, Receiver<DroneEvent>>, //
    pub send_packet_server: HashMap<NodeId, Sender<Packet>>, //canali diversi per client e server vedi nodeCommand
    pub send_packet_client: HashMap<NodeId, Sender<Packet>>,
    pub ui_reciver: Receiver<UIcommand>, //TODO cosa invia controller a sender
    pub ui_sender: Sender<UIcommand>
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



impl Controller{

    pub fn new(
        drones: HashMap<NodeId, Box<dyn Drone>>,
        clients: HashMap<NodeId, Box<dyn Client>>,
        servers: HashMap<NodeId, Box<dyn Server>>,
        send_command: HashMap<NodeId, Sender<DroneCommand>>,
        recive_event: HashMap<NodeId, Receiver<DroneEvent>>,
        send_packet_server: HashMap<NodeId, Sender<Packet>>,
        send_packet_client: HashMap<NodeId, Sender<Packet>>,
        ui_reciver: Receiver<UIcommand>, //TODO cosa invia controller a sender
        ui_sender: Sender<UIcommand>
        ) -> Self{
        Self{
            drones,
            clients,
            servers,
            send_command,
            recive_event,
            send_packet_server,
            send_packet_client,
            ui_comunication: ui_reciver,
            1,
        }
    }

    //TODO complete run
    pub fn run(&mut self){
        loop{
            select_biased!{
                recv(self.ui_comunication) -> command =>{
                    if let Ok(command) = interaction{
                        self.ui_command_handler(command); //TODO completare una voltaa fatta la UI
                    }
                }
                default => {
                    for (_, i) in self.recive_event{
                        select! {
                            recv(i) -> event =>{
                                if let Ok(event) = event{
                                    self.event_handler(event);
                                }
                            }
                    }
                    }
                }
            }
            if let Ok(command) = self.ui_reciver.try_recv() {
                self.ui_command_handler(command);
                continue;
            }

            for (_, i) in &self.recive_event {
                if let Ok(event) = i.try_recv() {
                    self.event_handler(event);
                }
            }

            // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }

    pub fn new_drone_balanced(&mut self, id: NodeId,
                              sender_event: Sender<DroneEvent>,
                              receiver_command: Receiver<DroneCommand>,
                              receiver_packet: Receiver<DroneCommand>,
                              packet_sender: HashMap<NodeId, Sender<Packet>>) -> Option<Box<dyn Drone>> {
        match self.counter {
            1 => {
                Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 2;
            },
            2 => {
                Box::new(BagelBomber::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 3;
            },
            3 => {
                Box::new(LockheedRustin::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 4;
            },
            4 => {
                Box::new(RollingDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 5;
            },
            5 => {
                Box::new(RustDoIt::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 6;
            },
            6 => {
                Box::new(RustRoveri::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 7;
            },
            7 => {
                Box::new(RustasticDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 8;
            },
            8 => {
                Box::new(RustBustersDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 9;
            },
            9 => {
                Box::new(LeDronJames_drone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 10;
            },
            10 => {
                Box::new(RustyDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate));
                self.counter = 1;
            },
            _ => None,
        }
    }

    pub fn ui_command_handler(&self,ui_command: UIcommand){
        match ui_command {
            UIcommand::NewDrone(connections) => self.spawn(connections),
            UIcommand::Crash(id) => self.crash(id),
            UIcommand::RemoveSender(id1,id2) => self.remove_sender(id1, id2),
            UIcommand::AddSender(id1,id2) => self.add_sender(id1,id2),
            UIcommand::SetPacketDropRate(id,pdr) => self.set_packet_drop_rate(id, pdr),
            _ => None,
        }
    }

    pub fn event_handler(&mut self, event: DroneEvent){
        match event{
            DroneEvent::PacketSent(packet) => self.notify_packet_sent(),
            DroneEvent::PacketDropped(packet) => self.notify_packet_dropped(packet),
            DroneEvent::ControllerShortcut(packet) => self.send_packet(packet), //da drone a server/client
            _ => None,
        }
    }

    // TODO sistemare questa funzione: il controller si occupa di mandare macchetti che non possono essere persi a client/server
    pub fn send_packet(&self, packet: Packet){
        let hops = packet.routing_header.hops.clone();
        let destination = hops.pop();
        match destination{
            Ok(id) => self.send_packet_client.get(id).unwrap().send(packet).unwrap(),
            _ => None
        }
    }

    pub fn notify_packet_sent(&self, packet: Packet){
        self.ui_sender.send(PacketDropped(packet));
    }

    pub fn notify_packet_dropped(&self, packet: Packet){
        self.ui_sender.send(PacketDropped(packet));
    }

    //funzione per creare un nuovo drone
    pub fn new_drone(& mut self, id: &NodeId, connections: &Vec<NodeId>) -> (Drone, Sender<DroneCommand>, Receiver<DroneEvent>) {

        let (sender_command, receiver_command): (Sender<DroneCommand>, Receiver<DroneCommand>) = unbounded();
        let (sender_event, receiver_event): (Sender<DroneEvent>, Receiver<DroneEvent>) = unbounded();
        let (sender_packet, receiver_packet): (Sender<Packet>, Receiver<Packet>) = unbounded();

        let mut packet_sender: HashMap<NodeId, Sender<Packet>> = Default::default(); //vedere se questa inizializzazione crea problemi
        //testare se il drone è inizializzato correttamente

        for i in connections{
            if self.drones.contains_key(i){
                self.send_command.get_key_value(i).unwrap().1.send(DroneCommand::AddSender(*id, sender_packet.clone())); //inviamo un messaggio al nodo per dirgli di aggiungere una nuova connessione
                packet_sender.insert(*id, sender_packet.clone()); //sbagliato ci vogliono i sender degli altri nodi FORSE GIUSTO
            }
        }

        let drone = drone_random(id, sender_event, receiver_command, receiver_packet, packet_sender).unwrap();

        (drone, sender_command, receiver_event)
    }

    //adds a new dorne to the network
    pub fn spawn(&mut self, id: &NodeId, connections: &Vec<NodeId>, drone: Box<dyn Drone>){
        if !self.check_network_before_add_drone(id,connections){
            eprintln!("Controller: Drone with id = {} can't be added to the network due to a violation of the network requirement", id);
            return;
        }
        self.add_drone(id,connections);
    }

    pub fn crash(&mut self, id: &NodeId){
        if !self.is_drone(id){
            eprintln!("Controller: Node with id = {} can't be removed to the network cause it isn't a drone", id);
            return;
        }
        if !self.check_network_before_removing_drone(id){
            eprintln!("Controller: Node with id = {} can't be removed to the network due to a violation of the network requirement", id);
            return;
        }
        self.remove_drone(id);
    }

    pub fn add_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender1: Sender<Packet>, sender2: Sender<Packet>){
        if !self.is_drone(id){
            eprintln!("Controller: Can't be added a new sender to node with id = {} cause it isn't a drone", id);
            return;
        }
        if !self.is_drone(dst_id){
            eprintln!("Controller: Node with id = {} can't be added as a new sender cause it isn't a drone", dst_id);
            return;
        }

        if !self.check_network(AddSender, (id, dst_id)){
            eprintln!("Controller: Node with id = {} can't be removed to the network due to a violation of the network requirement", id);
            return;
        }
        self.new_sender(id, dst_id, sender1);
        self.new_sender(dst_id, id, sender2);
    }

    pub fn remove_sender(&mut self, id: &NodeId, nghb_id: &NodeId){
        if !self.check_network(RemoveSender, (id, nghb_id)) || !self.is_drone(id) || !self.is_drone(nghb_id){
            return;
        }
        self.close_sender(id, nghb_id);
    }

    //sends also a remove sender to its neighbours
    //TODO capire quando far rimuovere il drone dalle connection del controller
    pub fn remove_drone(&mut self, id: &NodeId){
        if let Some(sender) = self.send_command.get(id){
            if let Err(e) = sender.send(DroneCommand::Crash){
                eprintln!("Controller: Node with id = {} doesen't recive correctly the DroneCommand", id);
            }
        }
        self.remove_all_senders(id);
    }

    //This command adds dst_id to the drone neighbors, with dst_id crossbeam::Sender


    pub fn add_drone(&mut self, id: &NodeId, connections: &Vec<NodeId>){

        let (drone, sender_command, receiver_event) = self.new_drone(&id, &connections);

        self.drones.insert(*id, drone); //insieriamo il drone nell'elenco

        self.send_command.insert(*id, sender_command);
        self.recive_event.insert(*id, receiver_event);

    }




    //close the channel with all neighbours of a drone
    fn remove_all_senders(&mut self, id: &NodeId){
        let (_, drone) = self.drones.get_key_value(&id).unwrap(); //SISTEMARE
        for i in drone.connected_node_ids{
            //per ora contiamo solo come se fossero tutti droni
            //TODO per client e server
            self.close_sender(id, &i);
            //ci pensa il drone ad aggiornare le connessioni?
        }
    }

    //close the channel with a neighbour drone
    fn close_sender(&mut self, id: &NodeId, nghb_id: &NodeId){
        if let Some(sender) = self.send_command.get(id){
            if let Err(e) = sender.send(DroneCommand::RemoveSender(*nghb_id)){
                eprintln!("Controller: The DroneCommand RemoveSender to the drone with id = {} hasn't been sent correctly", id);
            }
        }
    }

    //adds dst_id to the drone neighbors (with dst_id crossbeam::Sender)
    fn new_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>){
        if let Some(sender) = self.send_command.get(id){
            if let Err(e) = sender.send(DroneCommand::AddSender(*dst_id, sender)){
                eprintln!("Controller: The DroneCommand AddSender to the drone with id = {} hasn't been sent correctly", id);
            }
        }
    }

//TODO sistemare la funzione is_drone in modo tale che sia lei a far stampare il messaggio

    //alter the pdr of a drone
    fn set_packet_drop_rate(&mut self, id:&NodeId, new_pdr: f32){
        if !self.is_drone(dst_id){
            eprintln!("Controller: Node with id = {} can't be added as a new sender cause it isn't a drone", dst_id);
            return;
        }
        if let Some(sender) = self.send_command.get(id){
            if let Err(e) = sender.send(DroneCommand::SetPacketDropRate(new_pdr)){
                eprintln!("Controller: The DroneCommand SetPacketDropRate to the drone with id = {} hasn't been sent correctly", id);
            }
        }
    }

    fn is_drone (&mut self, id: &NodeId) -> bool {
        self.drones.contains_key(id)
    }


    pub fn check_network_before_add_drone(&self, drone_id: &NodeId, connection: &Vec<NodeId>) -> bool{
        let mut adj_list = self.connections.clone();

        adj_list.insert(*drone_id, connection.clone());
        for neighbor in connection{
            if let Some(neighbor) = adj_list.get_mut(&neighbor){
                neighbor.push(*drone_id);
            };
        }

        self.are_server_and_clients_requirements_respected(&adj_list)
    }

    pub fn check_network_before_removing_drone(&self, drone_id: &NodeId) -> bool{
        let mut adj_list = self.connections.clone();

        if adj_list.remove(drone_id).is_none(){
            println!("Controller: Drone with id = {} can't be removed cause it doesen't exist", drone_id);
        }
        for neighbors in adj_list.values_mut(){
            neighbors.retain(|&id| id != *drone_id);
        }

        self.are_server_and_clients_requirements_respected(&adj_list) && is_connected(drone_id,&adj_list)
    }

    pub fn check_network_before_add_sender(&self, drone1_id: &NodeId, drone2_id: &NodeId) -> bool{
        let mut adj_list = self.connections.clone();

        if let Some(neighbors) = adj_list.get_mut(drone1_id){
            neighbors.push(*drone2_id);
        }
        if let Some(neighbors) = adj_list.get_mut(drone2_id){
            neighbors.push(*drone1_id);
        }

        self.are_server_and_clients_requirements_respected(&adj_list)
    }

    pub fn check_network_remove_sender(&self, drone1_id: &NodeId, drone2_id: &NodeId) -> bool{
        let mut adj_list = self.connections.clone();

        if let Some(neighbors) = adj_list.get_mut(drone1_id){
            neighbors.retain(|&id| id != *drone2_id);
        }
        if let Some(neighbors) = adj_list.get_mut(&drone2_id){
            neighbors.retain(|&id| id != *drone1_id);
        }

        self.are_server_and_clients_requirements_respected(&adj_list) && is_connected(drone1_id,&adj_list)
    }

    // each client must remain connected to at least one and at most two drones
    // and each server must remain connected to at least two drones
    pub fn are_server_and_clients_requirements_respected(&self, adj_list: &HashMap<NodeId, Vec<NodeId>>) -> bool{
        self.clients.iter().all(|(&client, _)| {
            adj_list
                .get(&client)
                .map_or(false, |neighbors| neighbors.len() > 0 && neighbors.len() < 3)
        }) && self.servers.iter().all(|(&server, _)| {
            adj_list
                .get(&server)
                .map_or(false, |neighbors| neighbors.len() >= 2)
        })
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
    visited[id] = true;
    if let Some(neighbors) = adj_list.get(id){
        for n in neighbors{
            if !visited[n]{
                dfs(n, adj_list, visited);
            }
        }
    }
}