use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{Color32, TextureId, Pos2, Vec2, Rect};
use petgraph::{Undirected, stable_graph::{NodeIndex, StableGraph}};
use wg_2024::network::NodeId;
use client::ui::UiState;
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType};
use client::ui::ClientState;
use crate::NodeType::Client;

type NodePayload = (NodeId, NodeType);

// Struttura semplificata per i nodi visuali
#[derive(Clone)]
pub struct VisualNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub position: Pos2,
    pub size: Vec2,
    pub selected: bool,
}

impl VisualNode {
    pub fn new(id: NodeId, node_type: NodeType, position: Pos2) -> Self {
        Self {
            id,
            node_type,
            position,
            size: Vec2::new(60.0, 60.0),
            selected: false,
        }
    }

    pub fn contains_point(&self, point: Pos2) -> bool {
        let half_size = self.size / 2.0;
        let min_pos = self.position - half_size;
        let max_pos = self.position + half_size;

        point.x >= min_pos.x && point.x <= max_pos.x &&
            point.y >= min_pos.y && point.y <= max_pos.y
    }

    pub fn draw(&self, painter: &egui::Painter, texture_id: Option<TextureId>) {
        let half_size = self.size / 2.0;
        let min_pos = self.position - half_size;
        let max_pos = self.position + half_size;
        let rect = Rect::from_min_max(min_pos, max_pos);

        // Aureola se selezionato
        if self.selected {
            let expanded_rect = Rect::from_min_max(
                min_pos - Vec2::new(4.0, 4.0),
                max_pos + Vec2::new(4.0, 4.0)
            );
            painter.rect_filled(expanded_rect, 8.0, Color32::from_rgba_unmultiplied(255, 215, 0, 100));
        }

        // Disegna texture o colore di fallback
        if let Some(texture_id) = texture_id {
            painter.image(
                texture_id,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE
            );
        } else {
            // Colore di fallback basato sul tipo
            let color = match self.node_type {
                NodeType::Client => Color32::from_rgb(100, 200, 100),
                NodeType::Drone => Color32::from_rgb(100, 150, 255),
                NodeType::Server => Color32::from_rgb(255, 150, 100),
            };
            painter.rect_filled(rect, 6.0, color);
        }

        // Bordo
        let border_color = if self.selected {
            Color32::from_rgb(255, 215, 0)
        } else {
            Color32::BLACK
        };
        let stroke_width = if self.selected { 3.0 } else { 2.0 };
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(stroke_width, border_color));

        // Label
        let label = format!("{} {}",
                            match self.node_type {
                                NodeType::Client => "💻",
                                NodeType::Drone => "🚁",
                                NodeType::Server => "🖥️",
                            },
                            self.id
        );

        painter.text(
            Pos2::new(self.position.x, max_pos.y + 15.0),
            egui::Align2::CENTER_CENTER,
            &label,
            egui::FontId::default(),
            if self.selected { Color32::from_rgb(255, 215, 0) } else { Color32::BLACK }
        );
    }
}

#[derive(Clone)]
pub struct VisualEdge {
    pub from_id: NodeId,
    pub to_id: NodeId,
    pub selected: bool,
}

impl VisualEdge {
    pub fn new(from_id: NodeId, to_id: NodeId) -> Self {
        Self {
            from_id,
            to_id,
            selected: false,
        }
    }

    pub fn draw(&self, painter: &egui::Painter, from_pos: Pos2, to_pos: Pos2) {
        let color = if self.selected {
            Color32::from_rgb(255, 215, 0)
        } else {
            Color32::from_rgb(100, 100, 100)
        };

        let width = if self.selected { 3.0 } else { 2.0 };

        painter.line_segment(
            [from_pos, to_pos],
            egui::Stroke::new(width, color)
        );
    }

    // Hit detection per edge
    pub fn contains_point(&self, from_pos: Pos2, to_pos: Pos2, point: Pos2) -> bool {
        // Calcola distanza dal punto alla linea
        let line_vec = to_pos - from_pos;
        let point_vec = point - from_pos;

        if line_vec.length_sq() < f32::EPSILON {
            return false;
        }

        let t = (point_vec.dot(line_vec) / line_vec.length_sq()).clamp(0.0, 1.0);
        let projection = from_pos + t * line_vec;
        let distance = (point - projection).length();

        let tolerance = if self.selected { 8.0 } else { 5.0 };
        distance < tolerance
    }
}

pub struct GraphApp {
    // Dati del grafo (per la logica)
    pub node_id_to_index: HashMap<NodeId, NodeIndex<u32>>,
    pub index_to_node_id: HashMap<NodeIndex<u32>, NodeId>,
    pub edges: Vec<(NodeId, NodeId)>,

    // Nodi visuali (per il rendering)
    pub visual_nodes: HashMap<NodeId, VisualNode>,
    pub visual_edges: Vec<VisualEdge>,

    // Texture per i nodi
    pub node_textures: HashMap<NodeType, TextureId>,
    pub client_ui_state: Arc<Mutex<UiState>>,

    // Stato di selezione avanzato
    pub selected_nodes: Vec<NodeId>,           // Massimo 2 nodi (FIFO)
    pub selected_edge: Option<(NodeId, NodeId)>, // Edge selezionato (mutuamente esclusivo con nodi)

    // CHANNELS (mantenuti per compatibilità)
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
        let mut visual_nodes = HashMap::new();
        let mut edges = Vec::new();
        let mut visual_edges = Vec::new();

        // Crea nodi visuali con posizioni automatiche in cerchio
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

            // Crea nodo visuale
            let visual_node = VisualNode::new(node_id, node_type, position);
            visual_nodes.insert(node_id, visual_node);
        }

        // Crea edge
        for (&node_id, connections) in &connection {
            for &target_id in connections {
                // Evita duplicati per grafo non-diretto
                if node_id < target_id {
                    edges.push((node_id, target_id));
                    visual_edges.push(VisualEdge::new(node_id, target_id));
                }
            }
        }

        Self {
            node_id_to_index,
            index_to_node_id,
            edges,
            visual_nodes,
            visual_edges,
            node_textures,
            client_ui_state,
            selected_nodes: Vec::new(),
            selected_edge: None,
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
            let mut found_texture_id = None;

            for (base_path, _files) in &possible_paths {
                let full_path = format!("{}/{}", base_path, image_name);
                if std::path::Path::new(&full_path).exists() {
                    println!("✅ Trovato {} in: {}", image_name, full_path);
                    found_texture_id = Some(load_texture_from_file(cc, &full_path));
                    break;
                }
            }

            let final_texture_id = match found_texture_id {
                Some(id) => {
                    println!("✅ Texture caricata per {:?}", node_type);
                    id
                },
                None => {
                    println!("❌ Nessuna immagine trovata per {:?}", node_type);
                    println!("   Uso texture fallback distintiva");
                    create_distinctive_texture(cc, node_type)
                }
            };

            node_textures.insert(node_type, final_texture_id);
        }

        println!("✅ Texture caricate completate");
        node_textures
    }

    pub fn rebuild_visual_graph(&mut self) {
        println!("Graph updated with {} nodes and {} total connections",
                 self.visual_nodes.len(),
                 self.visual_edges.len());
    }

    pub fn graph_action_handler(&mut self, action: GraphAction) {
        let result = match action {
            GraphAction::AddNode(id, node_type) => {
                self.add_node(id, node_type)
            }
            GraphAction::RemoveNode(id) => {
                self.remove_node(id)
            }
            GraphAction::AddEdge(id1, id2) => {
                self.add_edge(id1, id2)
            }
            GraphAction::RemoveEdge(id1, id2) => {
                self.remove_edge(id1, id2)
            }
        };

        match result {
            Ok(()) => {
                self.rebuild_visual_graph();
            }
            Err(e) => {
                println!("Errore nell'esecuzione GraphAction: {}", e);
            }
        }
    }

    pub fn run(&mut self) {
        loop {
            if let Ok(command) = self.receiver_updates.try_recv() {
                self.graph_action_handler(command);
            }

            if let Ok(command) = self.reciver_buttom_messages.try_recv() {
                self.button_messages_handler(command);
            }

            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn button_messages_handler(&mut self, message: ButtonsMessages) {
        match message {
            ButtonsMessages::DeselectNode(id) => {
                self.deselect_node(id);
            }
            ButtonsMessages::MultipleSelectionAllowed => {
                // Già implementato - supporta fino a 2 nodi
            }
            ButtonsMessages::UpdateSelection(node1, node2) => {
                self.clear_all_selections();
                if let Some(node_id) = node1 {
                    self.select_node(node_id);
                }
                if let Some(node_id) = node2 {
                    self.select_node(node_id);
                }
            }
            ButtonsMessages::ClearAllSelections => {
                self.clear_all_selections();
            }
        }
    }

    fn clear_all_selections(&mut self) {
        // Deseleziona tutti i nodi
        for node in self.visual_nodes.values_mut() {
            node.selected = false;
        }
        // Deseleziona tutti gli edge
        for edge in &mut self.visual_edges {
            edge.selected = false;
        }
        self.selected_nodes.clear();
        self.selected_edge = None;
    }

    fn select_node(&mut self, node_id: NodeId) {
        // Deseleziona eventuali edge (mutuamente esclusivo)
        self.deselect_all_edges();

        if let Some(node) = self.visual_nodes.get_mut(&node_id) {
            node.selected = true;

            // Gestisce la coda FIFO di massimo 2 nodi
            if !self.selected_nodes.contains(&node_id) {
                self.selected_nodes.push(node_id);

                // Se supera il limite di 2, rimuovi il più vecchio
                if self.selected_nodes.len() > 2 {
                    let oldest_node = self.selected_nodes.remove(0);
                    if let Some(old_node) = self.visual_nodes.get_mut(&oldest_node) {
                        old_node.selected = false;
                    }
                }
            }
        }
    }

    fn deselect_node(&mut self, node_id: NodeId) {
        if let Some(node) = self.visual_nodes.get_mut(&node_id) {
            node.selected = false;
        }
        self.selected_nodes.retain(|&id| id != node_id);
    }

    fn toggle_node_selection(&mut self, node_id: NodeId) {
        if self.selected_nodes.contains(&node_id) {
            self.deselect_node(node_id);
        } else {
            self.select_node(node_id);
        }
    }

    fn select_edge(&mut self, edge_index: usize) {
        // Deseleziona tutti i nodi (mutuamente esclusivo)
        self.deselect_all_nodes();

        if let Some(edge) = self.visual_edges.get_mut(edge_index) {
            edge.selected = true;
            self.selected_edge = Some((edge.from_id, edge.to_id));
        }
    }

    fn deselect_edge(&mut self, edge_index: usize) {
        if let Some(edge) = self.visual_edges.get_mut(edge_index) {
            edge.selected = false;
        }
        self.selected_edge = None;
    }

    fn toggle_edge_selection(&mut self, edge_index: usize) {
        if let Some(edge) = self.visual_edges.get(edge_index) {
            if edge.selected {
                self.deselect_edge(edge_index);
            } else {
                self.select_edge(edge_index);
            }
        }
    }

    fn deselect_all_nodes(&mut self) {
        for node in self.visual_nodes.values_mut() {
            node.selected = false;
        }
        self.selected_nodes.clear();
    }

    fn deselect_all_edges(&mut self) {
        for edge in &mut self.visual_edges {
            edge.selected = false;
        }
        self.selected_edge = None;
    }

    fn get_next_node_id(&self) -> NodeId {
        let mut max_id = 0;
        for &node_id in self.visual_nodes.keys() {
            if node_id > max_id {
                max_id = node_id;
            }
        }
        max_id + 1
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        if self.visual_nodes.contains_key(&new_node_id) {
            return Err(format!("Node with ID {} already exists", new_node_id));
        }

        // Gestione client state (mantenuta dal codice originale)
        if node_type == Client {
            if let Ok(_command) = self.client_state_receiver.try_recv() {
                match self.client_ui_state.lock() {
                    Ok(mut state) => {
                        if let Ok((id, client_state)) = self.client_state_receiver.try_recv() {
                            state.add_client(id, client_state);
                        }
                    }
                    Err(_poisoned) => {
                        eprintln!("Error: Mutex is poisoned");
                    }
                }
            }
        }

        // Calcola posizione per il nuovo nodo (posizionamento casuale migliorabile)
        let position = Pos2::new(
            300.0 + (new_node_id as f32 * 50.0) % 400.0,
            200.0 + (new_node_id as f32 * 30.0) % 300.0
        );

        // Crea nodo visuale
        let visual_node = VisualNode::new(new_node_id, node_type, position);
        self.visual_nodes.insert(new_node_id, visual_node);

        // Aggiungi ai mapping
        let node_index = NodeIndex::new(self.node_id_to_index.len());
        self.node_id_to_index.insert(new_node_id, node_index);
        self.index_to_node_id.insert(node_index, new_node_id);

        println!("Added node: ID={}, Type={:?}", new_node_id, node_type);
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), String> {
        if !self.visual_nodes.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        // Rimuovi il nodo visuale
        self.visual_nodes.remove(&node_id);

        // Rimuovi dai mapping
        if let Some(node_index) = self.node_id_to_index.remove(&node_id) {
            self.index_to_node_id.remove(&node_index);
        }

        // Rimuovi tutti gli edge connessi a questo nodo
        self.edges.retain(|(from, to)| *from != node_id && *to != node_id);
        self.visual_edges.retain(|edge| edge.from_id != node_id && edge.to_id != node_id);

        // Deseleziona se era selezionato
        self.deselect_node(node_id);

        println!("Removed node {}", node_id);
        Ok(())
    }

    pub fn add_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if !self.visual_nodes.contains_key(&id1) {
            return Err(format!("Node with ID {} not found", id1));
        }
        if !self.visual_nodes.contains_key(&id2) {
            return Err(format!("Node with ID {} not found", id2));
        }
        if id1 == id2 {
            return Err("Cannot create edge to the same node (self-loop)".to_string());
        }

        // Controlla se l'edge esiste già
        let edge_exists = self.edges.iter().any(|(from, to)|
            (*from == id1 && *to == id2) || (*from == id2 && *to == id1)
        );

        if edge_exists {
            return Err(format!("Edge between {} and {} already exists", id1, id2));
        }

        // Aggiungi l'edge (normalizza sempre con il minor ID per primo)
        let (from, to) = if id1 < id2 { (id1, id2) } else { (id2, id1) };
        self.edges.push((from, to));
        self.visual_edges.push(VisualEdge::new(from, to));

        println!("Added edge between nodes {} and {}", id1, id2);
        Ok(())
    }

    pub fn remove_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        let edge_removed = self.edges.iter().position(|(from, to)|
            (*from == id1 && *to == id2) || (*from == id2 && *to == id1)
        );

        if let Some(index) = edge_removed {
            self.edges.remove(index);
            self.visual_edges.remove(index);

            // Deseleziona se era selezionato
            if self.selected_edge == Some((id1, id2)) || self.selected_edge == Some((id2, id1)) {
                self.selected_edge = None;
            }

            println!("Removed edge between nodes {} and {}", id1, id2);
            Ok(())
        } else {
            Err(format!("Edge between {} and {} does not exist", id1, id2))
        }
    }

    pub fn remove_all_edges(&mut self, node_id: NodeId) -> Result<usize, String> {
        if !self.visual_nodes.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        let initial_count = self.edges.len();

        // Rimuovi tutti gli edge connessi al nodo
        self.edges.retain(|(from, to)| *from != node_id && *to != node_id);
        self.visual_edges.retain(|edge| edge.from_id != node_id && edge.to_id != node_id);

        let removed_count = initial_count - self.edges.len();

        println!("Removed {} edges for node {}", removed_count, node_id);
        Ok(removed_count)
    }

    fn get_selected_nodes_info(&self) -> Vec<(NodeId, NodeType)> {
        self.selected_nodes.iter()
            .filter_map(|&node_id| {
                self.visual_nodes.get(&node_id)
                    .map(|node| (node.id, node.node_type))
            })
            .collect()
    }

    fn get_selected_edge_info(&self) -> Option<((NodeId, NodeId), (NodeType, NodeType))> {
        if let Some((id1, id2)) = self.selected_edge {
            if let (Some(node1), Some(node2)) = (
                self.visual_nodes.get(&id1),
                self.visual_nodes.get(&id2)
            ) {
                return Some(((id1, id2), (node1.node_type, node2.node_type)));
            }
        }
        None
    }

    fn get_node_connections(&self, node_id: NodeId) -> Vec<NodeId> {
        self.edges.iter()
            .filter_map(|(from, to)| {
                if *from == node_id {
                    Some(*to)
                } else if *to == node_id {
                    Some(*from)
                } else {
                    None
                }
            })
            .collect()
    }

    fn handle_graph_click(&mut self, click_pos: Pos2) {
        // Prima raccoglie gli ID senza modificare nulla (borrow immutabile)
        let mut clicked_node: Option<NodeId> = None;
        let mut clicked_edge: Option<usize> = None;

        // PRIORITÀ 1: Controllo nodi
        for (node_id, node) in &self.visual_nodes {
            if node.contains_point(click_pos) {
                clicked_node = Some(*node_id);
                break;
            }
        }

        // PRIORITÀ 2: Controllo edge (solo se non ha cliccato nodi)
        if clicked_node.is_none() {
            for (edge_index, edge) in self.visual_edges.iter().enumerate() {
                if let (Some(from_node), Some(to_node)) = (
                    self.visual_nodes.get(&edge.from_id),
                    self.visual_nodes.get(&edge.to_id)
                ) {
                    if edge.contains_point(from_node.position, to_node.position, click_pos) {
                        clicked_edge = Some(edge_index);
                        break;
                    }
                }
            }
        }

        // Ora modifica lo stato (borrow mutabile)
        if let Some(node_id) = clicked_node {
            self.toggle_node_selection(node_id);

            // Invia evento
            if let Err(e) = self.sender_node_clicked.send(node_id) {
                println!("Errore nell'invio node_clicked: {}", e);
            }
        } else if let Some(edge_index) = clicked_edge {
            self.toggle_edge_selection(edge_index);

            // Invia evento
            if let Some(edge) = self.visual_edges.get(edge_index) {
                if let Err(e) = self.sender_edge_clicked.send((edge.from_id, edge.to_id)) {
                    println!("Errore nell'invio edge_clicked: {}", e);
                }
            }
        } else {
            // Click su sfondo = deseleziona tutto
            self.clear_all_selections();
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        // Gestione tasto ESC per deselezionare tutto
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clear_all_selections();
            println!("ESC pressed - deselezionato tutto");
        }
    }
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Gestisce input da tastiera (ESC per deselezionare tutto)
        self.handle_keyboard_input(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::SidePanel::left("info_panel")
                .resizable(true)
                .default_width(250.0)
                .show_inside(ui, |ui| {
                    ui.heading("📊 Info Grafo");
                    ui.separator();

                    ui.label(format!("Nodi totali: {}", self.visual_nodes.len()));
                    ui.label(format!("Collegamenti: {}", self.visual_edges.len()));

                    ui.separator();

                    // Info nodi selezionati (massimo 2)
                    let selected_nodes = self.get_selected_nodes_info();
                    if !selected_nodes.is_empty() {
                        ui.heading("🔵 Nodi Selezionati");
                        for (i, (node_id, node_type)) in selected_nodes.iter().enumerate() {
                            ui.label(format!("Nodo {}: ID={}, Tipo={:?}", i + 1, node_id, node_type));

                            let connections = self.get_node_connections(*node_id);
                            ui.label(format!("  Connesso a: {:?}", connections));
                        }

                        if ui.button("Deseleziona Nodi").clicked() {
                            self.deselect_all_nodes();
                        }
                    }

                    // Info edge selezionato
                    if let Some(((id1, id2), (type1, type2))) = self.get_selected_edge_info() {
                        ui.separator();
                        ui.heading("🔗 Edge Selezionato");
                        ui.label(format!("Connessione: {} ↔ {}", id1, id2));
                        ui.label(format!("Tipo: {:?} ↔ {:?}", type1, type2));

                        if ui.button("Deseleziona Edge").clicked() {
                            self.deselect_all_edges();
                        }
                    }

                    if selected_nodes.is_empty() && self.get_selected_edge_info().is_none() {
                        ui.label("Nessuna selezione");
                        ui.label("Clicca su nodi o edge per selezionarli");
                    }

                    ui.separator();

                    // Regole di selezione
                    ui.label("📋 Regole di Selezione:");
                    ui.label("• Massimo 2 nodi contemporaneamente");
                    ui.label("• Click su elemento selezionato = deseleziona");
                    ui.label("• Nodi ed edge sono mutuamente esclusivi");
                    ui.label("• ESC = deseleziona tutto");

                    ui.separator();

                    // Statistiche tipi di nodi
                    if !self.visual_nodes.is_empty() {
                        ui.label("Tipi di nodi presenti:");
                        let mut type_counts = HashMap::new();
                        for node in self.visual_nodes.values() {
                            *type_counts.entry(node.node_type).or_insert(0) += 1;
                        }

                        for (node_type, count) in type_counts {
                            let icon = match node_type {
                                NodeType::Client => "💻",
                                NodeType::Drone => "🚁",
                                NodeType::Server => "🖥️",
                            };
                            ui.label(format!("{} {:?}: {}", icon, node_type, count));
                        }
                    }

                    ui.separator();
                    ui.label("🎨 Texture PNG caricate:");
                    for (node_type, _texture_id) in &self.node_textures {
                        ui.label(format!("✅ {:?}.png", node_type));
                    }
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.heading("🌐 Grafo di Rete");

                // Area per disegnare il grafo
                let desired_size = ui.available_size();
                let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

                if ui.is_rect_visible(rect) {
                    // Disegna lo sfondo
                    ui.painter().rect_filled(rect, 5.0, egui::Color32::from_gray(250));

                    // Disegna griglia di sfondo (opzionale)
                    let grid_spacing = 50.0;
                    let mut x = rect.min.x;
                    while x < rect.max.x {
                        ui.painter().vline(x, rect.min.y..=rect.max.y,
                                           egui::Stroke::new(0.5, egui::Color32::from_gray(230)));
                        x += grid_spacing;
                    }
                    let mut y = rect.min.y;
                    while y < rect.max.y {
                        ui.painter().hline(rect.min.x..=rect.max.x, y,
                                           egui::Stroke::new(0.5, egui::Color32::from_gray(230)));
                        y += grid_spacing;
                    }

                    // Disegna gli edge prima dei nodi (così i nodi appaiono sopra)
                    for edge in &self.visual_edges {
                        if let (Some(from_node), Some(to_node)) = (
                            self.visual_nodes.get(&edge.from_id),
                            self.visual_nodes.get(&edge.to_id)
                        ) {
                            edge.draw(ui.painter(), from_node.position, to_node.position);
                        }
                    }

                    // Disegna i nodi con le texture PNG
                    for node in self.visual_nodes.values() {
                        let texture_id = self.node_textures.get(&node.node_type).copied();
                        node.draw(ui.painter(), texture_id);
                    }
                }

                // Gestisce i click
                if response.clicked() {
                    if let Some(click_pos) = response.interact_pointer_pos() {
                        // Le coordinate sono già relative al widget, possiamo usarle direttamente
                        self.handle_graph_click(click_pos);
                    }
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.label("🖱️ Click: seleziona/deseleziona nodi ed edge");
                    ui.separator();
                    ui.label("⌨️ ESC: deseleziona tutto");
                });

                ui.horizontal(|ui| {
                    ui.label("🎯 Massimo 2 nodi o 1 edge alla volta");
                    ui.separator();
                    ui.label("🎨 Texture PNG per i nodi");
                });

                ui.horizontal(|ui| {
                    ui.label("💻 Client");
                    ui.separator();
                    ui.label("🚁 Drone");
                    ui.separator();
                    ui.label("🖥️ Server");
                });
            });
        });
    }
}

// *** FUNZIONI DI UTILITÀ PER LE TEXTURE (mantenute dal codice originale) ***

fn load_texture_from_file(cc: &eframe::CreationContext<'_>, path: &str) -> TextureId {
    match std::fs::read(path) {
        Ok(image_data) => {
            match image::load_from_memory(&image_data) {
                Ok(dynamic_image) => {
                    let rgba_image = dynamic_image.to_rgba8();
                    let size = [rgba_image.width() as usize, rgba_image.height() as usize];
                    let pixels: Vec<egui::Color32> = rgba_image
                        .pixels()
                        .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                        .collect();

                    let color_image = egui::ColorImage { size, pixels };

                    cc.egui_ctx.load_texture(
                        path,
                        color_image,
                        egui::TextureOptions::default(),
                    ).id()
                }
                Err(e) => {
                    println!("❌ Errore decodifica immagine {}: {}", path, e);
                    create_fallback_texture(cc, &format!("fallback_{}", path))
                }
            }
        }
        Err(e) => {
            println!("❌ Errore lettura file {}: {}", path, e);
            create_fallback_texture(cc, &format!("fallback_{}", path))
        }
    }
}

fn create_distinctive_texture(cc: &eframe::CreationContext<'_>, node_type: NodeType) -> TextureId {
    let size = 64;
    let mut pixels = Vec::new();

    for y in 0..size {
        for x in 0..size {
            let color = match node_type {
                NodeType::Client => {
                    if (x + y) % 8 < 4 {
                        Color32::from_rgb(100, 200, 100)
                    } else {
                        Color32::from_rgb(80, 160, 80)
                    }
                }
                NodeType::Drone => {
                    let center_x = size as f32 / 2.0;
                    let center_y = size as f32 / 2.0;
                    let distance = ((x as f32 - center_x).powi(2) + (y as f32 - center_y).powi(2)).sqrt();
                    if distance < size as f32 / 3.0 {
                        Color32::from_rgb(100, 150, 255)
                    } else {
                        Color32::from_rgb(80, 120, 200)
                    }
                }
                NodeType::Server => {
                    if y % 4 < 2 {
                        Color32::from_rgb(255, 150, 100)
                    } else {
                        Color32::from_rgb(200, 120, 80)
                    }
                }
            };
            pixels.push(color);
        }
    }

    let color_image = egui::ColorImage {
        size: [size, size],
        pixels,
    };

    cc.egui_ctx.load_texture(
        &format!("{:?}_pattern", node_type),
        color_image,
        egui::TextureOptions::default(),
    ).id()
}

fn create_fallback_texture(cc: &eframe::CreationContext<'_>, name: &str) -> TextureId {
    let size = 64;
    let pixels = vec![Color32::from_rgb(128, 128, 128); size * size];

    let color_image = egui::ColorImage {
        size: [size, size],
        pixels,
    };

    cc.egui_ctx.load_texture(
        name,
        color_image,
        egui::TextureOptions::default(),
    ).id()
}


impl GraphApp {
    /// Gestisce gli eventi pending senza loop infinito (per integrazione UI)
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
}