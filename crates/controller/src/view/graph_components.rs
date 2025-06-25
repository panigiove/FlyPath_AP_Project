use egui::{Pos2, Vec2, TextureId, Color32};
use eframe::epaint::{Rect, Stroke, Rounding, Shape};
use wg_2024::network::NodeId;
use crate::utility::NodeType;
use std::collections::HashSet;

type NodePayload = (NodeId, NodeType);

#[derive(Clone)]
pub struct CustomNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub image_path: String,
    pub position: Pos2,
    pub label: String,
    pub size: Vec2,
    pub texture_id: Option<TextureId>,
    pub selected: bool,
}

impl CustomNode {
    pub fn new(id: NodeId, node_type: NodeType, image_path: String, position: Pos2, size: Vec2, texture_id: TextureId) -> Self {
        let label = match node_type {
            NodeType::Drone => format!("Drone {}", id),
            NodeType::Server => format!("Server {}", id),
            NodeType::Client => format!("Client {}", id),
        };

        Self {
            id,
            node_type,
            image_path,
            position,
            label,
            size,
            texture_id: Some(texture_id),
            selected: false,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    // Colori coerenti con il design originale
    fn get_color(&self, selected: bool) -> Color32 {
        if selected {
            // Oro quando selezionato
            Color32::from_rgb(255, 215, 0)
        } else {
            // Colori normali per tipo
            match self.node_type {
                NodeType::Client => Color32::from_rgb(100, 200, 100),   // Verde
                NodeType::Drone => Color32::from_rgb(100, 150, 255),    // Blu
                NodeType::Server => Color32::from_rgb(255, 150, 100),   // Arancione
            }
        }
    }

    fn get_border_color(&self, selected: bool) -> Color32 {
        if selected {
            Color32::from_rgb(255, 215, 0) // Oro quando selezionato
        } else {
            Color32::BLACK // Nero normale
        }
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn set_position(&mut self, position: Pos2) {
        self.position = position;
    }

    pub fn get_position(&self) -> Pos2 {
        self.position
    }

    pub fn draw(&self, ui: &mut egui::Ui, painter: &egui::Painter) {
        let half_size = Vec2::new(self.size.x / 2.0, self.size.y / 2.0);
        let min_pos = Pos2::new(self.position.x - half_size.x, self.position.y - half_size.y);
        let max_pos = Pos2::new(self.position.x + half_size.x, self.position.y + half_size.y);
        let rect = Rect::from_min_max(min_pos, max_pos);

        // Aureola se selezionato
        if self.selected {
            let expanded_rect = Rect::from_min_max(
                Pos2::new(min_pos.x - 4.0, min_pos.y - 4.0),
                Pos2::new(max_pos.x + 4.0, max_pos.y + 4.0)
            );
            let halo_shape = Shape::rect_filled(expanded_rect, 8.0, Color32::from_rgba_unmultiplied(255, 215, 0, 200));
            painter.add(halo_shape);
        }

        // Texture o colore di riempimento
        if let Some(texture_id) = self.texture_id {
            let epaint_texture_id: epaint::TextureId = texture_id.into();
            let texture_shape = Shape::image(
                epaint_texture_id,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE
            );
            painter.add(texture_shape);
        } else {
            let color = self.get_color(self.selected);
            let fill_shape = Shape::rect_filled(rect, 6.0, color);
            painter.add(fill_shape);
        }

        // Bordo
        let border_color = self.get_border_color(self.selected);
        let stroke_width = if self.selected { 4.0 } else { 2.0 };
        let rounding = Rounding::same(6.0);
        let stroke = Stroke::new(stroke_width, border_color);
        let border_shape = Shape::rect_stroke(rect, rounding, stroke);
        painter.add(border_shape);

        // Label (opzionale)
        if !self.label.is_empty() {
            let text_pos = Pos2::new(self.position.x, max_pos.y + 15.0);
            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                &self.label,
                egui::FontId::default(),
                if self.selected { Color32::from_rgb(255, 215, 0) } else { Color32::BLACK }
            );
        }
    }

    pub fn contains_point(&self, point: Pos2) -> bool {
        let half_size = self.size / 2.0;
        let min_pos = self.position - half_size;
        let max_pos = self.position + half_size;

        point.x >= min_pos.x && point.x <= max_pos.x &&
            point.y >= min_pos.y && point.y <= max_pos.y
    }

    pub fn closest_boundary_point(&self, direction: Vec2) -> Pos2 {
        let half_size = self.size / 2.0;
        let center = self.position;

        let dir_normalized = if direction.length() > 0.0 {
            direction.normalized()
        } else {
            Vec2::new(1.0, 0.0)
        };

        let t_x = if dir_normalized.x != 0.0 {
            half_size.x / dir_normalized.x.abs()
        } else {
            f32::INFINITY
        };

        let t_y = if dir_normalized.y != 0.0 {
            half_size.y / dir_normalized.y.abs()
        } else {
            f32::INFINITY
        };

        let t = t_x.min(t_y);
        let offset = dir_normalized * t;
        Pos2::new(center.x + offset.x, center.y + offset.y)
    }

    pub fn update_from_payload(&mut self, payload: &NodePayload) {
        self.id = payload.0;
        self.node_type = payload.1;

        // Aggiorna label se è quella di default
        if self.label.starts_with("Client ") ||
            self.label.starts_with("Drone ") ||
            self.label.starts_with("Server ") {
            self.label = match self.node_type {
                NodeType::Drone => format!("Drone {}", self.id),
                NodeType::Server => format!("Server {}", self.id),
                NodeType::Client => format!("Client {}", self.id),
            };
        }
    }
}

#[derive(Clone)]
pub struct CustomEdge {
    pub selected: bool,
    pub label: String,
    pub width: f32,
}

impl CustomEdge {
    pub fn new() -> Self {
        Self {
            selected: false,
            label: String::new(),
            width: 2.0,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    fn get_edge_color(&self, selected: bool) -> Color32 {
        if selected {
            Color32::from_rgb(255, 215, 0) // Oro quando selezionato
        } else {
            Color32::from_rgb(150, 150, 150) // Grigio normale
        }
    }

    fn get_edge_width(&self, selected: bool) -> f32 {
        if selected {
            self.width + 2.0
        } else {
            self.width
        }
    }

    pub fn draw(&self, painter: &egui::Painter, start_pos: Pos2, end_pos: Pos2) {
        let color = self.get_edge_color(self.selected);
        let width = self.get_edge_width(self.selected);

        // Aureola se selezionato
        if self.selected {
            let glow_stroke = Stroke::new(width + 2.0, Color32::from_rgba_unmultiplied(255, 215, 0, 100));
            let glow_shape = Shape::line_segment([start_pos, end_pos], glow_stroke);
            painter.add(glow_shape);
        }

        // Linea principale
        let stroke = Stroke::new(width, color);
        let line_shape = Shape::line_segment([start_pos, end_pos], stroke);
        painter.add(line_shape);

        // Label (opzionale)
        if !self.label.is_empty() {
            let mid_point = Pos2::new(
                (start_pos.x + end_pos.x) / 2.0,
                (start_pos.y + end_pos.y) / 2.0
            );
            painter.text(
                mid_point,
                egui::Align2::CENTER_CENTER,
                &self.label,
                egui::FontId::default(),
                if self.selected { Color32::from_rgb(255, 215, 0) } else { Color32::BLACK }
            );
        }
    }

    pub fn contains_point(&self, start_pos: Pos2, end_pos: Pos2, point: Pos2) -> bool {
        // Calcola distanza dal punto alla linea
        let line_vec = end_pos - start_pos;
        let point_vec = point - start_pos;

        if line_vec.length_sq() < f32::EPSILON {
            return false;
        }

        let t = (point_vec.dot(line_vec) / line_vec.length_sq()).clamp(0.0, 1.0);
        let projection = start_pos + t * line_vec;
        let distance = (point - projection).length();

        let tolerance = if self.selected { 8.0 } else { 5.0 };
        distance < tolerance
    }
}

impl Default for CustomEdge {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct GraphSelectionState {
    pub selected_nodes: HashSet<NodeId>,
    pub selected_edges: HashSet<(NodeId, NodeId)>,
}

impl GraphSelectionState {
    pub fn new() -> Self {
        Self {
            selected_nodes: HashSet::new(),
            selected_edges: HashSet::new(),
        }
    }

    pub fn select_node(&mut self, node_id: NodeId) {
        self.selected_edges.clear();
        self.selected_nodes.insert(node_id);
    }

    pub fn deselect_node(&mut self, node_id: NodeId) {
        self.selected_nodes.remove(&node_id);
    }

    pub fn toggle_node(&mut self, node_id: NodeId) {
        if self.selected_nodes.contains(&node_id) {
            self.deselect_node(node_id);
        } else {
            self.select_node(node_id);
        }
    }

    pub fn select_edge(&mut self, edge: (NodeId, NodeId)) {
        self.selected_nodes.clear();
        let normalized_edge = if edge.0 < edge.1 { edge } else { (edge.1, edge.0) };
        self.selected_edges.insert(normalized_edge);
    }

    pub fn deselect_edge(&mut self, edge: (NodeId, NodeId)) {
        let normalized_edge = if edge.0 < edge.1 { edge } else { (edge.1, edge.0) };
        self.selected_edges.remove(&normalized_edge);
    }

    pub fn toggle_edge(&mut self, edge: (NodeId, NodeId)) {
        let normalized_edge = if edge.0 < edge.1 { edge } else { (edge.1, edge.0) };
        if self.selected_edges.contains(&normalized_edge) {
            self.deselect_edge(edge);
        } else {
            self.select_edge(edge);
        }
    }

    pub fn is_node_selected(&self, node_id: NodeId) -> bool {
        self.selected_nodes.contains(&node_id)
    }

    pub fn is_edge_selected(&self, edge: (NodeId, NodeId)) -> bool {
        let normalized_edge = if edge.0 < edge.1 { edge } else { (edge.1, edge.0) };
        self.selected_edges.contains(&normalized_edge)
    }

    pub fn clear_all(&mut self) {
        self.selected_nodes.clear();
        self.selected_edges.clear();
    }

    pub fn get_selected_nodes(&self) -> &HashSet<NodeId> {
        &self.selected_nodes
    }

    pub fn get_selected_edges(&self) -> &HashSet<(NodeId, NodeId)> {
        &self.selected_edges
    }
}

// Strutture helper per l'integrazione con egui_graphs
#[derive(Clone)]
pub struct NodeData {
    pub payload: NodePayload,
    pub location: Pos2,
    pub selected: bool,
    pub custom_node: CustomNode,
}

impl NodeData {
    pub fn new(payload: NodePayload, location: Pos2, texture_id: Option<TextureId>) -> Self {
        let image_path = match payload.1 {
            NodeType::Client => "assets/client.png".to_string(),
            NodeType::Drone => "assets/drone.png".to_string(),
            NodeType::Server => "assets/server.png".to_string(),
        };

        let mut custom_node = CustomNode::new(
            payload.0,
            payload.1,
            image_path,
            location,
            Vec2::new(50.0, 50.0),
            texture_id.unwrap_or(TextureId::Managed(0))
        );

        if texture_id.is_none() {
            custom_node.texture_id = None;
        }

        Self {
            payload,
            location,
            selected: false,
            custom_node,
        }
    }

    pub fn update(&mut self) {
        self.custom_node.set_position(self.location);
        self.custom_node.set_selected(self.selected);
        self.custom_node.update_from_payload(&self.payload);
    }
}

#[derive(Clone)]
pub struct EdgeData {
    pub payload: (),
    pub selected: bool,
    pub custom_edge: CustomEdge,
}

impl EdgeData {
    pub fn new(payload: ()) -> Self {
        Self {
            payload,
            selected: false,
            custom_edge: CustomEdge::new(),
        }
    }

    pub fn update(&mut self) {
        self.custom_edge.set_selected(self.selected);
    }
}

// Funzioni helper per creare nodi e edge con le texture appropriate
pub fn create_node_with_texture(
    node_id: NodeId,
    node_type: NodeType,
    position: Pos2,
    texture_id: Option<TextureId>
) -> NodeData {
    NodeData::new((node_id, node_type), position, texture_id)
}

pub fn create_edge() -> EdgeData {
    EdgeData::new(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function per creare un texture ID mock
    fn create_mock_texture_id() -> TextureId {
        TextureId::Managed(0)
    }

    // Helper function per creare un CustomNode di test
    fn create_test_node(id: NodeId, node_type: NodeType) -> CustomNode {
        CustomNode::new(
            id,
            node_type,
            "test_path.png".to_string(),
            Pos2::new(100.0, 100.0),
            Vec2::new(50.0, 50.0),
            create_mock_texture_id()
        )
    }

    #[test]
    fn test_custom_node_new() {
        let node = CustomNode::new(
            1,
            NodeType::Drone,
            "assets/drone.png".to_string(),
            Pos2::new(50.0, 50.0),
            Vec2::new(40.0, 40.0),
            create_mock_texture_id()
        );

        assert_eq!(node.id, 1);
        assert_eq!(node.node_type, NodeType::Drone);
        assert_eq!(node.image_path, "assets/drone.png");
        assert_eq!(node.position, Pos2::new(50.0, 50.0));
        assert_eq!(node.label, "Drone 1");
        assert_eq!(node.size, Vec2::new(40.0, 40.0));
        assert!(node.texture_id.is_some());
        assert!(!node.selected);
    }

    #[test]
    fn test_custom_node_selection() {
        let mut node = create_test_node(1, NodeType::Drone);

        // Test stato iniziale
        assert!(!node.is_selected());

        // Test selezione
        node.set_selected(true);
        assert!(node.is_selected());

        // Test deselezione
        node.set_selected(false);
        assert!(!node.is_selected());
    }

    #[test]
    fn test_get_color_with_selection() {
        let node = create_test_node(1, NodeType::Drone);

        // Test colori normali
        let normal_color = node.get_color(false);
        assert_eq!(normal_color, Color32::from_rgb(100, 150, 255));

        // Test colori selezionati
        let selected_color = node.get_color(true);
        assert_eq!(selected_color, Color32::from_rgb(255, 215, 0)); // Oro

        // Verifica che siano diversi
        assert_ne!(normal_color, selected_color);
    }

    #[test]
    fn test_custom_node_contains_point() {
        let node = create_test_node(1, NodeType::Drone);

        // Test punto all'interno
        assert!(node.contains_point(Pos2::new(100.0, 100.0))); // Centro
        assert!(node.contains_point(Pos2::new(90.0, 90.0)));   // Interno

        // Test punto all'esterno
        assert!(!node.contains_point(Pos2::new(200.0, 200.0))); // Esterno
        assert!(!node.contains_point(Pos2::new(50.0, 50.0)));   // Bordo esterno
    }

    #[test]
    fn test_graph_selection_state() {
        let mut state = GraphSelectionState::new();

        // Test stato iniziale
        assert!(!state.is_node_selected(1));
        assert!(!state.is_edge_selected((1, 2)));

        // Test selezione nodo
        state.select_node(1);
        assert!(state.is_node_selected(1));

        // Test selezione edge (dovrebbe deselezionare nodi)
        state.select_edge((1, 2));
        assert!(!state.is_node_selected(1));
        assert!(state.is_edge_selected((1, 2)));
        assert!(state.is_edge_selected((2, 1))); // Test normalizzazione

        // Test clear
        state.clear_all();
        assert!(!state.is_node_selected(1));
        assert!(!state.is_edge_selected((1, 2)));
    }

    #[test]
    fn test_selection_state_edge_normalization() {
        let mut state = GraphSelectionState::new();

        // Test che (1,2) e (2,1) siano trattati come lo stesso edge
        state.select_edge((1, 2));
        assert!(state.is_edge_selected((1, 2)));
        assert!(state.is_edge_selected((2, 1)));

        state.deselect_edge((2, 1));
        assert!(!state.is_edge_selected((1, 2)));
        assert!(!state.is_edge_selected((2, 1)));
    }

    #[test]
    fn test_custom_edge_contains_point() {
        let edge = CustomEdge::new();
        let start = Pos2::new(0.0, 0.0);
        let end = Pos2::new(100.0, 0.0);

        // Test punto sulla linea
        assert!(edge.contains_point(start, end, Pos2::new(50.0, 0.0)));

        // Test punto vicino alla linea (entro tolleranza)
        assert!(edge.contains_point(start, end, Pos2::new(50.0, 3.0)));

        // Test punto lontano dalla linea
        assert!(!edge.contains_point(start, end, Pos2::new(50.0, 20.0)));
    }

    #[test]
    fn test_node_data_creation() {
        let payload = (1, NodeType::Client);
        let position = Pos2::new(100.0, 100.0);
        let texture_id = Some(create_mock_texture_id());

        let node_data = NodeData::new(payload, position, texture_id);

        assert_eq!(node_data.payload.0, 1);
        assert_eq!(node_data.payload.1, NodeType::Client);
        assert_eq!(node_data.location, position);
        assert!(!node_data.selected);
        assert!(node_data.custom_node.texture_id.is_some());
    }

    #[test]
    fn test_edge_data_creation() {
        let edge_data = EdgeData::new(());

        assert!(!edge_data.selected);
        assert!(!edge_data.custom_edge.is_selected());
    }
}