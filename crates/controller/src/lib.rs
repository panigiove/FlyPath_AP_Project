mod controller_test;

//conditions to be mantieindes:
// - The network graph must remain connected.
// - Each client must remain connected to at least one and at most two drones.
// - Each server must remain connected to at least two drones.
//Drone must prioritize messages sent


// - Rustastic


use crossbeam_channel::{unbounded, Receiver, Sender};
use std::collections::HashMap;
use std::fmt::Pointer;
use wg_2024::config::{Client, Drone, Server};
use wg_2024::network::NodeId;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use rust_do_it::RustDoIt;
use bagel_bomber::BagelBomber;
use rustbusters_drone::RustBustersDrone;
use rusty_drones::RustyDrone;
use lockheedrustin_drone::LockheedRustin;
use rolling_drone::RollingDrone;
use LeDron_James;
use Rust_in_pea;
use rusgit
use rustastic_drone::RustasticDrone;

#[derive(Debug, Clone)]
pub struct Controller{
    pub drones: HashMap<NodeId, Drone>,
    pub clients: HashMap<NodeId, Client>,
    pub servers: HashMap<NodeId, Server>,
    pub send_command: HashMap<NodeId, Sender<DroneCommand>>,
    pub recive_event: HashMap<NodeId, Receiver<DroneEvent>>,

}

impl Controller{

    pub fn new(
        drones: HashMap<NodeId, Drone>,
        clients: HashMap<NodeId, Client>,
        servers: HashMap<NodeId, Server>,
        send_command: HashMap<NodeId, Sender<DroneCommand>>,
        recive_event: HashMap<NodeId, Receiver<DroneEvent>>,
        ) -> Self{
        Self{
            drones,
            clients,
            servers,
            send_command,
            recive_event,
        }
    }

    //TODO: loop per il controllo dei canali

    //funzione per creare un nuovo drone
    pub fn new_drone(& mut self, id: &NodeId, connections: &Vec<NodeId>) -> (Drone, Sender<DroneCommand>, Receiver<DroneEvent>) {

        let (sc, rc): (Sender<DroneCommand>, Receiver<DroneCommand>) = unbounded();
        let (se, re): (Sender<DroneEvent>, Receiver<DroneEvent>) = unbounded();
        let (sp, rp): (Sender<Packet>, Receiver<Packet>) = unbounded();

        let mut packet_sender: HashMap<NodeId, Sender<Packet>> = Default::default(); //vedere se questa inizializzazione crea problemi
        //testare se il drone è inizializzato correttamente

        for i in connections{
            if self.drones.contains_key(i){
                self.send_command.get_key_value(i).send(DroneCommand::AddSender(id, sp.clone())); //inviamo un messaggio al nodo per dirgli di aggiungere una nuova connessione
                packet_sender.insert(*id, sp.clone()); //sbagliato ci vogliono i sender degli altri nodi FORSE GIUSTO
            }
        }

        //random drone
        //TODO creatore di nuovi droni
        let drone = Drone::new(id, se, rc, rp, packet_sender);

        (drone, sc, re)
    }

    //adds a new dorne to the network
    pub fn spawn(&mut self, id: NodeId, connections: Vec<NodeId>){

        //se è già presente usciamo
        if self.drones.contains_key(&id){
            return
        }

        let (drone, sc, re) = self.new_drone(&id, &connections);

        self.drones.insert(id, drone); //insieriamo il drone nell'elenco

        self.send_command.insert(id, sc);
        self.recive_event.insert(id, re);

    }

    //sends also a remove sender to its neighbours
    fn crash(&mut self, id: &NodeId){

        //controlliamo se il nodo sia un drone
        if !self.is_drone(id){
            return
        }

        //inviamo il messaggio per il crash
        self.send_command.get(*id.clone()).unwrap().send(DroneCommand::Crash).unwrap();

        //inviamo il remove sender ad ogni vicino del nodo crashato
        self.remove_all_senders(id);


        // match self.drones.remove(&id) {
        //     Some(drone) => {
        //         for i in drone.connected_node_ids{
        //             //per ora consideriamo solo droni
        //             self.RemoveSender()
        //         }
        //         // Qui `drone` è un `Drone` (valore posseduto)
        //     }
        //     None => {
        //         println!("Nessun drone trovato con id {}", id);
        //     }
        // }
    }

    //close the channel with all neighbours of a drone
    fn remove_all_senders(&mut self, id: &NodeId){
        let (_, drone) = self.drones.get_key_value(&id).unwrap();
        for i in drone.connected_node_ids{
            //per ora contiamo solo come se fossero tutti droni
            //TODO per client e server
            self.remove_sender(id, &i);
            //ci pensa il drone ad aggiornare le connessioni?
        }
    }

    //close the channel with a neighbour drone
    fn remove_sender(&mut self, id: &NodeId, nghb_id: &NodeId){
        if !self.is_drone(id){
            return
        }
        if !self.is_drone(nghb_id){
            return
        }
        self.send_command.get(id).unwrap().send(DroneCommand::RemoveSender(*nghb_id)).unwrap();
    }

    //adds dst_id to the drone neighbors (with dst_id crossbeam::Sender)
    fn add_sender(&mut self, id: &NodeId, dst_id: &NodeId, sender: Sender<Packet>){
        self.send_command.get(id).unwrap().send(DroneCommand::AddSender(*dst_id, sender)).unwrap();
    }

    //alter the pdr of a drone
    fn set_packet_drop_rate(&mut self, id:&NodeId, new_pdr: f32){
        if !self.is_drone(id){
            return
        }
        self.send_command.get(id).unwrap().send(DroneCommand::SetPacketDropRate(new_pdr)).unwrap();
    }

    //TODO command handler
    // fn command_handler(&mut self, event: DroneEvent){
    //     match event{
    //         PacketSent(packet) =>;
    //         PackedDropped(packet) =>;
    //         _ => ;
    //     }
    // }

    fn is_drone (&mut self, id: &NodeId) -> bool {
        self.drones.contains_key(id)
    }

    fn check_network(){
        //The network graph must remain connected.
        // Each client must remain connected to at least one and at most two drones.
        // Each server must remain connected to at least two drones.
    }

}

#[cfg(test)]
mod tests {

}
