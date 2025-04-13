//TODO statistica di azioni che utente fa maggiormente
use std::collections::HashMap;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::time::Instant;
use crossbeam_channel::select_biased;
use eframe::{run_native, App, CreationContext, Frame};
use egui::{Context, containers::Window};
use egui::Shape::Vec;
use egui_graphs::{Graph, GraphView, LayoutRandom, LayoutStateRandom, Node, DefaultGraphView, Edge, random_graph};
use petgraph::{
    stable_graph::{StableGraph, StableUnGraph},
    Undirected,
};
use egui_graphs::events::Event;
use petgraph::graph::NodeIndex;
use petgraph::graphmap::Nodes;
use wg_2024::network::{NodeId, SourceRoutingHeader};
use wg_2024::drone::Drone;
use wg_2024::controller::{DroneCommand, DroneEvent};
use wg_2024::packet::Packet;
use crate::utility::{GraphAction, NodeType, ButtonsMessages};

pub struct WindowGraph {
    pub graph: Graph<(), (), Undirected>,
    pub connections: HashMap<NodeId, Vec<NodeId>>,
    pub node_type: HashMap<NodeId, NodeType>,
    pub nodes: HashMap<NodeId, NodeIndex>,
    pub node_images: HashMap<NodeType, egui::TextureHandle>,
    pub is_multiple_selection_allowed: bool, //TODO inizializzare a false

    pub max_selected: usize, // TODO inizializzare a 2

    sim: ForceGraph<f32, 2, Node<(), ()>, Edge<(), ()>>,
    force: FruchtermanReingold<f32, 2>,

    settings_simulation: settings::SettingsSimulation,

    settings_graph: settings::SettingsGraph,
    settings_interaction: settings::SettingsInteraction,
    settings_navigation: settings::SettingsNavigation,
    settings_style: settings::SettingsStyle,

    last_events: std::vec::Vec<String>,
    pub selected_nodes: Vec<NodeId>,

    simulation_stopped: bool,

    fps: f32,
    last_update_time: Instant,
    frames_last_time_span: usize,

    event_publisher: crossbeam_channel::Sender<Event>,
    event_consumer: crossbeam_channel::Receiver<Event>,

    pan: [f32; 2],
    zoom: f32,

    //TODO forse invece che fare ti mandare tutta la struttura facciamo un tipo di messaggio per le modifiche?
    pub receiver_connections: Receiver<HashMap<NodeId, Vec<NodeId>>>, //TODO mettere il sender in lib -> manda ogni volta la situa del grafo
    pub receiver_node_type: Receiver<HashMap<NodeId, NodeType>>, //TODO stesso di sopra
    pub receiver_updates: Receiver<GraphAction>, //riceve dal controller gli aggiornamenti sulla struttura del grafo
    pub sender_node_clicked: Sender<NodeId>, // dice alla finestra dei pulsanti qual è stato il nodo ad essere premuto
    pub reciver_buttom_messages: Receiver<ButtonsMessages>,

}

impl App for WindowGraph {
    fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {


        egui::CentralPanel::default().show(ctx, |ui| {
            let settings_interaction = &egui_graphs::SettingsInteraction::new()
                .with_node_selection_enabled(self.settings_interaction.node_selection_enabled)
                .with_node_selection_multi_enabled(
                    self.settings_interaction.node_selection_multi_enabled,
                )
                .with_dragging_enabled(self.settings_interaction.dragging_enabled) //da togliere
                .with_node_clicking_enabled(self.settings_interaction.node_clicking_enabled)
                .with_edge_clicking_enabled(self.settings_interaction.edge_clicking_enabled) //togliere
                .with_edge_selection_enabled(self.settings_interaction.edge_selection_enabled) //togliere
                .with_edge_selection_multi_enabled( //togliere
                    self.settings_interaction.edge_selection_multi_enabled,
                );
            let settings_navigation = &egui_graphs::SettingsNavigation::new()
                .with_zoom_and_pan_enabled(self.settings_navigation.zoom_and_pan_enabled)
                .with_fit_to_screen_enabled(self.settings_navigation.fit_to_screen_enabled)
                .with_zoom_speed(self.settings_navigation.zoom_speed);
            let settings_style = &egui_graphs::SettingsStyle::new() //da togliere
                .with_labels_always(self.settings_style.labels_always);
            ui.add(
                &mut DefaultGraphView::new(&mut self.g)
                    .with_interactions(settings_interaction)
                    .with_navigations(settings_navigation)
                    .with_styles(settings_style)
                    .with_events(&self.event_publisher),
            );
        });


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
            if let Ok(command) = self.reciver_buttom_messages.try_recv(){

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

    fn button_messages_handler(&mut self, buttons_messages: ButtonsMessages){
        match buttons_messages {
            ButtonsMessages::MultipleSelectionAllowed =>{
                self.is_multiple_selection_allowed = true; //when we want to add an edge between two nodes
            }
            ButtonsMessages::DeselectNode(id) =>{

            }
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

    fn handle_events_user_interaction(&mut self) {
        self.event_consumer.try_iter().for_each(|e| {
            if self.last_events.len() > 100 {
                self.last_events.remove(0);
            }
            self.last_events.push(serde_json::to_string(&e).unwrap());

            match e {

                //TODO gestire il discorso che selezione multipla si puo fare solo se variabile è a true -> diventa a true quando dai pulsanti riceviamo il fatto che dobbiamo selezionare più nodi
                //manages the displacement of the view
                Event::Pan(payload) => self.pan = payload.new_pan,
                //manages the zoom of the view
                Event::Zoom(payload) => self.zoom = payload.new_zoom, //TODO da sistemare c'è un limite di zoom?
                Event::NodeMove(payload) => {
                    let node_id = NodeIndex::new(payload.id);

                    self.sim.node_weight_mut(node_id).unwrap().1.coords.x = payload.new_pos[0];
                    self.sim.node_weight_mut(node_id).unwrap().1.coords.y = payload.new_pos[1];
                }
                Event::NodeClick(payload) => {
                    let clicked_node = NodeIndex::new(payload.id);

                    //if the node has already been clicked
                    if let Some(pos) = self.selected_nodes.iter().position(|&x| x == clicked_node){
                        self.selected_nodes.remove(pos);
                        if let node = self.graph.node_mut(clicked_node){
                            node.set_selected(false); //in this way we are deselecting the node
                        }
                    }
                    else{
                        if self.is_multiple_selection_allowed{
                            if self.selected_nodes.len() >= self.max_selected{
                                self.selected_nodes.remove(1); //RIMUOVIAMO IL PENUTLIMO NON IL PRIMO
                            }
                            self.selected_nodes.push(clicked_node);
                        }
                        else{
                            self.selected_nodes.clear();
                            self.selected_nodes.push(clicked_node);
                        }

                    }

                    self.update_node_styles(); //TODO

                    //we're telling the buttons area which node has been clicked
                    if let Some((id, _)) = self.nodes.iter().find(|(_, &idx)| idx == clicked_node){
                        let _ = self.sender_node_clicked.send(*id); //TODO vedere come gestirlo
                    }

                    //TODO fare in modo che se già esiste una connessione il nodo non è selezionabile e abbiamo una notifica nel pannello dello stato?


                    let selected = self.graph.selected_nodes();

                    //if the node has already been clicked we're gonna to deselect it
                    if selected.contains(&clicked_node) {
                        self.graph.
                    }

                    for (id, index) in self.nodes{
                        if index == clicked_node {
                            //in this way we can update the buttons
                            self.sender_node_clicked.send(id); //TODO sistema il caso in cui non va bene
                            //utile per le statistiche
                            self.last_events.push(format!(
                                "Nodo {} selezionato",
                                clicked_node.index(),
                            ));

                        }
                    }
                    println!("Node clicked: {:?}", payload.id);
                    //diciamo al coso pulsanti che è stato premuto il nodo
                    // Aggiungi qui la logica desiderata

                },
                _ => {} //da togliere perchè non facciamo i nodi che si muovono
            }
        });
    }

    fn update_node_styles(&mut self){
        //TODO!
        for (node_idx, node) in self.graph.nodes_iter(){
            //TODO
        }

        for &selected_id in &self.selected_nodes{
            if let Some(node) = self.graph.node_mut(selected_id){
                //TODO!
            }
        }
    }

}

