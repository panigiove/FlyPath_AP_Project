mod controller_test;
mod utility;

use crossbeam_channel::{select, select_biased, unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;
use wg_2024::config::{Client, Drone, Server};
use wg_2024::network::NodeId;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use rand::Rng;
use std::cmp;
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

#[derive(Debug, Clone)]
pub struct Controller{
    pub drones: HashMap<NodeId, Box<dyn Drone>>,
    pub clients: HashMap<NodeId, Box<dyn Client>>,
    pub servers: HashMap<NodeId, Box<dyn Server>>,
    pub send_command: HashMap<NodeId, Sender<DroneCommand>>,
    pub recive_event: HashMap<NodeId, Receiver<DroneEvent>>, //
    pub send_packet_server: HashMap<NodeId, Sender<Packet>>,
    pub send_packet_client: HashMap<NodeId, Sender<Packet>>,
    pub ui_reciver: Receiver<UIcommand>, //TODO cosa invia controller a sender
    pub ui_sender: Sender<UIcommand>
}

//initializes the drones, distributing the implementations bought from the other groups(impl)
// as evenly as possible, having at most a difference of 1 between the group with the most drones
// running and the one with the least:
//TODO funzione per decidere che drone creare
pub fn drone_random(id: NodeId,
                    sender_event: Sender<DroneEvent>,
                    receiver_command: Receiver<DroneCommand>,
                    receiver_packet: Receiver<DroneCommand>,
                    packet_sender: HashMap<NodeId, Sender<Packet>>) -> Option<Box<Drone>> {
    let mut rng = rand::thread_rng();
    let rand = rng.gen_range(1..11);
    let drop_rate = rng.gen_range(0.0 .. 1.1);
    match rand{
        1 => Box::new(NoSoundDroneRIP::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        2 => Box::new(BagelBomber::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        3 => Box::new(LockheedRustin::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        4 => Box::new(RollingDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        5 => Box::new(RustDoIt::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        6 => Box::new(RustRoveri::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        7 => Box::new(RustasticDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        8 => Box::new(RustBustersDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        9 => Box::new(LeDronJames_drone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        10 => Box::new(RustyDrone::new(id, sender_event, receiver_command,receiver_packet,packet_sender,drop_rate)),
        _ => None,
    }
}

impl Controller{

    pub fn new(
        drones: HashMap<NodeId, Box<Drone>>,
        clients: HashMap<NodeId, Box<Client>>,
        servers: HashMap<NodeId, Box<Server>>,
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
        }
    }

    pub fn initialize_network(){
        //creo 10 droni
    }

    pub fn run(&mut self){
        loop{
            // select_biased!{
            //     recv(self.ui_comunication) -> command =>{
            //         if let Ok(command) = interaction{
            //             self.ui_command_handler(command);
            //         }
            //     }
            //     default => {
            //         for (_, i) in self.recive_event{
            //             select! {
            //                 recv(i) -> event =>{
            //                     if let Ok(event) = event{
            //                         self.event_handler(event);
            //                     }
            //                 }
            //         }
            //         }
            //     }
            // }
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
            DroneEvent::ControllerShortcut(packet) => self.send_packet(packet),
            _ => None,
        }
    }

    pub fn send_packet(&self, packet: Packet){
        let hops = packet.routing_header.hops.clone();
        let destination = hops.pop();
        match destination{
            Ok(id) => self.send_packet_client.get(id).unwrap().send(packet).unwrap(), //pacchetti sempre da server a client?
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
    pub fn spawn(&mut self, connections: Vec<NodeId>){
        if !self.check_network(AddDrone, (id, _)){
            return
        }
        let mut id = max(self.servers.len(), self.clients.len());
        id = max(id, self.drones.len()) + 1; //in this way in the vec of connections the drones are well organized
        self.add_drone(id,connections);
    }

    pub fn crash(&mut self, id: &NodeId){
        if !self.is_drone(id) || !self.check_network(RemoveDrone, (id, _)){
            return
        }
        self.remove_drone(id);
    }

    //sends also a remove sender to its neighbours
    pub fn remove_drone(&mut self, id: &NodeId){

        //inviamo il messaggio per il crash
        self.send_command.get(*id.clone()).unwrap().send(DroneCommand::Crash).unwrap();

        //inviamo il remove sender ad ogni vicino del nodo crashato
        self.remove_all_senders(id);
    }

    //This command adds dst_id to the drone neighbors, with dst_id crossbeam::Sender
    pub fn add_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>){
        if !self.check_network(AddSender, (id,dst_id)) {
            return;
        }
        self.new_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>);
    }

    pub fn add_drone(&mut self, id: NodeId, connections: Vec<NodeId>){

        let (drone, sender_command, receiver_event) = self.new_drone(&id, &connections);

        self.drones.insert(id, drone); //insieriamo il drone nell'elenco

        self.send_command.insert(id, sender_command);
        self.recive_event.insert(id, receiver_event);

    }


    pub fn remove_sender(&mut self, id: &NodeId, nghb_id: &NodeId){
        if !self.check_network(RemoveSender, (id, dst_id)) || !self.is_drone(id) || !self.is_drone(nghb_id){
            return;
        }
        self.close_sender(id, nghb_id);
    }

    //close the channel with all neighbours of a drone
    fn remove_all_senders(&mut self, id: &NodeId){
        let (_, drone) = self.drones.get_key_value(&id).unwrap();
        for i in drone.connected_node_ids{
            //per ora contiamo solo come se fossero tutti droni
            //TODO per client e server
            self.close_sender(id, &i);
            //ci pensa il drone ad aggiornare le connessioni?
        }
    }

    //close the channel with a neighbour drone
    fn close_sender(&mut self, id: &NodeId, nghb_id: &NodeId){
        self.send_command.get(id).unwrap().send(DroneCommand::RemoveSender(*nghb_id)).unwrap();
    }

    //adds dst_id to the drone neighbors (with dst_id crossbeam::Sender)
    fn new_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>){
        self.send_command.get(id).unwrap().send(DroneCommand::AddSender(*dst_id, sender)).unwrap();
    }

    //alter the pdr of a drone
    fn set_packet_drop_rate(&mut self, id:&NodeId, new_pdr: f32){
        if !self.is_drone(id){
            return
        }
        self.send_command.get(id).unwrap().send(DroneCommand::SetPacketDropRate(new_pdr)).unwrap();
    }

    fn is_drone (&mut self, id: &NodeId) -> bool {
        self.drones.contains_key(id)
    }


    fn check_network(&mut self, operation: Operation, (drone_id, drone_id2): NodeId) -> bool{
        let clients = self.clients.clone();
        let server = self.servers.clone();
        let adj_list = Vec::new();

        for (id, connections) in self.drones {
            adj_list[id] = connections.id;
        }

        for (id, connections) in self.clients {
            adj_list[id] = connections.id;
        }

        for (id, connections) in self.servers {
            adj_list[id] = connections.id;
        }

        match operation{
            Operation::AddDrone =>  {
                adj_list[drone_id] = drone.connected_node_ids;
                for id in drone.connected_node_ids{
                    adj_list[id].push(drone_id);
                }
            },
            Operation::RemoveDrone => {
                adj_list.remove(drone_id as usize); // deleting the connections of the drone that must be removed
                for id in drone.connected_node_ids{
                    for i in id{
                        if id[i] == drone_id{
                            adj_list[id].remove(i); // deleting the id of the removed drone from other drone's connections
                        }
                    }
                }
                //the network must reman connected
                let mut result = is_connected(drone_id, adj_list);
            },
            Operation::AddSender =>{
                adj_list[drone_id].push(drone_id2); // adding a connection
            },
            Operation::RemoveSender => {
                for i in adj_list[drone_id]{
                    if id[i] == drone_id{
                        adj_list[id].remove(i); // deleting the connection
                    }
                }
                let mut result = is_connected(drone_id, adj_list);
            },
            _ => Null,
        }

        pub fn is_connected(id: NodeId, adj_list: &Vec<Vec<NodeId>>) -> bool {
            let n_nodes = adj_list.len();
            if n_nodes == 0 {
                return true; // an empty graph is connected
            }
            let mut visited = vec![false; n_nodes];

            pub fn dfs(&self, id: NodeId, adj_list: &Vec<Vec<NodeId>>, visited: &mut Vec<bool>) {
                visited[id] = true;
                for &neighbor in &adj_list[id] {
                    if !visited[neighbor] {
                        dfs(neighbor, adj_list, visited);
                    }
                }
            }
            dfs(id, adj_list, &mut visited);
            visited.into_iter().all(|v| v) // evry drone has been visited?
        }

        // Each client must remain connected to at least one and at most two drones.
        for (client, _) in self.clients{
            if !(adj_list[client].len() > 0 && adj_list[client].len() < 3){
                result = false;
            }
        }

        // Each server must remain connected to at least two drones.
        for (server, _) in self.servers{
            if adj_list[server].len() < 3 {
                result = false;
            }
        }
        result

    }

}