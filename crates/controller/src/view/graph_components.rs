use egui::{Pos2, Vec2, ColorImage, TextureHandle, TextureId, Shape, Rect, Stroke};
use egui_graphs::{Graph, GraphView, Node, Edge, NodeProps, EdgeProps, SettingsInteraction, SettingsNavigation, DefaultNodeShape, DefaultEdgeShape, DisplayNode, DrawContext, DisplayEdge};
use wg_2024::network::NodeId;
use crate::utility::NodeType;
use eframe::{egui, emath};
use eframe::epaint;
use eframe::epaint::Color32;

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
            texture_id: texture
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }

    fn get_color(&self) -> Color32 {
        match self.node_type {
            NodeType::Drone => Color32::from_rgb(100, 149, 237),  // Cornflower blue
            NodeType::Server => Color32::from_rgb(34, 139, 34),   // Forest green
            NodeType::Client => Color32::from_rgb(255, 69, 0),    // Orange red
        }
    }

    fn get_border_color(&self, selected: bool) -> Color32 {
        if selected {
            Color32::YELLOW
        } else {
            Color32::GRAY
        }
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
            NodeType::Client => "assets/client.png".to_string(),
            NodeType::Drone => "assets/drone.png".to_string(),
            NodeType::Server => "assets/server.png".to_string(),
        };

        label.push_str(&node_props.payload.0.to_string());

        // You might need to extract position from node_props if available
        // Check the NodeProps structure to see if it has location/position fields
        let position = Pos2::new(0.0, 0.0); // Default position

        Self {
            id: node_props.payload.0,
            node_type: node_props.payload.1,
            image_path,
            position,
            label,
            size: Vec2::new(50.0, 50.0),
            texture_id: None
        }
    }
}

impl <E: Clone> DisplayNode <NodePayload, E, Undirected, u32> for CustomNode{
    fn closest_boundary_point(&self, dir: emath::Vec2) -> emath::Pos2 {
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
        emath::Pos2::new(center.x + offset.x, center.y + offset.y)
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<epaint::Shape> {
        let mut shapes = Vec::new();

        // Calculate rectangle for texture
        let half_size = Vec2::new(self.size.x / 2.0, self.size.y / 2.0);
        let min_pos = emath::Pos2::new(self.position.x - half_size.x, self.position.y - half_size.y);
        let max_pos = emath::Pos2::new(self.position.x + half_size.x, self.position.y + half_size.y);
        let rect = Rect::from_min_max(min_pos, max_pos);

        // If we have a texture, render it
        if let Some(texture_id) = self.texture_id {
            let epaint_texture_id: epaint::TextureId = texture_id.into();
            let texture_shape = Shape::image(
                epaint_texture_id,
                rect,
                Rect::from_min_max(emath::Pos2::new(0.0, 0.0), emath::Pos2::new(1.0, 1.0)), // UV coordinates
                Color32::WHITE
            );
            shapes.push(texture_shape);
        } else {
            // Fallback: render colored rectangle if no texture
            let color = self.get_color();
            let epaint_color = Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a());
            let fill_shape = Shape::rect_filled(rect, 5.0, epaint_color);
            shapes.push(fill_shape);
        }

        // Add border
        let border_color = self.get_border_color(false);
        let stroke = Stroke::new(2.0, border_color);

        use eframe::epaint::{Shape, Stroke, Color32, Rect, Rounding};

        let rounding = Rounding::same(5.0);
        let stroke = Stroke::new(2.0, Color32::GRAY);
        let border_shape = Shape::rect_stroke(rect, rounding, stroke);
        shapes.push(border_shape);

        shapes
    }

    fn update(&mut self, state: &NodeProps<NodePayload>) {
        // Update id and type if necessary
        self.id = state.clone().payload.0;
        self.node_type = state.clone().payload.1;

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

        // You might need to update position from state if NodeProps contains position info
        // Check NodeProps structure for location/position fields
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
    pub default: DefaultEdgeShape
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
        }
    }
}

impl<E: Clone> From<EdgeProps<E>> for CustomEdge {
    fn from(props: EdgeProps<E>) -> Self {
        Self {
            default: DefaultEdgeShape::from(props),
        }
    }
}

impl<N: Clone, E: Clone, Ty: EdgeType, Ix: IndexType, D: DisplayNode<N, E, Ty, Ix>> DisplayEdge<N, E, Ty, Ix, D> for CustomEdge{
    fn shapes(&mut self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, ctx: &DrawContext) -> Vec<egui::Shape> {
        let start_display = start.display();
        let end_display = end.display();

        // Uso emath::Vec2 e emath::Pos2 per closest_boundary_point
        let start_center_emath = start_display.closest_boundary_point(emath::Vec2::new(0.0, 0.0));
        let end_center_emath = end_display.closest_boundary_point(emath::Vec2::new(0.0, 0.0));

        // Converto in egui::Pos2 per usarli in Shape
        let start_center = egui::Pos2::new(start_center_emath.x, start_center_emath.y);
        let end_center = egui::Pos2::new(end_center_emath.x, end_center_emath.y);

        let start_to_end = egui::Vec2::new(end_center.x - start_center.x, end_center.y - start_center.y);
        let end_to_start = egui::Vec2::new(start_center.x - end_center.x, start_center.y - end_center.y);

        // Ottengo punti di bordo sempre in emath, poi converto in egui
        let start_pos_emath = start_display.closest_boundary_point(emath::Vec2::new(start_to_end.x, start_to_end.y));
        let end_pos_emath = end_display.closest_boundary_point(emath::Vec2::new(end_to_start.x, end_to_start.y));

        let start_pos = egui::Pos2::new(start_pos_emath.x, start_pos_emath.y);
        let end_pos = egui::Pos2::new(end_pos_emath.x, end_pos_emath.y);

        let stroke = egui::Stroke::new(2.0, egui::Color32::GRAY);

        // Ora posso passare i valori convertiti a Shape::line_segment senza errori di tipo
        let line_shape = egui::Shape::line_segment([start_pos, end_pos], stroke);

        vec![line_shape]
    }

    fn update(&mut self, state: &EdgeProps<E>) {
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

        distance < 5.0 // 5 pixel tolerance
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // Non importare egui di nuovo, è già incluso in super::*
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
    }

    #[test]
    fn test_custom_node_with_label() {
        let node = create_test_node(1, NodeType::Server)
            .with_label("Custom Server".to_string());

        assert_eq!(node.label, "Custom Server");
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
    fn test_get_color() {
        let drone = create_test_node(1, NodeType::Drone);
        let server = create_test_node(2, NodeType::Server);
        let client = create_test_node(3, NodeType::Client);

        assert_eq!(drone.get_color(), Color32::from_rgb(100, 149, 237));
        assert_eq!(server.get_color(), Color32::from_rgb(34, 139, 34));
        assert_eq!(client.get_color(), Color32::from_rgb(255, 69, 0));
    }

    #[test]
    fn test_get_border_color() {
        let node = create_test_node(1, NodeType::Drone);

        assert_eq!(node.get_border_color(false), Color32::GRAY);
        assert_eq!(node.get_border_color(true), Color32::YELLOW);
    }

    #[test]
    fn test_from_node_props() {
        let props = NodeProps {
            payload: (5, NodeType::Client),
            location: Pos2::new(0.0, 0.0), // Posizione di default
            selected: false,
            dragged: false,
            label: String::new(), // Label vuota
        };

        let node = CustomNode::from(props);

        assert_eq!(node.id, 5);
        assert_eq!(node.node_type, NodeType::Client);
        assert_eq!(node.label, "Client #5");
        assert_eq!(node.image_path, "assets/client.png");
        assert_eq!(node.size, Vec2::new(50.0, 50.0));
        assert!(node.texture_id.is_none());
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
        let point_right = test_node_closest_boundary_point(&node, emath::Vec2::new(1.0, 0.0));
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
    fn test_closest_boundary_point_zero_vector() {
        let node = create_test_node(1, NodeType::Drone);

        // Test with zero vector (should default to right direction)
        let point = test_node_closest_boundary_point(&node, emath::Vec2::new(0.0, 0.0));
        assert_eq!(point, emath::Pos2::new(125.0, 100.0));
    }

    #[test]
    fn test_update_node() {
        let mut node = create_test_node(1, NodeType::Drone);

        let new_props = NodeProps {
            payload: (2, NodeType::Server),
            location: Pos2::new(150.0, 150.0), // Nuova posizione
            selected: false,
            dragged: false,
            label: "New Label".to_string(), // Label personalizzata
        };

        test_node_update(&mut node, &new_props);

        assert_eq!(node.id, 2);
        assert_eq!(node.node_type, NodeType::Server);
        // La label non viene aggiornata automaticamente se già impostata
        // perché il nodo è stato creato con "Drone 1" come label
        assert_eq!(node.label, "Drone 1");
    }

    #[test]
    fn test_update_node_with_custom_label() {
        let mut node = create_test_node(1, NodeType::Drone)
            .with_label("My Custom Drone".to_string());

        let new_props = NodeProps {
            payload: (2, NodeType::Server),
            location: Pos2::new(200.0, 200.0), // Nuova posizione
            selected: false,
            dragged: false,
            label: "Another Label".to_string(), // Altra label
        };

        test_node_update(&mut node, &new_props);

        // Custom label should not be updated
        assert_eq!(node.label, "My Custom Drone");
        assert_eq!(node.id, 2);
        assert_eq!(node.node_type, NodeType::Server);
    }

    #[test]
    fn test_custom_edge_new() {
        let edge = CustomEdge::new("test_payload");
        // Non possiamo accedere direttamente ai campi di DefaultEdgeShape
        // perché potrebbe essere privato. Verifichiamo solo che l'edge sia creato
        // Il test reale dipende dalla struttura di DefaultEdgeShape
    }

    #[test]
    fn test_custom_edge_from_props() {
        let props = EdgeProps {
            payload: "edge_data",
            order: 5,
            selected: true,
            label: "Test Edge".to_string(),
        };

        let edge = CustomEdge::from(props.clone());
        // Non possiamo verificare direttamente i campi interni di DefaultEdgeShape
        // ma possiamo verificare che l'edge sia stato creato correttamente
        // testando che implementi From<EdgeProps>
    }

    #[test]
    fn test_node_type_equality() {
        assert_eq!(NodeType::Drone, NodeType::Drone);
        assert_ne!(NodeType::Drone, NodeType::Server);
        assert_ne!(NodeType::Server, NodeType::Client);
    }

    #[test]
    fn test_node_type_hash() {
        use std::collections::HashMap;

        let mut map = HashMap::new();
        map.insert(NodeType::Drone, "drone");
        map.insert(NodeType::Server, "server");
        map.insert(NodeType::Client, "client");

        assert_eq!(map.get(&NodeType::Drone), Some(&"drone"));
        assert_eq!(map.get(&NodeType::Server), Some(&"server"));
        assert_eq!(map.get(&NodeType::Client), Some(&"client"));
    }

    // Test per shapes() - richiede un mock context
    // NOTA: Questi test sono commentati perché richiedono un DrawContext completo
    // che non è facilmente mockabile senza un Context e Painter reali
    /*
    #[test]
    fn test_shapes_generation() {
        let mut node = create_test_node(1, NodeType::Drone);
        node.texture_id = None; // Test senza texture
        
        // Creiamo un DrawContext mock
        let ctx = DrawContext {
            ctx: &egui::Context::default(),
            painter: None, // Assumendo che sia optional
            meta: None,    // Assumendo che sia optional
        };
        
        let shapes = node.shapes(&ctx);
        
        // Dovremmo avere almeno 2 shape: fill e border
        assert!(shapes.len() >= 2);
        
        // Verifica che il primo sia un rettangolo riempito
        // e il secondo sia un bordo
        // (I dettagli dipendono dall'implementazione esatta di Shape)
    }

    #[test]
    fn test_shapes_with_texture() {
        let mut node = create_test_node(1, NodeType::Drone);
        
        let ctx = DrawContext {
            ctx: &egui::Context::default(),
            painter: None,
            meta: None,
        };
        
        let shapes = node.shapes(&ctx);
        
        // Con texture dovremmo avere shape per immagine e bordo
        assert!(shapes.len() >= 2);
    }
    */

    // Test per CustomEdge::is_inside
    // NOTA: Questo test richiede Node mock completi che implementano DisplayNode
    // che è complesso da creare nei test unitari
    /*
    #[test]
    fn test_edge_is_inside() {
        // Questo test richiede la creazione di Node mock completi
        // che implementano il trait DisplayNode
        
        // Esempio semplificato:
        let start_node = create_mock_node(Pos2::new(0.0, 0.0));
        let end_node = create_mock_node(Pos2::new(100.0, 0.0));
        let edge = CustomEdge::new("test");
        
        // Punto sulla linea
        assert!(edge.is_inside(&start_node, &end_node, Pos2::new(50.0, 0.0)));
        
        // Punto vicino alla linea (entro 5 pixel)
        assert!(edge.is_inside(&start_node, &end_node, Pos2::new(50.0, 4.0)));
        
        // Punto lontano dalla linea
        assert!(!edge.is_inside(&start_node, &end_node, Pos2::new(50.0, 10.0)));
    }
    */

    #[test]
    fn test_edge_is_inside_tolerance() {
        // Test per verificare che la tolleranza di 5 pixel funzioni correttamente
        let edge = CustomEdge::new("test");

        // La tolleranza dovrebbe essere 5.0 pixel come definito nel codice
        // Questo richiede test con nodi mock
    }

    // Test di integrazione per verificare che i tipi siano compatibili
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