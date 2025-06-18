use egui::{Pos2, Vec2, TextureId, Color32};
use eframe::epaint::{Rect, Stroke, Rounding, Shape};
use egui_graphs::{Graph, Node, NodeProps, EdgeProps, DefaultNodeShape, DefaultEdgeShape, DisplayNode, DrawContext, DisplayEdge};
use wg_2024::network::NodeId;
use crate::utility::NodeType;
use eframe::egui;

use petgraph::{EdgeType, stable_graph::IndexType, Undirected};

const COLOR: Color32 = Color32::WHITE;
const RADIUS: f32 = 5.0;

// Make sure NodeId is u8 or define the correct type alias
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
    pub selected: bool, // NUOVO: stato di selezione interno
}

impl CustomNode {
    pub fn new(id: NodeId, node_type: NodeType, image_path: String, position: Pos2, size: Vec2, texture_id: TextureId) -> Self {
        let label = match node_type {
            NodeType::Drone => format!("Drone {}", id),
            NodeType::Server => format!("Server {}", id),
            NodeType::Client => format!("Client {}", id),
        };
        let texture = Some(texture_id);

        Self {
            id,
            node_type,
            image_path,
            position,
            label,
            size,
            texture_id: texture,
            selected: false, // NUOVO: inizialmente non selezionato
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    // MODIFICATO: Aggiornato per accettare il parametro selected
    fn get_color(&self, selected: bool) -> Color32 {
        if selected {
            // Colori più vivaci quando selezionato
            match self.node_type {
                NodeType::Client => Color32::from_rgb(14, 137, 145),
                NodeType::Drone => Color32::from_rgb(14, 137, 145),
                NodeType::Server => Color32::from_rgb(14, 137, 145),
            }
        } else {
            // Colori normali
            match self.node_type {
                NodeType::Client => Color32::from_rgb(141, 182, 188),
                NodeType::Drone => Color32::from_rgb(141, 182, 188),
                NodeType::Server => Color32::from_rgb(141, 182, 188),
            }
        }
    }

    fn get_border_color(&self, selected: bool) -> Color32 {
        if selected {
            Color32::YELLOW
        } else {
            Color32::GRAY
        }
    }

    // NUOVO: Metodo per impostare lo stato di selezione
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    // NUOVO: Metodo per ottenere lo stato di selezione
    pub fn is_selected(&self) -> bool {
        self.selected
    }
}

impl From<NodeProps<(NodeId, NodeType)>> for CustomNode {
    fn from(node_props: NodeProps<(NodeId, NodeType)>) -> Self {
        let mut label = match node_props.payload.1 {
            NodeType::Client => "Client #".to_string(),
            NodeType::Drone => "Drone #".to_string(),
            NodeType::Server => "Server #".to_string(),
        };
        let mut image_path = match node_props.payload.1 {
            NodeType::Client => "crates/controller/src/view/assets/client.png".to_string(),
            NodeType::Drone => "crates/controller/src/view/assets/drone.png".to_string(),
            NodeType::Server => "crates/controller/src/view/assets/server.png".to_string(),
        };

        label.push_str(&node_props.payload.0.to_string());

        // Extract position from node_props if available
        let position = node_props.location; // Usa la posizione dalle proprietà

        Self {
            id: node_props.payload.0,
            node_type: node_props.payload.1,
            image_path,
            position,
            label,
            size: Vec2::new(50.0, 50.0),
            texture_id: None,
            selected: node_props.selected, // NUOVO: usa lo stato di selezione dalle proprietà
        }
    }
}

impl <E: Clone> DisplayNode <NodePayload, E, Undirected, u32> for CustomNode{
    fn closest_boundary_point(&self, dir: Vec2) -> Pos2 {
        let half_size = self.size / 2.0;
        let center = self.position;

        // Normalize direction
        let dir_normalized = if dir.length() > 0.0 {
            dir.normalized()
        } else {
            emath::Vec2::new(1.0, 0.0) // Default direction if zero
        };

        // Calculate intersection with rectangle border
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

        // Take minimum to find first intersection point
        let t = t_x.min(t_y);

        let offset = dir_normalized * t;
        Pos2::new(center.x + offset.x, center.y + offset.y)
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        let mut shapes = Vec::new();

        // Calculate rectangle for texture
        let half_size = Vec2::new(self.size.x / 2.0, self.size.y / 2.0);
        let min_pos = Pos2::new(self.position.x - half_size.x, self.position.y - half_size.y);
        let max_pos = Pos2::new(self.position.x + half_size.x, self.position.y + half_size.y);
        let rect = Rect::from_min_max(min_pos, max_pos);

        // MODIFICATO: Usa lo stato di selezione interno
        let is_selected = self.selected;

        // Se è selezionato, aggiungi un'aureola
        if is_selected {
            let expanded_rect = Rect::from_min_max(
                Pos2::new(min_pos.x - 3.0, min_pos.y - 3.0),
                Pos2::new(max_pos.x + 3.0, max_pos.y + 3.0)
            );
            let halo_shape = Shape::rect_filled(expanded_rect, 8.0, Color32::from_rgba_unmultiplied(255, 255, 0, 128));
            shapes.push(halo_shape);
        }

        // If we have a texture, render it
        if let Some(texture_id) = self.texture_id {
            let epaint_texture_id: epaint::TextureId = texture_id.into();
            let texture_shape = Shape::image(
                epaint_texture_id,
                rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE
            );
            shapes.push(texture_shape);
        } else {
            // MODIFICATO: Usa il colore basato sulla selezione
            let color = self.get_color(is_selected);
            let epaint_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a());
            let fill_shape = Shape::rect_filled(rect, 5.0, epaint_color);
            shapes.push(fill_shape);
        }

        // MODIFICATO: Bordo più evidente se selezionato
        let border_color = self.get_border_color(is_selected);
        let stroke_width = if is_selected { 4.0 } else { 2.0 };

        use eframe::epaint::{Shape, Stroke, Color32, Rect, Rounding};

        let rounding = Rounding::same(5.0);
        let stroke = Stroke::new(stroke_width, border_color);
        let border_shape = Shape::rect_stroke(rect, rounding, stroke);
        shapes.push(border_shape);

        shapes
    }

    fn update(&mut self, state: &NodeProps<NodePayload>) {
        // Update id and type if necessary
        self.id = state.clone().payload.0;
        self.node_type = state.clone().payload.1;

        // NUOVO: Aggiorna lo stato di selezione
        self.selected = state.selected;

        // Update position from state
        self.position = state.location;

        // If label hasn't been customized, update it based on type
        if self.label.starts_with("Client #") ||
            self.label.starts_with("Drone #") ||
            self.label.starts_with("Server #") {
            self.label = match self.node_type {
                NodeType::Drone => format!("Drone #{}", self.id),
                NodeType::Server => format!("Server #{}", self.id),
                NodeType::Client => format!("Client #{}", self.id),
            };
        }
    }

    fn is_inside(&self, pos: Pos2) -> bool {
        let half_size = self.size / 2.0;
        let min_pos = self.position - half_size;
        let max_pos = self.position + half_size;

        pos.x >= min_pos.x && pos.x <= max_pos.x &&
            pos.y >= min_pos.y && pos.y <= max_pos.y
    }
}

#[derive(Clone)]
pub struct CustomEdge {
    pub default: DefaultEdgeShape,
    pub selected: bool, // NUOVO: stato di selezione interno
}

impl CustomEdge {
    pub fn new<E: Clone>(payload: E) -> Self {
        let props = EdgeProps {
            payload,
            order: 0,
            selected: false,
            label: String::new(),
        };
        Self {
            default: DefaultEdgeShape::from(props),
            selected: false, // NUOVO: inizialmente non selezionato
        }
    }

    // NUOVO: Metodo per impostare lo stato di selezione
    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    // NUOVO: Metodo per ottenere lo stato di selezione
    pub fn is_selected(&self) -> bool {
        self.selected
    }

    // NUOVO: Metodo per ottenere il colore dell'edge
    fn get_edge_color(&self, selected: bool) -> Color32 {
        if selected {
            Color32::from_rgb(234, 162, 124)
        } else {
            Color32::from_rgb(232, 187, 166)
        }
    }

    // NUOVO: Metodo per ottenere lo spessore dell'edge
    fn get_edge_width(&self, selected: bool) -> f32 {
        if selected {
            4.0 // Più spesso quando selezionato
        } else {
            2.0 // Spessore normale
        }
    }
}

impl<E: Clone> From<EdgeProps<E>> for CustomEdge {
    fn from(props: EdgeProps<E>) -> Self {
        Self {
            default: DefaultEdgeShape::from(props.clone()),
            selected: props.selected, // NUOVO: usa lo stato di selezione dalle proprietà
        }
    }
}

impl<N: Clone, E: Clone, Ty: EdgeType, Ix: IndexType, D: DisplayNode<N, E, Ty, Ix>> DisplayEdge<N, E, Ty, Ix, D> for CustomEdge{
    fn shapes(&mut self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, ctx: &DrawContext) -> Vec<Shape> {
        let start_display = start.display();
        let end_display = end.display();

        // Uso emath::Vec2 e emath::Pos2 per closest_boundary_point
        let start_center_emath = start_display.closest_boundary_point(Vec2::new(0.0, 0.0));
        let end_center_emath = end_display.closest_boundary_point(Vec2::new(0.0, 0.0));

        // Converto in egui::Pos2 per usarli in Shape
        let start_center = egui::Pos2::new(start_center_emath.x, start_center_emath.y);
        let end_center = egui::Pos2::new(end_center_emath.x, end_center_emath.y);

        let start_to_end = egui::Vec2::new(end_center.x - start_center.x, end_center.y - start_center.y);
        let end_to_start = egui::Vec2::new(start_center.x - end_center.x, start_center.y - end_center.y);

        // Ottengo punti di bordo sempre in emath, poi converto in egui
        let start_pos_emath = start_display.closest_boundary_point(Vec2::new(start_to_end.x, start_to_end.y));
        let end_pos_emath = end_display.closest_boundary_point(Vec2::new(end_to_start.x, end_to_start.y));

        let start_pos = egui::Pos2::new(start_pos_emath.x, start_pos_emath.y);
        let end_pos = egui::Pos2::new(end_pos_emath.x, end_pos_emath.y);

        // MODIFICATO: Usa colore e spessore basati sulla selezione
        let is_selected = self.selected;
        let color = self.get_edge_color(is_selected);
        let width = self.get_edge_width(is_selected);

        let stroke = egui::Stroke::new(width, color);

        // Se è selezionato, aggiungi un effetto glow
        let mut shapes = Vec::new();

        if is_selected {
            // Aggiungi un'aureola per l'edge selezionato
            let glow_stroke = egui::Stroke::new(width + 2.0, Color32::from_rgba_unmultiplied(255, 255, 0, 100));
            let glow_shape = egui::Shape::line_segment([start_pos, end_pos], glow_stroke);
            shapes.push(glow_shape);
        }

        // Ora aggiungi la linea principale
        let line_shape = egui::Shape::line_segment([start_pos, end_pos], stroke);
        shapes.push(line_shape);

        shapes
    }

    fn update(&mut self, state: &EdgeProps<E>) {
        // NUOVO: Aggiorna lo stato di selezione
        self.selected = state.selected;

        // Delega al default per altri aggiornamenti
        <DefaultEdgeShape as DisplayEdge<N, E, Ty, Ix, DefaultNodeShape>>::update(&mut self.default, state)
    }

    fn is_inside(&self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, pos: Pos2) -> bool {
        // Simple implementation for hit testing
        let start_display = start.display();
        let end_display = end.display();

        // Get center positions using closest_boundary_point with zero direction
        let start_center: egui::Pos2 = start_display.closest_boundary_point(egui::Vec2::new(0.0, 0.0));
        let end_center: egui::Pos2 = end_display.closest_boundary_point(egui::Vec2::new(0.0, 0.0));

        // Calculate direction vectors
        let start_to_end = end_center - start_center;
        let end_to_start = start_center - end_center;

        // Get boundary points
        let start_pos = start_display.closest_boundary_point(start_to_end);
        let end_pos = end_display.closest_boundary_point(end_to_start);

        // Calculate distance from point to line
        let line_vec = end_pos - start_pos;
        let point_vec = pos - start_pos;

        if line_vec.length_sq() < f32::EPSILON {
            return false;
        }

        let t = (point_vec.dot(line_vec) / line_vec.length_sq()).clamp(0.0, 1.0);
        let projection = start_pos + t * line_vec;
        let distance = (pos - projection).length();

        // MODIFICATO: Tolleranza maggiore se selezionato
        let tolerance = if self.selected { 8.0 } else { 5.0 };
        distance < tolerance
    }
}

// NUOVO: Struttura per gestire lo stato di selezione del grafo
#[derive(Default)]
pub struct GraphSelectionState {
    pub selected_nodes: std::collections::HashSet<NodeId>,
    pub selected_edges: std::collections::HashSet<(NodeId, NodeId)>,
}

impl GraphSelectionState {
    pub fn new() -> Self {
        Self {
            selected_nodes: std::collections::HashSet::new(),
            selected_edges: std::collections::HashSet::new(),
        }
    }

    pub fn select_node(&mut self, node_id: NodeId) {
        // Deseleziona tutti gli edge quando si seleziona un nodo
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
        // Deseleziona tutti i nodi quando si seleziona un edge
        self.selected_nodes.clear();
        // Normalizza l'edge (il più piccolo prima)
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

    pub fn get_selected_nodes(&self) -> &std::collections::HashSet<NodeId> {
        &self.selected_nodes
    }

    pub fn get_selected_edges(&self) -> &std::collections::HashSet<(NodeId, NodeId)> {
        &self.selected_edges
    }
}

// NUOVO: Utility function per aggiornare lo stato di selezione dei nodi nel grafo
pub fn update_node_selection_in_graph<E: Clone, Ty: EdgeType, Ix: IndexType>(
    graph: &mut Graph<CustomNode, E, Ty, Ix>,
    selection_state: &GraphSelectionState
) {
    let node_indices: Vec<_> = graph.g.node_indices().collect();
    for node_idx in node_indices {
        if let Some(node) = graph.node_mut(node_idx) {
            // ... your code that works with the mutable node
        }
    }
}

// NUOVO: Utility function per aggiornare lo stato di selezione degli edge nel grafo
pub fn update_edge_selection_in_graph<N: Clone, E: Clone, Ty: EdgeType, Ix: IndexType, D: DisplayNode<N, E, Ty, Ix>>(
    graph: &mut Graph<D, CustomEdge, Ty, Ix>,
    selection_state: &GraphSelectionState
) where
    D: DisplayNode<N, E, Ty, Ix>,
{
    let edge_indices: Vec<_> = graph.g.edge_indices().collect();
    for edge_idx in edge_indices {
        if let Some(edge) = graph.edge_mut(edge_idx) {
            // Ottieni gli ID dei nodi collegati dall'edge
            // Nota: Questo dipende dall'API di egui_graphs per ottenere i nodi source e target
            // Potrebbe essere necessario adattare in base alla struttura esatta di Edge

            // Esempio di implementazione (da adattare):
            // let source_id = edge.source().display().id;
            // let target_id = edge.target().display().id;
            // let is_selected = selection_state.is_edge_selected((source_id, target_id));
            // edge.display_mut().set_selected(is_selected);

            // Per ora, implementazione semplificata:
            let edge_display = edge.display_mut();
            // edge_display.set_selected(false); // Da implementare correttamente
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
            "crates/controller/src/view/assets/drone.png".to_string(),
            Pos2::new(50.0, 50.0),
            Vec2::new(40.0, 40.0),
            create_mock_texture_id()
        );

        assert_eq!(node.id, 1);
        assert_eq!(node.node_type, NodeType::Drone);
        assert_eq!(node.image_path, "crates/controller/src/view/assets/drone.png");
        assert_eq!(node.position, Pos2::new(50.0, 50.0));
        assert_eq!(node.label, "Drone 1");
        assert_eq!(node.size, Vec2::new(40.0, 40.0));
        assert!(node.texture_id.is_some());
        assert!(!node.selected); // NUOVO: verifica stato iniziale
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
    fn test_custom_node_with_label() {
        let node = create_test_node(1, NodeType::Server)
            .with_label("Custom Server".to_string());

        assert_eq!(node.label, "Custom Server");
        assert!(!node.selected); // Dovrebbe rimanere non selezionato
    }

    #[test]
    fn test_node_type_labels() {
        let drone = create_test_node(1, NodeType::Drone);
        let server = create_test_node(2, NodeType::Server);
        let client = create_test_node(3, NodeType::Client);

        assert_eq!(drone.label, "Drone 1");
        assert_eq!(server.label, "Server 2");
        assert_eq!(client.label, "Client 3");
    }

    #[test]
    fn test_get_color_with_selection() {
        let node = create_test_node(1, NodeType::Drone);

        // Test colori normali
        let normal_color = node.get_color(false);
        assert_eq!(normal_color, Color32::from_rgb(100, 149, 237));

        // Test colori selezionati
        let selected_color = node.get_color(true);
        assert_eq!(selected_color, Color32::from_rgb(150, 200, 255));

        // Verifica che siano diversi
        assert_ne!(normal_color, selected_color);
    }

    #[test]
    fn test_get_border_color() {
        let node = create_test_node(1, NodeType::Drone);

        assert_eq!(node.get_border_color(false), Color32::GRAY);
        assert_eq!(node.get_border_color(true), Color32::YELLOW);
    }

    #[test]
    fn test_from_node_props_with_selection() {
        let props = NodeProps {
            payload: (5, NodeType::Client),
            location: Pos2::new(150.0, 200.0),
            selected: true, // NUOVO: test con selezione
            dragged: false,
            label: String::new(),
        };

        let node = CustomNode::from(props);

        assert_eq!(node.id, 5);
        assert_eq!(node.node_type, NodeType::Client);
        assert_eq!(node.label, "Client #5");
        assert_eq!(node.position, Pos2::new(150.0, 200.0));
        assert!(node.selected); // NUOVO: verifica che la selezione sia stata copiata
    }

    #[test]
    fn test_custom_edge_new() {
        let edge = CustomEdge::new("test_payload");

        assert!(!edge.is_selected()); // Verifica stato iniziale
    }

    #[test]
    fn test_custom_edge_selection() {
        let mut edge = CustomEdge::new("test_payload");

        // Test stato iniziale
        assert!(!edge.is_selected());

        // Test selezione
        edge.set_selected(true);
        assert!(edge.is_selected());

        // Test deselezione
        edge.set_selected(false);
        assert!(!edge.is_selected());
    }

    #[test]
    fn test_edge_colors_and_width() {
        let edge = CustomEdge::new("test_payload");

        // Test colori
        assert_eq!(edge.get_edge_color(false), Color32::GRAY);
        assert_eq!(edge.get_edge_color(true), Color32::from_rgb(255, 255, 0));

        // Test spessori
        assert_eq!(edge.get_edge_width(false), 2.0);
        assert_eq!(edge.get_edge_width(true), 4.0);
    }

    #[test]
    fn test_custom_edge_from_props() {
        let props = EdgeProps {
            payload: "edge_data",
            order: 5,
            selected: true, // NUOVO: test con selezione
            label: "Test Edge".to_string(),
        };

        let edge = CustomEdge::from(props);
        assert!(edge.is_selected()); // Verifica che la selezione sia stata copiata
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

        // Test toggle
        state.toggle_node(3);
        assert!(state.is_node_selected(3));
        assert!(!state.is_edge_selected((1, 2))); // Edge deselezionato

        state.toggle_node(3);
        assert!(!state.is_node_selected(3));

        // Test clear
        state.select_node(1);
        state.select_edge((2, 3));
        state.clear_all();
        assert!(!state.is_node_selected(1));
        assert!(!state.is_edge_selected((2, 3)));
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
    fn test_selection_state_mutual_exclusion() {
        let mut state = GraphSelectionState::new();

        // Seleziona nodi
        state.select_node(1);
        state.select_node(2);
        assert_eq!(state.get_selected_nodes().len(), 2);

        // Seleziona edge - dovrebbe deselezionare tutti i nodi
        state.select_edge((3, 4));
        assert_eq!(state.get_selected_nodes().len(), 0);
        assert_eq!(state.get_selected_edges().len(), 1);

        // Seleziona nodo - dovrebbe deselezionare tutti gli edge
        state.select_node(5);
        assert_eq!(state.get_selected_edges().len(), 0);
        assert_eq!(state.get_selected_nodes().len(), 1);
    }

    // Helper function per testare is_inside senza dover specificare tutti i tipi generici
    fn test_node_is_inside(node: &CustomNode, pos: Pos2) -> bool {
        <CustomNode as DisplayNode<NodePayload, (), Undirected, u32>>::is_inside(node, pos)
    }

    // Helper function per testare closest_boundary_point
    fn test_node_closest_boundary_point(node: &CustomNode, dir: emath::Vec2) -> emath::Pos2 {
        <CustomNode as DisplayNode<NodePayload, (), Undirected, u32>>::closest_boundary_point(node, dir)
    }

    // Helper function per testare update
    fn test_node_update(node: &mut CustomNode, props: &NodeProps<NodePayload>) {
        <CustomNode as DisplayNode<NodePayload, (), Undirected, u32>>::update(node, props)
    }

    #[test]
    fn test_is_inside() {
        let node = create_test_node(1, NodeType::Drone);
        // Node at (100, 100) with size (50, 50)

        // Test center point
        assert!(test_node_is_inside(&node, Pos2::new(100.0, 100.0)));

        // Test corner points
        assert!(test_node_is_inside(&node, Pos2::new(75.0, 75.0)));   // Top-left
        assert!(test_node_is_inside(&node, Pos2::new(125.0, 75.0)));  // Top-right
        assert!(test_node_is_inside(&node, Pos2::new(75.0, 125.0)));  // Bottom-left
        assert!(test_node_is_inside(&node, Pos2::new(125.0, 125.0))); // Bottom-right

        // Test outside points
        assert!(!test_node_is_inside(&node, Pos2::new(50.0, 50.0)));
        assert!(!test_node_is_inside(&node, Pos2::new(150.0, 150.0)));
        assert!(!test_node_is_inside(&node, Pos2::new(100.0, 50.0)));
        assert!(!test_node_is_inside(&node, Pos2::new(100.0, 150.0)));
    }

    #[test]
    fn test_closest_boundary_point() {
        let node = create_test_node(1, NodeType::Drone);
        // Node at (100, 100) with size (50, 50)

        // Test horizontal direction
        let point_right = test_node_closest_boundary_point(&node, Vec2::new(1.0, 0.0));
        assert_eq!(point_right, emath::Pos2::new(125.0, 100.0));

        let point_left = test_node_closest_boundary_point(&node, emath::Vec2::new(-1.0, 0.0));
        assert_eq!(point_left, emath::Pos2::new(75.0, 100.0));

        // Test vertical direction
        let point_up = test_node_closest_boundary_point(&node, emath::Vec2::new(0.0, -1.0));
        assert_eq!(point_up, emath::Pos2::new(100.0, 75.0));

        let point_down = test_node_closest_boundary_point(&node, emath::Vec2::new(0.0, 1.0));
        assert_eq!(point_down, emath::Pos2::new(100.0, 125.0));

        // Test diagonal direction
        let point_diagonal = test_node_closest_boundary_point(&node, emath::Vec2::new(1.0, 1.0));
        // Should hit either right or bottom edge
        assert!(point_diagonal.x == 125.0 || point_diagonal.y == 125.0);
    }

    #[test]
    fn test_update_node_with_selection() {
        let mut node = create_test_node(1, NodeType::Drone);

        let new_props = NodeProps {
            payload: (2, NodeType::Server),
            location: Pos2::new(150.0, 150.0),
            selected: true, // NUOVO: test aggiornamento selezione
            dragged: false,
            label: "New Label".to_string(),
        };

        test_node_update(&mut node, &new_props);

        assert_eq!(node.id, 2);
        assert_eq!(node.node_type, NodeType::Server);
        assert_eq!(node.position, Pos2::new(150.0, 150.0)); // Posizione aggiornata
        assert!(node.selected); // NUOVO: selezione aggiornata
    }

    #[test]
    fn test_node_type_equality() {
        assert_eq!(NodeType::Drone, NodeType::Drone);
        assert_ne!(NodeType::Drone, NodeType::Server);
        assert_ne!(NodeType::Server, NodeType::Client);
    }

    #[test]
    fn test_type_compatibility() {
        // Verifica che NodeId sia u8 (o il tipo corretto)
        let _id: NodeId = 255u8;

        // Verifica che NodePayload sia costruibile
        let payload: NodePayload = (1, NodeType::Drone);
        assert_eq!(payload.0, 1);
        assert_eq!(payload.1, NodeType::Drone);
    }
}