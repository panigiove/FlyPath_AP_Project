use eframe::epaint::StrokeKind;
use egui::{ColorImage, TextureHandle, TextureId, Pos2};
use egui_graphs::{Graph, GraphView, Node, Edge, NodeProps, EdgeProps, SettingsInteraction, SettingsNavigation, DefaultNodeShape, DefaultEdgeShape, DisplayNode, DrawContext, DisplayEdge};
use wg_2024::network::NodeId;
use wg_2024::packet::NodeType;
use eframe::egui::{Color32, Shape, Rect, Rounding, Stroke};
use epaint::text;

use petgraph::{EdgeType, stable_graph::IndexType};

use egui::{emath, epaint, Context, Rounding, Vec2};
use egui::epaint::TextureManager;
use image::io::Reader as ImageReader;

use petgraph::{Graph as PetGraph, Undirected};

const COLOR: Color32 = Color32::WHITE;
const RADIUS: f32 = 5.0;

// Make sure NodeId is u8 or define the correct type alias
type NodePayload = (NodeId, NodeType);


#[derive(Clone)]
pub struct CustomNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub image_path: String,
    pub position: emath::Pos2,
    pub label: String,
    pub size: Vec2,
    pub texture_id: Option<TextureId>,
}

impl CustomNode {
    pub fn new(id: NodeId, node_type: NodeType, image_path: String, position: Pos2, size: Vec2, texture_id:TextureId) -> Self {
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

    // Metodo helper per ottenere il colore del bordo
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
        Self {
            id: node_props.payload.0,
            node_type: node_props.payload.1,
            image_path,
            position: Pos2::new(node_props.location().x, node_props.location().y),
            label,
            size: Vec2::new(50.0, 50.0),
            texture_id: None
        }
    }
}

impl <E: Clone> DisplayNode <NodePayload, E, Undirected, u32> for CustomNode{
    fn closest_boundary_point(&self, dir: eframe::egui::Vec2) -> eframe::egui::Pos2 {
        let half_size = self.size / 2.0;
        let center = self.position;

        // Normalizza la direzione
        let dir_normalized = if dir.length() > 0.0 {
            dir.normalized()
        } else {
            eframe::egui::Vec2::new(1.0, 0.0) // Default direction se la direzione è zero
        };

        // Calcola l'intersezione con il bordo del rettangolo
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

        // Prendi il minimo tra t_x e t_y per trovare il primo punto di intersezione
        let t = t_x.min(t_y);

        let t = t_x.min(t_y);

        let offset = dir_normalized * t;
        eframe::egui::Pos2::new(center.x + offset.x, center.y + offset.y)
    }

    fn shapes(&mut self, ctx: &DrawContext) -> Vec<Shape> {
        let mut shapes = Vec::new();

        // Calcola il rettangolo per la texture
        let half_size = self.size / 2.0;
        let rect = Rect::from_center_size(self.position, emath::Vec2::new(self.size.x, self.size.y));

        // Se abbiamo una texture, renderizzala
        if let Some(texture_id) = self.texture_id {
            let texture_shape = Shape::image(
                texture_id,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), // UV coordinates (full texture)
                Color32::WHITE // Tint color
            );
            shapes.push(texture_shape);
        } else {
            // Fallback: renderizza un rettangolo colorato se non c'è texture
            let color = self.get_color();
            let fill_shape = Shape::rect_filled(rect, Rounding::same(5.0), color);
            shapes.push(fill_shape);
        }

        // Aggiungi il bordo
        let border_color = self.get_border_color(ctx.is_selected);
        let stroke = Stroke::new(2.0, border_color);
        let border_shape = Shape::rect_stroke(rect, Rounding::same(5.0), stroke);
        shapes.push(border_shape);

        // Aggiungi il testo del label sotto il nodo
        if !self.label.is_empty() {
            let text_pos = Pos2::new(
                self.position.x,
                self.position.y + half_size.y + 15.0 // Posiziona il testo sotto il nodo
            );

            let text_shape = ctx.ctx.fonts(|fonts| {
                Shape::text(
                    fonts,
                    text_pos,
                    egui::Align2::CENTER_TOP,
                    &self.label,
                    egui::FontId::default(),
                    Color32::WHITE,
                )
            });
            shapes.push(text_shape);
        }

        shapes
    }

    fn update(&mut self, state: &NodeProps<NodePayload>) {
        self.position = Pos2::new(state.location().x, state.location().y);

        // Aggiorna l'id e il tipo se necessario
        self.id = state.payload.0;
        self.node_type = state.payload.1;

        // Se il label non è stato personalizzato, aggiornalo basandosi sul tipo
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
    fn shapes(&mut self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, ctx: &DrawContext) -> Vec<epaint::shape::Shape> {
        self.default.shapes(start, end, ctx)
    }

    fn update(&mut self, state: &EdgeProps<E>) {
        self.default.update(state)
    }

    fn is_inside(&self, start: &Node<N, E, Ty, Ix, D>, end: &Node<N, E, Ty, Ix, D>, pos: egui::Pos2) -> bool {
        self.default.is_inside(start, end, pos)
    }
}
