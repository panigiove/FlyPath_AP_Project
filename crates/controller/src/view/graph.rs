use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use egui::{Pos2, Vec2, RichText};
use petgraph::stable_graph::{NodeIndex, EdgeIndex, StableGraph};
use petgraph::Undirected;
use wg_2024::network::NodeId;
use client::ui::UiState;
use crate::utility::{ButtonsMessages, GraphAction, MessageType, NodeType, DARK_BLUE};
use client::ui::ClientState;

use egui_graphs::{Graph, GraphView, to_graph, SettingsStyle, SettingsInteraction};

use crate::drawable::{Drawable, PanelDrawable, PanelType};

pub struct GraphApp {
    pub petgraph: StableGraph<GraphNodeData, GraphEdgeData, Undirected>,
    pub g: Graph<GraphNodeData, GraphEdgeData, Undirected>,
    pub node_id_to_index: HashMap<NodeId, NodeIndex>,

    pub selected_nodes: Vec<NodeId>,
    pub selected_edge: Option<(NodeId, NodeId)>,

    pub client_ui_state: Arc<Mutex<UiState>>,

    //channels
    pub receiver_updates: Receiver<GraphAction>,
    pub sender_node_clicked: Sender<NodeId>,
    pub receiver_bottom_messages: Receiver<ButtonsMessages>,
    pub sender_message_type: Sender<MessageType>,
    pub client_state_receiver: Receiver<(NodeId, ClientState)>,

    //UI settings
    pub labels_always: bool,
    pub dragging_enabled: bool,
    pub node_clicking_enabled: bool,
    pub edge_clicking_enabled: bool,
    pub node_selection_enabled: bool,

    //state flags
    pub graph_dirty: bool,
    pub selection_dirty: bool,
}

#[derive(Clone, Debug)]
pub struct GraphNodeData {
    pub node_id: NodeId,
    pub node_type: NodeType,
    pub label: String,
    pub selected: bool,
    pub position: Pos2,
}

impl GraphNodeData {
    pub fn new(node_id: NodeId, node_type: NodeType, position: Pos2) -> Self {
        let label = match node_type {
            NodeType::Client => format!("Client {}", node_id),
            NodeType::Drone => format!("Drone {}", node_id),
            NodeType::Server => format!("Server {}", node_id),
        };

        Self {
            node_id,
            node_type,
            label,
            selected: false,
            position,
        }
    }

    pub fn set_position(&mut self, new_position: Pos2) {
        self.position = new_position;
    }
}

#[derive(Clone, Debug)]
pub struct GraphEdgeData {
    pub from_id: NodeId,
    pub to_id: NodeId,
    pub selected: bool,
    pub label: Option<String>,
}

impl GraphEdgeData {
    pub fn new(from_id: NodeId, to_id: NodeId) -> Self {
        Self {
            from_id,
            to_id,
            selected: false,
            label: None,
        }
    }
}

impl GraphApp {
    pub fn new(
        connections: HashMap<NodeId, Vec<NodeId>>,
        node_types: HashMap<NodeId, NodeType>,
        receiver_updates: Receiver<GraphAction>,
        sender_node_clicked: Sender<NodeId>,
        receiver_bottom_messages: Receiver<ButtonsMessages>,
        sender_message_type: Sender<MessageType>,
        client_ui_state: Arc<Mutex<UiState>>,
        client_state_receiver: Receiver<(NodeId, ClientState)>,
    ) -> Self {
        
        let mut petgraph = StableGraph::<GraphNodeData, GraphEdgeData, Undirected>::with_capacity(0, 0);
        let mut node_id_to_index = HashMap::new();
        
        let initial_rect = egui::Rect::from_min_size(
            Pos2::new(0.0, 0.0),
            Vec2::new(1050.0, 700.0)
        );
        let initial_positions = Self::calculate_initial_positions_for_nodes(&node_types, initial_rect);
        
        for (&node_id, &node_type) in &node_types {
            let initial_pos = initial_positions.get(&node_id).copied()
                .unwrap_or(Pos2::new(525.0, 350.0));

            let node_data = GraphNodeData::new(node_id, node_type, initial_pos);
            let node_index = petgraph.add_node(node_data);
            node_id_to_index.insert(node_id, node_index);
        }
        
        for (&from_id, targets) in &connections {
            for &to_id in targets {
                if from_id < to_id {
                    if let (Some(&from_idx), Some(&to_idx)) =
                        (node_id_to_index.get(&from_id), node_id_to_index.get(&to_id)) {

                        let edge_data = GraphEdgeData::new(from_id, to_id);
                        let _ = petgraph.add_edge(from_idx, to_idx, edge_data);
                    }
                }
            }
        }

        let g = to_graph(&petgraph);

        let mut app = Self {
            petgraph,
            g,
            node_id_to_index,
            selected_nodes: Vec::new(),
            selected_edge: None,
            client_ui_state,
            receiver_updates,
            sender_node_clicked,
            receiver_bottom_messages,
            sender_message_type,
            client_state_receiver,
            labels_always: true,
            dragging_enabled: true,
            node_clicking_enabled: true,
            edge_clicking_enabled: true,
            node_selection_enabled: true,
            graph_dirty: false,
            selection_dirty: false,
        };

        app.sync_labels_to_egui_graph();
        app
    }
    
    fn get_node_data_mut(&mut self, node_id: NodeId) -> Option<&mut GraphNodeData> {
        let node_index = *self.node_id_to_index.get(&node_id)?;
        self.petgraph.node_weight_mut(node_index)
    }

    fn get_node_data(&self, node_id: NodeId) -> Option<&GraphNodeData> {
        let node_index = *self.node_id_to_index.get(&node_id)?;
        self.petgraph.node_weight(node_index)
    }
    
    fn send_node_clicked(&self, node_id: NodeId) {
        if let Err(e) = self.sender_node_clicked.try_send(node_id) {
            match e {
                crossbeam_channel::TrySendError::Full(_) => {
                    eprintln!("Channel for node click is full");
                }
                crossbeam_channel::TrySendError::Disconnected(_) => {
                    eprintln!("Channel for node click is disconnected");
                }
            }
        }
    }
    
    fn handle_node_click(&mut self, ui: &mut egui::Ui) {
        if let Some(&node_index) = self.g.selected_nodes().last() {
            for (node_id, idx) in &self.node_id_to_index {
                if *idx == node_index {
                    self.send_node_clicked(*node_id);
                    ui.ctx().request_repaint();
                    break;
                }
            }
        }
    }

    fn calculate_initial_positions_for_nodes(
        node_types: &HashMap<NodeId, NodeType>,
        rect: egui::Rect
    ) -> HashMap<NodeId, Pos2> {
        let mut positions = HashMap::new();
        let mut node_ids: Vec<NodeId> = node_types.keys().copied().collect();
        node_ids.sort();

        let node_count = node_ids.len();

        match node_count {
            0 => {},
            1 => {
                let center_pos = rect.center();
                positions.insert(node_ids[0], center_pos);
            },
            2..=8 => {
                // circular layout
                let center = rect.center();
                let radius = (rect.width().min(rect.height()) * 0.35).min(200.0);

                for (i, &node_id) in node_ids.iter().enumerate() {
                    let angle = 2.0 * std::f32::consts::PI * i as f32 / node_count as f32;
                    let pos = Pos2::new(
                        center.x + radius * angle.cos(),
                        center.y + radius * angle.sin()
                    );
                    positions.insert(node_id, pos);
                }
            },
            _ => {
                // grid layout
                let cols = (node_count as f32).sqrt().ceil() as usize;
                let rows = (node_count + cols - 1) / cols;
                let margin = 100.0;
                let available_width = rect.width() - 2.0 * margin;
                let available_height = rect.height() - 2.0 * margin;
                let cell_width = available_width / cols as f32;
                let cell_height = available_height / rows as f32;

                for (i, &node_id) in node_ids.iter().enumerate() {
                    let row = i / cols;
                    let col = i % cols;
                    let pos = Pos2::new(
                        rect.left() + margin + cell_width * (col as f32 + 0.5),
                        rect.top() + margin + cell_height * (row as f32 + 0.5)
                    );
                    positions.insert(node_id, pos);
                }
            }
        }
        positions
    }

    // ✅ Semplificato - usa direttamente GraphNodeData.position
    pub fn update_node_position(&mut self, node_id: NodeId, new_position: Pos2) -> Result<(), String> {
        if let Some(node_data) = self.get_node_data_mut(node_id) {
            node_data.set_position(new_position);
            Ok(())
        } else {
            Err(format!("Node {} non trovato", node_id))
        }
    }

    pub fn get_node_position(&self, node_id: NodeId) -> Option<Pos2> {
        self.get_node_data(node_id).map(|data| data.position)
    }

    fn sync_labels_to_egui_graph(&mut self) {
        for node_index in self.petgraph.node_indices() {
            if let Some(node_data) = self.petgraph.node_weight(node_index) {
                if let Some(egui_node) = self.g.node_mut(node_index) {
                    egui_node.set_label(node_data.label.clone());
                }
            }
        }

        for edge_index in self.petgraph.edge_indices() {
            if let Some(egui_edge) = self.g.edge_mut(edge_index) {
                egui_edge.set_label(String::new());
            }
        }
    }

    // ✅ Draw graph semplificato
    fn draw_graph(&mut self, ui: &mut egui::Ui) {
        self.check_for_position_updates();

        let widget = &mut GraphView::new(&mut self.g)
            .with_styles(
                &SettingsStyle::default()
                    .with_labels_always(self.labels_always)
            )
            .with_interactions(
                &SettingsInteraction::default()
                    .with_node_selection_enabled(self.node_selection_enabled)
                    .with_node_clicking_enabled(self.node_clicking_enabled)
                    .with_edge_clicking_enabled(self.edge_clicking_enabled)
                    .with_dragging_enabled(self.dragging_enabled)
            );

        let response = ui.add(widget);

        // ✅ Unifica gestione click qui
        if response.clicked() {
            self.handle_node_click(ui);
        }
    }

    fn check_for_position_updates(&mut self) {
        let mut updates: Vec<(NodeIndex, NodeId, Pos2)> = Vec::new();

        // Fase 1: Raccogli i nodi da aggiornare
        for node_index in self.petgraph.node_indices() {
            if let Some(node_data) = self.petgraph.node_weight(node_index) {
                let node_id = node_data.node_id;

                if let Some(egui_node) = self.g.node(node_index) {
                    let egui_pos = egui_node.location();
                    let current_pos = node_data.position;
                    let position_delta = (egui_pos - current_pos).length();

                    if position_delta > 1.0 {
                        updates.push((node_index, node_id, egui_pos));
                    }
                }
            }
        }

        // Fase 2: Applica gli aggiornamenti
        if !updates.is_empty() {
            for (node_index, _node_id, new_pos) in updates {
                if let Some(node_data_mut) = self.petgraph.node_weight_mut(node_index) {
                    node_data_mut.set_position(new_pos);
                }
            }
        }
    }

    fn update_egui_graph(&mut self) {
        if self.graph_dirty {
            self.g = to_graph(&self.petgraph);
            self.sync_positions_to_egui_graph();
            self.sync_labels_to_egui_graph();
            self.graph_dirty = false;
        }

        if self.selection_dirty {
            self.apply_selection_changes();
            self.selection_dirty = false;
        }
    }

    fn sync_positions_to_egui_graph(&mut self) {
        for node_index in self.petgraph.node_indices() {
            if let Some(node_data) = self.petgraph.node_weight(node_index) {
                if let Some(egui_node) = self.g.node_mut(node_index) {
                    egui_node.set_location(node_data.position);
                }
            }
        }
    }

    fn apply_selection_changes(&mut self) {
        let node_indices: Vec<NodeIndex> = self.petgraph.node_indices().collect();
        let edge_indices: Vec<EdgeIndex> = self.petgraph.edge_indices().collect();

        for node_index in node_indices {
            if let Some(node_data) = self.petgraph.node_weight_mut(node_index) {
                node_data.selected = self.selected_nodes.contains(&node_data.node_id);
            }
        }

        for edge_index in edge_indices {
            if let Some(edge_data) = self.petgraph.edge_weight_mut(edge_index) {
                edge_data.selected = self.selected_edge == Some((edge_data.from_id, edge_data.to_id)) ||
                    self.selected_edge == Some((edge_data.to_id, edge_data.from_id));
            }
        }
    }

    pub fn handle_pending_events(&mut self) {
        // GraphAction events
        if let Ok(command) = self.receiver_updates.try_recv() {
            let _ = match command {
                GraphAction::AddNode(id, node_type) => self.add_node(id, node_type),
                GraphAction::RemoveNode(id) => self.remove_node(id),
                GraphAction::AddEdge(id1, id2) => self.add_edge(id1, id2),
                GraphAction::RemoveEdge(id1, id2) => self.remove_edge(id1, id2),
            };
        }

        // Messaggi da ButtonWindow
        if let Ok(message) = self.receiver_bottom_messages.try_recv() {
            match message {
                ButtonsMessages::DeselectNode(id) => {
                    self.selected_nodes.retain(|&node_id| node_id != id);
                    self.selected_edge = None;
                    self.selection_dirty = true;
                }
                ButtonsMessages::UpdateSelection(node1, node2) => {
                    self.selected_nodes.clear();
                    if let Some(node_id) = node1 {
                        self.selected_nodes.push(node_id);
                    }
                    if let Some(node_id) = node2 {
                        self.selected_nodes.push(node_id);
                    }
                    self.selected_edge = None;
                    self.selection_dirty = true;
                }
                ButtonsMessages::UpdateEdgeSelection(from_id, to_id) => {
                    self.selected_nodes.clear();
                    self.selected_edge = Some((from_id, to_id));
                    self.selection_dirty = true;
                }
                ButtonsMessages::ClearAllSelections => {
                    self.selected_nodes.clear();
                    self.selected_edge = None;
                    self.selection_dirty = true;
                }
                _ => {}
            }
        }
    }

    pub fn add_node(&mut self, new_node_id: NodeId, node_type: NodeType) -> Result<(), String> {
        if self.node_id_to_index.contains_key(&new_node_id) {
            return Err(format!("Node {} già esiste", new_node_id));
        }

        let new_position = self.calculate_position_for_new_node();
        let node_data = GraphNodeData::new(new_node_id, node_type, new_position);

        let node_index = self.petgraph.add_node(node_data);
        self.node_id_to_index.insert(new_node_id, node_index);

        self.graph_dirty = true;

        if node_type == NodeType::Client {
            if let Ok((id, client_state)) = self.client_state_receiver.try_recv() {
                match self.client_ui_state.lock() {
                    Ok(mut state) => {
                        state.add_client(id, client_state)
                    }
                    Err(_) => {
                        eprintln!("Error: Mutex is poisoned")
                    }
                }
            }
        }

        Ok(())
    }

    fn calculate_position_for_new_node(&self) -> Pos2 {
        // Posiziona al centro o trova spazio libero
        let center = Pos2::new(525.0, 350.0);
        let mut candidate_pos = center;
        let mut offset = 60.0;
        let mut attempts = 0;

        // Controlla se la posizione è troppo vicina ad altri nodi esistenti
        while self.is_position_too_close(candidate_pos, 120.0) && attempts < 16 {
            // Spirale attorno al centro
            let angle = 2.0 * std::f32::consts::PI * attempts as f32 / 8.0;
            candidate_pos = Pos2::new(
                center.x + offset * angle.cos(),
                center.y + offset * angle.sin()
            );

            attempts += 1;
            if attempts % 8 == 0 {
                offset += 60.0;
            }
        }

        candidate_pos
    }
    
    fn is_position_too_close(&self, pos: Pos2, min_distance: f32) -> bool {
        self.petgraph.node_weights()
            .any(|node_data| pos.distance(node_data.position) < min_distance)
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Result<(), String> {
        if let Some(&node_index) = self.node_id_to_index.get(&node_id) {
            self.petgraph.remove_node(node_index);
            self.node_id_to_index.remove(&node_id);

            self.selected_nodes.retain(|&id| id != node_id);
            if let Some((from, to)) = self.selected_edge {
                if from == node_id || to == node_id {
                    self.selected_edge = None;
                }
            }

            self.graph_dirty = true;
            Ok(())
        } else {
            Err(format!("Node {} non trovato", node_id))
        }
    }

    pub fn add_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if let (Some(&idx1), Some(&idx2)) =
            (self.node_id_to_index.get(&id1), self.node_id_to_index.get(&id2)) {

            if self.petgraph.find_edge(idx1, idx2).is_some() {
                return Err(format!("Edge {} ↔ {} già esiste", id1, id2));
            }

            let edge_data = GraphEdgeData::new(id1, id2);
            self.petgraph.add_edge(idx1, idx2, edge_data);

            self.graph_dirty = true;
            Ok(())
        } else {
            Err("Uno o entrambi i nodi non trovati".to_string())
        }
    }

    pub fn remove_edge(&mut self, id1: NodeId, id2: NodeId) -> Result<(), String> {
        if let (Some(&idx1), Some(&idx2)) =
            (self.node_id_to_index.get(&id1), self.node_id_to_index.get(&id2)) {

            if let Some(edge_index) = self.petgraph.find_edge(idx1, idx2) {
                self.petgraph.remove_edge(edge_index);

                if self.selected_edge == Some((id1, id2)) || self.selected_edge == Some((id2, id1)) {
                    self.selected_edge = None;
                }

                self.graph_dirty = true;
                Ok(())
            } else {
                Err(format!("Edge {} ↔ {} non esiste", id1, id2))
            }
        } else {
            Err("Uno o entrambi i nodi non trovati".to_string())
        }
    }
}

impl Drawable for GraphApp {
    fn update(&mut self) {
    }
    
    fn render(&mut self, ui: &mut egui::Ui) {
        self.handle_pending_events();
        self.update_egui_graph();

        ui.label(
            RichText::new("Network Graph")
                .heading()
                .color(DARK_BLUE)
        );

        self.draw_graph(ui);
        ui.separator();
    }

    fn needs_continuous_updates(&self) -> bool {
        true
    }
}

impl PanelDrawable for GraphApp {
    fn preferred_panel(&self) -> PanelType {
        PanelType::Central
    }

    fn preferred_size(&self) -> Option<Vec2> {
        None
    }

    fn is_resizable(&self) -> bool {
        true
    }
}