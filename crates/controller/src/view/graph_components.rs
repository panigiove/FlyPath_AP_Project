// use eframe::epaint::StrokeKind;
// use egui::{ColorImage, TextureHandle, TextureId, Pos2, StrokeKind};
// use egui_graphs::{Graph, GraphView, Node, Edge, NodeProps, EdgeProps, SettingsInteraction, SettingsNavigation, DefaultNodeShape, DefaultEdgeShape, DisplayNode, DrawContext, DisplayEdge};
// use wg_2024::network::NodeId;
// use wg_2024::packet::NodeType;
// use eframe::egui::{Color32, Shape, Rect, Rounding, Stroke};
// use epaint::text;
// 
// use petgraph::{EdgeType, stable_graph::IndexType};
// 
// use egui::{emath, epaint, Context, Rounding, Vec2};
// use egui::epaint::TextureManager;
// use image::io::Reader as ImageReader;
// 
// use petgraph::{Graph as PetGraph, Undirected};
// 
// const COLOR: Color32 = Color32::WHITE;
// const RADIUS: f32 = 5.0;
// 
// // Make sure NodeId is u8 or define the correct type alias
// type NodePayload = (NodeId, NodeType);
// 
// 
// #[derive(Clone)]
// pub struct CustomNode {
//     pub id: NodeId,
//     pub node_type: NodeType,
//     pub image_path: String,
//     pub position: emath::Pos2,
//     pub label: String,
//     pub size: Vec2,
//     pub texture_id: Option<TextureId>,
// }
// 
// impl CustomNode {
//     pub fn new(id: NodeId, node_type: NodeType, image_path: String, position: Pos2, size: Vec2, texture_id:TextureId) -> Self {
//         let label = match node_type {
//             NodeType::Drone => format!("Drone {}", id),
//             NodeType::Server => format!("Server {}", id),
//             NodeType::Client => format!("Client {}", id),
//         };
//         let texture = Some(texture_id);
// 
//         Self {
//             id,
//             node_type,
//             image_path,
//             position,
//             label,
//             size,
//             texture_id: texture
//         }
//     }
// 
//     pub fn with_label(mut self, label: String) -> Self {
//         self.label = label;
//         self
//     }
//     fn get_color(&self) -> Color32 {
//         match self.node_type {
//             NodeType::Drone => Color32::from_rgb(100, 149, 237),  // Cornflower blue
//             NodeType::Server => Color32::from_rgb(34, 139, 34),   // Forest green
//             NodeType::Client => Color32::from_rgb(255, 69, 0),    // Orange red
//         }
//     }
// 
//     // Metodo helper per ottenere il colore del bordo
//     fn get_border_color(&self, selected: bool) -> Color32 {
//         if selected {
//             Color32::YELLOW
//         } else {
//             Color32::GRAY
//         }
//     }
// }
// impl From<NodeProps<(NodeId, NodeType)>> for CustomNode {
//     fn from(node_props: NodeProps<(NodeId, NodeType)>) -> Self {
//         let mut label = match node_props.payload.1 {
//             NodeType::Client => "Client #".to_string(),
//             NodeType::Drone => "Drone #".to_string(),
//             NodeType::Server => "Server #".to_string(),
//         };
//         let mut image_path = match node_props.payload.1 {
//             NodeType::Client => "assets/client.png".to_string(),
//             NodeType::Drone => "assets/drone.png".to_string(),
//             NodeType::Server => "assets/server.png".to_string(),
//         };
//         
//         label.push_str(&node_props.payload.0.to_string());
//         Self {
//             id: node_props.payload.0,
//             node_type: node_props.payload.1,
//             image_path,
//             position: Pos2::new(node_props.location().x, node_props.location().y),
//             label,
//             size: Vec2::new(50.0, 50.0),
//             texture_id: None
//         }
//     }
// }
// 
// impl <E: Clone> DisplayNode <NodePayload, E, Undirected, u32> for CustomNode{
//     fn closest_boundary_point(&self, dir: eframe::egui::Vec2) -> eframe::egui::Pos2 {
//         let half_size = self.size / 2.0;
//         let center = self.position;
// 
//         // Normalizza la direzione
//         let dir_normalized = if dir.length() > 0.0 {
//             dir.normalized()
//         } else {
//             eframe::egui::Vec2::new(1.0, 0.0) // Default direction se la direzione è zero
//         };
// 
//         // Calcola l'intersezione con il bordo del rettangolo
//         let t_x = if dir_normalized.x != 0.0 {
//             half_size.x / dir_normalized.x.abs()
//         } else {
//             f32::INFINITY
//         };
// 
//         let t_y = if dir_normalized.y != 0.0 {
//             half_size.y / dir_normalized.y.abs()
//         } else {
//             f32::INFINITY
//         };
// 
//         // Prendi il minimo tra t_x e t_y per trovare il primo punto di intersezione
//         let t = t_x.min(t_y);
// 
//         let t = t_x.min(t_y);
// 
//         let offset = dir_normalized * t;
//         eframe::egui::Pos2::new(center.x + offset.x, center.y + offset.y)
//     }
// 
//     fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
//         let mut shapes = Vec::new();
// 
//         // Calcola il rettangolo per la texture
//         let half_size = emath::Vec2::new(self.size.x / 2.0, self.size.y / 2.0);
//         let min_pos = emath::Pos2::new(self.position.x - half_size.x, self.position.y - half_size.y);
//         let max_pos = emath::Pos2::new(self.position.x + half_size.x, self.position.y + half_size.y);
//         let rect = emath::Rect::from_min_max(min_pos, max_pos);
// 
//         let epaint_rect = epaint::Rect::from_min_max(
//             epaint::pos2(min_pos.x, min_pos.y),
//             epaint::pos2(max_pos.x, max_pos.y)
//         );
//         
//         // Se abbiamo una texture, renderizzala
//         if let Some(texture_id) = self.texture_id {
//             // Usa il costruttore diretto di Shape per evitare conflitti di versione
//             let texture_shape = epaint::Shape::image(
//                 texture_id,
//                 epaint_rect,
//                 epaint::Rect::from_min_max(epaint::pos2(0.0, 0.0), epaint::pos2(1.0, 1.0)), // UV coordinates
//                 epaint::Color32::WHITE
//             );
//             shapes.push(texture_shape);
//         } else {
//             // Fallback: renderizza un rettangolo colorato se non c'è texture
//             let color = self.get_color();
//             let epaint_color = epaint::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), color.a());
//             let fill_shape = epaint::Shape::rect_filled(epaint_rect, epaint::CornerRadius::same(5.0), epaint_color);
//             shapes.push(fill_shape);
//         }
// 
//         // Aggiungi il bordo
//         let border_color = self.get_border_color(false); // O controlla se ctx ha altri modi per determinare la selezione
//         let epaint_border_color = epaint::Color32::from_rgba_unmultiplied(
//             border_color.r(), border_color.g(), border_color.b(), border_color.a()
//         );
//         let stroke = epaint::Stroke::new(2.0, epaint_border_color);
//         let border_shape = epaint::Shape::rect_stroke(epaint_rect, epaint::CornerRadius::same(5.0 as u8), stroke, StrokeKind::Outside);
//         shapes.push(border_shape);
// 
//         // Aggiungi il testo del label sotto il nodo
//         //TODO aggiungere label
//         // if !self.label.is_empty() {
//         //     let text_pos = Pos2::new(
//         //         self.position.x,
//         //         self.position.y + half_size.y + 15.0 // Posiziona il testo sotto il nodo
//         //     );
//         // 
//         //     let text_shape = epaint::Shape::text(
//         //         &ctx.fonts,
//         //         text_pos,
//         //         epaint::Align2::CENTER_TOP,
//         //         &self.label,
//         //         epaint::FontId::default(),
//         //         epaint::Color32::WHITE,
//         //     );
//         //     shapes.push(text_shape);
//         // }
// 
//         shapes
//     }
// 
//     fn update(&mut self, state: &NodeProps<NodePayload>) {
//         self.position = Pos2::new(state.location().x, state.location().y);
// 
//         // Aggiorna l'id e il tipo se necessario
//         self.id = state.payload.0;
//         self.node_type = state.payload.1;
// 
//         // Se il label non è stato personalizzato, aggiornalo basandosi sul tipo
//         if self.label.starts_with("Client #") ||
//             self.label.starts_with("Drone #") ||
//             self.label.starts_with("Server #") {
//             self.label = match self.node_type {
//                 NodeType::Drone => format!("Drone #{}", self.id),
//                 NodeType::Server => format!("Server #{}", self.id),
//                 NodeType::Client => format!("Client #{}", self.id),
//             };
//         }
//     }
// 
//     fn is_inside(&self, pos: Pos2) -> bool {
//         let half_size = self.size / 2.0;
//         let min_pos = self.position - half_size;
//         let max_pos = self.position + half_size;
// 
//         pos.x >= min_pos.x && pos.x <= max_pos.x &&
//             pos.y >= min_pos.y && pos.y <= max_pos.y
// 
//     }
// }
// 
// #[derive(Clone)]
// pub struct CustomEdge {
//     pub default: DefaultEdgeShape
// }
// 
// impl<E: Clone> From<EdgeProps<E>> for CustomEdge {
//     fn from(props: EdgeProps<E>) -> Self {
//         Self { 
//             default: DefaultEdgeShape::from(props),
//         }
//     }
// }
// 
// impl<N: Clone, E: Clone, Ty: EdgeType, Ix: IndexType, D: DisplayNode<N, E, Ty, Ix>> DisplayEdge<N, E, Ty, Ix, D> for CustomEdge{
//     fn shapes(&mut self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, ctx: &DrawContext) -> Vec<epaint::shape::Shape> {
//         self.default.shapes(start, end, ctx)
//     }
// 
//     fn update(&mut self, state: &EdgeProps<E>) {
//         self.default.update(state)
//     }
// 
//     fn is_inside(&self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, pos: egui::Pos2) -> bool {
//         self.default.is_inside(start, end, pos)
//     }
// }


use egui::{Pos2, Vec2, ColorImage, TextureHandle, TextureId, Shape, Rect, Stroke};
use egui_graphs::{Graph, GraphView, Node, Edge, NodeProps, EdgeProps, SettingsInteraction, SettingsNavigation, DefaultNodeShape, DefaultEdgeShape, DisplayNode, DrawContext, DisplayEdge};
use wg_2024::network::NodeId;
use wg_2024::packet::NodeType;
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
        self.id = state.payload.0;
        self.node_type = state.payload.1;

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

