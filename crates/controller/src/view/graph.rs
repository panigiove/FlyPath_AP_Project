use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{Color32, TextureId};
use wg_2024::network::NodeId;
use client::ui::UiState;
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType};
use client::ui::ClientState;
use crate::NodeType::Client;

type NodePayload = (NodeId, NodeType);

pub struct GraphApp {
    pub selected_node_id1: Option<NodeId>,
    pub selected_node_id2: Option<NodeId>,
    pub selected_edge: Option<(NodeId, NodeId)>,
    pub node_textures: HashMap<NodeType, TextureId>,

    pub connection: HashMap<NodeId, Vec<NodeId>>,
    pub node_types: HashMap<NodeId, NodeType>,

    pub client_ui_state: Arc<Mutex<UiState>>,

    //CHANNELS
    pub receiver_updates: Receiver<GraphAction>,
    pub sender_node_clicked: Sender<NodeId>,
    pub sender_edge_clicked: Sender<(NodeId, NodeId)>,
    pub reciver_buttom_messages: Receiver<ButtonsMessages>,
    pub sender_message_type: Sender<MessageType>,
    pub client_state_receiver: Receiver<(NodeId, ClientState)>
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>,
               connection: HashMap<NodeId, Vec<NodeId>>,
               node_types: HashMap<NodeId, NodeType>,
               receiver_updates: Receiver<GraphAction>,
               sender_node_clicked: Sender<NodeId>,
               sender_edge_clicked: Sender<(NodeId, NodeId)>,
               reciver_buttom_messages: Receiver<ButtonsMessages>,
               sender_message_type: Sender<MessageType>,
               client_ui_state: Arc<Mutex<UiState>>,
               client_state_receiver: Receiver<(NodeId, ClientState)>) -> Self {

        let mut node_textures = HashMap::new();

        // CORREZIONE: Ricerca più estensiva dei file immagine
        println!("🔍 Tentativo caricamento texture...");
        println!("🔍 Directory corrente: {:?}", std::env::current_dir());

        // Lista di tutti i possibili percorsi basati su diverse strutture di progetto
        let possible_paths = [
            // Percorsi relativi comuni
            ("controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("view/assets", vec!["client.png", "drone.png", "server.png"]),
            ("assets", vec!["client.png", "drone.png", "server.png"]),
            // Percorsi con crates
            ("crates/controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
            // Percorsi assoluti dalla root del workspace
            ("./controller/src/view/assets", vec!["client.png", "drone.png", "server.png"]),
        ];

        let node_types_vec = [NodeType::Client, NodeType::Drone, NodeType::Server];
        let image_names = ["client.png", "drone.png", "server.png"];

        for (i, &node_type) in node_types_vec.iter().enumerate() {
            let image_name = image_names[i];
            let mut found_texture_id = None;

            // Prova tutti i percorsi possibili
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
                    println!("   Percorsi tentati:");
                    for (base_path, _) in &possible_paths {
                        println!("     - {}/{}", base_path, image_name);
                    }
                    println!("   Uso texture fallback distintiva");
                    create_distinctive_texture(cc, node_type)
                }
            };

            node_textures.insert(node_type, final_texture_id);
        }

        println!("✅ Texture caricate completate");

        Self {
            selected_node_id1: None,
            selected_node_id2: None,
            selected_edge: None,
            node_textures,
            client_ui_state,
            connection,
            node_types,
            receiver_updates,
            sender_node_clicked,
            sender_edge_clicked,
            reciver_buttom_messages,
            sender_message_type,
            client_state_receiver
        }
    }

    pub fn rebuild_visual_graph(&mut self) {
        println!("Graph updated with {} nodes and {} total connections",
                 self.node_types.len(),
                 self.connection.values().map(|v| v.len()).sum::<usize>() / 2);
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
            ButtonsMessages::DeselectNode(_id) => {
                self.selected_node_id1 = None;
                self.selected_node_id2 = None;
                self.selected_edge = None;
            }
            ButtonsMessages::MultipleSelectionAllowed => {
                // Abilita la selezione multipla
            }
            ButtonsMessages::UpdateSelection(node1, node2) => {
                self.selected_node_id1 = node1;
                self.selected_node_id2 = node2;
                if node1.is_some() || node2.is_some() {
                    self.selected_edge = None;
                }
            }
            ButtonsMessages::ClearAllSelections => {
                self.selected_node_id1 = None;
                self.selected_node_id2 = None;
                self.selected_edge = None;
            }
        }
    }

    fn handle_node_click(&mut self, node_id: NodeId) {
        self.selected_edge = None;

        let is_already_selected = self.selected_node_id1 == Some(node_id) ||
            self.selected_node_id2 == Some(node_id);

        if is_already_selected {
            self.selected_node_id1 = None;
            self.selected_node_id2 = None;
        } else {
            match (self.selected_node_id1, self.selected_node_id2) {
                (None, _) => self.selected_node_id1 = Some(node_id),
                (Some(_), None) => self.selected_node_id2 = Some(node_id),
                (Some(_), Some(_)) => {
                    self.selected_node_id1 = Some(node_id);
                    self.selected_node_id2 = None;
                }
            }
        }
    }

    fn handle_edge_click(&mut self, edge: (NodeId, NodeId)) {
        self.selected_node_id1 = None;
        self.selected_node_id2 = None;

        if self.selected_edge == Some(edge) || self.selected_edge == Some((edge.1, edge.0)) {
            self.selected_edge = None;
        } else {
            self.selected_edge = Some(edge);
        }

        println!("Edge cliccato: {:?}", edge);
    }

    fn get_next_node_id(&self) -> NodeId {
        let mut max_id = 0;
        for &node_id in self.node_types.keys() {
            if node_id > max_id {
                max_id = node_id;
            }
        }
        max_id + 1
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        if self.node_types.contains_key(&new_node_id) {
            return Err(format!("Node with ID {} already exists", new_node_id));
        }
        if self.node_types.get(&new_node_id) == Some(&Client){
            if let Ok(command) = self.client_state_receiver.try_recv() {
                match self.client_ui_state.lock() {
                    Ok(mut state) => {
                        if let Some((id, client_state)) = self.client_state_receiver.try_recv(){
                            state.add_client(id, client_state)
                        }
                    }
                    Err(poisoned) => {
                        eprintln!("Error: Mutex is poisoned")
                    }
                }
            }
        }

        self.node_types.insert(new_node_id, node_type);

        if !self.connection.contains_key(&new_node_id) {
            self.connection.insert(new_node_id, Vec::new());
        }

        println!("Added node: ID={}, Type={:?}", new_node_id, node_type);
        Ok(())
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), String> {
        if !self.node_types.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        let _ = self.remove_all_edges(node_id)?;

        self.node_types.remove(&node_id);
        self.connection.remove(&node_id);

        if self.selected_node_id1 == Some(node_id) {
            self.selected_node_id1 = None;
        }
        if self.selected_node_id2 == Some(node_id) {
            self.selected_node_id2 = None;
        }

        if let Some((id1, id2)) = self.selected_edge {
            if id1 == node_id || id2 == node_id {
                self.selected_edge = None;
            }
        }

        println!("Removed node {}", node_id);
        Ok(())
    }

    pub fn add_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if !self.node_types.contains_key(&id1) {
            return Err(format!("Node with ID {} not found", id1));
        }
        if !self.node_types.contains_key(&id2) {
            return Err(format!("Node with ID {} not found", id2));
        }

        if id1 == id2 {
            return Err("Cannot create edge to the same node (self-loop)".to_string());
        }

        if let Some(connections) = self.connection.get(&id1) {
            if connections.contains(&id2) {
                return Err(format!("Edge between {} and {} already exists", id1, id2));
            }
        }
        
        self.connection.entry(id1).or_insert_with(Vec::new).push(id2);
        self.connection.entry(id2).or_insert_with(Vec::new).push(id1);

        println!("Added edge between nodes {} and {}", id1, id2);
        Ok(())
    }

    pub fn remove_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if !self.node_types.contains_key(&id1) {
            return Err(format!("Node with ID {} not found", id1));
        }
        if !self.node_types.contains_key(&id2) {
            return Err(format!("Node with ID {} not found", id2));
        }

        let edge_exists = if let Some(connections) = self.connection.get(&id1) {
            connections.contains(&id2)
        } else {
            false
        };

        if !edge_exists {
            return Err(format!("Edge between {} and {} does not exist", id1, id2));
        }

        if let Some(connections) = self.connection.get_mut(&id1) {
            connections.retain(|&x| x != id2);
        }

        if let Some(connections) = self.connection.get_mut(&id2) {
            connections.retain(|&x| x != id1);
        }

        if self.selected_edge == Some((id1, id2)) || self.selected_edge == Some((id2, id1)) {
            self.selected_edge = None;
        }

        println!("Removed edge between nodes {} and {}", id1, id2);
        Ok(())
    }

    pub fn remove_all_edges(&mut self, node_id: NodeId) -> Result<usize, String> {
        if !self.node_types.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        let mut removed_count = 0;
        let connections_to_remove: Vec<NodeId> = if let Some(connections) = self.connection.get(&node_id) {
            connections.clone()
        } else {
            Vec::new()
        };

        for connected_id in connections_to_remove {
            if self.remove_edge(node_id, connected_id).is_ok() {
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }

    fn draw_custom_graph(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(600.0, 400.0),
            egui::Sense::click()
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            let get_node_color = |node_type: NodeType, is_selected: bool| -> Color32 {
                if is_selected {
                    Color32::from_rgb(255, 215, 0) // Oro quando selezionato
                } else {
                    match node_type {
                        NodeType::Client => Color32::from_rgb(100, 200, 100),
                        NodeType::Drone => Color32::from_rgb(100, 150, 255),
                        NodeType::Server => Color32::from_rgb(255, 150, 100),
                    }
                }
            };

            let get_edge_color = |edge: (NodeId, NodeId)| -> Color32 {
                if self.selected_edge == Some(edge) || self.selected_edge == Some((edge.1, edge.0)) {
                    Color32::from_rgb(255, 215, 0) // Oro quando selezionato
                } else {
                    Color32::from_rgb(150, 150, 150) // Grigio normale
                }
            };

            let center = rect.center();
            let radius = 150.0;
            let node_count = self.node_types.len();

            let mut node_positions = HashMap::new();
            let mut edge_segments = Vec::new();

            if node_count > 0 {
                for (i, (&node_id, &_node_type)) in self.node_types.iter().enumerate() {
                    let angle = (i as f32 / node_count as f32) * 2.0 * std::f32::consts::PI;
                    let pos = egui::Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin()
                    );
                    node_positions.insert(node_id, pos);
                }

                // Disegna gli edge
                for (&id1, connections) in &self.connection {
                    if let Some(&pos1) = node_positions.get(&id1) {
                        for &id2 in connections {
                            if let Some(&pos2) = node_positions.get(&id2) {
                                if id1 < id2 {
                                    let node_radius = 25.0;
                                    let direction = (pos2 - pos1).normalized();
                                    let start_pos = pos1 + direction * node_radius;
                                    let end_pos = pos2 - direction * node_radius;

                                    let edge_color = get_edge_color((id1, id2));
                                    let stroke_width = if self.selected_edge == Some((id1, id2)) ||
                                        self.selected_edge == Some((id2, id1)) {
                                        4.0
                                    } else {
                                        2.0
                                    };

                                    painter.line_segment(
                                        [start_pos, end_pos],
                                        egui::Stroke::new(stroke_width, edge_color)
                                    );

                                    edge_segments.push((id1, id2, start_pos, end_pos));
                                }
                            }
                        }
                    }
                }

                // Disegna i nodi
                for (i, (&node_id, &node_type)) in self.node_types.iter().enumerate() {
                    let angle = (i as f32 / node_count as f32) * 2.0 * std::f32::consts::PI;
                    let pos = egui::Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin()
                    );

                    let node_radius = 25.0;
                    let is_selected = self.selected_node_id1 == Some(node_id) ||
                        self.selected_node_id2 == Some(node_id);

                    // CORREZIONE: Usa sempre le texture, anche se sono fallback
                    if let Some(&texture_id) = self.node_textures.get(&node_type) {
                        let image_size = 50.0;
                        let image_rect = egui::Rect::from_center_size(pos, egui::Vec2::splat(image_size));

                        // Aureola quando selezionato
                        if is_selected {
                            painter.rect_filled(
                                image_rect.expand(4.0),
                                8.0,
                                Color32::from_rgba_unmultiplied(255, 215, 0, 200)
                            );
                        }

                        // Disegna la texture (anche se è una texture fallback)
                        painter.image(
                            texture_id,
                            image_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                            Color32::WHITE
                        );

                        // Bordo
                        let border_color = if is_selected { Color32::from_rgb(255, 215, 0) } else { Color32::BLACK };
                        painter.rect_stroke(image_rect, 6.0, egui::Stroke::new(2.0, border_color));
                    } else {
                        // Fallback estremo: cerchio colorato
                        let node_color = get_node_color(node_type, is_selected);
                        if is_selected {
                            painter.circle_filled(pos, node_radius + 3.0, Color32::from_rgb(255, 215, 0));
                        }
                        painter.circle_filled(pos, node_radius, node_color);
                        painter.circle_stroke(pos, node_radius, egui::Stroke::new(2.0, Color32::BLACK));
                    }

                    // Testo del nodo
                    let text_pos = egui::Pos2::new(pos.x, pos.y + 35.0);
                    let text = format!("{}\n{:?}", node_id, node_type);
                    painter.text(
                        text_pos,
                        egui::Align2::CENTER_CENTER,
                        text,
                        egui::FontId::default(),
                        Color32::WHITE,
                    );
                }
            }

            // Gestione click
            if response.clicked() {
                if let Some(click_pos) = response.interact_pointer_pos() {
                    let mut clicked_something = false;

                    let mut closest_node = None;
                    let mut closest_node_distance = f32::MAX;

                    for (&node_id, &pos) in &node_positions {
                        let distance = (click_pos - pos).length();
                        if distance < 30.0 && distance < closest_node_distance {
                            closest_node_distance = distance;
                            closest_node = Some(node_id);
                        }
                    }

                    if let Some(clicked_node_id) = closest_node {
                        self.handle_node_click(clicked_node_id);

                        if let Err(e) = self.sender_node_clicked.send(clicked_node_id) {
                            println!("Errore nell'invio node_clicked: {}", e);
                        }

                        clicked_something = true;
                    } else {
                        let mut closest_edge = None;
                        let mut closest_edge_distance = f32::MAX;

                        for &(id1, id2, start_pos, end_pos) in &edge_segments {
                            let distance = distance_point_to_line(click_pos, start_pos, end_pos);
                            if distance < 8.0 && distance < closest_edge_distance {
                                closest_edge_distance = distance;
                                closest_edge = Some((id1, id2));
                            }
                        }

                        if let Some(clicked_edge) = closest_edge {
                            self.handle_edge_click(clicked_edge);

                            if let Err(e) = self.sender_edge_clicked.send(clicked_edge) {
                                println!("Errore nell'invio edge_clicked: {}", e);
                            }

                            clicked_something = true;
                        }
                    }

                    if !clicked_something {
                        self.selected_node_id1 = None;
                        self.selected_node_id2 = None;
                        self.selected_edge = None;
                    }
                }
            }
        }

        ui.separator();
        ui.label("🖱️ Clicca sui NODI per selezionarli");
        ui.label("🔗 Clicca sugli EDGE per selezionarli");
        ui.label("💻 Client | 🚁 Drone | 🖥️ Server");
        if self.selected_node_id1.is_some() || self.selected_edge.is_some() {
            ui.label("💡 Clicca sullo sfondo per deselezionare tutto");
        }
    }
}

fn distance_point_to_line(point: egui::Pos2, line_start: egui::Pos2, line_end: egui::Pos2) -> f32 {
    let line_vec = line_end - line_start;
    let point_vec = point - line_start;

    if line_vec.length_sq() < f32::EPSILON {
        return (point - line_start).length();
    }

    let t = (point_vec.dot(line_vec) / line_vec.length_sq()).clamp(0.0, 1.0);
    let projection = line_start + t * line_vec;
    (point - projection).length()
}

impl eframe::App for GraphApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::SidePanel::left("info_panel")
                .resizable(true)
                .default_width(200.0)
                .show_inside(ui, |ui| {
                    ui.heading("Info Grafo");
                    ui.separator();

                    ui.label(format!("Nodi totali: {}", self.node_types.len()));
                    ui.label(format!("Collegamenti: {}", self.connection.values().map(|v| v.len()).sum::<usize>() / 2));

                    ui.separator();

                    if let Some(selected_node_id) = self.selected_node_id1 {
                        if let Some(&node_type) = self.node_types.get(&selected_node_id) {
                            ui.heading("🔵 Nodo Selezionato");
                            ui.label(format!("ID: {}", selected_node_id));
                            ui.label(format!("Tipo: {:?}", node_type));

                            if let Some(connections) = self.connection.get(&selected_node_id) {
                                ui.label(format!("Connesso a: {:?}", connections));
                            }

                            if ui.button("Deseleziona Nodo").clicked() {
                                self.selected_node_id1 = None;
                                self.selected_node_id2 = None;
                            }
                        }
                    }

                    if let Some((id1, id2)) = self.selected_edge {
                        ui.separator();
                        ui.heading("🔗 Edge Selezionato");
                        ui.label(format!("Connessione: {} ↔ {}", id1, id2));

                        if let (Some(&type1), Some(&type2)) = (self.node_types.get(&id1), self.node_types.get(&id2)) {
                            ui.label(format!("Tipo: {:?} ↔ {:?}", type1, type2));
                        }

                        if ui.button("Deseleziona Edge").clicked() {
                            self.selected_edge = None;
                        }
                    }

                    if self.selected_node_id1.is_none() && self.selected_edge.is_none() {
                        ui.label("Nessuna selezione");
                        ui.label("Clicca su un nodo o edge per selezionarlo");
                    }

                    ui.separator();

                    if !self.node_types.is_empty() {
                        ui.label("Tipi di nodi presenti:");
                        for node_type in [NodeType::Client, NodeType::Drone, NodeType::Server] {
                            let count = self.node_types.values().filter(|&&nt| nt == node_type).count();
                            if count > 0 {
                                ui.label(format!("{:?}: {}", node_type, count));
                            }
                        }
                    }
                });

            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.heading("Grafo di Rete");
                self.draw_custom_graph(ui);
            });
        });
    }
}

// CORREZIONE: Funzione migliorata per caricare file immagine
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

// CORREZIONE: Crea texture distintive per ogni tipo di nodo
fn create_distinctive_texture(cc: &eframe::CreationContext<'_>, node_type: NodeType) -> TextureId {
    let size = 64;
    let mut pixels = Vec::new();

    // Crea pattern diversi per ogni tipo di nodo invece di colori solidi
    for y in 0..size {
        for x in 0..size {
            let color = match node_type {
                NodeType::Client => {
                    // Pattern a scacchiera verde
                    if (x + y) % 8 < 4 {
                        Color32::from_rgb(100, 200, 100)
                    } else {
                        Color32::from_rgb(80, 160, 80)
                    }
                }
                NodeType::Drone => {
                    // Pattern circolare blu
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
                    // Pattern a righe arancione
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

// Fallback generico
fn create_fallback_texture(cc: &eframe::CreationContext<'_>, name: &str) -> TextureId {
    let size = 64;
    let pixels = vec![Color32::from_rgb(128, 128, 128); size * size]; // Grigio neutro

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