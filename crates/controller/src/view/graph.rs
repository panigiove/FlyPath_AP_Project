use std::collections::HashMap;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui::{self, Pos2, Vec2, Color32};
use egui::TextureHandle;
use egui_graphs::{DefaultEdgeShape, Graph, GraphView, Node};
use wg_2024::network::NodeId;
use crate::view::graph_components::{CustomNode, CustomEdge};
use crate::utility::{ButtonsMessages, GraphAction, NodeType};
use petgraph::Undirected;


pub struct GraphApp {
    graph: Graph<(NodeId, NodeType), (), petgraph::Undirected, usize, CustomNode, CustomEdge>,
    graph_view: GraphView<(NodeId, NodeType), (), petgraph::Undirected, usize, CustomNode, CustomEdge>,
    textures: HashMap<String, TextureHandle>,
    selected_node: Option<petgraph::graph::NodeIndex>,
    interaction_message: String,
    
    pub connection: HashMap<NodeId, Vec<NodeId>>,
    pub node_types: HashMap<NodeId, NodeType>,
    
    //CHANNELS
    pub receiver_updates: Receiver<GraphAction>, //riceve dal controller gli aggiornamenti sulla struttura del grafo
    pub sender_node_clicked: Sender<NodeId>, // tells to buttons wich node has been clicked
    pub reciver_buttom_messages: Receiver<ButtonsMessages>, //from buttons
}

impl GraphApp {
    pub fn new(connection: HashMap<NodeId, Vec<NodeId>>, node_types: HashMap<NodeId, NodeType>) -> Self {
        let mut graph = ZoomableGraph::new();

        // Crea alcuni nodi di esempio
        let drone1 = CustomNode::new(
            1,
            NodeType::Drone,
            "assets/drone_icon.png".to_string(),
            Pos2::new(-100.0, -50.0)
        ).with_size(Vec2::new(80.0, 80.0));

        let drone2 = CustomNode::new(
            2,
            NodeType::Drone,
            "assets/drone_icon.png".to_string(),
            Pos2::new(100.0, -50.0)
        ).with_size(Vec2::new(80.0, 80.0));

        let server = CustomNode::new(
            3,
            NodeType::Server,
            "assets/server_icon.png".to_string(),
            Pos2::new(0.0, 0.0)
        ).with_size(Vec2::new(100.0, 100.0))
            .with_label("Main Server".to_string());

        let client1 = CustomNode::new(
            4,
            NodeType::Client,
            "assets/client_icon.png".to_string(),
            Pos2::new(-150.0, 100.0)
        ).with_size(Vec2::new(60.0, 60.0));

        let client2 = CustomNode::new(
            5,
            NodeType::Client,
            "assets/client_icon.png".to_string(),
            Pos2::new(0.0, 150.0)
        ).with_size(Vec2::new(60.0, 60.0));

        let client3 = CustomNode::new(
            6,
            NodeType::Client,
            "assets/client_icon.png".to_string(),
            Pos2::new(150.0, 100.0)
        ).with_size(Vec2::new(60.0, 60.0));

        // Aggiungi i nodi al grafo
        graph.add_node(drone1);
        graph.add_node(drone2);
        graph.add_node(server);
        graph.add_node(client1);
        graph.add_node(client2);
        graph.add_node(client3);

        // Crea le connessioni (edges)
        // Drone -> Server
        graph.add_edge(CustomEdge::new(1, 3)
            .with_color(Color32::BLUE)
            .with_thickness(3.0));
        graph.add_edge(CustomEdge::new(2, 3)
            .with_color(Color32::BLUE)
            .with_thickness(3.0));

        // Server -> Clients
        graph.add_edge(CustomEdge::new(3, 4)
            .with_color(Color32::GREEN)
            .with_thickness(2.0));
        graph.add_edge(CustomEdge::new(3, 5)
            .with_color(Color32::GREEN)
            .with_thickness(2.0));
        graph.add_edge(CustomEdge::new(3, 6)
            .with_color(Color32::GREEN)
            .with_thickness(2.0));

        // Connessioni tra client (peer-to-peer)
        graph.add_edge(CustomEdge::new(4, 5)
            .with_color(Color32::YELLOW)
            .with_thickness(1.5));
        graph.add_edge(CustomEdge::new(5, 6)
            .with_color(Color32::YELLOW)
            .with_thickness(1.5));

        Self {graph, connection, node_types}
    }
    
    //FORSE CONNECTION E NODE TYPE SONO MEGLIO DARLI COMW PARAMENTRI QUANDO CREIAMO UN NUOVO GRAPH APP
    
    pub fn run(&mut self){
        loop {
            if let Ok(command) = self.receiver_connections.try_recv() {
                self.connection_handler(command)
            }
            if let Ok(command) = self.receiver_node_type.try_recv() {
                self.node_type_handler(command)
            }
            if let Ok(command) = self.receiver_updates.try_recv() {
                self.graph_action_handler(command)
            }
            if let Ok(command) = self.reciver_buttom_messages.try_recv() {
                self.button_messages_handler(command)
            }

            //TODO aggiungere il meccanismo per fermare loop

            // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::yield_now();
        }
    }
    
    pub fn connection_handler(&mut self, connection: HashMap<NodeId, Vec<NodeId>>){
        self.connection = connection
    }
    
    pub fn node_type_handler(&mut self, node_types: HashMap<NodeId, NodeType>){
        self.node_types = node_types
    }
    
    //TODO sistemare errori
    pub fn graph_action_handler(&mut self, action: GraphAction){
        match action{
            GraphAction::AddNode(id,node_type) => {
                self.add_node(id, node_type).unwrap()
            }
            
            GraphAction::RemoveNode(id) => {
                self.remove_node_by_id(id).unwrap()
            }
            
            GraphAction::AddEdge(id1, id2) => {
                self.add_edge_between_nodes(id1, id2).unwrap()
            }
            GraphAction::RemoveEdge(id1, id2) => {
                self.remove_edge_between_nodes(id1, id2).unwrap()
            }
        }
    }
    
    pub fn button_messages_handler(&mut self, message: ButtonsMessages){
        match message{
            ButtonsMessages::DeselectNode(id) => {
                
            }
            ButtonsMessages::MultipleSelectionAllowed => {
                
            }
        }
        
    }

    // Metodo per creare un grafo con layout circolare
    pub fn new_circular_layout() -> Self {
        let mut graph = ZoomableGraph::new();
        let center = Pos2::new(0.0, 0.0);
        let radius = 150.0;
        let num_nodes = 8;

        // Server centrale
        let server = CustomNode::new(
            0,
            NodeType::Server,
            "assets/server_icon.png".to_string(),
            center
        ).with_size(Vec2::new(120.0, 120.0))
            .with_label("Central Hub".to_string());
        graph.add_node(server);

        // Nodi disposti in cerchio
        for i in 0..num_nodes {
            let angle = (i as f32) * 2.0 * std::f32::consts::PI / (num_nodes as f32);
            let pos = Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin()
            );

            let node_type = match i % 3 {
                0 => NodeType::Drone,
                1 => NodeType::Client,
                _ => NodeType::Server,
            };

            let image_path = match node_type {
                NodeType::Drone => "assets/drone_icon.png",
                NodeType::Server => "assets/server_icon.png",
                NodeType::Client => "assets/client_icon.png",
            }.to_string();

            let node = CustomNode::new(
                i + 1,
                node_type,
                image_path,
                pos
            ).with_size(Vec2::new(70.0, 70.0));

            graph.add_node(node);

            // Connetti ogni nodo al server centrale
            graph.add_edge(CustomEdge::new(0, i + 1)
                .with_color(Color32::from_rgb(100, 150, 200))
                .with_thickness(2.0));
        }

        // Aggiungi alcune connessioni tra nodi adiacenti
        for i in 0..num_nodes {
            let next = (i + 1) % num_nodes;
            graph.add_edge(CustomEdge::new(i + 1, next + 1)
                .with_color(Color32::from_rgb(200, 100, 100))
                .with_thickness(1.0));
        }

        Self { graph }
    }

    // Metodo per creare un grafo gerarchico
    pub fn new_hierarchical() -> Self {
        let mut graph = ZoomableGraph::new();

        // Livello 1 - Root server
        let root = CustomNode::new(
            1,
            NodeType::Server,
            "assets/server_icon.png".to_string(),
            Pos2::new(0.0, -150.0)
        ).with_size(Vec2::new(100.0, 100.0))
            .with_label("Root Server".to_string());
        graph.add_node(root);

        // Livello 2 - Intermediate servers
        let positions_l2 = [
            Pos2::new(-200.0, 0.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(200.0, 0.0),
        ];

        for (i, pos) in positions_l2.iter().enumerate() {
            let server = CustomNode::new(
                (i as u32 + 2) as NodeId,
                NodeType::Server,
                "assets/server_icon.png".to_string(),
                *pos
            ).with_size(Vec2::new(80.0, 80.0))
                .with_label(format!("Server L2-{}", i + 1));

            graph.add_node(server);

            // Connetti al root
            graph.add_edge(CustomEdge::new(1, (i as u32 + 2) as NodeId)
                .with_color(Color32::RED)
                .with_thickness(3.0));
        }

        // Livello 3 - Clients e Droni
        let mut node_id = 5;
        for server_idx in 0..3 {
            let server_pos = positions_l2[server_idx];

            // Aggiungi 2 client e 1 drone per ogni server
            for j in 0..3 {
                let offset_x = (j as f32 - 1.0) * 80.0;
                let pos = Pos2::new(server_pos.x + offset_x, server_pos.y + 120.0);

                let (node_type, image_path) = if j == 2 {
                    (NodeType::Drone, "assets/drone_icon.png")
                } else {
                    (NodeType::Client, "assets/client_icon.png")
                };

                let node = CustomNode::new(
                    node_id,
                    node_type,
                    image_path.to_string(),
                    pos
                ).with_size(Vec2::new(60.0, 60.0));

                graph.add_node(node);

                // Connetti al server parent
                graph.add_edge(CustomEdge::new((server_idx as u32 + 2) as NodeId, node_id)
                    .with_color(Color32::GREEN)
                    .with_thickness(2.0));

                node_id += 1;
            }
        }

        Self { graph }
    }

    fn delete_selected_node(&mut self) {
        if let Some(selected_id) = self.graph.selected_node {
            // Rimuovi il nodo
            self.graph.nodes.retain(|n| n.id != selected_id);

            // Rimuovi tutti gli archi che coinvolgono questo nodo
            self.graph.edges.retain(|e| e.from != selected_id && e.to != selected_id);

            // Pulisci la selezione
            self.graph.selected_node = None;
        }
    }

    // Metodo per eliminare l'arco selezionato
    fn delete_selected_edge(&mut self) {
        if let Some((from_id, to_id)) = self.graph.selected_edge {
            // Rimuovi l'arco
            self.graph.edges.retain(|e|
                !((e.from == from_id && e.to == to_id) ||
                    (e.from == to_id && e.to == from_id))
            );

            // Pulisci la selezione
            self.graph.selected_edge = None;
        }
    }

    // Metodo per aggiungere un nuovo nodo nella posizione del mouse
    pub fn add_node_at_cursor(&mut self, world_pos: Pos2, node_type: NodeType) {
        let new_id = self.graph.nodes.iter().map(|n| n.id).max().unwrap_or(0) + 1;

        let image_path = match node_type {
            NodeType::Drone => "assets/drone_icon.png",
            NodeType::Server => "assets/server_icon.png",
            NodeType::Client => "assets/client_icon.png",
        }.to_string();

        let new_node = CustomNode::new(new_id, node_type, image_path, world_pos)
            .with_size(Vec2::new(70.0, 70.0));

        self.graph.add_node(new_node);
    }

    // Metodo per connettere due nodi selezionati
    pub fn connect_selected_nodes(&mut self) {
        // Questo richiederebbe di tenere traccia dell'ultimo nodo selezionato
        // e del nodo attualmente selezionato per creare una connessione
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        
        if self.graph.nodes.iter().any(|n| n.id == new_node_id) {
            return Err(format!("Node with ID {} already exists", new_node_id));
        }
        
        let random_x = (rand::random::<f32>() - 0.5) * 400.0; // Range -200 to 200
        let random_y = (rand::random::<f32>() - 0.5) * 400.0;
        let new_position = Pos2::new(random_x, random_y);
        
        let image_path = match node_type {
            NodeType::Drone => "assets/drone_icon.png",
            NodeType::Server => "assets/server_icon.png",
            NodeType::Client => "assets/client_icon.png",
        }.to_string();
        
        let size = match node_type {
            NodeType::Server => Vec2::new(100.0, 100.0),
            NodeType::Drone => Vec2::new(80.0, 80.0),
            NodeType::Client => Vec2::new(60.0, 60.0),
        };
        
        let new_node = CustomNode::new(
            new_node_id,
            node_type,
            image_path,
            new_position
        ).with_size(size);
        
        self.graph.add_node(new_node);

        Ok(())
    }
    pub fn remove_node_by_id(&mut self, node_id: NodeId) -> Result<(), String> {
        
        if !self.graph.nodes.iter().any(|n| n.id == node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }
        
        self.graph.nodes.retain(|n| n.id != node_id);
        
        self.graph.edges.retain(|e| e.from != node_id && e.to != node_id);
        
        if self.graph.selected_node == Some(node_id) {
            self.graph.selected_node = None;
        }

        if let Some((from, to)) = self.graph.selected_edge {
            if from == node_id || to == node_id {
                self.graph.selected_edge = None;
            }
        }

        Ok(())
    }

    pub fn add_edge_between_nodes(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        // Verifica che entrambi i nodi esistano
        let exists1 = self.graph.nodes.iter().any(|n| n.id == id1);
        let exists2 = self.graph.nodes.iter().any(|n| n.id == id2);

        if !exists1 {
            return Err(format!("Node with ID {} not found",id1 ));
        }
        if !exists2 {
            return Err(format!("Node with ID {} not found", id2));
        }

        // Verifica che l'edge non esista già
        let edge_exists = self.graph.edges.iter().any(|e|
            (e.from == id1 && e.to == id2) ||
                (e.from == id2 && e.to == id1)
        );

        if edge_exists {
            return Err(format!("Edge betweeen {} and {} already exists", id1, id2));
        }

        // Determina il colore dell'edge basato sui tipi di nodo
        // let from_type = self.graph.nodes.iter()
        //     .find(|n| n.id == id1)
        //     .map(|n| n.node_type)
        //     .unwrap_or(NodeType::Client);
        // 
        // let to_type = self.graph.nodes.iter()
        //     .find(|n| n.id == id2)
        //     .map(|n| n.node_type)
        //     .unwrap_or(NodeType::Client);
        // 
        // let edge_color = match (from_type, to_type) {
        //     (NodeType::Server, _) | (_, NodeType::Server) => Color32::RED,
        //     (NodeType::Drone, NodeType::Drone) => Color32::BLUE,
        //     (NodeType::Client, NodeType::Client) => Color32::YELLOW,
        //     _ => Color32::GREEN,
        // };
        
        let new_edge = CustomEdge::new(id1, id2)
            //.with_color(edge_color)
            .with_thickness(2.0);

        self.graph.add_edge(new_edge);

        Ok(())
    }

    pub fn remove_edge_between_nodes(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        let edge_index = self.graph.edges.iter().position(|e|
            (e.from == id1 && e.to == id2) ||
                (e.from == id1 && e.to == id2)
        );

        match edge_index {
            Some(_) => {
                self.graph.edges.retain(|e|
                    !((e.from == id1 && e.to == id2) ||
                        (e.from == id2 && e.to == id1))
                );
                
                if let Some((selected_from, selected_to)) = self.graph.selected_edge {
                    if (selected_from == id1 && selected_to == id2) ||
                        (selected_from == id2 && selected_to == id1) {
                        self.graph.selected_edge = None;
                    }
                }

                Ok(())
            },
            None => Err(format!("Edge between {} and {} not found", id1, id2)),
        }
    }
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Zoomable Network Graph");

            ui.horizontal(|ui| {
                if ui.button("Reset to Example Graph").clicked() {
                    *self = Self::new();
                }
                if ui.button("Circular Layout").clicked() {
                    *self = Self::new_circular_layout();
                }
                if ui.button("Hierarchical Layout").clicked() {
                    *self = Self::new_hierarchical();
                }

                // Nuovi pulsanti per la gestione delle selezioni
                if ui.button("Reset View").clicked() {
                    self.graph.reset_view();
                }
                if ui.button("Clear Selection").clicked() {
                    self.graph.clear_selection();
                }
            });

            ui.separator();

            // Instruzioni aggiornate
            ui.horizontal(|ui| {
                ui.label("Controls:");
                ui.label("• Mouse wheel: zoom");
                ui.label("• Drag nodes: move");
                ui.label("• Drag background: pan");
                ui.label("• Click node/edge: select");
                ui.label("• Click background: deselect");
            });

            ui.separator();

            // Mostra informazioni dettagliate sulle selezioni
            ui.horizontal(|ui| {
                if let Some(node) = self.graph.get_selected_node() {
                    ui.label(format!("Selected Node: {} ({})", node.label, node.id));
                    ui.label(format!("Type: {:?}", node.node_type));

                    // Potresti aggiungere qui controlli per modificare il nodo
                    if ui.button("Delete Node").clicked() {
                        self.delete_selected_node();
                    }
                } else if let Some(edge) = self.graph.get_selected_edge() {
                    ui.label(format!("Selected Edge: {} -> {}", edge.from, edge.to));
                    ui.label(format!("Thickness: {:.1}", edge.thickness));

                    // Potresti aggiungere qui controlli per modificare l'arco
                    if ui.button("Delete Edge").clicked() {
                        self.delete_selected_edge();
                    }
                } else {
                    ui.label("Nothing selected");
                }
            });

            ui.separator();

            // Disegna il grafo
            self.graph.draw_graph(ui);
        });
    }
}


// Funzione main per avviare l'applicazione
pub fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Zoomable Graph Demo",
        options,
        Box::new(|_cc| Ok(Box::new(GraphApp::new()))),
    )
}

// Funzione di utilità per creare un grafo personalizzato
pub fn create_custom_graph() -> ZoomableGraph {
    let mut graph = ZoomableGraph::new();

    // Esempio di rete mesh
    let positions = [
        Pos2::new(-100.0, -100.0),
        Pos2::new(100.0, -100.0),
        Pos2::new(-100.0, 100.0),
        Pos2::new(100.0, 100.0),
        Pos2::new(0.0, 0.0),
    ];

    let node_types = [
        NodeType::Drone,
        NodeType::Drone,
        NodeType::Client,
        NodeType::Client,
        NodeType::Server,
    ];

    // Crea i nodi
    for (i, (pos, node_type)) in positions.iter().zip(node_types.iter()).enumerate() {
        let nt = node_type.clone();
        let image_path = match node_type {
            NodeType::Drone => "assets/drone_icon.png",
            NodeType::Server => "assets/server_icon.png",
            NodeType::Client => "assets/client_icon.png",
        }.to_string();

        let node = CustomNode::new(
            (i as u32) as NodeId,
            nt,
            image_path,
            *pos
        ).with_size(Vec2::new(75.0, 75.0));

        graph.add_node(node);
    }

    // Crea connessioni mesh (tutti connessi al server centrale)
    for i in 0..4 {
        graph.add_edge(CustomEdge::new(i, 4)
            .with_color(Color32::from_rgb(50, 150, 50))
            .with_thickness(2.5));
    }

    // Connessioni peer-to-peer tra droni
    graph.add_edge(CustomEdge::new(0, 1)
        .with_color(Color32::BLUE)
        .with_thickness(2.0));

    // Connessioni peer-to-peer tra client
    graph.add_edge(CustomEdge::new(2, 3)
        .with_color(Color32::ORANGE)
        .with_thickness(2.0));

    graph
}

















































// //TODO statistica di azioni che utente fa maggiormente
// use std::vec::Vec;
// use std::collections::HashMap;
// use std::sync::mpsc::Receiver;
// use std::sync::mpsc::Sender;
// use std::time::Instant;
// use crossbeam_channel::select_biased;
// use eframe::{run_native, App, CreationContext, Frame};
// use egui::{Context, containers::Window};
// use egui_graphs::{Graph, GraphView, LayoutRandom, LayoutStateRandom, Node, DefaultGraphView, Edge, random_graph};
// use petgraph::{
//     stable_graph::{StableGraph, StableUnGraph},
//     Undirected,
// };
// 
// use egui::ColorImage;
// use image::io::Reader as ImageReader;
// use image::GenericImageView;
// 
// use egui_graphs::events::Event;
// use fdg::ForceGraph;
// use fdg::fruchterman_reingold::FruchtermanReingold;
// use petgraph::graph::NodeIndex;
// use petgraph::graphmap::Nodes;
// use wg_2024::network::{NodeId, SourceRoutingHeader};
// use wg_2024::drone::Drone;
// use wg_2024::controller::{DroneCommand, DroneEvent};
// use wg_2024::packet::Packet;
// use crate::utility::{GraphAction, NodeType, ButtonsMessages};
// 
// pub struct GraphWindow {
//     pub graph: Graph<(), (), Undirected>,
//     pub connections: HashMap<NodeId, Vec<NodeId>>,
//     pub node_type: HashMap<NodeId, NodeType>,
//     //pub nodes: HashMap<NodeId, NodeIndex>,
//     pub node_images: HashMap<NodeType, egui::TextureHandle>,
//     pub is_multiple_selection_allowed: bool, //TODO inizializzare a false
// 
//     pub max_selected: usize, // TODO inizializzare a 2
// 
// 
//     pub sim: ForceGraph<f32, 2, Node<(), ()>, Edge<(), ()>>,
//     pub force: FruchtermanReingold<f32, 2>,
// 
//     pub settings_interaction: egui_graphs::SettingsInteraction,
//     pub settings_navigation: egui_graphs::SettingsNavigation,
//     pub settings_style: egui_graphs::SettingsStyle,
// 
//     pub last_events: std::vec::Vec<String>,
//     pub selected_nodes: Vec<NodeId>,
// 
//     pub simulation_stopped: bool,
// 
//     pub fps: f32,
//     pub last_update_time: Instant,
//     pub frames_last_time_span: usize,
// 
//     pub event_publisher: crossbeam_channel::Sender<Event>,
//     pub event_consumer: crossbeam_channel::Receiver<Event>,
// 
//     pub pan: [f32; 2],
//     pub zoom: f32,
// 
//     //TODO forse invece che fare ti mandare tutta la struttura facciamo un tipo di messaggio per le modifiche?
//     pub receiver_connections: Receiver<HashMap<NodeId, Vec<NodeId>>>, //TODO mettere il sender in lib -> manda ogni volta la situa del grafo
//     pub receiver_node_type: Receiver<HashMap<NodeId, NodeType>>, //TODO stesso di sopra
//     pub receiver_updates: Receiver<GraphAction>, //riceve dal controller gli aggiornamenti sulla struttura del grafo
//     pub sender_node_clicked: Sender<NodeId>, // dice alla finestra dei pulsanti qual è stato il nodo ad essere premuto
//     pub reciver_buttom_messages: Receiver<ButtonsMessages>,
// 
// }
// 
// impl App for GraphWindow {
//     fn update(&mut self, ctx: &Context, _: &mut eframe::Frame) {
// 
// 
//         egui::CentralPanel::default().show(ctx, |ui| {
//             let settings_interaction = &egui_graphs::SettingsInteraction::new()
//                 .with_node_selection_enabled(self.settings_interaction.node_selection_enabled)
//                 .with_node_selection_multi_enabled(
//                     self.settings_interaction.node_selection_multi_enabled,
//                 )
//                 .with_node_clicking_enabled(self.settings_interaction.node_clicking_enabled)
//             let settings_navigation = &egui_graphs::SettingsNavigation::new()
//                 .with_zoom_and_pan_enabled(self.settings_navigation.zoom_and_pan_enabled)
//                 .with_fit_to_screen_enabled(self.settings_navigation.fit_to_screen_enabled)
//                 .with_zoom_speed(self.settings_navigation.zoom_speed);
//             let settings_style = &egui_graphs::SettingsStyle::new() //da togliere
//                 .with_labels_always(self.settings_style.labels_always);
//             ui.add(
//                 &mut DefaultGraphView::new(&mut self.g)
//                     .with_interactions(settings_interaction)
//                     .with_navigations(settings_navigation)
//                     .with_styles(settings_style)
//                     .with_events(&self.event_publisher),
//             );
//         });
// 
// 
//         Window::new("graph").show(ctx, |ui| {
//             ui.add(&mut GraphView::<
//                 _,
//                 _,
//                 _,
//                 _,
//                 _,
//                 _,
//                 LayoutStateRandom,
//                 LayoutRandom,
//             >::new(&mut self.graph));
//         });
//     }
// }
// 
// impl GraphWindow {
//     pub fn new(
//         ctx: &CreationContext<'_>,
//         connections: HashMap<NodeId, Vec<NodeId>>,
//         node_type: HashMap<NodeId, NodeType>,
//         nodes: HashMap<NodeId, NodeIndex>,
//         receiver_connections: Receiver<HashMap<NodeId, Vec<NodeId>>>,
//         receiver_node_type: Receiver<HashMap<NodeId, NodeType>>,
//         receiver_updates: Receiver<GraphAction>,
//         sender_node_clicked: Sender<NodeId>,
//         reciver_buttom_messages: Receiver<ButtonsMessages>,
//     ) -> Self {
//         let mut node_images = HashMap::new();
// 
//         // 🔽 Carica immagini per ogni tipo di nodo
//         let load_and_insert = |name: &str, nodetype: NodeType, map: &mut HashMap<_, _>| {
//             let bytes = include_bytes!(concat!("../assets/", name, ".png"));
//             if let Ok(image) = Self::load_image_from_bytes(bytes) {
//                 let texture = ctx.egui_ctx.load_texture(name, image, Default::default());
//                 map.insert(nodetype, texture);
//             } else {
//                 eprintln!("Errore nel caricare l'immagine per {:?}", nodetype);
//             }
//         };
// 
//         load_and_insert("drone", NodeType::Drone, &mut node_images);
//         load_and_insert("controller", NodeType::Controller, &mut node_images);
//         load_and_insert("gateway", NodeType::Gateway, &mut node_images);
// 
//         let mut g = StableUnGraph::default();
//         let graph = Graph::from(&g);
// 
//         // Costruzione base della finestra
//         Self {
//             graph,
//             connections,
//             node_type,
//             nodes,
//             node_images,
//             is_multiple_selection_allowed: false,
//             max_selected: 2,
// 
//             sim: ForceGraph::default(),
//             force: FruchtermanReingold::default(),
// 
//             settings_simulation: settings::SettingsSimulation::default(),
//             settings_graph: settings::SettingsGraph::default(),
//             settings_interaction: settings::SettingsInteraction::default(),
//             settings_navigation: settings::SettingsNavigation::default(),
//             settings_style: settings::SettingsStyle::default(),
// 
//             last_events: vec![],
//             selected_nodes: vec![],
// 
//             simulation_stopped: false,
//             fps: 0.0,
//             last_update_time: Instant::now(),
//             frames_last_time_span: 0,
// 
//             event_publisher: crossbeam_channel::unbounded().0,
//             event_consumer: crossbeam_channel::unbounded().1,
// 
//             pan: [0.0, 0.0],
//             zoom: 1.0,
// 
//             receiver_connections,
//             receiver_node_type,
//             receiver_updates,
//             sender_node_clicked,
//             reciver_buttom_messages,
//         }
//     }
// 
//     fn load_image_from_bytes(bytes: &[u8]) -> Result<ColorImage, image::ImageError> {
//         let image = image::load_from_memory(bytes)?;
//         let rgba_image = image.to_rgba8();
//         let (width, height) = rgba_image.dimensions();
//         let size = [width as usize, height as usize];
//         let pixels = rgba_image.into_vec();
//         Ok(ColorImage::from_rgba_unmultiplied(size, &pixels))
//     }
// 
//     pub fn run(&mut self){
//         if let Ok(command) = self.receiver_connections.try_recv(){
//             //qui riceviamo connessioni, lo mettiamo fuori dal loop
//             self.connections = command;
//         }
//         self.inizialize_graph();
// 
//         loop{
//            // a fine loop inseriamo la fn update?
//             select_biased!{
// 
//                 recv(self.receiver_updates) -> command =>{
//                     if let Ok(command) = command{
//                         self.updates_handler(command); //aggiornamenti da parte del controller sulla struttura del grafo
//                     }
//                 }
//                 default => {
//                     for (_, reciver) in self.receiver_updates.cloned(){
//                         if let Ok(event) = reciver.try_recv(){
//                             self.event_handler(event);
//                         }
//                     }
//                 }
//             }
//             if let Ok(command) = self.reciver_buttom_messages.try_recv(){
// 
//             }
// 
//             if let Ok(command) = self.reciver_buttom_messages.try_recv(){
// 
//             }
// 
//             if let Ok(command) = self.reciver_buttom_messages.try_recv(){
// 
//             }
//             if let Ok(command) = self.ui_receiver.try_recv() {
//                 self.ui_command_handler(command);
//                 continue;
//             }
// 
//             for (_, i) in self.receive_event.clone() {
//                 if let Ok(event) = i.try_recv() {
//                     self.event_handler(event);
//                 }
//             }
// 
//             // // Piccola pausa per evitare un ciclo troppo intenso
//             std::thread::yield_now();
//         }
//     }
// 
//     fn button_messages_handler(&mut self, buttons_messages: ButtonsMessages){
//         match buttons_messages {
//             ButtonsMessages::MultipleSelectionAllowed =>{
//                 self.is_multiple_selection_allowed = true; //when we want to add an edge between two nodes
//             }
//             ButtonsMessages::DeselectNode(id) =>{
// 
//             }
//         }
// 
//     }
//     fn inizialize_graph(&mut self) {
//         for (id, t) in &self.node_type{
//             self.nodes.insert(*id, self.graph.add_node(()));
//             //TODO aggiungere discorso immagini
//         }
// 
//         let a = self.graph.add_node(());
//         let b = self.graph.add_node(());
//         let c = self.graph.add_node(());
// 
//         for (key, value) in &self.connections{
//             for v in value{
//                 if let Some(a) = self.nodes.get(&key) {
//                     if let Some(b) = self.nodes.get(&v) {
//                         self.graph.add_edge(*a, *b, ());
//                     }
//                 }
//             }
//         }
// 
//         self.graph.add_edge(a, b, ());
//         self.graph.add_edge(a, b, ());
//         self.graph.add_edge(b, c, ());
//         self.graph.add_edge(c, a, ());
//     }
// 
//     fn updates_handler(&mut self, update: GraphAction){
//         match update{
//             GraphAction::AddNode(id, node_type)=> {
//                 self.nodes.insert(id, self.graph.add_node(()));
//                 let mut v: Vec<NodeId> = vec![];
//                 self.connections.insert(id, v);
//                 //TODO sistemare discorso di node_type
//                 //TODO aggiungere altro?
//                 //TODO quando aggiungiamo un nodo dovremmo sapere anche il tipo
//             },
//             GraphAction::RemoveNode(id)=> {
//                 if let Some (node) = self.nodes.remove(&id){
//                     //TODO messaggio che il nodo è stato rimosso? forse più dal controller
//                     self.graph.remove_node(node);
//                     if let r = self.connections.remove(&id){};
//                     for connessioni in self.connections.values_mut(){ //let's delete the node from the connections of other nodes
//                         connessioni.retain(|&x| x != id);
//                     }
//                 }
//                 //TODO rimuovi anche da connections
//             },
//             GraphAction::AddEdge(id1, id2)=> {
//                 if let Some(node1) = self.nodes.get(&id1){
//                     if let Some(node2) = self.nodes.get(&id2){
//                         self.graph.add_edge(*node1, *node2, ());
//                         if let Some(connections) = self.connections.get_mut(&id1){
//                             connections.push(id2)
//                         }
//                         if let Some(connections) = self.connections.get_mut(&id2){
//                             connections.push(id1)
//                         }
//                     }
//                 }
//             },
//             GraphAction::RemoveEdge(id1, id2)=> {
//                 if let Some(node1) = self.nodes.get(&id1){
//                     if let Some(node2) = self.nodes.get(&id2){
//                         self.graph.remove_edges_between(*node1, *node2);
//                         if let Some(connections) = self.connections.get_mut(&id1){
//                             connections.retain(|&x| x != id2)
//                         }
//                         if let Some(connections) = self.connections.get_mut(&id2){
//                             connections.retain(|&x| x != id1)
//                         }
//                     }
//                 }
//             },
//         }
//     }
// 
//     fn handle_events_user_interaction(&mut self) {
//         self.event_consumer.try_iter().for_each(|e| {
//             if self.last_events.len() > 100 {
//                 self.last_events.remove(0);
//             }
//             self.last_events.push(serde_json::to_string(&e).unwrap());
// 
//             match e {
// 
//                 //TODO gestire il discorso che selezione multipla si puo fare solo se variabile è a true -> diventa a true quando dai pulsanti riceviamo il fatto che dobbiamo selezionare più nodi
//                 //manages the displacement of the view
//                 Event::Pan(payload) => self.pan = payload.new_pan,
//                 //manages the zoom of the view
//                 Event::Zoom(payload) => self.zoom = payload.new_zoom, //TODO da sistemare c'è un limite di zoom?
//                 Event::NodeMove(payload) => {
//                     let node_id = NodeIndex::new(payload.id);
// 
//                     self.sim.node_weight_mut(node_id).unwrap().1.coords.x = payload.new_pos[0];
//                     self.sim.node_weight_mut(node_id).unwrap().1.coords.y = payload.new_pos[1];
//                 }
//                 Event::NodeClick(payload) => {
//                     let clicked_node = NodeIndex::new(payload.id);
// 
//                     //if the node has already been clicked
//                     if let Some(pos) = self.selected_nodes.iter().position(|&x| x == clicked_node){
//                         self.selected_nodes.remove(pos);
//                         if let node = self.graph.node_mut(clicked_node){
//                             node.set_selected(false); //in this way we are deselecting the node
//                         }
//                     }
//                     else{
//                         if self.is_multiple_selection_allowed{
//                             if self.selected_nodes.len() >= self.max_selected{
//                                 self.selected_nodes.remove(1); //RIMUOVIAMO IL PENUTLIMO NON IL PRIMO
//                             }
//                             self.selected_nodes.push(clicked_node);
//                         }
//                         else{
//                             self.selected_nodes.clear();
//                             self.selected_nodes.push(clicked_node);
//                         }
// 
//                     }
// 
//                     self.update_node_styles(); //TODO
// 
//                     //we're telling the buttons area which node has been clicked
//                     if let Some((id, _)) = self.nodes.iter().find(|(_, &idx)| idx == clicked_node){
//                         let _ = self.sender_node_clicked.send(*id); //TODO vedere come gestirlo
//                     }
// 
//                     //TODO fare in modo che se già esiste una connessione il nodo non è selezionabile e abbiamo una notifica nel pannello dello stato?
// 
// 
//                     let selected = self.graph.selected_nodes();
// 
//                     //if the node has already been clicked we're gonna to deselect it
//                     if selected.contains(&clicked_node) {
//                         //self.graph.
//                     }
// 
//                     for (id, index) in self.nodes{
//                         if index == clicked_node {
//                             //in this way we can update the buttons
//                             self.sender_node_clicked.send(id); //TODO sistema il caso in cui non va bene
//                             //utile per le statistiche
//                             self.last_events.push(format!(
//                                 "Nodo {} selezionato",
//                                 clicked_node.index(),
//                             ));
// 
//                         }
//                     }
//                     println!("Node clicked: {:?}", payload.id);
//                     //diciamo al coso pulsanti che è stato premuto il nodo
//                     // Aggiungi qui la logica desiderata
// 
//                 },
//                 _ => {} //da togliere perchè non facciamo i nodi che si muovono
//             }
//         });
//     }
// 
//     fn update_node_styles(&mut self){
//         //TODO!
//         for (node_idx, node) in self.graph.nodes_iter(){
//             //TODO
//         }
// 
//         for &selected_id in &self.selected_nodes{
//             if let Some(node_index) = self.nodes.get(&selected_id){
//                 if let Some(node) = self.graph.node_mut(*node_index){
//                     //TODO!
//                 }
//             }
//         }
//     }
// }