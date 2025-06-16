use std::collections::{HashMap, HashSet};
use crossbeam_channel::{Receiver, Sender};
use eframe::{egui, emath};
use egui::{Color32, TextureId};
use wg_2024::network::NodeId;
use crate::utility::{ButtonsMessages, GraphAction, NodeType};
use rand::Rng;

type NodePayload = (NodeId, NodeType);

pub struct GraphApp {
    // Rimuoviamo il grafo di egui_graphs e usiamo solo le mappe interne
    selected_node_id: Option<NodeId>,
    node_textures: HashMap<NodeType, TextureId>,

    pub connection: HashMap<NodeId, Vec<NodeId>>,
    pub node_types: HashMap<NodeId, NodeType>,

    //CHANNELS
    pub receiver_updates: Receiver<GraphAction>,
    pub sender_node_clicked: Sender<NodeId>,
    pub sender_edge_clicked: Sender<(NodeId, NodeId)>,
    pub reciver_buttom_messages: Receiver<ButtonsMessages>,
}

impl GraphApp {
    pub fn new(cc: &eframe::CreationContext<'_>,
               connection: HashMap<NodeId, Vec<NodeId>>,
               node_types: HashMap<NodeId, NodeType>,
               receiver_updates: Receiver<GraphAction>,
               sender_node_clicked: Sender<NodeId>,
               sender_edge_clicked: Sender<(NodeId, NodeId)>, // Nuovo parametro
               reciver_buttom_messages: Receiver<ButtonsMessages>) -> Self {

        // Carica le texture per i diversi tipi di nodi
        let mut node_textures = HashMap::new();
        let client_texture = load_texture_from_path(cc, "assets/client.png");
        let drone_texture = load_texture_from_path(cc, "assets/drone.png");
        let server_texture = load_texture_from_path(cc, "assets/server.png");

        node_textures.insert(NodeType::Client, client_texture);
        node_textures.insert(NodeType::Drone, drone_texture);
        node_textures.insert(NodeType::Server, server_texture);

        let mut app = Self {
            selected_node_id: None,
            node_textures,
            connection,
            node_types,
            receiver_updates,
            sender_node_clicked,
            sender_edge_clicked,
            reciver_buttom_messages
        };

        app
    }

    /// Ricostruisce il grafo - ora non fa nulla perché usiamo solo le mappe interne
    pub fn rebuild_visual_graph(&mut self) {
        // Non serve più ricostruire un grafo visuale separato
        // La visualizzazione usa direttamente le mappe interne
        println!("Graph updated with {} nodes and {} total connections",
                 self.node_types.len(),
                 self.connection.values().map(|v| v.len()).sum::<usize>() / 2);
    }

    /// Handler che aggiorna automaticamente il grafo visuale
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
                // Ricostruisci il grafo visuale dopo ogni modifica
                self.rebuild_visual_graph();
            }
            Err(e) => {
                println!("Errore nell'esecuzione GraphAction: {}", e);
            }
        }
    }

    /// Il loop principale che gestisce i canali
    pub fn run(&mut self) {
        loop {
            // Gestisci i comandi dal controller
            if let Ok(command) = self.receiver_updates.try_recv() {
                self.graph_action_handler(command);
            }

            // Gestisci i messaggi dai bottoni
            if let Ok(command) = self.reciver_buttom_messages.try_recv() {
                self.button_messages_handler(command);
            }

            // Piccola pausa per evitare un ciclo troppo intenso
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    pub fn button_messages_handler(&mut self, message: ButtonsMessages) {
        match message {
            ButtonsMessages::DeselectNode(_id) => {
                self.selected_node_id = None;
            }
            ButtonsMessages::MultipleSelectionAllowed => {
                // TODO: implementare selezione multipla
            }
        }
    }

    fn handle_node_click(&mut self, node_id: NodeId) {
        self.selected_node_id = Some(node_id);
        println!("Nodo cliccato: ID={}, Tipo={:?}", node_id, self.node_types.get(&node_id));
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

    /// Aggiunge un nodo alle mappe interne
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

    /// Rimuove un nodo dalle mappe interne
    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), String> {
        if !self.node_types.contains_key(&node_id) {
            return Err(format!("Node with ID {} not found", node_id));
        }

        // Rimuovi tutti gli edge collegati al nodo
        let _ = self.remove_all_edges(node_id)?;

        // Rimuovi il nodo dalle mappe interne
        self.node_types.remove(&node_id);
        self.connection.remove(&node_id);

        // Deseleziona se era selezionato
        if self.selected_node_id == Some(node_id) {
            self.selected_node_id = None;
        }

        println!("Removed node {}", node_id);
        Ok(())
    }

    /// Aggiunge un edge alle mappe interne
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

    /// Rimuove un edge dalle mappe interne
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

        println!("Removed edge between nodes {} and {}", id1, id2);
        Ok(())
    }

    /// Rimuove tutti gli edge di un nodo
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

    /// Visualizzazione personalizzata del grafo usando primitive egui
    fn draw_custom_graph(&mut self, ui: &mut egui::Ui) {
        let (rect, response) = ui.allocate_exact_size(
            egui::Vec2::new(600.0, 400.0),
            egui::Sense::click()
        );

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);

            // Colori per i diversi tipi di nodi
            let get_node_color = |node_type: NodeType| -> Color32 {
                match node_type {
                    NodeType::Client => Color32::from_rgb(255, 100, 100),  // Rosso
                    NodeType::Drone => Color32::from_rgb(100, 150, 255),   // Blu
                    NodeType::Server => Color32::from_rgb(100, 255, 100),  // Verde
                }
            };

            // Calcola posizioni dei nodi in cerchio (semplice layout)
            let center = rect.center();
            let radius = 150.0;
            let node_count = self.node_types.len();

            let mut node_positions = HashMap::new();
            let mut edge_segments = Vec::new(); // Per memorizzare i segmenti degli edge

            if node_count > 0 {
                for (i, (&node_id, &node_type)) in self.node_types.iter().enumerate() {
                    let angle = (i as f32 / node_count as f32) * 2.0 * std::f32::consts::PI;
                    let pos = egui::Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin()
                    );
                    node_positions.insert(node_id, pos);
                }

                // Prima disegna gli edge (sotto i nodi)
                for (&id1, connections) in &self.connection {
                    if let Some(&pos1) = node_positions.get(&id1) {
                        for &id2 in connections {
                            if let Some(&pos2) = node_positions.get(&id2) {
                                // Evita di disegnare la stessa linea due volte
                                if id1 < id2 {
                                    // Calcola i punti di inizio e fine sull'edge dei nodi
                                    let node_radius = 20.0;
                                    let direction = (pos2 - pos1).normalized();
                                    let start_pos = pos1 + direction * node_radius;
                                    let end_pos = pos2 - direction * node_radius;

                                    painter.line_segment(
                                        [start_pos, end_pos],
                                        egui::Stroke::new(3.0, Color32::GRAY)
                                    );

                                    // Memorizza il segmento per il click detection
                                    edge_segments.push((id1, id2, start_pos, end_pos));
                                }
                            }
                        }
                    }
                }

                // Poi disegna i nodi (sopra gli edge)
                for (i, (&node_id, &node_type)) in self.node_types.iter().enumerate() {
                    let angle = (i as f32 / node_count as f32) * 2.0 * std::f32::consts::PI;
                    let pos = egui::Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin()
                    );

                    // Disegna il nodo
                    let node_radius = 20.0;
                    let node_color = get_node_color(node_type);

                    // Evidenzia se selezionato
                    let is_selected = self.selected_node_id == Some(node_id);

                    if is_selected {
                        painter.circle_filled(pos, node_radius + 5.0, Color32::YELLOW);
                    }

                    painter.circle_filled(pos, node_radius, node_color);
                    painter.circle_stroke(pos, node_radius, egui::Stroke::new(2.0, Color32::BLACK));

                    // Disegna il testo del nodo
                    let text = format!("{}\n{:?}", node_id, node_type);
                    painter.text(
                        pos,
                        egui::Align2::CENTER_CENTER,
                        text,
                        egui::FontId::default(),
                        Color32::WHITE,
                    );
                }
            }

            // Gestisci i click
            if response.clicked() {
                if let Some(click_pos) = response.interact_pointer_pos() {
                    let mut clicked_something = false;

                    // Prima controlla se è stato cliccato un nodo
                    let mut closest_node = None;
                    let mut closest_node_distance = f32::MAX;

                    for (&node_id, &pos) in &node_positions {
                        let distance = (click_pos - pos).length();
                        if distance < 25.0 && distance < closest_node_distance {
                            closest_node_distance = distance;
                            closest_node = Some(node_id);
                        }
                    }

                    if let Some(clicked_node_id) = closest_node {
                        // Click su nodo
                        self.handle_node_click(clicked_node_id);

                        if let Err(e) = self.sender_node_clicked.send(clicked_node_id) {
                            println!("Errore nell'invio node_clicked: {}", e);
                        }

                        clicked_something = true;
                    } else {
                        // Se non è stato cliccato un nodo, controlla gli edge
                        let mut closest_edge = None;
                        let mut closest_edge_distance = f32::MAX;

                        for &(id1, id2, start_pos, end_pos) in &edge_segments {
                            let distance = distance_point_to_line(click_pos, start_pos, end_pos);
                            if distance < 8.0 && distance < closest_edge_distance {
                                closest_edge_distance = distance;
                                closest_edge = Some((id1, id2));
                            }
                        }

                        if let Some((edge_id1, edge_id2)) = closest_edge {
                            // Click su edge
                            println!("Edge cliccato: {} -> {}", edge_id1, edge_id2);

                            if let Err(e) = self.sender_edge_clicked.send((edge_id1, edge_id2)) {
                                println!("Errore nell'invio edge_clicked: {}", e);
                            }

                            clicked_something = true;
                        }
                    }

                    // Se non è stato cliccato niente, deseleziona
                    if !clicked_something {
                        self.selected_node_id = None;
                    }
                }
            }
        }

        // Istruzioni per l'utente
        ui.separator();
        ui.label("🔵 Clicca sui NODI per selezionarli");
        ui.label("🔗 Clicca sugli EDGE per informazioni sulla connessione");
        ui.label("Colori: 🔴 Client, 🔵 Drone, 🟢 Server");
    }
}

/// Calcola la distanza da un punto a una linea
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
            // Panel laterale per le informazioni
            egui::SidePanel::left("info_panel")
                .resizable(true)
                .default_width(200.0)
                .show_inside(ui, |ui| {
                    ui.heading("Info Grafo");
                    ui.separator();

                    ui.label(format!("Nodi totali: {}", self.node_types.len()));
                    ui.label(format!("Collegamenti: {}", self.connection.values().map(|v| v.len()).sum::<usize>() / 2));

                    ui.separator();

                    if let Some(selected_node_id) = self.selected_node_id {
                        if let Some(&node_type) = self.node_types.get(&selected_node_id) {
                            ui.heading("Nodo Selezionato");
                            ui.label(format!("ID: {}", selected_node_id));
                            ui.label(format!("Tipo: {:?}", node_type));

                            if let Some(connections) = self.connection.get(&selected_node_id) {
                                ui.label(format!("Connesso a: {:?}", connections));
                            }

                            if ui.button("Deseleziona").clicked() {
                                self.selected_node_id = None;
                            }
                        }
                    } else {
                        ui.label("Nessun nodo selezionato");
                        ui.label("Clicca su un nodo per selezionarlo");
                    }

                    ui.separator();

                    ui.label("Le modifiche al grafo vengono");
                    ui.label("ricevute dal controller esterno");

                    if !self.node_types.is_empty() {
                        ui.separator();
                        ui.label("Tipi di nodi presenti:");
                        for node_type in [NodeType::Client, NodeType::Drone, NodeType::Server] {
                            let count = self.node_types.values().filter(|&&nt| nt == node_type).count();
                            if count > 0 {
                                ui.label(format!("{:?}: {}", node_type, count));
                            }
                        }
                    }
                });

            // Area principale per il grafo
            egui::CentralPanel::default().show_inside(ui, |ui| {
                ui.heading("Grafo di Rete");

                // Usa direttamente la visualizzazione personalizzata
                self.draw_custom_graph(ui);
            });
        });
    }
}

// Funzione helper per caricare texture da file (semplificata)
fn load_texture_from_path(cc: &eframe::CreationContext<'_>, path: &str) -> TextureId {
    match std::fs::read(path) {
        Ok(image_data) => {
            load_texture_from_bytes(cc, &image_data)
        }
        Err(_) => {
            create_colored_texture(cc, Color32::LIGHT_GRAY, 64, 64)
        }
    }
}

fn load_texture_from_bytes(cc: &eframe::CreationContext<'_>, _bytes: &[u8]) -> TextureId {
    create_colored_texture(cc, Color32::LIGHT_BLUE, 64, 64)
}

fn create_colored_texture(cc: &eframe::CreationContext<'_>, color: Color32, width: usize, height: usize) -> TextureId {
    let pixels = vec![color; width * height];
    let color_image = egui::ColorImage {
        size: [width, height],
        pixels,
    };

    cc.egui_ctx.load_texture(
        "colored_texture",
        color_image,
        egui::TextureOptions::default(),
    ).id()
}


#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::{unbounded, bounded};
    use std::time::Duration;

    // Helper function per creare un GraphApp di test
    fn create_test_app() -> (GraphApp, Receiver<NodeId>, Receiver<(NodeId, NodeId)>, Sender<GraphAction>, Sender<ButtonsMessages>) {
        let (tx_updates, rx_updates) = unbounded();
        let (tx_node_clicked, rx_node_clicked) = unbounded();
        let (tx_edge_clicked, rx_edge_clicked) = unbounded();
        let (tx_button_messages, rx_button_messages) = unbounded();

        let connection = HashMap::new();
        let node_types = HashMap::new();

        // Simula il creation context
        // Per i test, sostituiamo le texture con ID fittizi
        let mut node_textures = HashMap::new();
        node_textures.insert(NodeType::Client, TextureId::default());
        node_textures.insert(NodeType::Drone, TextureId::default());
        node_textures.insert(NodeType::Server, TextureId::default());

        let app = GraphApp {
            selected_node_id: None,
            node_textures,
            connection,
            node_types,
            receiver_updates: rx_updates,
            sender_node_clicked: tx_node_clicked,
            sender_edge_clicked: tx_edge_clicked,
            reciver_buttom_messages: rx_button_messages,
        };

        (app, rx_node_clicked, rx_edge_clicked, tx_updates, tx_button_messages)
    }

    #[test]
    fn test_add_node() {
        let (mut app, _, _, _, _) = create_test_app();

        // Test aggiunta nodo valida
        assert!(app.add_node(1, NodeType::Client).is_ok());
        assert_eq!(app.node_types.len(), 1);
        assert_eq!(app.node_types.get(&1), Some(&NodeType::Client));
        assert!(app.connection.contains_key(&1));

        // Test aggiunta nodo con ID duplicato
        assert!(app.add_node(1, NodeType::Drone).is_err());
        assert_eq!(app.node_types.len(), 1); // Ancora 1 nodo

        // Test aggiunta di più nodi
        assert!(app.add_node(2, NodeType::Drone).is_ok());
        assert!(app.add_node(3, NodeType::Server).is_ok());
        assert_eq!(app.node_types.len(), 3);
    }

    #[test]
    fn test_remove_node() {
        let (mut app, _, _, _, _) = create_test_app();

        // Setup: aggiungi alcuni nodi
        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();
        app.add_node(3, NodeType::Server).unwrap();
        app.add_edge(1, 2).unwrap();
        app.add_edge(2, 3).unwrap();

        // Test rimozione nodo esistente
        assert!(app.remove_node(2).is_ok());
        assert_eq!(app.node_types.len(), 2);
        assert!(!app.node_types.contains_key(&2));
        assert!(!app.connection.contains_key(&2));

        // Verifica che gli edge siano stati rimossi
        assert!(!app.connection.get(&1).unwrap().contains(&2));
        assert!(!app.connection.get(&3).unwrap().contains(&2));

        // Test rimozione nodo non esistente
        assert!(app.remove_node(99).is_err());
    }

    #[test]
    fn test_add_edge() {
        let (mut app, _, _, _, _) = create_test_app();

        // Setup: aggiungi nodi
        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();
        app.add_node(3, NodeType::Server).unwrap();

        // Test aggiunta edge valido
        assert!(app.add_edge(1, 2).is_ok());
        assert!(app.connection.get(&1).unwrap().contains(&2));
        assert!(app.connection.get(&2).unwrap().contains(&1));

        // Test aggiunta edge duplicato
        assert!(app.add_edge(1, 2).is_err());

        // Test self-loop
        assert!(app.add_edge(1, 1).is_err());

        // Test edge con nodo non esistente
        assert!(app.add_edge(1, 99).is_err());
        assert!(app.add_edge(99, 1).is_err());
    }

    #[test]
    fn test_remove_edge() {
        let (mut app, _, _, _, _) = create_test_app();

        // Setup
        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();
        app.add_node(3, NodeType::Server).unwrap();
        app.add_edge(1, 2).unwrap();
        app.add_edge(2, 3).unwrap();

        // Test rimozione edge esistente
        assert!(app.remove_edge(1, 2).is_ok());
        assert!(!app.connection.get(&1).unwrap().contains(&2));
        assert!(!app.connection.get(&2).unwrap().contains(&1));

        // Test rimozione edge non esistente
        assert!(app.remove_edge(1, 2).is_err());
        assert!(app.remove_edge(1, 3).is_err());

        // Test rimozione con nodo non esistente
        assert!(app.remove_edge(1, 99).is_err());
    }

    #[test]
    fn test_remove_all_edges() {
        let (mut app, _, _, _, _) = create_test_app();

        // Setup: crea una rete complessa
        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();
        app.add_node(3, NodeType::Server).unwrap();
        app.add_node(4, NodeType::Drone).unwrap();

        app.add_edge(2, 1).unwrap();
        app.add_edge(2, 3).unwrap();
        app.add_edge(2, 4).unwrap();

        // Test rimozione di tutti gli edge del nodo 2
        let removed_count = app.remove_all_edges(2).unwrap();
        assert_eq!(removed_count, 3);
        assert!(app.connection.get(&2).unwrap().is_empty());

        // Verifica che gli altri nodi non abbiano più connessioni con 2
        assert!(!app.connection.get(&1).unwrap().contains(&2));
        assert!(!app.connection.get(&3).unwrap().contains(&2));
        assert!(!app.connection.get(&4).unwrap().contains(&2));

        // Test su nodo non esistente
        assert!(app.remove_all_edges(99).is_err());
    }

    #[test]
    fn test_get_next_node_id() {
        let (mut app, _, _, _, _) = create_test_app();

        // Test con grafo vuoto
        assert_eq!(app.get_next_node_id(), 1);

        // Aggiungi nodi e verifica
        app.add_node(5, NodeType::Client).unwrap();
        assert_eq!(app.get_next_node_id(), 6);

        app.add_node(10, NodeType::Drone).unwrap();
        assert_eq!(app.get_next_node_id(), 11);

        app.add_node(3, NodeType::Server).unwrap();
        assert_eq!(app.get_next_node_id(), 11); // Ancora 11, perché 10 è il max
    }

    #[test]
    fn test_handle_node_click() {
        let (mut app, rx_node_clicked, _, _, _) = create_test_app();

        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();

        // Click sul nodo 1
        app.handle_node_click(1);
        assert_eq!(app.selected_node_id, Some(1));

        // Click sul nodo 2
        app.handle_node_click(2);
        assert_eq!(app.selected_node_id, Some(2));
    }

    #[test]
    fn test_graph_action_handler() {
        let (mut app, _, _, _, _) = create_test_app();

        // Test AddNode action
        app.graph_action_handler(GraphAction::AddNode(1, NodeType::Client));
        assert!(app.node_types.contains_key(&1));

        // Test AddEdge action
        app.graph_action_handler(GraphAction::AddNode(2, NodeType::Drone));
        app.graph_action_handler(GraphAction::AddEdge(1, 2));
        assert!(app.connection.get(&1).unwrap().contains(&2));

        // Test RemoveEdge action
        app.graph_action_handler(GraphAction::RemoveEdge(1, 2));
        assert!(!app.connection.get(&1).unwrap().contains(&2));

        // Test RemoveNode action
        app.graph_action_handler(GraphAction::RemoveNode(1));
        assert!(!app.node_types.contains_key(&1));
    }

    #[test]
    fn test_button_messages_handler() {
        let (mut app, _, _, _, _) = create_test_app();

        app.add_node(1, NodeType::Client).unwrap();
        app.selected_node_id = Some(1);

        // Test DeselectNode
        app.button_messages_handler(ButtonsMessages::DeselectNode(1));
        assert_eq!(app.selected_node_id, None);
    }

    #[test]
    fn test_channel_communication() {
        let (mut app, rx_node_clicked, rx_edge_clicked, tx_updates, tx_button_messages) = create_test_app();

        // Test invio GraphAction tramite canale
        tx_updates.send(GraphAction::AddNode(1, NodeType::Client)).unwrap();

        // Simula un ciclo di run limitato
        if let Ok(command) = app.receiver_updates.try_recv() {
            app.graph_action_handler(command);
        }

        assert!(app.node_types.contains_key(&1));

        // Test invio ButtonsMessages tramite canale
        app.selected_node_id = Some(1);
        tx_button_messages.send(ButtonsMessages::DeselectNode(1)).unwrap();

        if let Ok(command) = app.reciver_buttom_messages.try_recv() {
            app.button_messages_handler(command);
        }

        assert_eq!(app.selected_node_id, None);
    }

    #[test]
    fn test_distance_point_to_line() {
        // Usa egui dal contesto corrente (stesso usato dalla funzione)

        // Test punto sulla linea
        let line_start = egui::Pos2::new(0.0, 0.0);
        let line_end = egui::Pos2::new(10.0, 0.0);
        let point = egui::Pos2::new(5.0, 0.0);
        assert!((distance_point_to_line(point, line_start, line_end) - 0.0).abs() < f32::EPSILON);

        // Test punto perpendicolare alla linea
        let point = egui::Pos2::new(5.0, 5.0);
        assert!((distance_point_to_line(point, line_start, line_end) - 5.0).abs() < f32::EPSILON);

        // Test punto fuori dal segmento
        let point = egui::Pos2::new(15.0, 0.0);
        assert!((distance_point_to_line(point, line_start, line_end) - 5.0).abs() < f32::EPSILON);

        // Test linea degenere (punto singolo)
        let line_start = egui::Pos2::new(0.0, 0.0);
        let line_end = egui::Pos2::new(0.0, 0.0);
        let point = egui::Pos2::new(3.0, 4.0);
        assert!((distance_point_to_line(point, line_start, line_end) - 5.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_rebuild_visual_graph() {
        let (mut app, _, _, _, _) = create_test_app();

        // Aggiungi alcuni nodi e edge
        app.add_node(1, NodeType::Client).unwrap();
        app.add_node(2, NodeType::Drone).unwrap();
        app.add_edge(1, 2).unwrap();

        // rebuild_visual_graph dovrebbe solo stampare info, non modificare lo stato
        let nodes_before = app.node_types.len();
        let connections_before = app.connection.len();

        app.rebuild_visual_graph();

        assert_eq!(app.node_types.len(), nodes_before);
        assert_eq!(app.connection.len(), connections_before);
    }
}
