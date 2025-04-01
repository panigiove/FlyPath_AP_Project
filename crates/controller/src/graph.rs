use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use crossbeam_channel::select_biased;
use eframe::{run_native, App, CreationContext};
use egui::{Context, containers::Window};
use egui::Shape::Vec;
use egui_graphs::{Graph, GraphView, LayoutRandom, LayoutStateRandom, Node};
use petgraph::{
    stable_graph::{StableGraph, StableUnGraph},
    Undirected,
};
use petgraph::graph::NodeIndex;
use petgraph::graphmap::Nodes;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::drone::Drone;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use crate::utility::{GraphAction, NodeType};

pub struct WindowGraph {
    pub graph: Graph<(), (), Undirected>,
    pub connections: HashMap<NodeId, Vec<NodeId>>,
    pub node_type: HashMap<NodeId, NodeType>,
    pub nodes: HashMap<NodeId, NodeIndex>,

    //TODO forse invece che fare ti mandare tutta la struttura facciamo un tipo di messaggio per le modifiche?
    pub receiver_connections: Receiver<HashMap<NodeId, Vec<NodeId>>>, //TODO mettere il sender in lib -> manda ogni volta la situa del grafo
    pub receiver_node_type: Receiver<HashMap<NodeId, NodeType>>, //TODO stesso di sopra
    pub receiver_updates: Receiver<GraphAction>, //riceve dal controller gli aggiornamenti sulla struttura del grafo
    pub sender_node_clicked: Sender<NodeId>, // dice alla finestra dei pulsanti qual è stato il nodo ad essere premuto

}

impl App for WindowGraph {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
        Window::new("graph").show(ctx, |ui| {
            ui.add(&mut GraphView::<
                _,
                _,
                _,
                _,
                _,
                _,
                LayoutStateRandom,
                LayoutRandom,
            >::new(&mut self.graph));
        });
    }
}

impl WindowGraph {
    fn new(_: &CreationContext<'_>,
           connections: HashMap<NodeId, Vec<NodeId>>,
           node_type: HashMap<NodeId, NodeType>,
           nodes: HashMap<NodeId, NodeIndex>,
           receiver_connections: Receiver<HashMap<NodeId, Vec<NodeId>>>,
           receiver_node_type: Receiver<HashMap<NodeId, NodeType>>,
           receiver_updates: Receiver<GraphAction>,
           sender_node_clicked: Sender<NodeId>) -> Self {
        let mut g = StableUnGraph::default();
        Self { graph: Graph::from(&g), connections, node_type, nodes, receiver_connections, receiver_node_type, receiver_updates: receiver_updates, sender_node_clicked}
    }

    pub fn run(&mut self){
        self.inizialize_graph();

        loop{
            select_biased!{
                recv(self.receiver_updates) -> command =>{
                    if let Ok(command) = command{
                        self.updates_handler(command); //aggiornamenti da parte del controller sulla struttura del grafo
                    }
                }
                default => {
                    // for (_, reciver) in self.receive_event.clone(){
                    //     if let Ok(event) = reciver.try_recv(){
                    //         self.event_handler(event);
                    //     }
                    // }
                }
            }
            // if let Ok(command) = self.ui_receiver.try_recv() {
            //     self.ui_command_handler(command);
            //     continue;
            // }
            //
            // for (_, i) in self.receive_event.clone() {
            //     if let Ok(event) = i.try_recv() {
            //         self.event_handler(event);
            //     }
            // }
            //
            // // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }

    fn inizialize_graph(&mut self) {
        for (id, t) in self.node_type{
            self.nodes.insert(id, self.graph.add_node(()));
            //TODO aggiungere discorso immagini
        }

        //let a = g.add_node(());
        // let b = g.add_node(());
        // let c = g.add_node(());

        for (key, value) in self.connections{
            for v in value{
                if let Some(a) = self.nodes.get(&key) {
                    if let Some(b) = self.nodes.get(&v) {
                        self.graph.add_edge(*a, *b, ());
                    }
                }
            }
        }

        // g.add_edge(a, b, ());
        // g.add_edge(a, b, ());
        // g.add_edge(b, c, ());
        // g.add_edge(c, a, ());
    }

    fn updates_handler(&mut self, update: GraphAction){
        match update{
            GraphAction::AddNode(id, node_type)=> {
                self.nodes.insert(id, self.graph.add_node(()));
                let mut v: Vec<NodeId> = vec![];
                self.connections.insert(id, v);
                //TODO sistemare discorso di node_type
                //TODO aggiungere altro?
                //TODO quando aggiungiamo un nodo dovremmo sapere anche il tipo
            },
            GraphAction::RemoveNode(id)=> {
                if let Some (node) = self.nodes.remove(&id){
                    //TODO messaggio che il nodo è stato rimosso? forse più dal controller
                    self.graph.remove_node(node);
                    if let r = self.connections.remove(&id){};
                    for connessioni in self.connections.values_mut(){ //let's delete the node from the connections of other nodes
                        connessioni.retain(|&x| x != id);
                    }
                }
                //TODO rimuovi anche da connections
            },
            GraphAction::AddEdge(id1, id2)=> {
                if let Some(node1) = self.nodes.get(&id1){
                    if let Some(node2) = self.nodes.get(&id2){
                        self.graph.add_edge(*node1, *node2, ());
                        if let Some(connections) = self.connections.get_mut(&id1){
                            connections.push(id2)
                        }
                        if let Some(connections) = self.connections.get_mut(&id2){
                            connections.push(id1)
                        }
                    }
                }
            },
            GraphAction::RemoveEdge(id1, id2)=> {
                if let Some(node1) = self.nodes.get(&id1){
                    if let Some(node2) = self.nodes.get(&id2){
                        self.graph.remove_edges_between(*node1, *node2);
                        if let Some(connections) = self.connections.get_mut(&id1){
                            connections.retain(|&x| x != id2)
                        }
                        if let Some(connections) = self.connections.get_mut(&id2){
                            connections.retain(|&x| x != id1)
                        }
                    }
                }
            },
        }
    }
}
