use egui::{Pos2, Vec2, TextureId, Color32, Painter};
use eframe::epaint::{Rect, Stroke};
use wg_2024::network::NodeId;
use crate::utility::NodeType;
use std::collections::HashSet;

#[derive(Clone)]
pub struct GraphNode {
    // Dati essenziali
    pub id: NodeId,
    pub node_type: NodeType,

    // Posizione e dimensioni
    pub position: Pos2,
    pub size: Vec2,

    // Aspetto visivo
    pub texture_id: Option<TextureId>,
    pub label: Option<String>,

    // Stato
    pub selected: bool,
    pub dragging: bool,
}

impl GraphNode {
    /// Crea un nuovo nodo con i parametri essenziali
    pub fn new(id: NodeId, node_type: NodeType, position: Pos2) -> Self {
        Self {
            id,
            node_type,
            position,
            size: Vec2::new(60.0, 60.0), // Dimensione di default
            texture_id: None,
            label: None,
            selected: false,
            dragging: false,
        }
    }

    /// Builder pattern per configurazione fluida
    pub fn with_size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    pub fn with_texture(mut self, texture_id: TextureId) -> Self {
        self.texture_id = Some(texture_id);
        self
    }

    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn with_auto_label(mut self) -> Self {
        let label = match self.node_type {
            NodeType::Client => format!("💻 {}", self.id),
            NodeType::Drone => format!("🚁 {}", self.id),
            NodeType::Server => format!("🖥️ {}", self.id),
        };
        self.label = Some(label);
        self
    }

    // === METODI DI STATO ===

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn set_position(&mut self, position: Pos2) {
        self.position = position;
    }

    pub fn move_by(&mut self, delta: Vec2) {
        self.position += delta;
    }

    // === GEOMETRIA ===

    pub fn contains_point(&self, point: Pos2) -> bool {
        let half_size = self.size / 2.0;
        let min_pos = self.position - half_size;
        let max_pos = self.position + half_size;

        point.x >= min_pos.x && point.x <= max_pos.x &&
            point.y >= min_pos.y && point.y <= max_pos.y
    }

    pub fn get_rect(&self) -> Rect {
        let half_size = self.size / 2.0;
        Rect::from_min_max(
            self.position - half_size,
            self.position + half_size
        )
    }

    pub fn get_edge_connection_point(&self, direction: Vec2) -> Pos2 {
        let half_size = self.size / 2.0;
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
        self.position + offset
    }

    // === COLORI ===

    fn get_fill_color(&self) -> Color32 {
        if self.selected {
            Color32::from_rgb(255, 215, 0) // Oro quando selezionato
        } else {
            match self.node_type {
                NodeType::Client => Color32::from_rgb(100, 200, 100),   // Verde
                NodeType::Drone => Color32::from_rgb(100, 150, 255),    // Blu
                NodeType::Server => Color32::from_rgb(255, 150, 100),   // Arancione
            }
        }
    }

    fn get_border_color(&self) -> Color32 {
        if self.selected {
            Color32::from_rgb(255, 215, 0) // Oro quando selezionato
        } else {
            Color32::BLACK
        }
    }

    fn get_border_width(&self) -> f32 {
        if self.selected { 3.0 } else { 2.0 }
    }

    // === RENDERING ===

    pub fn draw(&self, painter: &Painter) {
        let rect = self.get_rect();

        // 1. Aureola se selezionato
        if self.selected {
            let expanded_rect = Rect::from_min_max(
                rect.min - Vec2::new(4.0, 4.0),
                rect.max + Vec2::new(4.0, 4.0)
            );
            painter.rect_filled(expanded_rect, 8.0,
                                Color32::from_rgba_unmultiplied(255, 215, 0, 100));
        }

        // 2. Contenuto (texture o colore)
        if let Some(texture_id) = self.texture_id {
            painter.image(
                texture_id,
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE
            );
        } else {
            painter.rect_filled(rect, 6.0, self.get_fill_color());
        }

        // 3. Bordo
        painter.rect_stroke(
            rect,
            6.0,
            Stroke::new(self.get_border_width(), self.get_border_color())
        );

        // 4. Label (se presente)
        if let Some(ref label) = self.label {
            let text_pos = Pos2::new(self.position.x, rect.max.y + 15.0);
            let text_color = if self.selected {
                Color32::from_rgb(255, 215, 0)
            } else {
                Color32::BLACK
            };

            painter.text(
                text_pos,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::default(),
                text_color
            );
        }
    }
}

// === STRUTTURA PER GLI EDGE ===

#[derive(Clone)]
pub struct GraphEdge {
    pub from_id: NodeId,
    pub to_id: NodeId,
    pub selected: bool,
    pub width: f32,
    pub label: Option<String>,
}

impl GraphEdge {
    pub fn new(from_id: NodeId, to_id: NodeId) -> Self {
        Self {
            from_id,
            to_id,
            selected: false,
            width: 2.0,
            label: None,
        }
    }

    pub fn with_width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn with_label<S: Into<String>>(mut self, label: S) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn contains_point(&self, from_pos: Pos2, to_pos: Pos2, point: Pos2) -> bool {
        let line_vec = to_pos - from_pos;
        let point_vec = point - from_pos;

        if line_vec.length_sq() < f32::EPSILON {
            return false;
        }

        let t = (point_vec.dot(line_vec) / line_vec.length_sq()).clamp(0.0, 1.0);
        let projection = from_pos + t * line_vec;
        let distance = (point - projection).length();

        let tolerance = if self.selected { 8.0 } else { 5.0 };
        distance < tolerance
    }

    pub fn draw(&self, painter: &Painter, from_pos: Pos2, to_pos: Pos2) {
        let color = if self.selected {
            Color32::from_rgb(255, 215, 0)
        } else {
            Color32::from_rgb(100, 100, 100)
        };

        let width = if self.selected {
            self.width + 1.0
        } else {
            self.width
        };

        // Aureola se selezionato
        if self.selected {
            painter.line_segment(
                [from_pos, to_pos],
                Stroke::new(width + 2.0, Color32::from_rgba_unmultiplied(255, 215, 0, 100))
            );
        }

        // Linea principale
        painter.line_segment([from_pos, to_pos], Stroke::new(width, color));

        // Label (se presente)
        if let Some(ref label) = self.label {
            let mid_point = Pos2::new(
                (from_pos.x + to_pos.x) / 2.0,
                (from_pos.y + to_pos.y) / 2.0
            );
            painter.text(
                mid_point,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::default(),
                if self.selected { Color32::from_rgb(255, 215, 0) } else { Color32::BLACK }
            );
        }
    }
}

// === GESTIONE SELEZIONI ===

#[derive(Default)]
pub struct GraphSelectionState {
    pub selected_nodes: Vec<NodeId>,           // Massimo 2 nodi (FIFO)
    pub selected_edge: Option<(NodeId, NodeId)>, // Massimo 1 edge
}

impl GraphSelectionState {
    pub fn new() -> Self {
        Self {
            selected_nodes: Vec::new(),
            selected_edge: None,
        }
    }

    pub fn select_node(&mut self, node_id: NodeId) {
        // Deseleziona eventuali edge (mutuamente esclusivo)
        self.selected_edge = None;

        if !self.selected_nodes.contains(&node_id) {
            self.selected_nodes.push(node_id);

            // Se supera il limite di 2, rimuovi il più vecchio
            if self.selected_nodes.len() > 2 {
                self.selected_nodes.remove(0);
            }
        }
    }

    pub fn deselect_node(&mut self, node_id: NodeId) {
        self.selected_nodes.retain(|&id| id != node_id);
    }

    pub fn toggle_node(&mut self, node_id: NodeId) {
        if self.selected_nodes.contains(&node_id) {
            self.deselect_node(node_id);
        } else {
            self.select_node(node_id);
        }
    }

    pub fn select_edge(&mut self, from_id: NodeId, to_id: NodeId) {
        // Deseleziona tutti i nodi (mutuamente esclusivo)
        self.selected_nodes.clear();

        // Normalizza l'edge (sempre il minore per primo)
        let edge = if from_id < to_id { (from_id, to_id) } else { (to_id, from_id) };
        self.selected_edge = Some(edge);
    }

    pub fn deselect_edge(&mut self) {
        self.selected_edge = None;
    }

    pub fn toggle_edge(&mut self, from_id: NodeId, to_id: NodeId) {
        let edge = if from_id < to_id { (from_id, to_id) } else { (to_id, from_id) };

        if self.selected_edge == Some(edge) {
            self.deselect_edge();
        } else {
            self.select_edge(from_id, to_id);
        }
    }

    pub fn is_node_selected(&self, node_id: NodeId) -> bool {
        self.selected_nodes.contains(&node_id)
    }

    pub fn is_edge_selected(&self, from_id: NodeId, to_id: NodeId) -> bool {
        let edge = if from_id < to_id { (from_id, to_id) } else { (to_id, from_id) };
        self.selected_edge == Some(edge)
    }

    pub fn clear_all(&mut self) {
        self.selected_nodes.clear();
        self.selected_edge = None;
    }

    pub fn get_selected_nodes(&self) -> &Vec<NodeId> {
        &self.selected_nodes
    }

    pub fn get_selected_edge(&self) -> Option<(NodeId, NodeId)> {
        self.selected_edge
    }
}

// === FACTORY FUNCTIONS ===

/// Crea un nodo standard con label automatica
pub fn create_node(id: NodeId, node_type: NodeType, position: Pos2) -> GraphNode {
    GraphNode::new(id, node_type, position).with_auto_label()
}

/// Crea un nodo con texture
pub fn create_node_with_texture(
    id: NodeId,
    node_type: NodeType,
    position: Pos2,
    texture_id: TextureId
) -> GraphNode {
    GraphNode::new(id, node_type, position)
        .with_texture(texture_id)
        .with_auto_label()
}

/// Crea un edge standard
pub fn create_edge(from_id: NodeId, to_id: NodeId) -> GraphEdge {
    GraphEdge::new(from_id, to_id)
}

// === UTILITY FUNCTIONS ===

/// Crea texture fallback per i nodi
pub fn create_fallback_texture(cc: &eframe::CreationContext<'_>, node_type: NodeType) -> TextureId {
    let size = 64;
    let mut pixels = Vec::new();

    for y in 0..size {
        for x in 0..size {
            let color = match node_type {
                NodeType::Client => {
                    if (x + y) % 8 < 4 {
                        Color32::from_rgb(100, 200, 100)
                    } else {
                        Color32::from_rgb(80, 160, 80)
                    }
                }
                NodeType::Drone => {
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
        &format!("{:?}_fallback", node_type),
        color_image,
        egui::TextureOptions::default(),
    ).id()
}

/// Carica texture da file
pub fn load_texture_from_file(cc: &eframe::CreationContext<'_>, path: &str) -> Option<TextureId> {
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

                    Some(cc.egui_ctx.load_texture(
                        path,
                        color_image,
                        egui::TextureOptions::default(),
                    ).id())
                }
                Err(_) => None
            }
        }
        Err(_) => None
    }
}