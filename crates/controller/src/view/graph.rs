use std::collections::HashMap;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{Color32, TextureId};
use wg_2024::network::NodeId;
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType};

type NodePayload = (NodeId, NodeType);

pub struct GraphApp {
    pub selected_node_id1: Option<NodeId>,
    pub selected_node_id2: Option<NodeId>,
    pub selected_edge: Option<(NodeId, NodeId)>,
    pub node_textures: HashMap<NodeType, TextureId>,

    pub connection: HashMap<NodeId, Vec<NodeId>>,
    pub node_types: HashMap<NodeId, NodeType>,

    //CHANNELS
    pub receiver_updates: Receiver<GraphAction>,
    pub sender_node_clicked: Sender<NodeId>,
    pub sender_edge_clicked: Sender<(NodeId, NodeId)>,
    pub reciver_buttom_messages: Receiver<ButtonsMessages>,
    pub sender_message_type: Sender<MessageType>,
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>,
               connection: HashMap<NodeId, Vec<NodeId>>,
               node_types: HashMap<NodeId, NodeType>,
               receiver_updates: Receiver<GraphAction>,
               sender_node_clicked: Sender<NodeId>,
               sender_edge_clicked: Sender<(NodeId, NodeId)>,
               reciver_buttom_messages: Receiver<ButtonsMessages>,
               sender_message_type: Sender<MessageType>) -> Self {

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
            connection,
            node_types,
            receiver_updates,
            sender_node_clicked,
            sender_edge_clicked,
            reciver_buttom_messages,
            sender_message_type
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

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::unbounded;
    use std::collections::HashMap;
    use wg_2024::packet::{Packet, PacketType, Fragment};
    use wg_2024::network::SourceRoutingHeader;
    use wg_2024::controller::{DroneCommand, DroneEvent};
    use crate::controller_handler::{ControllerError, ControllerHandler};
    use crate::utility::{ButtonEvent, DroneGroup};
    use crate::utility::MessageType::{Error, PacketSent};

    // Test base per la creazione del controller
    #[test]
    fn test_controller_creation() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        assert_eq!(controller.drones.len(), 0);
        assert_eq!(controller.clients.len(), 0);
        assert_eq!(controller.servers.len(), 0);
    }

    // Test per l'enum ControllerError
    #[test]
    fn test_controller_error_display() {
        let error1 = ControllerError::ChannelSend("test message".to_string());
        assert_eq!(error1.to_string(), "Channel send error: test message");

        let error2 = ControllerError::NodeNotFound(42);
        assert_eq!(error2.to_string(), "Node not found: 42");

        let error3 = ControllerError::InvalidOperation("invalid op".to_string());
        assert_eq!(error3.to_string(), "Invalid operation: invalid op");

        let error4 = ControllerError::NetworkConstraintViolation("constraint violated".to_string());
        assert_eq!(error4.to_string(), "Network constraint violation: constraint violated");
    }

    // Test per is_drone method
    #[test]
    fn test_is_drone() {
        let mut drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Test con ID inesistente
        assert!(!controller.is_drone(&1));
        assert!(!controller.is_drone(&99));
    }

    // Test per generate_random_id con ID limitati
    #[test]
    fn test_generate_random_id_with_available_ids() {
        let mut drones = HashMap::new();
        let drones_types = HashMap::new();
        let mut packet_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        // Occupa alcuni ID
        for i in 0..10 {
            let (sender, _) = unbounded::<Packet>();
            packet_senders.insert(i, sender);
        }

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            packet_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let result = controller.generate_random_id();
        assert!(result.is_ok());
        let id = result.unwrap();
        assert!(id >= 10); // Dovrebbe essere >= 10 dato che 0-9 sono occupati
    }

    // Test per validate_network_constraints con rete vuota
    #[test]
    fn test_validate_network_constraints_empty() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let adj_list = HashMap::new();
        assert!(controller.validate_network_constraints(&adj_list));
    }

    // Test per change_packet_drop_rate con nodo inesistente
    #[test]
    fn test_change_packet_drop_rate_invalid_node() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let mut controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let result = controller.change_packet_drop_rate(&99, 0.5);
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ControllerError::InvalidOperation(_)));
        }
    }

    // Test per send_packet_to_client con client inesistente
    #[test]
    fn test_send_packet_to_client_not_found() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        let packet = Packet {
            pack_type: PacketType::MsgFragment(Fragment {
                fragment_index: 0,
                total_n_fragments: 1,
                length: 10,
                data: [0; 128],
            }),
            routing_header: SourceRoutingHeader {
                hop_index: 0,
                hops: vec![1, 99], // Client inesistente
            },
            session_id: 123,
        };

        let result = controller.send_packet_to_client(packet);
        assert!(result.is_err());

        if let Err(error) = result {
            assert!(matches!(error, ControllerError::NodeNotFound(_)));
        }
    }

    // Test per message helpers
    #[test]
    fn test_message_helpers() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        controller.send_success_message("test success");
        controller.send_info_message("test info");
        controller.send_error_message("test error");

        // Verifica che i messaggi siano stati inviati
        let mut messages = Vec::new();
        while let Ok(msg) = message_receiver.try_recv() {
            messages.push(msg);
        }

        assert_eq!(messages.len(), 3);

        // Verifica i tipi di messaggio
        assert!(messages.iter().any(|msg| matches!(msg, MessageType::Ok(_))));
        assert!(messages.iter().any(|msg| matches!(msg, PacketSent(_))));
        assert!(messages.iter().any(|msg| matches!(msg, Error(_))));
    }

    // Test per select_drone_group
    #[test]
    fn test_select_drone_group_empty() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Con counter vuoto dovrebbe restituire None
        let result = controller.select_drone_group();
        assert!(result.is_none());
    }

    // Test per select_drone_group con valori
    #[test]
    fn test_select_drone_group_with_values() {
        let drones = HashMap::new();
        let drones_types = HashMap::new();
        let drone_senders = HashMap::new();
        let clients = HashMap::new();
        let servers = HashMap::new();
        let connections = HashMap::new();
        let send_command_drone = HashMap::new();
        let send_command_node = HashMap::new();
        let receiver_event = HashMap::new();
        let receriver_node_event = HashMap::new();
        let client_ui_state = client::ui::UiState::new();

        let (_button_sender, button_receiver) = unbounded::<ButtonEvent>();
        let (graph_action_sender, _graph_action_receiver) = unbounded::<GraphAction>();
        let (message_sender, _message_receiver) = unbounded::<MessageType>();

        let mut controller = ControllerHandler::new(
            drones,
            drones_types,
            drone_senders,
            clients,
            servers,
            connections,
            send_command_drone,
            send_command_node,
            receiver_event,
            receriver_node_event,
            client_ui_state,
            button_receiver,
            graph_action_sender,
            message_sender,
        );

        // Aggiungi alcuni contatori
        controller.drones_counter.insert(DroneGroup::RustInPeace, 3);
        controller.drones_counter.insert(DroneGroup::BagelBomber, 1);
        controller.drones_counter.insert(DroneGroup::LockheedRustin, 2);

        let result = controller.select_drone_group();
        assert!(result.is_some());

        // Dovrebbe selezionare il gruppo con il conteggio minimo (BagelBomber con 1)
        assert!(matches!(result.unwrap(), DroneGroup::BagelBomber));
    }
}