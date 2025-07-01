use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::Receiver;
use eframe::egui;
use egui::{TextureId, Pos2, Vec2, RichText, Context};
use petgraph::stable_graph::NodeIndex;
use wg_2024::network::NodeId;
use client::ui::UiState;
use client::ui::ClientState;
use crate::utility::{ButtonEvent, GraphAction, MessageType, NodeType};

// Importa i componenti custom dal modulo graph_components
use crate::view::graph_components::*;

pub struct UnifiedGraphController {
    // === GRAFO DATI ===
    pub node_id_to_index: HashMap<NodeId, NodeIndex<u32>>,
    pub index_to_node_id: HashMap<NodeIndex<u32>, NodeId>,
    pub nodes: HashMap<NodeId, GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub selection_state: GraphSelectionState,
    pub node_textures: HashMap<NodeType, TextureId>,
    pub client_ui_state: Arc<Mutex<UiState>>,

    // === BOTTONI STATO ===
    pub node_id1: Option<NodeId>,
    pub node_id2: Option<NodeId>,
    pub multiple_selection_mode: bool,
    pub selected_pdr: f32,

    // === MESSAGGI STATO ===
    pub message_log: Vec<MessageType>,
    pub max_messages: usize,
    pub auto_scroll: bool,

    // === COMUNICAZIONE ESTERNA ===
    pub graph_updates_receiver: Option<Receiver<GraphAction>>,
    pub button_event_handler: Option<Box<dyn Fn(ButtonEvent) + Send + Sync>>,
    pub client_state_receiver: Option<Receiver<(NodeId, ClientState)>>,
}

impl UnifiedGraphController {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        connections: HashMap<NodeId, Vec<NodeId>>,
        node_types: HashMap<NodeId, NodeType>,
        client_ui_state: Arc<Mutex<UiState>>,
        graph_updates_receiver: Option<Receiver<GraphAction>>,
        client_state_receiver: Option<Receiver<(NodeId, ClientState)>>,
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
                create_node_with_texture(node_id, node_type, position, texture_id)
            } else {
                create_node(node_id, node_type, position)
            };

            nodes.insert(node_id, node);
        }

        // Crea edge
        for (&node_id, connections) in &connections {
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

            // Bottoni stato
            node_id1: None,
            node_id2: None,
            multiple_selection_mode: false,
            selected_pdr: 0.1,

            // Messaggi stato
            message_log: Vec::new(),
            max_messages: 1000,
            auto_scroll: true,

            // Comunicazione esterna
            graph_updates_receiver,
            button_event_handler: None,
            client_state_receiver,
        }
    }

    // === METODI PUBBLICI PER CONFIGURAZIONE ===

    pub fn set_button_event_handler<F>(&mut self, handler: F)
    where
        F: Fn(ButtonEvent) + Send + Sync + 'static
    {
        self.button_event_handler = Some(Box::new(handler));
    }

    // === METODI GRAFO ===

    fn load_node_textures(cc: &eframe::CreationContext<'_>) -> HashMap<NodeType, TextureId> {
        let mut node_textures = HashMap::new();

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
                    if let Some(loaded_texture) = load_texture_from_file(cc, &full_path) {
                        texture_id = Some(loaded_texture);
                        break;
                    }
                }
            }

            let final_texture_id = match texture_id {
                Some(id) => id,
                None => create_fallback_texture(cc, node_type)
            };

            node_textures.insert(node_type, final_texture_id);
        }

        node_textures
    }

    pub fn handle_graph_action(&mut self, action: GraphAction) -> Result<(), String> {
        match action {
            GraphAction::AddNode(id, node_type) => self.add_node(id, node_type),
            GraphAction::RemoveNode(id) => self.remove_node(id),
            GraphAction::AddEdge(id1, id2) => self.add_edge(id1, id2),
            GraphAction::RemoveEdge(id1, id2) => self.remove_edge(id1, id2),
        }
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        if self.nodes.contains_key(&new_node_id) {
            return Err(format!("Node with ID {} already exists", new_node_id));
        }

        // Gestione client state
        let mut error_message = None;
        if node_type == NodeType::Client {
            if let Some(ref receiver) = &self.client_state_receiver {
                if let Ok((id, client_state)) = receiver.try_recv() {
                    match self.client_ui_state.lock() {
                        Ok(mut state) => {
                            state.add_client(id, client_state);
                        }
                        Err(_) => {
                            error_message = Some(MessageType::Error("Mutex is poisoned".to_string()));
                        }
                    }
                }
            }
        }

        // Aggiungi eventuali messaggi di errore dopo aver risolto i borrow
        if let Some(msg) = error_message {
            self.add_message(msg);
        }

        // Calcola posizione per il nuovo nodo
        let position = Pos2::new(
            300.0 + (new_node_id as f32 * 50.0) % 400.0,
            200.0 + (new_node_id as f32 * 30.0) % 300.0
        );

        // Crea nodo
        let texture_id = self.node_textures.get(&node_type).copied();
        let node = if let Some(texture_id) = texture_id {
            create_node_with_texture(new_node_id, node_type, position, texture_id)
        } else {
            create_node(new_node_id, node_type, position)
        };

        self.nodes.insert(new_node_id, node);

        // Aggiungi ai mapping
        let node_index = NodeIndex::new(self.node_id_to_index.len());
        self.node_id_to_index.insert(new_node_id, node_index);
        self.index_to_node_id.insert(node_index, new_node_id);

        self.add_message(MessageType::Ok(format!("Added node: ID={}, Type={:?}", new_node_id, node_type)));
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
        self.deselect_node_internal(node_id);

        self.add_message(MessageType::Ok(format!("Removed node {}", node_id)));
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

        self.add_message(MessageType::Ok(format!("Added edge between nodes {} and {}", id1, id2)));
        Ok(())
    }

    pub fn remove_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        let edge_index = self.edges.iter().position(|edge|
            (edge.from_id == id1 && edge.to_id == id2) ||
                (edge.from_id == id2 && edge.to_id == id1)
        );

        if let Some(index) = edge_index {
            self.edges.remove(index);
            self.selection_state.deselect_edge();
            self.sync_selection_state();
            self.add_message(MessageType::Ok(format!("Removed edge between nodes {} and {}", id1, id2)));
            Ok(())
        } else {
            Err(format!("Edge between {} and {} does not exist", id1, id2))
        }
    }

    // === METODI SELEZIONE ===

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

    pub fn handle_node_click(&mut self, node_id: NodeId) {
        self.selection_state.toggle_node(node_id);
        self.sync_selection_state();

        // Aggiorna lo stato dei bottoni
        if self.multiple_selection_mode {
            if self.node_id1.is_none() {
                self.node_id1 = Some(node_id);
            } else if self.node_id2.is_none() && self.node_id1 != Some(node_id) {
                self.node_id2 = Some(node_id);
            } else {
                self.node_id1 = Some(node_id);
                self.node_id2 = None;
            }
        } else {
            self.node_id1 = Some(node_id);
            self.node_id2 = None;
        }
    }

    pub fn handle_edge_click(&mut self, from_id: NodeId, to_id: NodeId) {
        self.selection_state.toggle_edge(from_id, to_id);
        self.sync_selection_state();
    }

    pub fn deselect_node_internal(&mut self, node_id: NodeId) {
        self.selection_state.deselect_node(node_id);
        if self.node_id1 == Some(node_id) {
            self.node_id1 = None;
        }
        if self.node_id2 == Some(node_id) {
            self.node_id2 = None;
        }
        self.sync_selection_state();
    }

    pub fn clear_selection(&mut self) {
        self.node_id1 = None;
        self.node_id2 = None;
        self.multiple_selection_mode = false;
        self.selection_state.clear_all();
        self.sync_selection_state();
    }

    // === METODI BOTTONI ===

    pub fn enter_multiple_selection_mode(&mut self) {
        self.multiple_selection_mode = true;
        self.node_id1 = None;
        self.node_id2 = None;
        self.add_message(MessageType::Info("Multi-selection mode activated".to_string()));
    }

    pub fn send_button_event(&self, event: ButtonEvent) {
        if let Some(ref handler) = self.button_event_handler {
            handler(event);
        } else {
            self.add_message_const(MessageType::Error("No button event handler set".to_string()));
        }
    }

    // === METODI MESSAGGI ===

    pub fn add_message(&mut self, message: MessageType) {
        self.message_log.push(message);
        if self.message_log.len() > self.max_messages {
            self.message_log.remove(0);
        }
    }

    // Versione const per chiamate da metodi const
    fn add_message_const(&self, _message: MessageType) {
        // In una implementazione reale, potresti voler usare un Arc<Mutex<Vec<MessageType>>>
        // per poter modificare i messaggi anche da metodi const
    }

    pub fn clear_messages(&mut self) {
        self.message_log.clear();
    }

    // === METODI DI AGGIORNAMENTO ===

    pub fn handle_pending_events(&mut self) {
        // Gestisce GraphAction dal backend
        if let Some(ref receiver) = &self.graph_updates_receiver {
            if let Ok(command) = receiver.try_recv() {
                if let Err(e) = self.handle_graph_action(command) {
                    self.add_message(MessageType::Error(format!("GraphAction error: {}", e)));
                }
            }
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
            self.handle_node_click(node_id);
        } else if let Some((from_id, to_id)) = clicked_edge {
            self.handle_edge_click(from_id, to_id);
        } else {
            // Click su sfondo = deseleziona tutto
            self.clear_selection();
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        // Gestione tasto ESC per deselezionare tutto
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.clear_selection();
            self.add_message(MessageType::Info("Selection cleared".to_string()));
        }
    }

    // === METODI RENDERING ===

    fn render_graph(&mut self, ui: &mut egui::Ui) {
        ui.heading("🌐 Grafo di Rete");

        // Area per disegnare il grafo
        let desired_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            // Disegna lo sfondo
            ui.painter().rect_filled(rect, 5.0, egui::Color32::from_gray(250));

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
    }

    fn render_buttons(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.heading("🎛️ Network Controls");
        ui.separator();

        // Selection info
        ui.heading("🎯 Selection Info");
        match (self.node_id1, self.node_id2) {
            (None, None) => {
                ui.label("No nodes selected");
                ui.label("Click on a node to select it");
            }
            (Some(id1), None) => {
                ui.label(format!("Selected Node: {}", id1));
            }
            (Some(id1), Some(id2)) => {
                ui.label(format!("Selected Nodes: {} and {}", id1, id2));
                ui.label("Ready to create/remove edge");
            }
            (None, Some(_)) => unreachable!(),
        }

        if self.multiple_selection_mode {
            ui.colored_label(egui::Color32::YELLOW, "🔗 Multi-selection mode active");
        }

        ui.separator();

        // Node operations
        ui.heading("🔧 Node Operations");

        ui.horizontal(|ui| {
            let can_remove = self.node_id1.is_some();
            ui.add_enabled_ui(can_remove, |ui| {
                if ui.button("🗑️ Remove Node").clicked() {
                    if let Some(id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::Crash(id));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.label("PDR:");
            ui.add(egui::Slider::new(&mut self.selected_pdr, 0.0..=1.0).step_by(0.01));
        });

        ui.horizontal(|ui| {
            let can_change_pdr = self.node_id1.is_some();
            ui.add_enabled_ui(can_change_pdr, |ui| {
                if ui.button("📡 Apply PDR").clicked() {
                    if let Some(id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::ChangePdr(id, self.selected_pdr));
                    }
                }
            });
        });

        ui.separator();

        // Connection operations
        ui.heading("🔗 Connection Operations");

        ui.horizontal(|ui| {
            if ui.button("🔗 Add Edge Mode").clicked() {
                self.enter_multiple_selection_mode();
            }

            let can_add_edge = self.node_id1.is_some() && self.node_id2.is_some();
            ui.add_enabled_ui(can_add_edge, |ui| {
                if ui.button("✅ Create Edge").clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::NewConnection(id1, id2));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            let can_remove_edge = self.node_id1.is_some() && self.node_id2.is_some();
            ui.add_enabled_ui(can_remove_edge, |ui| {
                if ui.button("❌ Remove Edge").clicked() {
                    if let (Some(id1), Some(id2)) = (self.node_id1, self.node_id2) {
                        self.send_button_event(ButtonEvent::RemoveConection(id1, id2));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.separator();

        // Creation operations
        ui.heading("➕ Create New Nodes");

        let has_selection = self.node_id1.is_some();

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("🤖 New Drone").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewDrone(connection_id, self.selected_pdr));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("💻 New Client").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewClient(connection_id));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.horizontal(|ui| {
            ui.add_enabled_ui(has_selection, |ui| {
                if ui.button("🖥️ New Server").clicked() {
                    if let Some(connection_id) = self.node_id1 {
                        self.send_button_event(ButtonEvent::NewServer(connection_id));
                        self.clear_selection();
                    }
                }
            });
        });

        ui.separator();
        if ui.button("🔄 Clear Selection").clicked() {
            self.clear_selection();
        }
    }

    fn render_messages(&mut self, ui: &mut egui::Ui) {
        ui.add_space(5.0);
        ui.horizontal(|ui| {
            ui.heading("📝 Messages");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("🗑️ Clear").clicked() {
                    self.clear_messages();
                }
                ui.checkbox(&mut self.auto_scroll, "Auto-scroll");
                ui.label(format!("{}/{}", self.message_log.len(), self.max_messages));
            });
        });

        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .max_height(150.0)
            .show(ui, |ui| {
                for (index, message) in self.message_log.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!("[{}]", index + 1));

                        match message {
                            MessageType::Error(t) => {
                                let text = RichText::new(format!("❌ {}", t))
                                    .color(egui::Color32::from_rgb(234, 162, 124));
                                ui.label(text);
                            }
                            MessageType::Ok(t) => {
                                let text = RichText::new(format!("✅ {}", t))
                                    .color(egui::Color32::from_rgb(232, 187, 166));
                                ui.label(text);
                            }
                            MessageType::PacketSent(t) => {
                                let text = RichText::new(format!("📤 {}", t))
                                    .color(egui::Color32::from_rgb(14, 137, 145));
                                ui.label(text);
                            }
                            MessageType::PacketDropped(t) => {
                                let text = RichText::new(format!("📥 {}", t))
                                    .color(egui::Color32::from_rgb(12, 49, 59));
                                ui.label(text);
                            }
                            MessageType::Info(t) => {
                                let text = RichText::new(format!("ℹ️ {}", t))
                                    .color(egui::Color32::from_rgb(141, 182, 188));
                                ui.label(text);
                            }
                            _ => {
                                let text = RichText::new("❓ Unclassified message")
                                    .color(egui::Color32::GRAY);
                                ui.label(text);
                            }
                        }
                    });
                }

                if self.auto_scroll && !self.message_log.is_empty() {
                    ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                }
            });

        ui.add_space(5.0);
    }
}

impl eframe::App for UnifiedGraphController {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Gestisce eventi pendenti
        self.handle_pending_events();

        // Gestisce input da tastiera
        self.handle_keyboard_input(ctx);

        // Layout identico a ControllerUI
        egui::TopBottomPanel::bottom("Message panel")
            .resizable(true)
            .default_height(180.0)
            .show(ctx, |ui| {
                self.render_messages(ui);
            });

        egui::SidePanel::right("Possible actions")
            .resizable(true)
            .default_width(320.0)
            .show(ctx, |ui| {
                self.render_buttons(ui);
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.render_graph(ui);
        });
    }
}