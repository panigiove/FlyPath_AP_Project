use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{TextureId, Pos2, Vec2};
use petgraph::stable_graph::NodeIndex;
use wg_2024::network::NodeId;
use client::ui::UiState;
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType};
use client::ui::ClientState;

// Importa i componenti custom dal modulo graph_components
use crate::view::graph_components::*;

pub struct GraphApp {
    // Dati del grafo (per la logica)
    pub node_id_to_index: HashMap<NodeId, NodeIndex<u32>>,
    pub index_to_node_id: HashMap<NodeIndex<u32>, NodeId>,

    // Componenti visuali dal modulo graph_components
    pub nodes: HashMap<NodeId, GraphNode>,
    pub edges: Vec<GraphEdge>,

    // Stato di selezione usando GraphSelectionState
    pub selection_state: GraphSelectionState,

    // Texture per i nodi
    pub node_textures: HashMap<NodeType, TextureId>,
    pub client_ui_state: Arc<Mutex<UiState>>,

    // CHANNELS
    pub receiver_updates: Receiver<GraphAction>,
    pub sender_node_clicked: Sender<NodeId>,
    pub sender_edge_clicked: Sender<(NodeId, NodeId)>,
    pub reciver_buttom_messages: Receiver<ButtonsMessages>,
    pub sender_message_type: Sender<MessageType>,
    pub client_state_receiver: Receiver<(NodeId, ClientState)>,
}

impl GraphApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        connection: HashMap<NodeId, Vec<NodeId>>,
        node_types: HashMap<NodeId, NodeType>,
        receiver_updates: Receiver<GraphAction>,
        sender_node_clicked: Sender<NodeId>,
        sender_edge_clicked: Sender<(NodeId, NodeId)>,
        reciver_buttom_messages: Receiver<ButtonsMessages>,
        sender_message_type: Sender<MessageType>,
        client_ui_state: Arc<Mutex<UiState>>,
        client_state_receiver: Receiver<(NodeId, ClientState)>
    ) -> Self {
        // Carica le texture PNG
        let node_textures = Self::load_node_textures(cc);

        // Crea mapping per gli indici
        let mut node_id_to_index = HashMap::new();
        let mut index_to_node_id = HashMap::new();
        let mut nodes = HashMap::new();
        let mut edges = Vec::new();

        // Crea nodi con posizioni automatiche in cerchio
        let center = Pos2::new(400.0, 300.0);
        let radius = 150.0;
        let node_count = node_types.len();

        for (i, (&node_id, &node_type)) in node_types.iter().enumerate() {
            // Calcola posizione in cerchio
            let angle = (i as f32 / node_count as f32) * 2.0 * std::f32::consts::PI;
            let position = Pos2::new(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin()
            );

            // Aggiungi ai mapping
            let node_index = NodeIndex::new(i);
            node_id_to_index.insert(node_id, node_index);
            index_to_node_id.insert(node_index, node_id);

            // Crea nodo usando il factory dal modulo graph_components
            let texture_id = node_textures.get(&node_type).copied();
            let node = if let Some(texture_id) = texture_id {
                println!("🖼️ Creando nodo {} con texture_id: {:?}", node_id, texture_id);
                create_node_with_texture(node_id, node_type, position, texture_id)
            } else {
                println!("⚠️ Creando nodo {} SENZA texture", node_id);
                create_node(node_id, node_type, position)
            };

            nodes.insert(node_id, node);
        }

        // Crea edge
        for (&node_id, connections) in &connection {
            for &target_id in connections {
                // Evita duplicati per grafo non-diretto
                if node_id < target_id {
                    edges.push(create_edge(node_id, target_id));
                }
            }
        }

        Self {
            node_id_to_index,
            index_to_node_id,
            nodes,
            edges,
            node_textures,
            client_ui_state,
            selection_state: GraphSelectionState::new(),
            receiver_updates,
            sender_node_clicked,
            sender_edge_clicked,
            reciver_buttom_messages,
            sender_message_type,
            client_state_receiver,
        }
    }

    fn load_node_textures(cc: &eframe::CreationContext<'_>) -> HashMap<NodeType, TextureId> {
        let mut node_textures = HashMap::new();

        println!("🔍 Tentativo caricamento texture...");

        let possible_paths = [
            ("controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("assets", vec!["client.png", "drone.png", "server.png"]),
            ("crates/controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("./controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
        ];

        let node_types_vec = [NodeType::Client, NodeType::Drone, NodeType::Server];
        let image_names = ["client.png", "drone.png", "server.png"];

        for (i, &node_type) in node_types_vec.iter().enumerate() {
            let image_name = image_names[i];
            let mut texture_id: Option<TextureId> = None;

            for (base_path, _files) in &possible_paths {
                let full_path = format!("{}/{}", base_path, image_name);
                if std::path::Path::new(&full_path).exists() {
                    println!("✅ Trovato {} in: {}", image_name, full_path);
                    // load_texture_from_file restituisce Option<TextureId>
                    if let Some(loaded_texture) = load_texture_from_file(cc, &full_path) {
                        texture_id = Some(loaded_texture);
                        break;
                    }
                }
            }

            let final_texture_id = match texture_id {
                Some(id) => {
                    println!("✅ Texture caricata per {:?} con id: {:?}", node_type, id);
                    id
                },
                None => {
                    println!("❌ Nessuna immagine trovata per {:?}", node_type);
                    println!("   Uso texture fallback");
                    create_fallback_texture(cc, node_type)
                }
            };

            node_textures.insert(node_type, final_texture_id);
        }

        println!("✅ Texture caricate completate: {} texture in totale", node_textures.len());
        node_textures
    }

    // Sincronizza lo stato di selezione con i nodi/edge
    fn sync_selection_state(&mut self) {
        // Sincronizza nodi
        for node in self.nodes.values_mut() {
            node.set_selected(self.selection_state.is_node_selected(node.id));
        }

        // Sincronizza edge
        for edge in &mut self.edges {
            edge.set_selected(self.selection_state.is_edge_selected(edge.from_id, edge.to_id));
        }
    }

    pub fn graph_action_handler(&mut self, action: GraphAction) {
        let result = match action {
            GraphAction::AddNode(id, node_type) => self.add_node(id, node_type),
            GraphAction::RemoveNode(id) => self.remove_node(id),
            GraphAction::AddEdge(id1, id2) => self.add_edge(id1, id2),
            GraphAction::RemoveEdge(id1, id2) => self.remove_edge(id1, id2),
        };

        match result {
            Ok(()) => {
                println!("GraphAction eseguita con successo");
            }
            Err(e) => {
                println!("Errore nell'esecuzione GraphAction: {}", e);
            }
        }
    }

    pub fn button_messages_handler(&mut self, message: ButtonsMessages) {
        match message {
            ButtonsMessages::DeselectNode(id) => {
                self.selection_state.deselect_node(id);
                self.sync_selection_state();
            }
            ButtonsMessages::MultipleSelectionAllowed => {
                // Già implementato - GraphSelectionState supporta fino a 2 nodi
            }
            ButtonsMessages::UpdateSelection(node1, node2) => {
                self.selection_state.clear_all();
                if let Some(node_id) = node1 {
                    self.selection_state.select_node(node_id);
                }
                if let Some(node_id) = node2 {
                    self.selection_state.select_node(node_id);
                }
                self.sync_selection_state();
            }
            ButtonsMessages::ClearAllSelections => {
                self.selection_state.clear_all();
                self.sync_selection_state();
            }
        }
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        if self.nodes.contains_key(&new_node_id) {
            return Err(format!("Node with ID {} already exists", new_node_id));
        }

        // Gestione client state
        if node_type == NodeType::Client {
            if let Ok((id, client_state)) = self.client_state_receiver.try_recv() {
                match self.client_ui_state.lock() {
                    Ok(mut state) => {
                        state.add_client(id, client_state);
                    }
                    Err(_) => {
                        eprintln!("Error: Mutex is poisoned");
                    }
                }
            }
        }

        // Calcola posizione per il nuovo nodo
        let position = Pos2::new(
            300.0 + (new_node_id as f32 * 50.0) % 400.0,
            200.0 + (new_node_id as f32 * 30.0) % 300.0
        );

        // Crea nodo usando il factory dal modulo graph_components
        let texture_id = self.node_textures.get(&node_type).copied();
        let node = if let Some(texture_id) = texture_id {
            println!("🖼️ Creando nuovo nodo {} con texture_id: {:?}", new_node_id, texture_id);
            create_node_with_texture(new_node_id, node_type, position, texture_id)
        } else {
            println!("⚠️ Creando nuovo nodo {} SENZA texture", new_node_id);
            create_node(new_node_id, node_type, position)
        };

        self.nodes.insert(new_node_id, node);

        // Aggiungi ai mapping
        let node_index = NodeIndex::new(self.node_id_to_index.len());
        self.node_id_to_index.insert(new_node_id, node_index);
        self.index_to_node_id.insert(node_index, new_node_id);

        println!("Added node: ID={}, Type={:?}", new_node_id, node_type);
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), String> {
        if !self.nodes.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        // Rimuovi il nodo
        self.nodes.remove(&node_id);

        // Rimuovi dai mapping
        if let Some(node_index) = self.node_id_to_index.remove(&node_id) {
            self.index_to_node_id.remove(&node_index);
        }

        // Rimuovi tutti gli edge connessi
        self.edges.retain(|edge| edge.from_id != node_id && edge.to_id != node_id);

        // Deseleziona se era selezionato
        self.selection_state.deselect_node(node_id);
        self.sync_selection_state();

        println!("Removed node {}", node_id);
        Ok(())
    }

    pub fn add_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if !self.nodes.contains_key(&id1) {
            return Err(format!("Node with ID {} not found", id1));
        }
        if !self.nodes.contains_key(&id2) {
            return Err(format!("Node with ID {} not found", id2));
        }
        if id1 == id2 {
            return Err("Cannot create edge to the same node (self-loop)".to_string());
        }

        // Controlla se l'edge esiste già
        let edge_exists = self.edges.iter().any(|edge|
            (edge.from_id == id1 && edge.to_id == id2) ||
                (edge.from_id == id2 && edge.to_id == id1)
        );

        if edge_exists {
            return Err(format!("Edge between {} and {} already exists", id1, id2));
        }

        // Aggiungi l'edge
        self.edges.push(create_edge(id1, id2));

        println!("Added edge between nodes {} and {}", id1, id2);
        Ok(())
    }

    pub fn remove_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        let edge_index = self.edges.iter().position(|edge|
            (edge.from_id == id1 && edge.to_id == id2) ||
                (edge.from_id == id2 && edge.to_id == id1)
        );

        if let Some(index) = edge_index {
            self.edges.remove(index);

            // Deseleziona se era selezionato
            self.selection_state.deselect_edge();
            self.sync_selection_state();

            println!("Removed edge between nodes {} and {}", id1, id2);
            Ok(())
        } else {
            Err(format!("Edge between {} and {} does not exist", id1, id2))
        }
    }

    fn handle_graph_click(&mut self, click_pos: Pos2) {
        let mut clicked_node: Option<NodeId> = None;
        let mut clicked_edge: Option<(NodeId, NodeId)> = None;

        // PRIORITÀ 1: Controllo nodi
        for (node_id, node) in &self.nodes {
            if node.contains_point(click_pos) {
                clicked_node = Some(*node_id);
                break;
            }
        }

        // PRIORITÀ 2: Controllo edge (solo se non ha cliccato nodi)
        if clicked_node.is_none() {
            for edge in &self.edges {
                if let (Some(from_node), Some(to_node)) = (
                    self.nodes.get(&edge.from_id),
                    self.nodes.get(&edge.to_id)
                ) {
                    if edge.contains_point(from_node.position, to_node.position, click_pos) {
                        clicked_edge = Some((edge.from_id, edge.to_id));
                        break;
                    }
                }
            }
        }

        // Aggiorna lo stato di selezione
        if let Some(node_id) = clicked_node {
            self.selection_state.toggle_node(node_id);
            self.sync_selection_state();

            // Invia evento
            if let Err(e) = self.sender_node_clicked.send(node_id) {
                println!("Errore nell'invio node_clicked: {}", e);
            }
        } else if let Some((from_id, to_id)) = clicked_edge {
            self.selection_state.toggle_edge(from_id, to_id);
            self.sync_selection_state();

            // Invia evento
            if let Err(e) = self.sender_edge_clicked.send((from_id, to_id)) {
                println!("Errore nell'invio edge_clicked: {}", e);
            }
        } else {
            // Click su sfondo = deseleziona tutto
            self.selection_state.clear_all();
            self.sync_selection_state();
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        // Gestione tasto ESC per deselezionare tutto
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selection_state.clear_all();
            self.sync_selection_state();
            println!("ESC pressed - deselezionato tutto");
        }
    }

    pub fn handle_pending_events(&mut self) {
        // Gestisce GraphAction dal backend
        if let Ok(command) = self.receiver_updates.try_recv() {
            self.graph_action_handler(command);
        }

        // Gestisce ButtonsMessages
        if let Ok(command) = self.reciver_buttom_messages.try_recv() {
            self.button_messages_handler(command);
        }
    }

    pub fn run(&mut self) {
        loop {
            self.handle_pending_events();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Gestisce eventi pendenti
        self.handle_pending_events();

        // Gestisce input da tastiera
        self.handle_keyboard_input(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Network Graph");

            // Debug: stampa tutte le texture caricate
            for (node_type, tex_id) in &self.node_textures {
                println!("   {:?} -> {:?}", node_type, tex_id);
            }

            // Area per disegnare il grafo
            let desired_size = ui.available_size();
            let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

            if ui.is_rect_visible(rect) {
                // Disegna lo sfondo
                ui.painter().rect_filled(rect, 5.0, egui::Color32::from_gray(250));

                // Debug dettagliato prima del rendering
                for (node_id, node) in &self.nodes {
                    println!("Nodo {}: tipo={:?}, texture={:?}",
                             node_id, node.node_type, node.texture_id);
                }

                // Disegna gli edge prima dei nodi
                for edge in &self.edges {
                    if let (Some(from_node), Some(to_node)) = (
                        self.nodes.get(&edge.from_id),
                        self.nodes.get(&edge.to_id)
                    ) {
                        edge.draw(ui.painter(), from_node.position, to_node.position);
                    }
                }

                // Disegna i nodi
                for node in self.nodes.values() {
                    // Debug: verifica se il nodo ha una texture
                    if let Some(tex_id) = node.texture_id {
                        
                    } else {
                        
                    }
                    node.draw(ui.painter());
                }
            }

            // Gestisce i click
            if response.clicked() {
                if let Some(click_pos) = response.interact_pointer_pos() {
                    self.handle_graph_click(click_pos);
                }
            }

            ui.separator();

            // Info panel
            ui.horizontal(|ui| {
                ui.label("🖱️ Click: seleziona/deseleziona nodi ed edge");
                ui.separator();
                ui.label("⌨️ ESC: deseleziona tutto");
            });

            ui.horizontal(|ui| {
                ui.label("🎯 Massimo 2 nodi o 1 edge alla volta");
                ui.separator();

                // Mostra selezione corrente
                let selected_nodes = self.selection_state.get_selected_nodes();
                if !selected_nodes.is_empty() {
                    ui.label(format!("✅ Nodi selezionati: {:?}", selected_nodes));
                } else if let Some((from, to)) = self.selection_state.get_selected_edge() {
                    ui.label(format!("✅ Edge selezionato: {} ↔ {}", from, to));
                } else {
                    ui.label("⚪ Nessuna selezione");
                }
            });

            // Statistiche
            ui.horizontal(|ui| {
                ui.label(format!("📊 Nodi: {}", self.nodes.len()));
                ui.separator();
                ui.label(format!("📊 Edge: {}", self.edges.len()));
            });
        });
    }
}