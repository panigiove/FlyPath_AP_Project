use std::collections::HashMap;
use eframe::egui::{self, ColorImage, TextureHandle, Shape, Rect, Pos2, Vec2, Color32, Stroke};
use egui_graphs::NodeProps;
use wg_2024::network::NodeId;
use crate::utility::NodeType;

#[derive(Clone)]
pub struct CustomNode {
    pub id: NodeId,
    pub node_type: NodeType,
    pub image_path: String,
    pub position: Pos2,
    pub size: Vec2,
    pub label: String,
}

impl CustomNode {
    pub fn new(id: NodeId, node_type: NodeType, image_path: String, position: Pos2) -> Self {
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
            size: Vec2::new(64.0, 64.0),
            label,
        }
    }

    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label;
        self
    }
}

pub struct NodeTextureManager {
    textures: HashMap<String, TextureHandle>,
    loading_errors: HashMap<String, String>,
}

#[derive(Clone)]
pub struct CustomEdge {
    pub from: NodeId,
    pub to: NodeId,
    pub color: egui::Color32,
    pub thickness: f32,
}

impl CustomEdge {
    pub fn new(from: NodeId, to: NodeId) -> Self {
        Self {
            from,
            to,
            color: egui::Color32::DARK_GRAY,
            thickness: 2.0,
        }
    }

    pub fn with_color(mut self, color: egui::Color32) -> Self {
        self.color = color;
        self
    }

    pub fn with_thickness(mut self, thickness: f32) -> Self {
        self.thickness = thickness;
        self
    }
}

impl NodeTextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            loading_errors: HashMap::new(),
        }
    }

    pub fn get_or_load_texture(&mut self, ctx: &egui::Context, path: &str) -> Option<&TextureHandle> {
        if self.textures.contains_key(path) {
            return self.textures.get(path);
        }
        
        if self.loading_errors.contains_key(path) {
            return None;
        }
        
        match self.load_image_from_path(path) {
            Ok(color_image) => {
                let texture = ctx.load_texture(
                    format!("node_texture_{}", path),
                    color_image,
                    egui::TextureOptions::default()
                );
                self.textures.insert(path.to_string(), texture);
                self.textures.get(path)
            }
            Err(e) => {
                self.loading_errors.insert(path.to_string(), e.to_string());
                None
            }
        }
    }

    fn load_image_from_path(&self, path: &str) -> Result<ColorImage, Box<dyn std::error::Error>> {
        let image = image::open(path)?;
        let image_buffer = image.to_rgba8();
        let size = [image_buffer.width() as usize, image_buffer.height() as usize];
        let pixels = image_buffer.as_flat_samples();
        Ok(ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
    }

    pub fn get_error(&self, path: &str) -> Option<&String> {
        self.loading_errors.get(path)
    }
}


// impl<N: Clone> From<NodeProps<N>> for CustomNode {
//     fn from(node_props: NodeProps<N>) -> Self {
//         Self {
//             id: 0,
//             node_type: NodeType::Drone,
//             image_path: "".to_string(),
//             position: Default::default(),
//             size: Default::default(),
//             label: node_props.label.clone(),
//             loc: node_props.location(),
// 
//             size_x: 0.,
//             size_y: 0.,
//         }
//     }
// }

pub struct ZoomableGraph {
    pub nodes: Vec<CustomNode>,
    pub edges: Vec<CustomEdge>,
    pub texture_manager: NodeTextureManager,
    pub selected_node: Option<NodeId>,
    pub dragging_node: Option<NodeId>,
    
    pub zoom: f32,
    pub pan_offset: Vec2,
    pub min_zoom: f32,
    pub max_zoom: f32,
    
    last_pointer_pos: Option<Pos2>,
    is_panning: bool,
}

impl ZoomableGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
            texture_manager: NodeTextureManager::new(),
            selected_node: None,
            dragging_node: None,
            zoom: 1.0,
            pan_offset: Vec2::ZERO,
            min_zoom: 0.1,
            max_zoom: 5.0,
            last_pointer_pos: None,
            is_panning: false,
        }
    }

    pub fn add_node(&mut self, node: CustomNode) {
        self.nodes.push(node);
    }

    pub fn add_edge(&mut self, edge: CustomEdge) {
        self.edges.push(edge);
    }

    // Converti coordinate mondo -> schermo
    fn world_to_screen(&self, world_pos: Pos2, viewport_rect: Rect) -> Pos2 {
        let centered_pos = world_pos.to_vec2() + self.pan_offset;
        let scaled_pos = centered_pos * self.zoom;
        viewport_rect.center() + scaled_pos
    }

    // Converti coordinate schermo -> mondo
    fn screen_to_world(&self, screen_pos: Pos2, viewport_rect: Rect) -> Pos2 {
        let centered_pos = (screen_pos - viewport_rect.center());
        let scaled_pos = centered_pos / self.zoom;
        let final_pos = scaled_pos - self.pan_offset;
        Pos2::new(final_pos.x, final_pos.y)
    }

    // Trova il nodo sotto il cursore
    fn find_node_at_position(&self, world_pos: Pos2) -> Option<NodeId> {
        for node in &self.nodes {
            let node_rect = Rect::from_min_size(
                node.position - node.size * 0.5,
                node.size
            );
            if node_rect.contains(world_pos) {
                return Some(node.id);
            }
        }
        None
    }

    pub fn handle_input(&mut self, ui: &mut egui::Ui, viewport_rect: Rect) -> egui::Response {
        let response = ui.allocate_rect(viewport_rect, egui::Sense::click_and_drag());

        // Gestione zoom con rotella del mouse
        if response.hovered() {
            let scroll_delta = ui.input(|i| i.smooth_scroll_delta);
            if scroll_delta.y != 0.0 {
                let zoom_factor = 1.0 + scroll_delta.y * 0.001;
                self.zoom = (self.zoom * zoom_factor).clamp(self.min_zoom, self.max_zoom);
            }
        }

        // Gestione click e drag
        if let Some(pointer_pos) = response.interact_pointer_pos() {
            let world_pos = self.screen_to_world(pointer_pos, viewport_rect);

            if response.drag_started() {
                // Controlla se stiamo cliccando su un nodo
                if let Some(node_id) = self.find_node_at_position(world_pos) {
                    self.selected_node = Some(node_id);
                    self.dragging_node = Some(node_id);
                } else {
                    // Inizia il panning
                    self.is_panning = true;
                    self.selected_node = None;
                }
                self.last_pointer_pos = Some(pointer_pos);
            }

            if response.dragged() {
                if let Some(last_pos) = self.last_pointer_pos {
                    let delta = pointer_pos - last_pos;

                    if let Some(dragging_id) = self.dragging_node {
                        // Muovi il nodo
                        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == dragging_id) {
                            let world_delta = delta / self.zoom;
                            node.position += world_delta;
                        }
                    } else if self.is_panning {
                        // Pan della vista
                        self.pan_offset += delta / self.zoom;
                    }
                }
                self.last_pointer_pos = Some(pointer_pos);
            }

            if response.drag_released() {
                self.dragging_node = None;
                self.is_panning = false;
                self.last_pointer_pos = None;
            }
        }

        response
    }

    fn draw_edges(&self, ui: &mut egui::Ui, viewport_rect: Rect) {
        for edge in &self.edges {
            if let (Some(from_node), Some(to_node)) = (
                self.nodes.iter().find(|n| n.id == edge.from),
                self.nodes.iter().find(|n| n.id == edge.to)
            ) {
                let from_screen = self.world_to_screen(from_node.position, viewport_rect);
                let to_screen = self.world_to_screen(to_node.position, viewport_rect);

                ui.painter().line_segment(
                    [from_screen, to_screen],
                    egui::Stroke::new(edge.thickness * self.zoom, edge.color)
                );
            }
        }
    }

    fn draw_node(&mut self, ui: &mut egui::Ui, node: &CustomNode, viewport_rect: Rect) {
        let screen_pos = self.world_to_screen(node.position, viewport_rect);
        let screen_size = node.size * self.zoom;
        let rect = Rect::from_center_size(screen_pos, screen_size);

        // Non disegnare nodi troppo piccoli
        if screen_size.x < 2.0 || screen_size.y < 2.0 {
            return;
        }

        let is_selected = self.selected_node == Some(node.id);
        let is_dragging = self.dragging_node == Some(node.id);

        // Background del nodo
        let bg_color = if is_dragging {
            egui::Color32::from_rgb(120, 120, 255)
        } else if is_selected {
            egui::Color32::from_rgb(100, 150, 255)
        } else {
            match node.node_type {
                NodeType::Drone => egui::Color32::from_rgb(255, 200, 100),
                NodeType::Server => egui::Color32::from_rgb(100, 255, 100),
                NodeType::Client => egui::Color32::from_rgb(255, 100, 100),
            }
        };

        ui.painter().rect_filled(rect, 4.0 * self.zoom, bg_color);

        // Disegna l'immagine se disponibile e abbastanza grande
        if screen_size.x > 16.0 {
            let texture = self.texture_manager.get_or_load_texture(ui.ctx(), &node.image_path);

            if let Some(texture) = texture {
                let image_rect = rect.shrink(2.0 * self.zoom);
                let shape = Shape::image(
                    texture.id(),
                    image_rect,
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    egui::Color32::WHITE
                );
                ui.painter().add(shape);
            } else {
                // Placeholder
                let placeholder_color = egui::Color32::from_rgb(128, 128, 128);
                ui.painter().rect_filled(rect.shrink(4.0 * self.zoom), 2.0, placeholder_color);

                if screen_size.x > 24.0 {
                    let text = match node.node_type {
                        NodeType::Drone => "D",
                        NodeType::Server => "S",
                        NodeType::Client => "C",
                    };

                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        text,
                        egui::FontId::proportional(16.0 * self.zoom),
                        egui::Color32::WHITE
                    );
                }
            }
        }

        // Bordo
        let stroke_color = if is_selected {
            egui::Color32::WHITE
        } else {
            egui::Color32::BLACK
        };
        ui.painter().rect_stroke(
            rect,
            4.0 * self.zoom,
            egui::Stroke::new(2.0 * self.zoom, stroke_color),
            egui::StrokeKind::Outside
        );


        // Etichetta (solo se abbastanza zoommato)
        if self.zoom > 0.5 {
            let label_pos = Pos2::new(rect.center().x, rect.max.y + 5.0 * self.zoom);
            ui.painter().text(
                label_pos,
                egui::Align2::CENTER_TOP,
                &node.label,
                egui::FontId::proportional(12.0 * self.zoom),
                egui::Color32::BLACK
            );
        }
    }

    pub fn draw_graph(&mut self, ui: &mut egui::Ui) {
        let available_rect = ui.available_rect_before_wrap();

        // Gestisci input
        self.handle_input(ui, available_rect);

        // Background
        ui.painter().rect_filled(
            available_rect,
            0.0,
            egui::Color32::from_rgb(240, 240, 240)
        );

        // Disegna gli archi prima dei nodi
        self.draw_edges(ui, available_rect);

        // Disegna i nodi
        for i in 0..self.nodes.len() {
            let node = self.nodes[i].clone();
            self.draw_node(ui, &node, available_rect);
        }

        // UI di controllo nell'angolo
        egui::Window::new("Graph Controls")
            .fixed_pos(available_rect.min + Vec2::new(10.0, 10.0))
            .fixed_size(Vec2::new(200.0, 120.0))
            .resizable(false)
            .collapsible(false)
            .title_bar(false)
            .show(ui.ctx(), |ui| {
                ui.label(format!("Zoom: {:.2}x", self.zoom));
                ui.label(format!("Pan: ({:.0}, {:.0})", self.pan_offset.x, self.pan_offset.y));
                if let Some(selected_id) = self.selected_node {
                    ui.label(format!("Selected: Node {}", selected_id));
                }
                if ui.button("Reset View").clicked() {
                    self.zoom = 1.0;
                    self.pan_offset = Vec2::ZERO;
                }
            });
    }
}